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

  // 추가 필드 state
  const [newName, setNewName] = useState("");
  const [manualLocalPath, setManualLocalPath] = useState("");

  // 전체 로그 관리 state
  const [logs, setLogs] = useState<string[]>([]);
  const [showFullLogs, setShowFullLogs] = useState(false);

  // 편집 / 삭제 모달 state
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [editName, setEditName] = useState("");
  const [editLocalPath, setEditLocalPath] = useState("");

  const [deletingIndex, setDeletingIndex] = useState<number | null>(null);
  const [deleteFromCloud, setDeleteFromCloud] = useState(false);
  const [isDeletingCloud, setIsDeletingCloud] = useState(false);

  const hasCheckedUpdate = useRef(false);
  const logContainerRef = useRef<HTMLDivElement>(null);

  // 로그 기록 함수
  const addLog = (msg: string) => {
    const time = new Date().toLocaleTimeString("ko-KR", { hour12: false });
    const formatted = `[${time}] ${msg}`;
    setLogs((prev) => [...prev.slice(-499), formatted]);
  };

  // status 변경 시 자동으로 최근 로그 기록
  const updateStatusAndLog = (msg: string) => {
    setStatus(msg);
    if (msg) addLog(msg);
  };

  // 토큰 만료 처리 함수
  const handleTokenExpiration = async () => {
    setDropboxToken("");
    setRefreshToken(null);
    localStorage.removeItem("dropbox-token");
    localStorage.removeItem("dropbox-refresh-token");
    updateStatusAndLog("세션이 만료되었습니다. 드롭박스를 다시 연결해주세요.");
    await message("드롭박스 인증 세션이 만료되었습니다. 보안을 위해 다시 연결이 필요합니다.", { title: "알림", kind: "warning" });
  };

  // 업데이트 확인 로직
  const checkForUpdates = async (manual = false, retryCount = 0) => {
    if (loading) return;
    
    const MAX_RETRIES = 3;
    const RETRY_DELAY = 2000;
    
    try {
      if (manual) updateStatusAndLog("업데이트 확인 중...");
      else if (retryCount === 0) addLog("🔄 자동 업데이트 확인 중...");
      
      const update = await check();
      
      if (update) {
        addLog(`✅ 새 버전 발견: v${update.version}`);
        if (manual) setStatus(""); 
        const shouldUpdate = await ask(
          `새로운 버전(${update.version})이 있습니다. 업데이트하시겠습니까?\n\n변경사항:\n${update.body || "개선사항 및 버그 수정"}`,
          { title: "업데이트 발견!", kind: "info" }
        );
        
        if (shouldUpdate) {
          setLoading(true);
          setIsUpdating(true);
          setProgress(0);
          updateStatusAndLog("업데이트 다운로드 중...");
          
          let downloaded = 0;
          let contentLength = 0;
          
          try {
            await update.downloadAndInstall((event) => {
              switch (event.event) {
                case "Started":
                  contentLength = event.data.contentLength || 0;
                  updateStatusAndLog(`업데이트 다운로드 중... (${Math.round(contentLength / 1024 / 1024)}MB)`);
                  break;
                case "Progress":
                  downloaded += event.data.chunkLength;
                  if (contentLength > 0) {
                    const progressPercent = (downloaded / contentLength) * 100;
                    setProgress(progressPercent);
                  }
                  break;
                case "Finished":
                  updateStatusAndLog("설치 완료! 잠시 후 재시작됩니다...");
                  break;
              }
            });
            
            setTimeout(async () => {
              await relaunch();
            }, 1500);
            
          } catch (installError) {
            addLog(`❌ 설치 실패: ${installError}`);
            await message(`업데이트 설치 중 오류가 발생했습니다:\n\n${installError}`, { 
              title: "설치 에러", 
              kind: "error" 
            });
          }
        }
      } else {
        addLog("✅ 최신 버전 사용 중");
        if (manual) {
          await message("현재 최신 버전을 사용 중입니다.", { 
            title: "업데이트 확인", 
            kind: "info" 
          });
        }
      }
    } catch (e) {
      const errorStr = String(e);
      addLog(`❌ 업데이트 확인 오류: ${errorStr}`);
      
      if (retryCount < MAX_RETRIES && !manual) {
        setTimeout(() => {
          checkForUpdates(false, retryCount + 1);
        }, RETRY_DELAY);
        return;
      }
      
      if (manual) {
        if (errorStr.includes("network") || errorStr.includes("fetch")) {
          await message("네트워크 연결을 확인하고 다시 시도해주세요.", { 
            title: "연결 오류", 
            kind: "error" 
          });
        } else {
          await message(`업데이트 확인 중 오류가 발생했습니다:\n\n${errorStr}`, { 
            title: "업데이트 확인 실패", 
            kind: "error" 
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
    addLog("CloudGameSaver 앱이 실행되었습니다.");

    if (!hasCheckedUpdate.current) {
      hasCheckedUpdate.current = true;
      setTimeout(() => checkForUpdates(false), 2000);
    }
    
    const updateInterval = setInterval(() => {
      checkForUpdates(false);
    }, 30 * 60 * 1000);

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
        addLog(`저장된 ${migrated.length}개의 게임 목록을 로드했습니다.`);
      } catch (e) { 
        console.error("Failed to load saved items", e); 
      }
    }
    if (token) setDropboxToken(token);
    if (rToken) setRefreshToken(rToken);

    const unConnect = listen("dropbox-code-received", async (event: any) => {
      setLoading(true);
      updateStatusAndLog("토큰 발급 중...");
      try {
        const res: any = await invoke("exchange_code_for_token", { code: event.payload as string });
        const { access_token, refresh_token } = res;
        
        setDropboxToken(access_token);
        setRefreshToken(refresh_token);
        localStorage.setItem("dropbox-token", access_token);
        if (refresh_token) localStorage.setItem("dropbox-refresh-token", refresh_token);
        
        updateStatusAndLog("드롭박스 연결 성공!");
        await fetchCloudFolders(access_token);
      } catch (e) { 
        updateStatusAndLog("연결 실패: " + e); 
      } finally { 
        setLoading(false); 
      }
    });

    const unTokenUpdated = listen("tokens-updated", (event: any) => {
      const updatedPairs = event.payload as [number, string][];
      addLog(`토큰 갱신 완료 (${updatedPairs.length}개 항목)`);
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
      if (payload.current_file) {
        addLog(`[진행] ${payload.current_file}`);
      }
    });

    const unSyncLog = listen("sync-log", (event: any) => {
      if (event.payload) {
        addLog(event.payload as string);
      }
    });

    const unSyncComplete = listen("sync-complete", (event: any) => {
      const payload = event.payload as { success: boolean; message: string };
      updateStatusAndLog(payload.message);
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
      unSyncLog.then(f => f());
      unSyncComplete.then(f => f());
    };
  }, []);

  // 풀로그 모달 자동 스크롤
  useEffect(() => {
    if (showFullLogs && logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [logs, showFullLogs]);

  const fetchCloudFolders = async (token: string, isRetry = false) => {
    setLoading(true);
    if (!isRetry) updateStatusAndLog("클라우드 목록 가져오는 중...");

    const rToken = localStorage.getItem("dropbox-refresh-token");

    try {
      const result: { folders: string[], new_token: string | null } = await invoke("list_dropbox_folders", { token, refreshToken: rToken });

      if (result.new_token) {
        setDropboxToken(result.new_token);
        localStorage.setItem("dropbox-token", result.new_token);
        addLog("드롭박스 토큰이 자동으로 갱신되었습니다.");
      }

      setItems(prevItems => {
        const existingCloudPaths = new Set(prevItems.map(i => i.cloud_path));
        const newCloudItems: SyncItem[] = result.folders
          .filter(path => !existingCloudPaths.has(path))
          .map(path => {
            const name = path.split("/").pop() || path;
            return {
              name,
              local_path: "",
              cloud_path: path,
              token: result.new_token || token,
              refresh_token: rToken,
              enabled: true
            };
          });
        const updatedItems = [...prevItems, ...newCloudItems];
        localStorage.setItem("sync-items", JSON.stringify(updatedItems));
        if (newCloudItems.length > 0) {
          updateStatusAndLog(`${newCloudItems.length}개의 새로운 클라우드 게임이 목록에 추가되었습니다.`);
        } else {
          updateStatusAndLog("이미 모든 클라우드 게임이 목록에 등록되어 있습니다.");
        }
        return updatedItems;
      });
    } catch (e) {
      const errStr = String(e);
      if (errStr.includes("expired_access_token") || errStr.includes("invalid_access_token") || errStr.includes("401")) {
        await handleTokenExpiration();
      } else {
        updateStatusAndLog("목록 가져오기 실패: " + e);
        await message(`클라우드 목록을 가져오지 못했습니다: ${e}`, { kind: "error" });
      }
    } finally {
      setLoading(false);
    }
  };

  const handleConnect = async () => {
    updateStatusAndLog("드롭박스 인증 진행 중...");
    try { 
      await invoke("open_auth_url"); 
    } catch (e) { 
      updateStatusAndLog("인증 에러: " + e); 
    }
  };

  const toggleItem = (index: number) => {
    const newItems = [...items];
    newItems[index].enabled = !newItems[index].enabled;
    setItems(newItems);
    localStorage.setItem("sync-items", JSON.stringify(newItems));
    addLog(`'${newItems[index].name}' 동기화 ${newItems[index].enabled ? '활성화' : '비활성화'}`);
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
          addLog(`'${newItems[index].name}' 로컬 폴더 연결 완료: ${selected}`);
        } else { 
          setManualLocalPath(selected); 
        }
      }
    } catch (e) { 
      await message("폴더 선택 오류: " + e, { kind: "error" }); 
    }
  };

  const addManualItem = async () => {
    if (!newName.trim() || !manualLocalPath) {
      await message("게임 이름과 로컬 폴더를 모두 설정해주세요.", { kind: "warning" });
      return;
    }
    const autoCloudPath = `/${newName.trim()}`;
    if (items.some(i => i.cloud_path.toLowerCase() === autoCloudPath.toLowerCase())) {
      await message("이미 존재하는 게임 이름입니다.", { kind: "warning" });
      return;
    }
    const newItems = [...items, { 
      name: newName.trim(), 
      local_path: manualLocalPath, 
      cloud_path: autoCloudPath, 
      token: dropboxToken, 
      refresh_token: refreshToken, 
      enabled: true 
    }];
    setItems(newItems);
    localStorage.setItem("sync-items", JSON.stringify(newItems));
    addLog(`➕ 새 게임 등록 완료: ${newName.trim()} (${manualLocalPath})`);
    setNewName(""); 
    setManualLocalPath("");
  };

  // 편집 모달 열기
  const openEditModal = (index: number) => {
    setEditingIndex(index);
    setEditName(items[index].name);
    setEditLocalPath(items[index].local_path);
  };

  // 편집 저장
  const saveEditItem = () => {
    if (editingIndex === null) return;
    if (!editName.trim()) {
      message("게임 이름을 입력해 주세요.", { kind: "warning" });
      return;
    }
    const newItems = [...items];
    newItems[editingIndex] = {
      ...newItems[editingIndex],
      name: editName.trim(),
      local_path: editLocalPath
    };
    setItems(newItems);
    localStorage.setItem("sync-items", JSON.stringify(newItems));
    addLog(`✏️ '${editName.trim()}' 항목 정보가 수정되었습니다.`);
    setEditingIndex(null);
  };

  // 삭제 모달 열기
  const openDeleteModal = (index: number) => {
    setDeletingIndex(index);
    setDeleteFromCloud(false);
  };

  // 삭제 실행 (드롭박스 삭제 옵션 포함)
  const confirmAndDeleteItem = async () => {
    if (deletingIndex === null) return;
    const targetItem = items[deletingIndex];

    if (deleteFromCloud) {
      setIsDeletingCloud(true);
      addLog(`🗑️ 드롭박스 클라우드 폴더 삭제 요청: ${targetItem.cloud_path}`);
      try {
        const rToken = localStorage.getItem("dropbox-refresh-token");
        const res: string | null = await invoke("delete_dropbox_folder", {
          path: targetItem.cloud_path,
          token: targetItem.token || dropboxToken,
          refreshToken: targetItem.refresh_token || rToken
        });

        if (res) {
          setDropboxToken(res);
          localStorage.setItem("dropbox-token", res);
          addLog("드롭박스 인증 토큰이 갱신되었습니다.");
        }
        addLog(`✅ 드롭박스 클라우드 폴더 삭제 완료: ${targetItem.cloud_path}`);
      } catch (e) {
        addLog(`❌ 드롭박스 클라우드 삭제 실패: ${e}`);
        await message(`드롭박스 클라우드 폴더를 삭제하는 도중 오류가 발생했습니다:\n${e}`, { kind: "error" });
        setIsDeletingCloud(false);
        return;
      }
      setIsDeletingCloud(false);
    }

    const newItems = items.filter((_, idx) => idx !== deletingIndex);
    setItems(newItems);
    localStorage.setItem("sync-items", JSON.stringify(newItems));
    addLog(`🗑️ '${targetItem.name}' 항목이 동기화 목록에서 제거되었습니다.`);
    setDeletingIndex(null);
    setDeleteFromCloud(false);
  };

  const handleCancel = async () => {
    try {
      addLog("동기화 취소 요청 중...");
      await invoke("cancel_sync");
      updateStatusAndLog("동기화가 취소되었습니다.");
      setLoading(false);
      setProgress(0);
      setCurrentFile("");
    } catch (e) { 
      updateStatusAndLog("취소 요청 실패: " + e);
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
      await message("동기화할 항목이 없거나 활성화된 게임이 없습니다. 로컬 폴더를 연결해주세요.", { kind: "warning" });
      return;
    }
    
    setLoading(true);
    setIsUpdating(false);
    setProgress(0);
    updateStatusAndLog("동기화 준비 중...");
    setCurrentFile("인증 확인 및 파일 분석 중...");
    
    let syncCompleted = false;
    
    try {
      addLog(`🚀 ${validItems.length}개 게임 동기화 시작...`);
      const res: any = await invoke("sync_folders", { items: validItems });
      
      if (!syncCompleted) {
        updateStatusAndLog(res.message);
        setLoading(false);
        syncCompleted = true;
      }
    } catch (e) {
      const errStr = String(e);
      addLog(`❌ 동기화 중 에러: ${errStr}`);
      
      if (errStr.includes("다시 로그인") || errStr.includes("세션이 만료")) {
        await handleTokenExpiration();
      } else if (errStr.includes("취소") || errStr.includes("중단")) {
        updateStatusAndLog("동기화가 중단되었습니다.");
      } else {
        updateStatusAndLog("실패: " + e);
        await message(`동기화 중 오류가 발생했습니다:\n${e}`, { kind: "error" });
      }
      
      if (!syncCompleted) {
        setLoading(false);
        syncCompleted = true;
      }
    }
  };

  const activeCount = items.filter(i => i.enabled && i.local_path).length;

  return (
    <div className="app-layout">
      {/* 로딩 오버레이 */}
      {loading && (
        <div className="loading-overlay">
          <div className="loading-card">
            <div className="spinner"></div>
            <h3>{isUpdating ? "🔄 업데이트 진행 중..." : "📁 동기화 진행 중..."}</h3>
            <div className="progress-container">
              <div 
                className="progress-bar" 
                style={{ 
                  width: `${progress}%`,
                  background: isUpdating ? "#3b82f6" : "#10b981"
                }}
              ></div>
            </div>
            {isUpdating ? (
              <div>
                <p className="progress-text">{Math.round(progress)}% 다운로드 완료</p>
                <small className="sub-note">업데이트가 완료되면 자동으로 재시작됩니다.</small>
              </div>
            ) : (
              <div>
                <p className="current-file">{currentFile || "파일 분석 중..."}</p>
                <p className="progress-text">{Math.round(progress)}% 완료</p>
                <button onClick={handleCancel} className="cancel-btn">동기화 취소</button>
              </div>
            )}
          </div>
        </div>
      )}

      {/* 왼쪽 고정 사이드바 (설정, 동기화 실행, 로그) */}
      <aside className="sidebar-left">
        <div className="brand-header">
          <div className="brand-title-row">
            <h1>CloudGameSaver</h1>
            <span className="version-badge">v0.2.0</span>
          </div>
          <p className="subtitle">드롭박스 세이브파일 완벽 동기화</p>
        </div>

        <div className="sidebar-section">
          <div className="section-title">⚙️ 설정 및 연결</div>
          <div className="button-group-vertical">
            <button onClick={handleConnect} className={`auth-btn ${dropboxToken ? 'connected' : ''}`}>
              {dropboxToken ? "✅ 드롭박스 연결됨" : "🔗 드롭박스 계정 연결"}
            </button>
            <button 
              onClick={() => checkForUpdates(true)} 
              disabled={loading} 
              className="secondary-action-btn"
            >
              🔄 업데이트 확인
            </button>
          </div>
        </div>

        <div className="sidebar-section sync-action-box">
          <div className="section-title">⚡ 동기화 실행</div>
          <button 
            className="main-sync-btn" 
            onClick={syncAll} 
            disabled={loading || !dropboxToken}
          >
            {loading && !isUpdating ? "🔄 동기화 진행 중..." : "🚀 전체 게임 동기화 시작"}
          </button>
          <small className="sync-info-text">
            {dropboxToken ? `활성화된 ${activeCount}개 게임 동기화` : "드롭박스를 연결해주세요."}
          </small>
        </div>

        <div className="sidebar-section log-section flex-1">
          <div className="log-header-row">
            <div className="section-title">📜 동기화 상태 및 로그</div>
            <button 
              onClick={() => setShowFullLogs(true)} 
              className="full-log-btn"
            >
              📋 전체 로그 보기
            </button>
          </div>
          
          <div className="status-box">
            <pre className="status-preview">{status || "동기화 준비 완료"}</pre>
            {currentFile && loading && (
              <small className="live-file-text">⏳ {currentFile}</small>
            )}
            {status && status.includes("완료") && !loading && (
              <small className="success-text">✅ 작업이 성공적으로 완료되었습니다.</small>
            )}
          </div>
        </div>
      </aside>

      {/* 오른쪽 메인 영역 (게임 추가, 동기화 리스트) */}
      <main className="main-content">
        {/* 새 게임 추가 영역 */}
        <section className="card-section">
          <div className="card-header">
            <h3>➕ 새 게임 동기화 추가</h3>
          </div>
          <div className="add-form-grid">
            <input 
              placeholder="게임 이름 (예: Cyberpunk 2077)" 
              value={newName} 
              onChange={e => setNewName(e.target.value)} 
              className="form-input"
            />
            <div className="folder-select-row">
              <input 
                readOnly 
                placeholder="PC의 세이브 폴더 경로" 
                value={manualLocalPath} 
                className="form-input path-input"
              />
              <button onClick={() => pickLocalFolder()} className="secondary-btn">
                📁 폴더 선택
              </button>
            </div>
            <button 
              onClick={addManualItem} 
              className="add-btn" 
              disabled={!newName.trim() || !manualLocalPath}
            >
              등록하기
            </button>
          </div>
        </section>

        {/* 게임 리스트 영역 */}
        <section className="card-section flex-1 list-section-container">
          <div className="card-header list-header">
            <div>
              <h3>🎮 게임 동기화 관리 목록</h3>
              <span className="count-badge">총 {items.length}개 중 {activeCount}개 준비됨</span>
            </div>
            {dropboxToken && (
              <button 
                onClick={() => fetchCloudFolders(dropboxToken)} 
                className="secondary-btn refresh-btn"
                disabled={loading}
              >
                🔄 클라우드 목록 갱신
              </button>
            )}
          </div>

          <div className="game-list-scroll">
            {items.length === 0 ? (
              <div className="empty-state">
                <p>등록된 게임이 없습니다.</p>
                <small>상단에서 게임을 추가하거나 "클라우드 목록 갱신"을 눌러보세요.</small>
              </div>
            ) : (
              items.map((item, i) => (
                <div 
                  key={i} 
                  className={`game-card ${item.local_path ? "connected" : "disconnected"} ${!item.enabled ? "disabled" : ""}`}
                >
                  <div className="game-card-left">
                    <label className="switch">
                      <input 
                        type="checkbox" 
                        checked={item.enabled} 
                        onChange={() => toggleItem(i)} 
                      />
                      <span className="slider round"></span>
                    </label>

                    <div className="game-info">
                      <strong className="game-name">{item.name}</strong>
                      <div className="path-tags">
                        <span className="tag cloud-tag">Cloud: {item.cloud_path}</span>
                        <span className={`tag local-tag ${item.local_path ? '' : 'warn'}`}>
                          Local: {item.local_path || "⚠️ 로컬 폴더 연결 안 됨"}
                        </span>
                      </div>
                    </div>
                  </div>

                  <div className="game-card-actions">
                    <button 
                      onClick={() => pickLocalFolder(i)} 
                      className="action-icon-btn" 
                      title="로컬 폴더 연결"
                    >
                      📁 연결
                    </button>
                    <button 
                      onClick={() => openEditModal(i)} 
                      className="action-icon-btn edit-btn" 
                      title="게임 정보 편집"
                    >
                      ✏️ 편집
                    </button>
                    <button 
                      onClick={() => openDeleteModal(i)} 
                      className="action-icon-btn delete-btn" 
                      title="게임 삭제"
                    >
                      🗑️ 삭제
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>
        </section>
      </main>

      {/* 풀 로그 모달 */}
      {showFullLogs && (
        <div className="modal-overlay" onClick={() => setShowFullLogs(false)}>
          <div className="modal-card log-modal-card" onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <h3>📜 전체 실행 및 동기화 로그</h3>
              <button className="close-btn" onClick={() => setShowFullLogs(false)}>✕</button>
            </div>
            <div className="log-content-box" ref={logContainerRef}>
              {logs.length === 0 ? (
                <p className="log-empty">기록된 로그가 없습니다.</p>
              ) : (
                logs.map((log, index) => (
                  <div key={index} className="log-line">{log}</div>
                ))
              )}
            </div>
            <div className="modal-footer">
              <button 
                className="secondary-btn"
                onClick={() => {
                  navigator.clipboard.writeText(logs.join("\n"));
                  message("로그가 클립보드에 복사되었습니다.", { title: "복사 완료", kind: "info" });
                }}
              >
                📋 로그 복사
              </button>
              <button 
                className="secondary-btn"
                onClick={() => setLogs([])}
              >
                🧹 로그 지우기
              </button>
              <button 
                className="primary-modal-btn"
                onClick={() => setShowFullLogs(false)}
              >
                닫기
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 편집 모달 */}
      {editingIndex !== null && (
        <div className="modal-overlay" onClick={() => setEditingIndex(null)}>
          <div className="modal-card" onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <h3>✏️ 게임 정보 편집</h3>
              <button className="close-btn" onClick={() => setEditingIndex(null)}>✕</button>
            </div>
            <div className="modal-body">
              <label className="input-label">게임 이름</label>
              <input 
                value={editName} 
                onChange={e => setEditName(e.target.value)} 
                className="form-input"
              />
              
              <label className="input-label">로컬 세이브 폴더 경로</label>
              <div className="folder-select-row">
                <input 
                  readOnly
                  value={editLocalPath} 
                  placeholder="로컬 폴더 경로"
                  className="form-input path-input"
                />
                <button 
                  onClick={async () => {
                    const selected: string | null = await invoke("pick_folder_dialog");
                    if (selected) setEditLocalPath(selected);
                  }} 
                  className="secondary-btn"
                >
                  📁 변경
                </button>
              </div>
            </div>
            <div className="modal-footer">
              <button className="secondary-btn" onClick={() => setEditingIndex(null)}>취소</button>
              <button className="primary-modal-btn" onClick={saveEditItem}>저장하기</button>
            </div>
          </div>
        </div>
      )}

      {/* 삭제 모달 */}
      {deletingIndex !== null && (
        <div className="modal-overlay" onClick={() => setDeletingIndex(null)}>
          <div className="modal-card" onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <h3>🗑️ 게임 삭제 확인</h3>
              <button className="close-btn" onClick={() => setDeletingIndex(null)}>✕</button>
            </div>
            <div className="modal-body">
              <p className="delete-warning-text">
                '<strong>{items[deletingIndex].name}</strong>' 항목을 목록에서 삭제하시겠습니까?
              </p>

              <div className="cloud-delete-option">
                <label className="checkbox-label">
                  <input 
                    type="checkbox" 
                    checked={deleteFromCloud} 
                    onChange={e => setDeleteFromCloud(e.target.checked)} 
                  />
                  <span>☁️ 드롭박스 클라우드 폴더({items[deletingIndex].cloud_path})도 함께 삭제</span>
                </label>
                <small className="cloud-delete-tip">
                  (드롭박스에서 삭제하면 '클라우드 목록 갱신'을 해도 다시 목록에 나타나지 않습니다.)
                </small>
              </div>
            </div>
            <div className="modal-footer">
              <button className="secondary-btn" onClick={() => setDeletingIndex(null)}>취소</button>
              <button 
                className="danger-modal-btn" 
                onClick={confirmAndDeleteItem}
                disabled={isDeletingCloud}
              >
                {isDeletingCloud ? "삭제 진행 중..." : "삭제하기"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
