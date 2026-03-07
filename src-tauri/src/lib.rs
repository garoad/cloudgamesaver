use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use reqwest::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use std::collections::HashSet;
use std::thread;
use tiny_http::{Response, Server};
use url::Url;
use tauri::{AppHandle, Runtime, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use std::sync::{Arc, Mutex};

fn get_app_key() -> String {
    option_env!("APP_KEY").unwrap_or("").to_string()
}

fn get_app_secret() -> String {
    option_env!("APP_SECRET").unwrap_or("").to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncItem {
    name: String,
    local_path: String,
    cloud_path: String,
    token: String,
    enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncResult {
    success: bool,
    message: String,
}

#[derive(Serialize, Clone)]
struct ProgressPayload {
    current_file: String,
    progress: f32,
}

#[derive(Deserialize, Debug)]
struct DropboxEntry {
    name: String,
    path_display: Option<String>,
    #[serde(rename = ".tag")]
    tag: String,
    server_modified: Option<String>,
}

#[derive(Deserialize, Debug)]
struct DropboxListResponse {
    entries: Vec<DropboxEntry>,
}

#[derive(Deserialize, Debug)]
struct TokenResponse {
    access_token: String,
}

#[tauri::command]
fn open_auth_url(app: AppHandle) -> Result<(), String> {
    let redirect_uri = "http://localhost:8421/callback";
    let auth_url = format!(
        "https://www.dropbox.com/oauth2/authorize?client_id={}&response_type=code&redirect_uri={}",
        get_app_key(), redirect_uri
    );
    let app_handle = app.clone();
    thread::spawn(move || {
        if let Ok(server) = Server::http("127.0.0.1:8421") {
            for request in server.incoming_requests() {
                let url = format!("http://localhost:8421{}", request.url());
                if let Ok(parsed) = Url::parse(&url) {
                    if let Some((_, code)) = parsed.query_pairs().find(|(k, _)| k == "code") {
                        let code_str = code.into_owned();
                        let _ = request.respond(Response::from_string("인증 성공!"));
                        let _ = app_handle.emit("dropbox-code-received", code_str);
                        break;
                    }
                }
            }
        }
    });
    app.opener().open_url(auth_url, None::<String>).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn pick_folder_dialog<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog().file().pick_folder(move |folder| {
        let path = folder.map(|f| f.to_string());
        tx.send(path).ok();
    });
    Ok(rx.recv().map_err(|e| e.to_string())?)
}

#[tauri::command]
async fn exchange_code_for_token(code: String) -> Result<String, String> {
    let client = Client::new();
    let res = client.post("https://api.dropboxapi.com/oauth2/token")
        .form(&[("code", code.as_str()), ("grant_type", "authorization_code"), ("client_id", get_app_key().as_str()), ("client_secret", get_app_secret().as_str()), ("redirect_uri", "http://localhost:8421/callback")])
        .send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        let token_data: TokenResponse = res.json().await.map_err(|e| e.to_string())?;
        Ok(token_data.access_token)
    } else {
        Err(res.text().await.unwrap_or_else(|_| "토큰 교환 실패".to_string()))
    }
}

#[tauri::command]
async fn list_dropbox_folders(token: String) -> Result<Vec<String>, String> {
    let client = Client::new();
    let res = client.post("https://api.dropboxapi.com/2/files/list_folder")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/json")
        .json(&serde_json::json!({"path": ""})).send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        let list: DropboxListResponse = res.json().await.map_err(|e| e.to_string())?;
        Ok(list.entries.into_iter().filter(|e| e.tag == "folder").map(|e| e.path_display.unwrap_or_else(|| format!("/{}", e.name))).collect())
    } else { Err("목록 조회 실패".to_string()) }
}

async fn list_dropbox_files(client: &Client, token: &str, path: &str) -> Vec<DropboxEntry> {
    let res = client.post("https://api.dropboxapi.com/2/files/list_folder")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/json")
        .json(&serde_json::json!({"path": path})).send().await;
    match res {
        Ok(r) => {
            let list: DropboxListResponse = r.json().await.unwrap_or(DropboxListResponse { entries: Vec::new() });
            list.entries.into_iter().filter(|e| e.tag == "file").collect()
        }
        Err(_) => Vec::new(),
    }
}

async fn upload_to_dropbox(client: &Client, token: &str, local_file: PathBuf, remote_path: String) -> Result<(), String> {
    let contents = tokio::fs::read(&local_file).await.map_err(|e| e.to_string())?;
    client.post("https://content.dropboxapi.com/2/files/upload")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/octet-stream")
        .header("Dropbox-API-Arg", serde_json::to_string(&serde_json::json!({"path": remote_path, "mode": "overwrite", "mute": true})).unwrap())
        .body(contents).send().await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn download_from_dropbox(client: &Client, token: &str, remote_path: String, local_file: PathBuf) -> Result<(), String> {
    let res = client.post("https://content.dropboxapi.com/2/files/download")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header("Dropbox-API-Arg", serde_json::to_string(&serde_json::json!({"path": remote_path})).unwrap())
        .send().await.map_err(|e| e.to_string())?;
    let contents = res.bytes().await.map_err(|e| e.to_string())?;
    tokio::fs::write(&local_file, contents).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn sync_folders(app: AppHandle, items: Vec<SyncItem>) -> SyncResult {
    let messages = Arc::new(Mutex::new(Vec::new()));
    let processed_count = Arc::new(Mutex::new(0));
    let client = Client::new();
    
    // 분석 단계 (비동기)
    let mut total_files_count = 0;
    for item in &items {
        let remote_files = list_dropbox_files(&client, &item.token, &item.cloud_path).await;
        total_files_count += remote_files.len();
        if let Ok(mut entries) = tokio::fs::read_dir(&item.local_path).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry.file_type().await.map(|t| t.is_file()).unwrap_or(false) {
                    total_files_count += 1;
                }
            }
        }
    }

    let total_denominator = total_files_count as f32;

    for item in items {
        let local_dir = PathBuf::from(&item.local_path);
        if !local_dir.exists() { let _ = tokio::fs::create_dir_all(&local_dir).await; }
        
        let remote_files = list_dropbox_files(&client, &item.token, &item.cloud_path).await;
        let remote_file_names: HashSet<String> = remote_files.iter().map(|e| e.name.clone()).collect();
        
        for remote_file in remote_files {
            let file_name = remote_file.name.clone();
            let local_path = local_dir.join(&file_name);
            let remote_path = format!("{}/{}", item.cloud_path.trim_end_matches('/'), file_name);
            let remote_modified = DateTime::parse_from_rfc3339(remote_file.server_modified.as_ref().unwrap()).unwrap().with_timezone(&Utc);
            
            {
                let mut p = processed_count.lock().unwrap();
                *p += 1;
                let progress = if total_denominator > 0.0 { *p as f32 / total_denominator } else { 1.0 };
                let _ = app.emit("sync-progress", ProgressPayload { current_file: file_name.clone(), progress });
            }

            if !local_path.exists() {
                let _ = download_from_dropbox(&client, &item.token, remote_path, local_path).await;
            } else {
                if let Ok(metadata) = tokio::fs::metadata(&local_path).await {
                    if let Ok(modified) = metadata.modified() {
                        let local_modified: DateTime<Utc> = modified.into();
                        if remote_modified > local_modified + chrono::Duration::seconds(1) {
                            let _ = download_from_dropbox(&client, &item.token, remote_path, local_path).await;
                        } else if local_modified > remote_modified + chrono::Duration::seconds(1) {
                            let _ = upload_to_dropbox(&client, &item.token, local_path, remote_path).await;
                        }
                    }
                }
            }
        }

        if let Ok(mut entries) = tokio::fs::read_dir(&local_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() { continue; }
                let file_name = path.file_name().unwrap().to_str().unwrap().to_string();
                
                if !remote_file_names.contains(&file_name) {
                    {
                        let mut p = processed_count.lock().unwrap();
                        *p += 1;
                        let progress = if total_denominator > 0.0 { *p as f32 / total_denominator } else { 1.0 };
                        let _ = app.emit("sync-progress", ProgressPayload { current_file: file_name.clone(), progress });
                    }
                    let remote_path = format!("{}/{}", item.cloud_path.trim_end_matches('/'), file_name);
                    let _ = upload_to_dropbox(&client, &item.token, path, remote_path).await;
                }
            }
        }
        messages.lock().unwrap().push(format!("{}: 동기화 완료!", item.name));
    }

    let final_message = messages.lock().unwrap().join("\n");
    SyncResult { success: true, message: final_message }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            if let Some(main_window) = app.get_webview_window("main") {
                let icon_bytes = include_bytes!("../icons/32x32.png");
                if let Ok(icon) = tauri::image::Image::from_bytes(icon_bytes) {
                    let _ = main_window.set_icon(icon);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![sync_folders, open_auth_url, pick_folder_dialog, exchange_code_for_token, list_dropbox_folders])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
