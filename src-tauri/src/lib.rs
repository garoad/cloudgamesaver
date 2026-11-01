use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use reqwest::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use std::collections::HashSet;
#[cfg(desktop)]
use std::thread;
#[cfg(desktop)]
use tiny_http::{Response, Server};
use url::Url;
use tauri::{AppHandle, Emitter, State};
#[cfg(desktop)]
use tauri::{Manager, Runtime};
#[cfg(desktop)]
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use sha2::{Sha256, Digest};
use futures::stream::{StreamExt, FuturesUnordered};

mod storage;
use storage::LocalFile;
#[cfg(target_os = "android")]
mod android_storage;

// 앱 전역 상태: 취소 플래그 관리
pub struct AppState {
    pub cancel_sync: AtomicBool,
    #[cfg(target_os = "android")]
    oauth_verifier: Mutex<Option<String>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            cancel_sync: AtomicBool::new(false),
            #[cfg(target_os = "android")]
            oauth_verifier: Mutex::new(None),
        }
    }
}

fn get_app_key() -> String {
    option_env!("APP_KEY").unwrap_or("").to_string()
}

#[cfg(desktop)]
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
    #[allow(unused_mut)]
    let mut form = vec![
        ("grant_type", "refresh_token".to_owned()),
        ("refresh_token", refresh_token.to_owned()),
        ("client_id", get_app_key()),
    ];
    #[cfg(desktop)]
    form.push(("client_secret", get_app_secret()));
    let res = client.post("https://api.dropboxapi.com/oauth2/token")
        .form(&form)
        .send().await.map_err(|e| e.to_string())?;

    if res.status().is_success() {
        let token_data: TokenResponse = res.json().await.map_err(|e| e.to_string())?;
        Ok(token_data.access_token)
    } else {
        let err_text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        Err(format!("토큰 갱신 실패: {}", err_text))
    }
}

async fn compute_dropbox_hash(app: &AppHandle, path: &LocalFile) -> Result<String, String> {
    let mut file = path.open_read(app).await?;

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
                Err(error) => return Err(error.to_string()),
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
        return Ok(hex::encode(empty_block_hasher.finalize()));
    }

    Ok(hex::encode(overall_hasher.finalize()))
}

#[tauri::command]
fn cancel_sync(state: State<'_, AppState>) {
    println!("[DEBUG] 동기화 취소 요청됨");
    state.cancel_sync.store(true, Ordering::SeqCst);
}

#[cfg(desktop)]
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

#[cfg(target_os = "android")]
#[tauri::command]
fn open_auth_url(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let app_key = get_app_key();
    if app_key.is_empty() { return Err("빌드 환경에 Dropbox APP_KEY를 설정해주세요.".to_owned()); }
    let mut random = [0u8; 32];
    getrandom::getrandom(&mut random).map_err(|e| e.to_string())?;
    let verifier = URL_SAFE_NO_PAD.encode(random);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut url = Url::parse("https://www.dropbox.com/oauth2/authorize").map_err(|e| e.to_string())?;
    // Dropbox displays a code when redirect_uri is omitted. This works without
    // registering a new redirect URI and survives switching to a browser.
    url.query_pairs_mut()
        .append_pair("client_id", &app_key)
        .append_pair("response_type", "code")
        .append_pair("token_access_type", "offline")
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", &challenge);
    *state.oauth_verifier.lock().map_err(|e| e.to_string())? = Some(verifier);
    app.opener().open_url(url.to_string(), None::<String>).map_err(|e| e.to_string())
}

#[cfg(desktop)]
#[tauri::command]
async fn pick_folder_dialog<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |folder| {
        let path = folder.map(|f| f.to_string());
        tx.send(path).ok();
    });
    rx.await.map_err(|e| e.to_string())
}

#[cfg(target_os = "android")]
#[tauri::command]
async fn pick_folder_dialog(app: AppHandle) -> Result<Option<String>, String> {
    android_storage::pick_folder(&app)
}

#[tauri::command]
fn get_platform() -> &'static str {
    if cfg!(target_os = "android") { "android" } else { "desktop" }
}

#[tauri::command]
async fn exchange_code_for_token(code: String, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    if code.trim().is_empty() { return Err("Dropbox 인증 코드를 입력해주세요.".to_owned()); }
    let _ = &state;
    let client = Client::new();
    #[allow(unused_mut)]
    let mut form = vec![
        ("code", code.trim().to_owned()),
        ("grant_type", "authorization_code".to_owned()),
        ("client_id", get_app_key()),
    ];
    #[cfg(desktop)]
    {
        form.push(("client_secret", get_app_secret()));
        form.push(("redirect_uri", "http://localhost:8421/callback".to_owned()));
    }
    #[cfg(target_os = "android")]
    {
        let verifier = state.oauth_verifier.lock().map_err(|e| e.to_string())?.clone()
            .ok_or("Dropbox 연결을 다시 시작해주세요.")?;
        form.push(("code_verifier", verifier));
    }
    let res = client.post("https://api.dropboxapi.com/oauth2/token")
        .form(&form)
        .send().await.map_err(|e| e.to_string())?;
    
    if res.status().is_success() {
        let token_data: TokenResponse = res.json().await.map_err(|e| e.to_string())?;
        #[cfg(target_os = "android")]
        { state.oauth_verifier.lock().map_err(|e| e.to_string())?.take(); }
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

async fn upload_to_dropbox(app: &AppHandle, client: &Client, token: &str, local_file: LocalFile, remote_path: String) -> Result<(), String> {
    // 폴더 경로 추출 및 생성 시도 (업로드할 파일이 있는 폴더)
    if let Some(parent_path) = std::path::Path::new(&remote_path).parent() {
        if let Some(parent_str) = parent_path.to_str() {
            if !parent_str.is_empty() && parent_str != "/" {
                // 폴더 생성 시도 (실패해도 계속 진행)
                let _ = create_dropbox_folder(client, token, parent_str).await;
            }
        }
    }

    let contents = local_file.read(app).await?;
    client.post("https://content.dropboxapi.com/2/files/upload")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/octet-stream")
        .header("Dropbox-API-Arg", serde_json::to_string(&serde_json::json!({"path": remote_path, "mode": "overwrite", "mute": true})).unwrap())
        .body(contents).send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?;
    Ok(())
}

async fn download_from_dropbox(app: &AppHandle, client: &Client, token: &str, remote_path: String, local_file: LocalFile) -> Result<(), String> {
    let res = client.post("https://content.dropboxapi.com/2/files/download")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header("Dropbox-API-Arg", serde_json::to_string(&serde_json::json!({"path": remote_path})).unwrap())
        .send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?;
    let contents = res.bytes().await.map_err(|e| e.to_string())?;
    local_file.write(app, &contents).await?;
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
        
        let local_dir = &items[i].local_path;
        storage::ensure_folder(&app, local_dir).await?;
        
        let mut remote_rel_paths = HashSet::new();
        for remote_file in remote_files {
            if remote_file.name.eq_ignore_ascii_case(".DS_Store") { continue; }
            let remote_path = remote_file.path_display.clone().unwrap_or_default();
            if remote_path.to_lowercase().ends_with(".ds_store") { continue; }
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
            
            let local_path = LocalFile::new(local_dir, &rel_path)?;
            
            let remote_hash = remote_file.content_hash.clone().unwrap_or_default();
            
            let local_modified = local_path.modified(&app).await?;
            if local_modified.is_none() {
                println!("[Sync] 신규 다운로드 감지: {}", rel_path);
                let _ = app.emit("sync-log", format!("[다운로드 감지] '{}' (클라우드 신규 파일)", rel_path));
                total_tasks.push(("down", current_token.clone(), remote_path, local_path, rel_path));
            } else {
                let local_hash = compute_dropbox_hash(&app, &local_path).await?;
                if local_hash != remote_hash {
                    let remote_modified_str = remote_file.server_modified.as_deref().unwrap_or("");
                    let remote_modified = DateTime::parse_from_rfc3339(remote_modified_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e| format!("서버 시간 파싱 실패: {}", e))?;
                        
                    if let Some(local_modified) = local_modified {
                            println!("[Sync] 해시 불일치 ({}): 로컬={}, 서버={}", rel_path, local_modified, remote_modified);
                            if remote_modified > local_modified {
                                let _ = app.emit("sync-log", format!("[다운로드 감지] '{}' (클라우드 데이터가 최신)", rel_path));
                                total_tasks.push(("down", current_token.clone(), remote_path, local_path, rel_path));
                            } else {
                                let _ = app.emit("sync-log", format!("[업로드 감지] '{}' (로컬 데이터가 최신)", rel_path));
                                total_tasks.push(("up", current_token.clone(), remote_path, local_path, rel_path));
                            }
                    }
                } else {
                    println!("[Sync] 파일 일치 (해시 동일): {}", rel_path);
                    let _ = app.emit("sync-log", format!("[동일 생략] '{}' (동일 해시)", rel_path));
                }
            }
        }

        // 로컬 파일 재귀 스캔
        let local_files = storage::list_files(&app, local_dir).await?;
        for (rel_path, path) in local_files {
            if !remote_rel_paths.contains(&rel_path) {
                let remote_path = format!("{}/{}", cloud_path_normalized.trim_end_matches('/'), rel_path);
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
    let transfer_errors = Arc::new(Mutex::new(Vec::new()));

    for (action, token, remote_path, local_path, file_name) in total_tasks {
        if state.cancel_sync.load(Ordering::SeqCst) {
            break;
        }

        let client = client_arc.clone();
        let app = app_arc.clone();
        let pc = processed_count.clone();
        let errors = transfer_errors.clone();
        
        worker_tasks.push(async move {
            let res = if action == "up" {
                upload_to_dropbox(&app, &client, &token, local_path, remote_path).await
            } else {
                download_from_dropbox(&app, &client, &token, remote_path, local_path).await
            };
            
            if let Err(ref e) = res {
                errors.lock().unwrap().push(format!("{}: {}", file_name, e));
                println!("[Sync Error] {} 처리 중 오류: {}", file_name, e);
                let _ = app.emit("sync-log", format!("[처리 오류] '{}': {}", file_name, e));
            } else {
                let act_label = if action == "up" { "업로드 완료" } else { "다운로드 완료" };
                let _ = app.emit("sync-log", format!("[{}] '{}'", act_label, file_name));
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
    } else if !transfer_errors.lock().unwrap().is_empty() {
        let errors = transfer_errors.lock().unwrap();
        let message = format!("{}개 파일 동기화 실패: {}", errors.len(), errors.iter().take(3).cloned().collect::<Vec<_>>().join("; "));
        let _ = app_arc.emit("sync-complete", serde_json::json!({ "success": false, "message": &message }));
        Err(message)
    } else {
        println!("[DEBUG] 동기화 성공 완료, 이벤트 발송");
        let _ = app_arc.emit("sync-complete", serde_json::json!({ "success": true, "message": format!("동기화 완료 ({}개의 파일 처리)", total_count) }));
        Ok(SyncResult { success: true, message: format!("동기화 완료 ({}개의 파일 처리)", total_count) })
    }
}

#[tauri::command]
async fn delete_dropbox_folder(path: String, token: String, refresh_token: Option<String>) -> Result<Option<String>, String> {
    let client = Client::new();
    let delete_url = "https://api.dropboxapi.com/2/files/delete_v2";
    
    let formatted_path = if path.starts_with('/') { path.clone() } else { format!("/{}", path) };

    println!("[Dropbox Delete] 폴더 삭제 요청: {}", formatted_path);

    let res = client.post(delete_url)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/json")
        .json(&serde_json::json!({ "path": formatted_path }))
        .send().await.map_err(|e| format!("네트워크 요청 실패: {}", e))?;

    if res.status().is_success() {
        println!("[Dropbox Delete] 폴더 삭제 성공: {}", formatted_path);
        Ok(None)
    } else {
        let status = res.status();
        let err_text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        
        if status.as_u16() == 401 || err_text.contains("expired_access_token") || err_text.contains("invalid_access_token") {
            if let Some(ref r_token) = refresh_token {
                println!("[Dropbox Delete] 토큰 만료 감지, 갱신 후 재시도: {}", formatted_path);
                match refresh_access_token(r_token).await {
                    Ok(new_token) => {
                        let retry_res = client.post(delete_url)
                            .header(AUTHORIZATION, format!("Bearer {}", new_token))
                            .header(CONTENT_TYPE, "application/json")
                            .json(&serde_json::json!({ "path": formatted_path }))
                            .send().await.map_err(|e| format!("재시도 네트워크 요청 실패: {}", e))?;
                        
                        if retry_res.status().is_success() {
                            println!("[Dropbox Delete] 토큰 갱신 후 삭제 성공: {}", formatted_path);
                            Ok(Some(new_token))
                        } else {
                            let retry_status = retry_res.status();
                            let retry_err = retry_res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                            Err(format!("드롭박스 삭제 실패 (HTTP {}): {}", retry_status, retry_err))
                        }
                    },
                    Err(e) => Err(format!("토큰 갱신 실패: {}", e))
                }
            } else {
                Err("인증 토큰이 만료되었습니다. 다시 연결해주세요.".to_string())
            }
        } else if status.as_u16() == 409 && (err_text.contains("path_lookup/not_found") || err_text.contains("not_found")) {
            println!("[Dropbox Delete] 폴더가 이미 존재하지 않음: {}", formatted_path);
            Ok(None)
        } else {
            Err(format!("드롭박스 폴더 삭제 실패 (HTTP {}): {}", status, err_text))
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .manage(AppState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(android_storage::init());
    builder.setup(|app| {
            let _ = &app;
            #[cfg(desktop)]
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
            list_dropbox_folders,
            delete_dropbox_folder,
            get_platform
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
