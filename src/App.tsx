import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import "./App.css";

interface SyncItem {
  name: string;
  local_path: string;
  cloud_path: string;
  token: string;
  enabled: boolean;
}

export default function App() {
  const [items, setItems] = useState<SyncItem[]>([]);
  const [dropboxToken, setDropboxToken] = useState("");
  const [status, setStatus] = useState("");
  const [loading, setLoading] = useState(false);
  const [progress, setProgress] = useState(0);
  const [currentFile, setCurrentFile] = useState("");

  const [newName, setNewName] = useState("");
  const [manualLocalPath, setManualLocalPath] = useState("");

  useEffect(() => {
    // 업데이트 확인
    const checkForUpdates = async () => {
      try {
        const update = await check();
        if (update) {
          console.log(`Update found: ${update.version}`);
          if (confirm(`새로운 버전(${update.version})이 있습니다. 업데이트하시겠습니까?\n\n내용: ${update.body}`)) {
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
                  if (contentLength > 0) {
                    setProgress((downloaded / contentLength) * 100);
                  }
                  break;
                case 'Finished':
                  setStatus("업데이트 완료! 재시작합니다.");
                  break;
              }
            });
            await relaunch();
          }
        }
      } catch (e) {
        console.error("Update check failed", e);
      }
    };
    checkForUpdates();

    const saved = localStorage.getItem("sync-items");
    const token = localStorage.getItem("dropbox-token");
    if (saved) {
      try { 
        const parsed = JSON.parse(saved);
        const migrated = parsed.map((item: any) => ({
          ...item,
          enabled: item.enabled !== undefined ? item.enabled : true
        }));
        setItems(migrated); 
      } catch (e) {
        console.error("Failed to load saved items", e);
      }
    }
    if (token) setDropboxToken(token);

    const unConnect = listen("dropbox-code-received", async (event: any) => {
      const code = event.payload as string;
      setLoading(true);
      setStatus("토큰 발급 중...");
      try {
        const token: string = await invoke("exchange_code_for_token", { code });
        setDropboxToken(token);
        localStorage.setItem("dropbox-token", token);
        setStatus("연결 성공!");
        await fetchCloudFolders(token);
      } catch (e) {
        setStatus("연결 실패: " + e);
      } finally {
        setLoading(false);
      }
    });

    const unProgress = listen("sync-progress", (event: any) => {
      const payload = event.payload as { current_file: string; progress: number };
      setCurrentFile(payload.current_file);
      setProgress(payload.progress * 100);
    });

    return () => {
      unConnect.then(f => f());
      unProgress.then(f => f());
    };
  }, []);

  const fetchCloudFolders = async (token: string) => {
    setLoading(true);
    try {
      const folders: string[] = await invoke("list_dropbox_folders", { token });
      const currentItems = [...items];
      const cloudItems: SyncItem[] = folders.map(path => {
        const name = path.split('/').pop() || path;
        const existing = currentItems.find(i => i.cloud_path === path);
        return existing || { name, local_path: "", cloud_path: path, token, enabled: true };
      });
      const manualItems = currentItems.filter(i => !folders.includes(i.cloud_path));
      const finalItems = [...cloudItems, ...manualItems];
      setItems(finalItems);
      localStorage.setItem("sync-items", JSON.stringify(finalItems));
    } catch (e) {
      setStatus("목록 가져오기 실패: " + e);
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
          setItems(newItems);
          localStorage.setItem("sync-items", JSON.stringify(newItems));
        } else {
          setManualLocalPath(selected);
        }
      }
    } catch (e) { alert("폴더 선택 오류: " + e); }
  };

  const addManualItem = () => {
    if (!newName || !manualLocalPath || !dropboxToken) return alert("정보를 입력하세요.");
    const autoCloudPath = `/${newName.trim()}`;
    if (items.some(i => i.cloud_path === autoCloudPath)) return alert("이미 존재합니다.");
    const newItems = [...items, { name: newName.trim(), local_path: manualLocalPath, cloud_path: autoCloudPath, token: dropboxToken, enabled: true }];
    setItems(newItems);
    localStorage.setItem("sync-items", JSON.stringify(newItems));
    setNewName(""); setManualLocalPath("");
  };

  const handleCancel = async () => {
    try {
      await invoke("cancel_sync");
      setStatus("취소 요청 중...");
    } catch (e) {
      console.error("Cancel failed", e);
    }
  };

  const syncAll = async () => {
    const validItems = items.filter(i => i.local_path !== "" && i.enabled);
    if (validItems.length === 0) return alert("동기화할 항목이 없거나 활성화된 게임이 없습니다.");
    setLoading(true);
    setProgress(0);
    setCurrentFile("파일 분석 중...");
    try {
      const res: any = await invoke("sync_folders", { items: validItems });
      setStatus(res.message);
    } catch (e) {
      setStatus("실패: " + e);
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
            <h3>동기화 진행 중...</h3>
            <div className="progress-container">
              <div className="progress-bar" style={{ width: `${progress}%` }}></div>
            </div>
            <p className="current-file">{currentFile}</p>
            <p className="progress-text">{Math.round(progress)}% 완료</p>
            <button onClick={handleCancel} className="cancel-btn">동기화 취소</button>
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
                  <input 
                    type="checkbox" 
                    checked={item.enabled} 
                    onChange={() => toggleItem(i)} 
                  />
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
              <button onClick={() => {
                if(confirm("삭제할까요?")) {
                  const n = items.filter((_, idx)=>idx!==i);
                  setItems(n); localStorage.setItem("sync-items", JSON.stringify(n));
                }
              }} className="delete-btn">삭제</button>
            </div>
          </div>
        ))}
      </div>

      <div className="action-section">
        <button className="sync-btn" onClick={syncAll} disabled={items.filter(i=>i.local_path!=="" && i.enabled).length === 0 || loading}>동기화 시작</button>
        <div className="status-box"><pre>{status || "준비 완료"}</pre></div>
      </div>
    </div>
  );
}
