import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

interface SyncItem {
  name: string;
  local_path: string;
  cloud_path: string;
  token: string;
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
    const saved = localStorage.getItem("sync-items");
    const token = localStorage.getItem("dropbox-token");
    if (saved) try { setItems(JSON.parse(saved)); } catch (e) {}
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
        return existing || { name, local_path: "", cloud_path: path, token };
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
    const newItems = [...items, { name: newName.trim(), local_path: manualLocalPath, cloud_path: autoCloudPath, token: dropboxToken }];
    setItems(newItems);
    localStorage.setItem("sync-items", JSON.stringify(newItems));
    setNewName(""); setManualLocalPath("");
  };

  const syncAll = async () => {
    const validItems = items.filter(i => i.local_path !== "");
    if (validItems.length === 0) return alert("연결된 폴더가 없습니다.");
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
          <div className="spinner"></div>
          <h3>동기화 진행 중...</h3>
          <div className="progress-container">
            <div className="progress-bar" style={{ width: `${progress}%` }}></div>
          </div>
          <p className="current-file">{currentFile}</p>
          <p className="progress-text">{Math.round(progress)}% 완료</p>
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
          <div key={i} className={`sync-item-card ${item.local_path ? 'active' : ''}`}>
            <div className="item-details">
              <strong>{item.name}</strong>
              <small>Cloud: {item.cloud_path}</small>
              <small className="path-display">Local: {item.local_path || "⚠️ 연결되지 않음"}</small>
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
        <button className="sync-btn" onClick={syncAll} disabled={items.filter(i=>i.local_path!=="").length === 0 || loading}>동기화 시작</button>
        <div className="status-box"><pre>{status || "준비 완료"}</pre></div>
      </div>
    </div>
  );
}
