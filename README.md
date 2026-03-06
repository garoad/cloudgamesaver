<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="128" height="128" alt="CloudGameSaver Icon">
  <h1>CloudGameSaver</h1>
  <p><b>Dropbox 기반 멀티플랫폼 게임 세이브 동기화 솔루션</b></p>

  <p>
    <img src="https://img.shields.io/badge/Tauri-FFC131?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri">
    <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust">
    <img src="https://img.shields.io/badge/React-61DAFB?style=for-the-badge&logo=react&logoColor=black" alt="React">
    <img src="https://img.shields.io/badge/TypeScript-3178C6?style=for-the-badge&logo=typescript&logoColor=white" alt="TypeScript">
  </p>
</div>

---

> **Note**
> CloudGameSaver는 여러 게임의 세이브 파일 위치를 지정하여 Dropbox 클라우드와 양방향 동기화를 수행합니다. 모든 파일은 수정 시간(Modified Time)을 기준으로 가장 최신 버전으로 업데이트됩니다.

## 🚀 주요 기능

*   **자동 인증 (OAuth2):** 복잡한 코드 복사 없이 클릭 한 번으로 Dropbox 계정과 연동됩니다.
*   **멀티스레드 병렬 처리:** `Rayon` 라이브러리를 활용하여 대량의 세이브 파일을 초고속으로 동기화합니다.
*   **스마트 목록 관리:** Dropbox 서버의 폴더를 자동으로 탐색하고 로컬 경로와 매핑합니다.
*   **실시간 진행 상황:** 동기화 중인 파일명과 전체 진행률을 프로그레스 바를 통해 확인할 수 있습니다.
*   **경량 및 고성능:** Tauri와 Rust를 기반으로 제작되어 약 8MB 수준의 초경량 단일 실행 파일을 제공합니다.

## 📂 폴더 구조

```text
cloudgamesaver/
├── src/                # React (TypeScript) 프론트엔드 코드
├── src-tauri/          # Rust 백엔드 코드
│   ├── src/            # 동기화 로직 및 Dropbox API 연동
│   ├── capabilities/   # Tauri 권한 설정
│   ├── icons/          # 앱 아이콘 리소스
│   └── tauri.conf.json # 앱 환경 설정
├── .github/workflows/  # 자동 빌드 및 릴리즈 (CI/CD)
├── build_windows.bat   # Windows용 빌드 스크립트
└── build_unix.sh       # macOS/Linux용 빌드 스크립트
```

## 🛠 사용된 기술

| 구분 | 사용 기술 | 비고 |
|:---:|:---:|:---|
| **Core** | Tauri v2 | 데스크톱 앱 런타임 |
| **Backend** | Rust | 파일 시스템 및 네트워크 로직 |
| **Frontend** | React, TypeScript | 사용자 인터페이스 |
| **Library** | reqwest | Dropbox API 통신 |
| **Parallel** | Rayon | 멀티스레딩 데이터 처리 |
| **CI/CD** | GitHub Actions | 멀티플랫폼 자동 빌드 |

## ⚙️ 실행 요구사항

*   **Windows:** Windows 10 이상 (WebView2 설치 필요)
*   **macOS:** High Sierra 이상
*   **Linux:** GTK 및 WebKit2GTK 관련 패키지 설치 필요
*   **Dropbox Account:** 세이브 데이터 저장을 위한 계정 필요

## 📦 빌드 방법

### Windows
```bash
# build_windows.bat 실행 또는
npm run tauri build
```

### macOS / Linux
```bash
# build_unix.sh 실행 또는
npm run tauri build
```

## 📄 라이선스

이 프로젝트는 **MIT License**를 따릅니다. 자세한 내용은 [LICENSE](LICENSE) 파일을 참조하세요.

---
<div align="center">
  <p>© 2026 TeamKuma. All rights reserved.</p>
</div>
