<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="128" height="128" alt="CloudGameSaver Icon">
  <h1>CloudGameSaver v0.1.4</h1>
  <p><b>드롭박스(Dropbox) 기반의 세이브 데이터 양방향 자동 동기화 도구</b></p>

  <p>
    <img src="https://img.shields.io/badge/Tauri-v2-FFC131?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri">
    <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust">
    <img src="https://img.shields.io/badge/React-19-61DAFB?style=for-the-badge&logo=react&logoColor=black" alt="React">
    <img src="https://img.shields.io/badge/TypeScript-3178C6?style=for-the-badge&logo=typescript&logoColor=white" alt="TypeScript">
  </p>
</div>

---

> **Note**
> CloudGameSaver는 여러 대의 PC에서 세이브 폴더 위치가 달라도 Dropbox 클라우드를 기반으로 무결한 양방향 동기화를 수행합니다. 모든 파일의 해시(Hash)와 수정 시간을 비교하여 가장 최신 버전으로 항상 안전하게 업데이트합니다.

## 💻 지원 운영체제

| OS | 배포판 | 상태 |
|:---:|:---:|:---|
| **Windows** | NSIS (.exe) | 텍스트 기반 로그 및 다크 모드 최적화 |
| **macOS** | DMG / App | 유니버설 바이너리 빌드 지원 예정 |
| **Linux** | AppImage | 스팀덱(Steam Deck) 최적화 배포 지원 |

## 🛠 주요 사용 기술 및 라이브러리

| 구성 요소 | 기술 / 라이브러리 | 역할 |
|:---:|:---:|:---|
| **Core** | Tauri v2 | 데스크톱 앱 프레임워크 |
| **Async** | Tokio, Futures | 고성능 비동기 파일 및 병렬 처리 |
| **Network** | Reqwest | Dropbox API 통신 |
| **Security** | Sha2, Hex | Dropbox Content Hash 무결성 검증 |
| **Updater** | Tauri Plugin Updater | 자동 업데이트 확인 및 설치 |
| **Frontend** | React 19, Vite | 모던한 반응형 UI 구성 |

## 📖 개발 가이드

### 환경 구축
- [Rust](https://www.rust-lang.org/) 및 [Node.js](https://nodejs.org/) (LTS) 설치 필수

### 빌드 및 실행
```bash
# 의존성 설치
npm install

# 앱 빌드 및 실행 (개발 모드)
npm run tauri dev

# 프로덕션 빌드 (Windows/Mac/Linux 자동 감지)
npm run tauri build
```

## 📄 라이선스

이 프로젝트는 **MIT License**를 따릅니다. 상세 내용은 [LICENSE](LICENSE) 파일을 참조해 주세요.

---

## 🚀 v0.1.4 주요 개선 사항

*   **영구 세션 유지 (Refresh Token)**: 4시간마다 만료되던 기존의 단기 액세스 토큰 방식을 탈피하여 리프레시 토큰 시스템을 도입했습니다. 이제 한 번의 로그인으로 세션이 반영구적으로 유지됩니다.
*   **지능형 양방향 동기화 강화**: 로컬과 클라우드 파일의 해시뿐만 아니라 수정 시간을 정밀하게 비교하여 단 1ms의 차이도 감지하고 최신 파일을 선택합니다.
*   **Dropbox 표준 해시 검증**: 4MB 블록 단위 SHA256 스트리밍 해시 계산을 통해 파일 내용이 실제로 다른지 완벽하게 검증합니다.
*   **실시간 동기화 피드백**: 동기화 진행 상황을 실시간 진행바(ProgressBar)와 현재 처리 중인 파일명을 통해 시각적으로 전달합니다.
*   **안정적인 네트워크 복구**: 동기화 중 토큰 만료(401)가 발생해도 자동으로 토큰을 갱신하고 작업을 즉시 재개합니다.

<div align="center">
  <p>© 2026 CloudGameSaver Project. All rights reserved.</p>
</div>
