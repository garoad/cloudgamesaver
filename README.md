<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="128" height="128" alt="CloudGameSaver Icon">
  <h1>CloudGameSaver v0.1.3</h1>
  <p><b>Dropbox 기반 차세대 멀티플랫폼 게임 세이브 동기화 솔루션</b></p>

  <p>
    <img src="https://img.shields.io/badge/Tauri-v2-FFC131?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri">
    <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust">
    <img src="https://img.shields.io/badge/React-19-61DAFB?style=for-the-badge&logo=react&logoColor=black" alt="React">
    <img src="https://img.shields.io/badge/TypeScript-3178C6?style=for-the-badge&logo=typescript&logoColor=white" alt="TypeScript">
  </p>
</div>

---

> **Note**
> CloudGameSaver는 여러 게임의 세이브 파일 위치를 지정하여 Dropbox 클라우드와 양방향 동기화를 수행합니다. 모든 파일은 수정 시간 및 해시(Hash)를 기준으로 가장 최신 버전으로 안전하게 업데이트됩니다.

## 📂 빌드 타겟 (Platform Support)

| OS | 형식 | 특징 |
|:---:|:---:|:---|
| **Windows** | NSIS (.exe) | 단일 실행 파일 설치 프로그램 |
| **macOS** | DMG / App | 유니버설 바이너리 지원 |
| **Linux** | AppImage | 스팀덱(Steam Deck) 최적화 배포 형식 |

## 🛠 사용된 기술 및 라이브러리

| 구분 | 기술 / 라이브러리 | 용도 |
|:---:|:---:|:---|
| **Core** | Tauri v2 | 데스크톱 앱 프레임워크 |
| **Async** | Tokio, Futures | 고성능 비동기 및 병렬 처리 |
| **Network** | Reqwest | Dropbox API 통신 |
| **Security** | Sha2, Hex | Dropbox Content Hash 계산 |
| **Updater** | Tauri Plugin Updater | GitHub 연동 자동 업데이트 |
| **Frontend** | React 19, Vite | 모던 프론트엔드 환경 |

## 📦 설치 및 빌드

### 요구사항
- [Rust](https://www.rust-lang.org/) 및 [Node.js](https://nodejs.org/) (LTS) 설치 필요

### 빌드 명령어
```bash
# 의존성 설치
npm install

# 프로덕션 빌드 (Windows/Mac/Linux 자동 감지)
npm run tauri build
```

## 📄 라이선스

이 프로젝트는 **MIT License**를 따릅니다. 자세한 내용은 [LICENSE](LICENSE) 파일을 참조하세요.

---

## 🚀 v0.1.3 주요 개선 사항

*   **⚡ 초고속 병렬 동기화:** `FuturesUnordered`를 도입하여 최대 5개의 파일을 동시에 업로드/다운로드합니다. 대량의 세이브 파일이 있는 게임에서 비약적인 속도 향상을 체감할 수 있습니다.
*   **🔍 스마트 해시 비교:** Dropbox 전용 해시 알고리즘(4MB 블록 단위 SHA256)을 구현하여, 파일 내용이 실제로 변경된 경우에만 전송합니다.
*   **🛑 안전한 동기화 취소:** 동기화 도중 언제든 안전하게 중단할 수 있는 기능을 추가했습니다. 현재 전송 중인 파일은 손상 없이 완료한 후 작업을 멈춥니다.
*   **🌐 자동 온라인 업데이트:** 앱 실행 시 최신 버전을 자동으로 확인하고, GitHub Releases와 연동하여 원클릭 업데이트 및 재시작이 가능합니다.
*   **🔑 토큰 만료 자동 처리:** 드롭박스 인증 세션이 만료되면 자동으로 감지하여 사용자에게 재연결을 안내합니다.
*   **📦 안정적인 네이티브 UI:** Tauri 공식 다이얼로그 플러그인을 사용하여 모든 팝업 및 확인창의 안정성을 높였습니다.
*   **🎨 현대적인 UI/UX:** 배경 흐림 효과(Blur)와 다크 모드 감성을 더한 로딩 카드, 상세한 진행 상황 표시 등 사용자 인터페이스를 대폭 개선했습니다.

<div align="center">
  <p>© 2026 TeamKuma. All rights reserved.</p>
</div>
