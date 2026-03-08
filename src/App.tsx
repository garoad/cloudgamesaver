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

  // 업데이트 확인 로직
  const checkForUpdates = async (manual = false) => {
    if (loading) return;
    try {
      if (manual) setStatus("업데이트 확인 중...");
      const update = await check();
      if (update) {
        if (manual) setStatus(""); 
        const shouldUpdate = await ask(
          `새로운 버전(${update.version})이 있습니다. 업데이트하시겠습니까?\n\n내용: ${update.body}`,
          { title: '업데이트 알림', kind: 'info' }
        );
        if (shouldUpdate) {
          setLoading(true);
          setIsUpdating(true);
          setProgress(0);
          let downloaded = 0;
          let contentLength = 0;
          await update.downloadAndInstall((event) => {
            switch (event.event) {
              case 'Started':
                contentLength = event.data.contentLength || 0;
                setStatus("업데이트 다운로드 중...");
                break;
              case 'Progress':
                downloaded += event.data.chunkLength;
                if (contentLength > 0) setProgress((downloaded / contentLength) * 100);
                break;
              case 'Finished':
                setStatus("업데이트 완료! 재시작합니다.");
                break;
            }
          });
          await relaunch();
        }
      } else {
        if (manual) await message("현재 최신 버전을 사용 중입니다.", { title: '업데이트 확인', kind: 'info' });
      }
    } catch (e) {
      console.error("Update check error:", e);
      const errorStr = String(e);
      if (manual && !errorStr.includes("404") && !errorStr.includes("Download request failed")) {
        await message(`업데이트 확인 중 오류가 발생했습니다: ${e}`, { title: '에러', kind: 'error' });
      }
    } finally {
      setLoading(false);
      setIsUpdating(false);
      if (manual) setStatus("");
    }
  };

  useEffect(() => {
    if (!hasCheckedUpdate.current) {
      hasCheckedUpdate.current = true;
      setTimeout(() => checkForUpdates(false), 1000);
    }

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

    return () => {
      unConnect.then(f => f());
      unTokenUpdated.then(f => f());
      unProgress.then(f => f());
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
      await invoke("cancel_sync");
      setStatus("취소 요청 중...");
    } catch (e) { console.error("Cancel failed", e); }
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
    setCurrentFile("파일 분석 중...");
    try {
      const res: any = await invoke("sync_folders", { items: validItems });
      setStatus(res.message);
    } catch (e) {
      const errStr = String(e);
      if (errStr.includes("expired_access_token") || errStr.includes("invalid_access_token") || errStr.includes("401")) {
        await handleTokenExpiration();
      } else {
        setStatus("실패: " + e);
      }
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="container">
      {loading && (
        <div className="loading-overlay">
          <div className="loading-card">
            <div className="spinner"></div>
            <h3>{isUpdating ? "업데이트 진행 중..." : "동기화 진행 중..."}</h3>
            <div className="progress-container">
              <div className="progress-bar" style={{ width: `${progress}%` }}></div>
            </div>
            {!isUpdating && <p className="current-file">{currentFile}</p>}
            <p className="progress-text">{Math.round(progress)}% 완료</p>
            {!isUpdating && <button onClick={handleCancel} className="cancel-btn">동기화 취소</button>}
          </div>
        </div>
      )}

      <header>
        <h1>CloudGameSaver</h1>
        <p className="subtitle">드롭박스와 게임 세이브를 완벽하게 동기화하세요.</p>
      </header>
      
      <div className="settings-section">
        <div style={{display: "flex", justifyContent: "space-between", alignItems: "center"}}>
          <button onClick={handleConnect} className="auth-btn" style={{width: "auto"}}>
            {dropboxToken ? "✅ 드롭박스 연결됨" : "🔗 드롭박스 연결하기"}
          </button>
          {dropboxToken && <button onClick={() => fetchCloudFolders(dropboxToken)} className="secondary-btn">목록 갱신</button>}
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
        <button className="sync-btn" onClick={syncAll} disabled={loading}>동기화 시작</button>
        <button className="update-check-btn" onClick={() => checkForUpdates(true)} disabled={loading}>🔄 업데이트 확인</button>
        <div className="status-box"><pre>{status || "준비 완료"}</pre></div>
      </div>
    </div>
  );
}
