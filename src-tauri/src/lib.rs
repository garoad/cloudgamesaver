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
    refresh_token: Option<String>,
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
    refresh_token: Option<String>,
}

async fn refresh_access_token(refresh_token: &str) -> Result<String, String> {
    let client = Client::new();
    let res = client.post("https://api.dropboxapi.com/oauth2/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", get_app_key().as_str()),
            ("client_secret", get_app_secret().as_str()),
        ])
        .send().await.map_err(|e| e.to_string())?;

    if res.status().is_success() {
        let token_data: TokenResponse = res.json().await.map_err(|e| e.to_string())?;
        Ok(token_data.access_token)
    } else {
        let err_text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        Err(format!("토큰 갱신 실패: {}", err_text))
    }
}

async fn compute_dropbox_hash(path: &PathBuf) -> String {
    let mut file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(_) => return String::new(),
    };

    let mut overall_hasher = Sha256::new();
    let mut has_any_block = false;

    loop {
        let mut buffer = vec![0u8; 4 * 1024 * 1024];
        let mut total_read = 0;
        
        while total_read < buffer.len() {
            use tokio::io::AsyncReadExt;
            match file.read(&mut buffer[total_read..]).await {
                Ok(0) => break,
                Ok(n) => total_read += n,
                Err(_) => break,
            }
        }

        if total_read == 0 { break; }

        let mut block_hasher = Sha256::new();
        block_hasher.update(&buffer[..total_read]);
        overall_hasher.update(block_hasher.finalize());
        has_any_block = true;

        if total_read < buffer.len() { break; }
    }

    if !has_any_block {
        let empty_block_hasher = Sha256::new();
        return hex::encode(empty_block_hasher.finalize());
    }

    hex::encode(overall_hasher.finalize())
}

#[tauri::command]
fn cancel_sync(state: State<'_, AppState>) {
    println!("[DEBUG] 동기화 취소 요청됨");
    state.cancel_sync.store(true, Ordering::SeqCst);
}

#[tauri::command]
fn open_auth_url(app: AppHandle) -> Result<(), String> {
    let redirect_uri = "http://localhost:8421/callback";
    let auth_url = format!(
        "https://www.dropbox.com/oauth2/authorize?client_id={}&response_type=code&redirect_uri={}&token_access_type=offline",
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
async fn exchange_code_for_token(code: String) -> Result<serde_json::Value, String> {
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
        Ok(serde_json::json!({
            "access_token": token_data.access_token,
            "refresh_token": token_data.refresh_token
        }))
    } else {
        let err_text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        Err(format!("토큰 교환 실패: {}", err_text))
    }
}

#[derive(Serialize, Debug)]
struct ListFoldersResult {
  folders: Vec<String>,
  new_token: Option<String>,
}

#[tauri::command]
async fn list_dropbox_folders(token: String, refresh_token: Option<String>) -> Result<ListFoldersResult, String> {
  let client = Client::new();
  let res = client.post("https://api.dropboxapi.com/2/files/list_folder")
    .header(AUTHORIZATION, format!("Bearer {}", token))
    .header(CONTENT_TYPE, "application/json")
    .json(&serde_json::json!({"path": "", "recursive": false}))
    .send().await.map_err(|e| format!("네트워크 요청 실패: {}", e))?;

  if res.status().is_success() {
    let list: DropboxListResponse = res.json().await.map_err(|e| format!("데이터 파싱 실패: {}", e))?;
    let folders = list.entries.into_iter()
      .filter(|e| e.tag == "folder")
      .map(|e| e.path_display.unwrap_or_else(|| format!("/{}", e.name)))
      .collect();
    Ok(ListFoldersResult { folders, new_token: None })
  } else {
    let status_code = res.status().as_u16();
    let err_text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());

    // 토큰 만료 에러인 경우 갱신 시도
    if err_text.contains("expired_access_token") || err_text.contains("invalid_access_token") || status_code == 401 {
      if let Some(ref r_token) = refresh_token {
        println!("[ListFolders] 토큰 만료 감지, 갱신 시도");
        match refresh_access_token(r_token).await {
          Ok(new_token) => {
            // 새 토큰으로 다시 시도
            let retry_res = client.post("https://api.dropboxapi.com/2/files/list_folder")
              .header(AUTHORIZATION, format!("Bearer {}", new_token))
              .header(CONTENT_TYPE, "application/json")
              .json(&serde_json::json!({"path": "", "recursive": false}))
              .send().await.map_err(|e| format!("재시도 네트워크 요청 실패: {}", e))?;

            if retry_res.status().is_success() {
              let list: DropboxListResponse = retry_res.json().await.map_err(|e| format!("재시도 데이터 파싱 실패: {}", e))?;
              let folders = list.entries.into_iter()
                .filter(|e| e.tag == "folder")
                .map(|e| e.path_display.unwrap_or_else(|| format!("/{}", e.name)))
                .collect();
              println!("[ListFolders] 토큰 갱신 성공, 목록 조회 완료");
              Ok(ListFoldersResult { folders, new_token: Some(new_token) })
            } else {
              let retry_err = retry_res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
              Err(format!("토큰 갱신 후에도 실패: {}", retry_err))
            }
          },
          Err(e) => Err(format!("토큰 갱신 실패: {}. 다시 로그인해주세요.", e))
        }
      } else {
        Err(format!("expired_access_token|{}", err_text))
      }
    } else {
      Err(format!("드롭박스 목록 조회 실패: {}", err_text))
    }
  }
}

async fn list_dropbox_files(client: &Client, token: &str, path: &str) -> Result<Vec<DropboxEntry>, String> {
    let res = client.post("https://api.dropboxapi.com/2/files/list_folder")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/json")
        .json(&serde_json::json!({
            "path": path,
            "recursive": true
        }))
        .send().await.map_err(|e| e.to_string())?;
    
    if res.status().is_success() {
        let list: DropboxListResponse = res.json().await.map_err(|e| e.to_string())?;
        Ok(list.entries.into_iter().filter(|e| e.tag == "file").collect())
    } else {
        let err_code = res.status().as_u16();
        let err_text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        
        // path_not_found 에러인지 확인
        if err_code == 409 && (err_text.contains("path_not_found") || err_text.contains("not_found")) {
            // 폴더가 존재하지 않으면 빈 목록 반환 (업로드 시 자동으로 폴더 생성됨)
            println!("폴더가 존재하지 않으므로 빈 목록 반환: {}", path);
            Ok(Vec::new())
        } else {
            Err(format!("ERR_CODE:{} | 클라우드 파일 목록 조회 실패 ({}): {}", err_code, path, err_text))
        }
    }
}

async fn get_local_files_recursive(base_path: &std::path::Path, current_path: &std::path::Path) -> Vec<(String, PathBuf)> {
    let mut files = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(current_path).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                let sub_files = Box::pin(get_local_files_recursive(base_path, &path)).await;
                files.extend(sub_files);
            } else {
                if let Ok(rel_path) = path.strip_prefix(base_path) {
                    if let Some(rel_str) = rel_path.to_str() {
                        // 윈도우 경로 구분자를 슬래시로 통일
                        files.push((rel_str.replace("\\", "/"), path));
                    }
                }
            }
        }
    }
    files
}

async fn create_dropbox_folder(client: &Client, token: &str, path: &str) -> Result<(), String> {
    let res = client.post("https://api.dropboxapi.com/2/files/create_folder_v2")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/json")
        .json(&serde_json::json!({
            "path": path
        }))
        .send().await.map_err(|e| e.to_string())?;
    
    if res.status().is_success() {
        Ok(())
    } else {
        let err_code = res.status().as_u16();
        let err_text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        Err(format!("ERR_CODE:{} | 폴더 생성 실패 ({}): {}", err_code, path, err_text))
    }
}

async fn upload_to_dropbox(client: &Client, token: &str, local_file: PathBuf, remote_path: String) -> Result<(), String> {
    // 폴더 경로 추출 및 생성 시도 (업로드할 파일이 있는 폴더)
    if let Some(parent_path) = std::path::Path::new(&remote_path).parent() {
        if let Some(parent_str) = parent_path.to_str() {
            if !parent_str.is_empty() && parent_str != "/" {
                // 폴더 생성 시도 (실패해도 계속 진행)
                let _ = create_dropbox_folder(client, token, parent_str).await;
            }
        }
    }

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
async fn sync_folders(app: AppHandle, state: State<'_, AppState>, mut items: Vec<SyncItem>) -> Result<SyncResult, String> {
    state.cancel_sync.store(false, Ordering::SeqCst);
    
    let client = Client::new();
    let mut total_tasks = Vec::new();
    let mut updated_tokens = Vec::new();
    
    let mut current_global_token: Option<String> = None;
    
    for i in 0..items.len() {
        if !items[i].enabled { continue; }
        
        // 이전에 갱신된 토큰이 있다면 그것을 사용, 없으면 항목의 토큰 사용
        let mut current_token = current_global_token.clone().unwrap_or_else(|| items[i].token.clone());
        
        // 드롭박스 경로는 대소문자를 구분하지 않으며 대개 /로 시작함
        let cloud_path_normalized = if items[i].cloud_path.starts_with('/') {
            items[i].cloud_path.clone()
        } else {
            format!("/{}", items[i].cloud_path)
        };
        let cloud_base = cloud_path_normalized.trim_end_matches('/').to_lowercase();
        
        println!("[Sync] 목록 조회 시도 ({}): {}", items[i].name, cloud_path_normalized);
        let res = list_dropbox_files(&client, &current_token, &cloud_path_normalized).await;
        
        let remote_files = match res {
            Ok(files) => files,
            Err(e) if e.contains("ERR_CODE:409") && (e.contains("path_not_found") || e.contains("not_found")) => {
                println!("[Sync] 폴더 없음 감지, 빈 상태로 진행: {}", items[i].name);
                Vec::new()
            },
            Err(e) if e.contains("401") || e.contains("expired") => {
                if let Some(ref refresh) = items[i].refresh_token {
                    println!("[Sync] 토큰 만료 감지, 갱신 시도: {}", items[i].name);
                    match refresh_access_token(refresh).await {
                        Ok(new_token) => {
                            current_token = new_token.clone();
                            current_global_token = Some(new_token.clone()); // 전역 토큰 업데이트
                            items[i].token = new_token.clone();
                            updated_tokens.push((i, new_token));
                            
                            // 토큰 갱신 후 다시 시도
                            match list_dropbox_files(&client, &current_token, &cloud_path_normalized).await {
                                Ok(files) => files,
                                Err(retry_err) => return Err(format!("토큰 갱신 후에도 실패: {}", retry_err))
                            }
                        },
                        Err(re) => return Err(format!("토큰 갱신 실패: {}. 다시 로그인해주세요.", re))
                    }
                } else {
                    return Err("세션이 만료되었습니다. 다시 로그인해주세요.".to_string());
                }
            },
            Err(e) => {
                println!("[Sync Error] {} 목록 조회 중 중대한 오류: {}", items[i].name, e);
                return Err(format!("클라우드 연결 오류 ({}): {}", items[i].name, e));
            }
        };
        
        // ... (이후 로직에서 cloud_base 사용 부분은 동일하거나 유사하게 동작)
        
        // 만약 이번 루프에서 토큰이 갱신되었다면, items의 나머지 항목들도 업데이트 (다음 루프에서 사용 위해)
        if let Some(ref latest_token) = current_global_token {
            for item in items.iter_mut().skip(i + 1) {
                item.token = latest_token.clone();
            }
        }
        
        // 취소 체크 추가
        if state.cancel_sync.load(Ordering::SeqCst) {
            println!("[DEBUG] 동기화 취소 감지됨");
            return Ok(SyncResult { success: false, message: "동기화가 취소되었습니다.".to_string() });
        }
        
        let local_dir = PathBuf::from(&items[i].local_path);
        if !local_dir.exists() { let _ = tokio::fs::create_dir_all(&local_dir).await; }
        
        let mut remote_rel_paths = HashSet::new();
        for remote_file in remote_files {
            let remote_path = remote_file.path_display.clone().unwrap_or_default();
            let remote_path_lower = remote_path.to_lowercase();
            
            // cloud_path 기준 상대 경로 추출
            let rel_path = if remote_path_lower.starts_with(&cloud_base) {
                let p = &remote_path[cloud_base.len()..];
                p.trim_start_matches('/').to_string()
            } else {
                remote_file.name.clone()
            };

            if rel_path.is_empty() { continue; }
            
            remote_rel_paths.insert(rel_path.clone());
            
            // OS별 경로 구분자 처리: Windows는 \로 변환, macOS/Linux는 / 그대로 사용
            #[cfg(windows)]
            let local_path = local_dir.join(rel_path.replace("/", "\\"));
            #[cfg(not(windows))]
            let local_path = local_dir.join(&rel_path);
            
            let remote_hash = remote_file.content_hash.clone().unwrap_or_default();
            
            if !local_path.exists() {
                println!("[Sync] 신규 다운로드 감지: {}", rel_path);
                total_tasks.push(("down", current_token.clone(), remote_path, local_path, rel_path));
            } else {
                let local_hash = compute_dropbox_hash(&local_path).await;
                if local_hash != remote_hash {
                    let remote_modified_str = remote_file.server_modified.as_deref().unwrap_or("");
                    let remote_modified = DateTime::parse_from_rfc3339(remote_modified_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e| format!("서버 시간 파싱 실패: {}", e))?;
                        
                    if let Ok(metadata) = tokio::fs::metadata(&local_path).await {
                        if let Ok(modified) = metadata.modified() {
                            let local_modified: DateTime<Utc> = modified.into();
                            println!("[Sync] 해시 불일치 ({}): 로컬={}, 서버={}", rel_path, local_modified, remote_modified);
                            if remote_modified > local_modified {
                                total_tasks.push(("down", current_token.clone(), remote_path, local_path, rel_path));
                            } else {
                                total_tasks.push(("up", current_token.clone(), remote_path, local_path, rel_path));
                            }
                        }
                    }
                } else {
                    println!("[Sync] 파일 일치 (해시 동일): {}", rel_path);
                }
            }
        }

        // 로컬 파일 재귀 스캔
        let local_files = get_local_files_recursive(&local_dir, &local_dir).await;
        for (rel_path, path) in local_files {
            if !remote_rel_paths.contains(&rel_path) {
                let remote_path = format!("{}/{}", items[i].cloud_path.trim_end_matches('/'), rel_path);
                total_tasks.push(("up", current_token.clone(), remote_path, path, rel_path));
            }
        }
    }

    if !updated_tokens.is_empty() {
        let _ = app.emit("tokens-updated", updated_tokens);
    }

    let total_count = total_tasks.len() as f32;
    if total_count == 0.0 {
        return Ok(SyncResult { success: true, message: "이미 모든 파일이 최신 상태입니다.".to_string() });
    }

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
            let res = if action == "up" {
                upload_to_dropbox(&client, &token, local_path, remote_path).await
            } else {
                // 다운로드 시 부모 디렉토리 생성 보장
                if let Some(parent) = local_path.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                download_from_dropbox(&client, &token, remote_path, local_path).await
            };
            
            if let Err(e) = res {
                println!("[Sync Error] {} 처리 중 오류: {}", file_name, e);
            }
            
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
        println!("[DEBUG] 동기화 중단 완료, 이벤트 발송");
        let _ = app_arc.emit("sync-complete", serde_json::json!({ "success": false, "message": "동기화가 중단되었습니다." }));
        Ok(SyncResult { success: false, message: "동기화가 중단되었습니다.".to_string() })
    } else {
        println!("[DEBUG] 동기화 성공 완료, 이벤트 발송");
        let _ = app_arc.emit("sync-complete", serde_json::json!({ "success": true, "message": format!("동기화 완료 ({}개의 파일 처리)", total_count) }));
        Ok(SyncResult { success: true, message: format!("동기화 완료 ({}개의 파일 처리)", total_count) })
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
