use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use reqwest::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use std::collections::HashSet;
use std::thread;
use tiny_http::{Response, Server};
use url::Url;
use tauri::{AppHandle, Runtime, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use sha2::{Sha256, Digest};
use futures::stream::{StreamExt, FuturesUnordered};

// 앱 전역 상태: 취소 플래그 관리
pub struct AppState {
    pub cancel_sync: AtomicBool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            cancel_sync: AtomicBool::new(false),
        }
    }
}

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

#[derive(Deserialize, Debug, Clone)]
struct DropboxEntry {
    name: String,
    path_display: Option<String>,
    #[serde(rename = ".tag")]
    tag: String,
    server_modified: Option<String>,
    content_hash: Option<String>,
}

#[derive(Deserialize, Debug)]
struct DropboxListResponse {
    entries: Vec<DropboxEntry>,
}

#[derive(Deserialize, Debug)]
struct TokenResponse {
    access_token: String,
}

async fn compute_dropbox_hash(path: &PathBuf) -> String {
    let file_data = match tokio::fs::read(path).await {
        Ok(d) => d,
        Err(_) => return String::new(),
    };
    
    let mut overall_hasher = Sha256::new();
    let mut blocks = 0;
    
    for chunk in file_data.chunks(4 * 1024 * 1024) {
        let mut block_hasher = Sha256::new();
        block_hasher.update(chunk);
        overall_hasher.update(block_hasher.finalize());
        blocks += 1;
    }
    
    if blocks == 0 {
        let block_hasher = Sha256::new();
        overall_hasher.update(block_hasher.finalize());
    }
    
    hex::encode(overall_hasher.finalize())
}

#[tauri::command]
fn cancel_sync(state: State<'_, AppState>) {
    state.cancel_sync.store(true, Ordering::SeqCst);
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
        .form(&[
            ("code", code.as_str()), 
            ("grant_type", "authorization_code"), 
            ("client_id", get_app_key().as_str()), 
            ("client_secret", get_app_secret().as_str()), 
            ("redirect_uri", "http://localhost:8421/callback")
        ])
        .send().await.map_err(|e| e.to_string())?;
    
    if res.status().is_success() {
        let token_data: TokenResponse = res.json().await.map_err(|e| e.to_string())?;
        Ok(token_data.access_token)
    } else {
        let err_text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        Err(format!("토큰 교환 실패: {}", err_text))
    }
}

#[tauri::command]
async fn list_dropbox_folders(token: String) -> Result<Vec<String>, String> {
    let client = Client::new();
    let res = client.post("https://api.dropboxapi.com/2/files/list_folder")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/json")
        .json(&serde_json::json!({"path": "", "recursive": false}))
        .send().await.map_err(|e| e.to_string())?;
    
    if res.status().is_success() {
        let list: DropboxListResponse = res.json().await.map_err(|e| e.to_string())?;
        Ok(list.entries.into_iter()
            .filter(|e| e.tag == "folder")
            .map(|e| e.path_display.unwrap_or_else(|| format!("/{}", e.name)))
            .collect())
    } else {
        let err_text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        Err(format!("목록 조회 실패: {}", err_text))
    }
}

async fn list_dropbox_files(client: &Client, token: &str, path: &str) -> Vec<DropboxEntry> {
    let res = client.post("https://api.dropboxapi.com/2/files/list_folder")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/json")
        .json(&serde_json::json!({"path": path}))
        .send().await;
    
    match res {
        Ok(r) => {
            if r.status().is_success() {
                let list: DropboxListResponse = r.json().await.unwrap_or(DropboxListResponse { entries: Vec::new() });
                list.entries.into_iter().filter(|e| e.tag == "file").collect()
            } else {
                Vec::new()
            }
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
async fn sync_folders(app: AppHandle, state: State<'_, AppState>, items: Vec<SyncItem>) -> Result<SyncResult, String> {
    state.cancel_sync.store(false, Ordering::SeqCst);
    
    let client = Client::new();
    let mut total_tasks = Vec::new();
    
    let mut fetch_tasks = FuturesUnordered::new();
    for item in items {
        let client_ref = client.clone();
        fetch_tasks.push(async move {
            let remote_files = list_dropbox_files(&client_ref, &item.token, &item.cloud_path).await;
            (item, remote_files)
        });
    }
    
    let mut item_meta = Vec::new();
    while let Some(res) = fetch_tasks.next().await {
        if state.cancel_sync.load(Ordering::SeqCst) {
            return Ok(SyncResult { success: false, message: "사용자에 의해 취소되었습니다.".to_string() });
        }
        item_meta.push(res);
    }

    for (item, remote_files) in item_meta {
        let local_dir = PathBuf::from(&item.local_path);
        if !local_dir.exists() { let _ = tokio::fs::create_dir_all(&local_dir).await; }
        
        let mut remote_file_names = HashSet::new();
        for remote_file in remote_files {
            let file_name = remote_file.name.clone();
            remote_file_names.insert(file_name.clone());
            let local_path = local_dir.join(&file_name);
            let remote_path = format!("{}/{}", item.cloud_path.trim_end_matches('/'), file_name);
            let remote_hash = remote_file.content_hash.clone().unwrap_or_default();
            
            if !local_path.exists() {
                total_tasks.push(("down", item.token.clone(), remote_path, local_path, file_name));
            } else {
                let local_hash = compute_dropbox_hash(&local_path).await;
                if local_hash != remote_hash {
                    let remote_modified = DateTime::parse_from_rfc3339(remote_file.server_modified.as_ref().unwrap()).unwrap().with_timezone(&Utc);
                    if let Ok(metadata) = tokio::fs::metadata(&local_path).await {
                        if let Ok(modified) = metadata.modified() {
                            let local_modified: DateTime<Utc> = modified.into();
                            if remote_modified > local_modified + chrono::Duration::seconds(1) {
                                total_tasks.push(("down", item.token.clone(), remote_path, local_path, file_name));
                            } else if local_modified > remote_modified + chrono::Duration::seconds(1) {
                                total_tasks.push(("up", item.token.clone(), remote_path, local_path, file_name));
                            }
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
                    let remote_path = format!("{}/{}", item.cloud_path.trim_end_matches('/'), file_name);
                    total_tasks.push(("up", item.token.clone(), remote_path, path, file_name));
                }
            }
        }
    }

    let total_count = total_tasks.len() as f32;
    let processed_count = Arc::new(Mutex::new(0));
    let mut worker_tasks = FuturesUnordered::new();
    let concurrency_limit = 5;

    let app_arc = Arc::new(app);
    let client_arc = Arc::new(client);

    for (action, token, remote_path, local_path, file_name) in total_tasks {
        if state.cancel_sync.load(Ordering::SeqCst) {
            break;
        }

        let client = client_arc.clone();
        let app = app_arc.clone();
        let pc = processed_count.clone();
        
        worker_tasks.push(async move {
            let _res = if action == "up" {
                upload_to_dropbox(&client, &token, local_path, remote_path).await
            } else {
                download_from_dropbox(&client, &token, remote_path, local_path).await
            };
            
            {
                let mut p = pc.lock().unwrap();
                *p += 1;
                let progress = if total_count > 0.0 { *p as f32 / total_count } else { 1.0 };
                let _ = app.emit("sync-progress", ProgressPayload { current_file: file_name, progress });
            }
        });

        if worker_tasks.len() >= concurrency_limit {
            let _ = worker_tasks.next().await;
        }
    }

    while let Some(_) = worker_tasks.next().await {}

    if state.cancel_sync.load(Ordering::SeqCst) {
        Ok(SyncResult { success: false, message: "동기화가 중단되었습니다. 일부 파일은 전송되지 않았을 수 있습니다.".to_string() })
    } else {
        Ok(SyncResult { success: true, message: "동기화 완료 (최적화 모드)".to_string() })
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
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
        .invoke_handler(tauri::generate_handler![
            sync_folders, 
            cancel_sync, 
            open_auth_url, 
            pick_folder_dialog, 
            exchange_code_for_token, 
            list_dropbox_folders
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
