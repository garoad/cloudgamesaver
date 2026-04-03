import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { ask, message } from "@tauri-apps/plugin-dialog";
import "./App.css";

interface SyncItem {
  name: string;
  local_path: string;
  cloud_path: string;
  token: string;
  refresh_token: string | null;
  enabled: boolean;
}

export default function App() {
  const [items, setItems] = useState<SyncItem[]>([]);
  const [dropboxToken, setDropboxToken] = useState("");
  const [refreshToken, setRefreshToken] = useState<string | null>(null);
  const [status, setStatus] = useState("");
  const [loading, setLoading] = useState(false);
  const [isUpdating, setIsUpdating] = useState(false);
  const [progress, setProgress] = useState(0);
  const [currentFile, setCurrentFile] = useState("");

  const [newName, setNewName] = useState("");
  const [manualLocalPath, setManualLocalPath] = useState("");
  
  const hasCheckedUpdate = useRef(false);

  // 토큰 만료 처리 함수
  const handleTokenExpiration = async () => {
    setDropboxToken("");
    setRefreshToken(null);
    localStorage.removeItem("dropbox-token");
    localStorage.removeItem("dropbox-refresh-token");
    setStatus("세션이 만료되었습니다. 드롭박스를 다시 연결해주세요.");
    await message("드롭박스 인증 세션이 만료되었습니다. 보안을 위해 다시 연결이 필요합니다.", { title: '알림', kind: 'warning' });
  };

  // 업데이트 확인 로직 (재시도 및 에러 처리 개선)
  const checkForUpdates = async (manual = false, retryCount = 0) => {
    if (loading) return;
    
    const MAX_RETRIES = 3;
    const RETRY_DELAY = 2000; // 2초
    
    try {
      if (manual) setStatus("업데이트 확인 중...");
      else if (retryCount === 0) console.log("🔄 자동 업데이트 확인 중...");
      
      const update = await check();
      
      if (update) {
        console.log(`✅ 새 버전 발견: ${update.version}`);
        if (manual) setStatus(""); 
        const shouldUpdate = await ask(
          `새로운 버전(${update.version})이 있습니다. 업데이트하시겠습니까?\n\n변경사항:\n${update.body || '개선사항 및 버그 수정'}`,
          { title: '업데이트 발견!', kind: 'info' }
        );
        
        if (shouldUpdate) {
          setLoading(true);
          setIsUpdating(true);
          setProgress(0);
          setStatus("업데이트 다운로드 중...");
          
          let downloaded = 0;
          let contentLength = 0;
          
          try {
            await update.downloadAndInstall((event) => {
              switch (event.event) {
                case 'Started':
                  contentLength = event.data.contentLength || 0;
                  setStatus(`업데이트 다운로드 중... (${Math.round(contentLength/1024/1024)}MB)`);
                  console.log(`📦 업데이트 시작: ${contentLength} bytes`);
                  break;
                case 'Progress':
                  downloaded += event.data.chunkLength;
                  if (contentLength > 0) {
                    const progressPercent = (downloaded / contentLength) * 100;
                    setProgress(progressPercent);
                    if (progressPercent % 10 < 1) { // 10%마다 로그
                      console.log(`📊 다운로드 진행률: ${Math.round(progressPercent)}%`);
                    }
                  }
                  break;
                case 'Finished':
                  setStatus("설치 완료! 잠시 후 재시작됩니다...");
                  console.log("✅ 업데이트 설치 완료");
                  break;
              }
            });
            
            // 설치 완료 후 재시작
            setTimeout(async () => {
              await relaunch();
            }, 1500);
            
          } catch (installError) {
            console.error("❌ 설치 실패:", installError);
            await message(`업데이트 설치 중 오류가 발생했습니다:\n\n${installError}\n\n나중에 다시 시도해주세요.`, { 
              title: '설치 에러', 
              kind: 'error' 
            });
          }
        }
      } else {
        console.log("✅ 최신 버전 사용 중");
        if (manual) {
          await message("현재 최신 버전을 사용 중입니다.", { 
            title: '업데이트 확인', 
            kind: 'info' 
          });
        }
      }
    } catch (e) {
      const errorStr = String(e);
      console.error("❌ 업데이트 확인 오류:", errorStr);
      
      // 네트워크 오류 등으로 재시도 가능한 경우
      if (retryCount < MAX_RETRIES && !manual) {
        console.log(`🔄 업데이트 확인 재시도... (${retryCount + 1}/${MAX_RETRIES})`);
        setTimeout(() => {
          checkForUpdates(false, retryCount + 1);
        }, RETRY_DELAY);
        return;
      }
      
      // 수동 확인이거나 재시도 횟수 초과 시 사용자에게 알림
      if (manual) {
        if (errorStr.includes('network') || errorStr.includes('fetch')) {
          await message('네트워크 연결을 확인하고 다시 시도해주세요.', { 
            title: '연결 오류', 
            kind: 'error' 
          });
        } else {
          await message(`업데이트 확인 중 오류가 발생했습니다:\n\n${errorStr}`, { 
            title: '업데이트 확인 실패', 
            kind: 'error' 
          });
        }
      }
    } finally {
      setLoading(false);
      setIsUpdating(false);
      if (manual) setStatus("");
    }
  };

  useEffect(() => {
    // 앱 시작 시 자동 업데이트 확인
    if (!hasCheckedUpdate.current) {
      hasCheckedUpdate.current = true;
      setTimeout(() => checkForUpdates(false), 2000); // 2초 후 확인
    }
    
    // 정기적인 업데이트 확인 (30분마다)
    const updateInterval = setInterval(() => {
      console.log("⏰ 정기 업데이트 확인 실행");
      checkForUpdates(false);
    }, 30 * 60 * 1000); // 30분

    const saved = localStorage.getItem("sync-items");
    const token = localStorage.getItem("dropbox-token");
    const rToken = localStorage.getItem("dropbox-refresh-token");
    
    if (saved) {
      try { 
        const parsed = JSON.parse(saved);
        const migrated = parsed.map((item: any) => ({
          ...item,
          enabled: item.enabled !== undefined ? item.enabled : true,
          refresh_token: item.refresh_token || rToken || null
        }));
        setItems(migrated); 
      } catch (e) { console.error("Failed to load saved items", e); }
    }
    if (token) setDropboxToken(token);
    if (rToken) setRefreshToken(rToken);

    const unConnect = listen("dropbox-code-received", async (event: any) => {
      setLoading(true);
      setStatus("토큰 발급 중...");
      try {
        const res: any = await invoke("exchange_code_for_token", { code: event.payload as string });
        const { access_token, refresh_token } = res;
        
        setDropboxToken(access_token);
        setRefreshToken(refresh_token);
        localStorage.setItem("dropbox-token", access_token);
        if (refresh_token) localStorage.setItem("dropbox-refresh-token", refresh_token);
        
        setStatus("연결 성공!");
        await fetchCloudFolders(access_token);
      } catch (e) { setStatus("연결 실패: " + e); }
      finally { setLoading(false); }
    });

    const unTokenUpdated = listen("tokens-updated", (event: any) => {
      const updatedPairs = event.payload as [number, string][];
      setItems(prev => {
        const next = [...prev];
        updatedPairs.forEach(([index, newToken]) => {
          if (next[index]) {
            next[index].token = newToken;
            setDropboxToken(newToken);
            localStorage.setItem("dropbox-token", newToken);
          }
        });
        localStorage.setItem("sync-items", JSON.stringify(next));
        return next;
      });
    });

    const unProgress = listen("sync-progress", (event: any) => {
      const payload = event.payload as { current_file: string; progress: number };
      setCurrentFile(payload.current_file);
      setProgress(payload.progress * 100);
    });

    const unSyncComplete = listen("sync-complete", (event: any) => {
      console.log("동기화 완료 이벤트 수신:", event.payload);
      const payload = event.payload as { success: boolean; message: string };
      setStatus(payload.message);
      setLoading(false);
      setProgress(payload.success ? 100 : 0);
      if (!payload.success) {
        setCurrentFile("");
      }
    });

    return () => {
      clearInterval(updateInterval);
      unConnect.then(f => f());
      unTokenUpdated.then(f => f());
      unProgress.then(f => f());
      unSyncComplete.then(f => f());
    };
  }, []);

  const fetchCloudFolders = async (token: string) => {
    setLoading(true);
    setStatus("클라우드 목록 가져오는 중...");
    try {
      const folders: string[] = await invoke("list_dropbox_folders", { token });
      const rToken = localStorage.getItem("dropbox-refresh-token");
      
      setItems(prevItems => {
        const existingCloudPaths = new Set(prevItems.map(i => i.cloud_path));
        const newCloudItems: SyncItem[] = folders
          .filter(path => !existingCloudPaths.has(path))
          .map(path => {
            const name = path.split('/').pop() || path;
            return { name, local_path: "", cloud_path: path, token, refresh_token: rToken, enabled: true };
          });
        const updatedItems = [...prevItems, ...newCloudItems];
        localStorage.setItem("sync-items", JSON.stringify(updatedItems));
        if (newCloudItems.length > 0) setStatus(`${newCloudItems.length}개의 새로운 게임이 발견되었습니다.`);
        else setStatus("이미 모든 클라우드 게임이 목록에 있습니다.");
        return updatedItems;
      });
    } catch (e) {
      const errStr = String(e);
      if (errStr.includes("expired_access_token") || errStr.includes("invalid_access_token") || errStr.includes("401")) {
        await handleTokenExpiration();
      } else {
        setStatus("목록 가져오기 실패: " + e);
        await message(`클라우드 목록을 가져오지 못했습니다: ${e}`, { kind: 'error' });
      }
    } finally {
      setLoading(false);
    }
  };

  const handleConnect = async () => {
    setStatus("인증 진행 중...");
    try { await invoke("open_auth_url"); } catch (e) { setStatus("에러: " + e); }
  };

  const toggleItem = (index: number) => {
    const newItems = [...items];
    newItems[index].enabled = !newItems[index].enabled;
    setItems(newItems);
    localStorage.setItem("sync-items", JSON.stringify(newItems));
  };

  const pickLocalFolder = async (index?: number) => {
    try {
      const selected: string | null = await invoke("pick_folder_dialog");
      if (selected) {
        if (index !== undefined) {
          const newItems = [...items];
          newItems[index].local_path = selected;
          newItems[index].token = dropboxToken;
          newItems[index].refresh_token = refreshToken;
          setItems(newItems);
          localStorage.setItem("sync-items", JSON.stringify(newItems));
        } else { setManualLocalPath(selected); }
      }
    } catch (e) { await message("폴더 선택 오류: " + e, { kind: 'error' }); }
  };

  const addManualItem = async () => {
    if (!newName || !manualLocalPath || !dropboxToken) {
      await message("정보를 입력하세요.", { kind: 'warning' });
      return;
    }
    const autoCloudPath = `/${newName.trim()}`;
    if (items.some(i => i.cloud_path === autoCloudPath)) {
      await message("이미 존재합니다.", { kind: 'warning' });
      return;
    }
    const newItems = [...items, { name: newName.trim(), local_path: manualLocalPath, cloud_path: autoCloudPath, token: dropboxToken, refresh_token: refreshToken, enabled: true }];
    setItems(newItems);
    localStorage.setItem("sync-items", JSON.stringify(newItems));
    setNewName(""); setManualLocalPath("");
  };

  const handleRemoveItem = async (index: number) => {
    const confirmed = await ask("목록에서 삭제할까요?", { title: '삭제 확인', kind: 'warning' });
    if (confirmed) {
      const newItems = items.filter((_, idx) => idx !== index);
      setItems(newItems);
      localStorage.setItem("sync-items", JSON.stringify(newItems));
    }
  };

  const handleCancel = async () => {
    try {
      console.log("동기화 취소 요청");
      await invoke("cancel_sync");
      setStatus("동기화가 취소되었습니다.");
      setLoading(false);
      setProgress(0);
      setCurrentFile("");
      console.log("취소 완료, 로딩창 종료");
    } catch (e) { 
      console.error("Cancel failed", e); 
      setStatus("취소 요청 실패: " + e);
      setLoading(false);
    }
  };

  const syncAll = async () => {
    if (loading) return;
    
    const rToken = localStorage.getItem("dropbox-refresh-token");
    const validItems = items.filter(i => i.local_path !== "" && i.enabled).map(i => ({
      ...i,
      token: i.token || dropboxToken,
      refresh_token: i.refresh_token || rToken
    }));

    if (validItems.length === 0) {
      await message("동기화할 항목이 없거나 활성화된 게임이 없습니다.", { kind: 'warning' });
      return;
    }
    
    setLoading(true);
    setIsUpdating(false);
    setProgress(0);
    setStatus("동기화 준비 중...");
    setCurrentFile("인증 확인 및 파일 분석 중...");
    
    let syncCompleted = false;
    
    try {
      console.log("동기화 시작...");
      const res: any = await invoke("sync_folders", { items: validItems });
      
      if (!syncCompleted) {
        console.log("동기화 함수 완료:", res);
        setStatus(res.message);
        setLoading(false);
        syncCompleted = true;
      }
    } catch (e) {
      const errStr = String(e);
      console.error("동기화 에러 발생:", errStr);
      
      // 토큰 갱신 실패로 인한 401 에러인 경우에만 세션 만료 처리
      if (errStr.includes("401") || errStr.includes("expired") || errStr.includes("토큰 갱신 실패")) {
        await handleTokenExpiration();
      } else if (errStr.includes("취소") || errStr.includes("중단")) {
        setStatus("동기화가 중단되었습니다.");
      } else {
        setStatus("실패: " + e);
        await message(`동기화 중 오류가 발생했습니다:\n${e}`, { kind: 'error' });
      }
      
      if (!syncCompleted) {
        setLoading(false);
        syncCompleted = true;
      }
    }
    
    // 안전장치: 20초 후에도 로딩중이면 강제 종료
    setTimeout(() => {
      if (!syncCompleted) {
        console.log("타임아웃으로 로딩 종료");
        setLoading(false);
        syncCompleted = true;
      }
    }, 20000);
  };

  return (
    <div className="container">
      {loading && (
        <div className="loading-overlay">
          <div className="loading-card">
            <div className="spinner"></div>
            <h3>
              {isUpdating ? "🔄 업데이트 진행 중..." : "📁 동기화 진행 중..."}
            </h3>
            <div className="progress-container">
              <div className="progress-bar" style={{ 
                width: `${progress}%`,
                background: isUpdating ? "#2196F3" : "#4CAF50"
              }}></div>
            </div>
            {isUpdating ? (
              <div>
                <p className="progress-text">
                  {Math.round(progress)}% 다운로드 완료
                </p>
                <small style={{color: "#666", display: "block", marginTop: "5px"}}>
                  업데이트가 완료되면 자동으로 재시작됩니다.
                </small>
              </div>
            ) : (
              <div>
                <p className="current-file">{currentFile}</p>
                <p className="progress-text">{Math.round(progress)}% 완료</p>
                <button onClick={handleCancel} className="cancel-btn">동기화 취소</button>
              </div>
            )}
          </div>
        </div>
      )}

      <header>
        <div style={{textAlign: "center"}}>
          <h1>CloudGameSaver</h1>
          <p className="subtitle">드롭박스와 게임 세이브를 완벽하게 동기화하세요.</p>
        </div>
      </header>
      
      <div className="settings-section">
        <div style={{display: "flex", justifyContent: "space-between", alignItems: "center", gap: "10px", flexWrap: "wrap"}}>
          <div style={{display: "flex", gap: "8px", flexShrink: 1}}>
            <button onClick={handleConnect} className="auth-btn" style={{
              padding: "8px 12px", 
              fontSize: "14px",
              maxWidth: dropboxToken ? "120px" : "150px",
              whiteSpace: "nowrap",
              overflow: "hidden",
              textOverflow: "ellipsis"
            }}>
              {dropboxToken ? "✅ 연결됨" : "🔗 드롭박스 연결"}
            </button>
            {dropboxToken && (
              <button 
                onClick={() => fetchCloudFolders(dropboxToken)} 
                className="secondary-btn"
                style={{
                  padding: "8px 12px",
                  fontSize: "14px",
                  width: "90px",
                  whiteSpace: "nowrap"
                }}
              >
                목록 갱신
              </button>
            )}
          </div>
          <button 
            onClick={() => checkForUpdates(true)} 
            disabled={loading} 
            className="update-check-btn" 
            style={{
              padding: "8px 12px", 
              fontSize: "14px",
              background: loading ? "#ddd" : "#2196F3",
              border: "none",
              borderRadius: "6px",
              color: "white",
              cursor: loading ? "not-allowed" : "pointer",
              display: "flex",
              alignItems: "center",
              gap: "6px",
              width: "140px",
              justifyContent: "center",
              transition: "all 0.2s",
              flexShrink: 0,
              whiteSpace: "nowrap"
            }}
            onMouseEnter={(e) => {
              if (!loading) e.currentTarget.style.background = "#1976D2";
            }}
            onMouseLeave={(e) => {
              if (!loading) e.currentTarget.style.background = "#2196F3";
            }}
          >
            🔄 업데이트 확인
          </button>
        </div>
      </div>

      <div className="add-section">
        <h3>➕ 새 게임 추가</h3>
        <div className="input-row">
          <input placeholder="게임 이름" value={newName} onChange={e=>setNewName(e.target.value)} />
          <button onClick={() => pickLocalFolder()} className="secondary-btn">로컬 폴더 선택</button>
        </div>
        <p className="path-text">{manualLocalPath || "PC의 세이브 폴더를 선택하세요"}</p>
        <button onClick={addManualItem} className="add-btn" disabled={!newName || !manualLocalPath}>등록하기</button>
      </div>

      <div className="list-section">
        <h3>관리 목록</h3>
        {items.map((item, i) => (
          <div key={i} className={`sync-item-card ${item.local_path ? 'active' : ''} ${!item.enabled ? 'disabled' : ''}`}>
            <div className="item-main">
              <div className="toggle-wrapper">
                <label className="switch">
                  <input type="checkbox" checked={item.enabled} onChange={() => toggleItem(i)} />
                  <span className="slider round"></span>
                </label>
              </div>
              <div className="item-details">
                <strong>{item.name}</strong>
                <small>Cloud: {item.cloud_path}</small>
                <small className="path-display">Local: {item.local_path || "⚠️ 연결되지 않음"}</small>
              </div>
            </div>
            <div style={{display: "flex", gap: "5px"}}>
              <button onClick={() => pickLocalFolder(i)} className="secondary-btn">연결</button>
              <button onClick={() => handleRemoveItem(i)} className="delete-btn">삭제</button>
            </div>
          </div>
        ))}
      </div>

      <div className="action-section">
        <button className="sync-btn" onClick={syncAll} disabled={loading}>
          {loading && !isUpdating ? "동기화 진행 중..." : "동기화 시작"}
        </button>
        <div className="status-box">
          <pre>{status || "준비 완료"}</pre>
          {status && status.includes("완료") && !loading && (
            <small style={{color: "#4CAF50", display: "block", marginTop: "5px"}}>
              ✅ 작업이 성공적으로 완료되었습니다.
            </small>
          )}
        </div>
      </div>
    </div>
  );
}
