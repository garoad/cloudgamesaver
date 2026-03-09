<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="128" height="128" alt="CloudGameSaver Icon">
  <h1>CloudGameSaver v0.1.5</h1>
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
> CloudGameSaver는 여러 대의 PC에서 세이브 폴더 위치가 달라도 Dropbox 클라우드를 기반으로 무결한 양방향 동기화를 수행합니다. 모든 파일의 해시(Hash)와 수정 시간을 비교하여 가장 최신 버전으로 항상 안전하게 업데이트합니다. **자동 업데이트 시스템**으로 항상 최신 기능을 제공받을 수 있습니다.

## 💻 지원 운영체제

| OS | 배포판 | 상태 |
|:---:|:---:|:---|
| **Windows** | Portable (.exe) | 단일 실행파일, 설치 불필요 |
| **macOS** | Universal App | ARM64/x86_64 유니버설 바이너리 |
| **Linux** | AppImage | 스팀덱(Steam Deck) 최적화 지원 |

## 🛠 주요 사용 기술 및 라이브러리

| 구성 요소 | 기술 / 라이브러리 | 역할 |
|:---:|:---:|:---|
| **Core** | Tauri v2 | 데스크톱 앱 프레임워크 |
| **Async** | Tokio, Futures | 고성능 비동기 파일 및 병렬 처리 |
| **Network** | Reqwest | Dropbox API 통신 |
| **Security** | Sha2, Hex | Dropbox Content Hash 무결성 검증 |
| **Updater** | Tauri Plugin Updater | 자동 업데이트 확인 및 설치 시스템 |
| **Frontend** | React 19, Vite | 모던한 반응형 UI 구성 |
| **CI/CD** | GitHub Actions | 자동화된 멀티플랫폼 빌드 및 배포 |

## 🔄 자동 업데이트 시스템

CloudGameSaver는 **완전 자동화된 업데이트 시스템**을 제공합니다:

- **자동 감지**: 앱 시작 시 및 30분마다 최신 버전 자동 확인
- **원클릭 업데이트**: 새 버전 발견 시 사용자 확인 후 자동 다운로드/설치  
- **안전한 업데이트**: 디지털 서명 검증을 통한 보안성 보장
- **백그라운드 처리**: 업데이트 중에도 기존 작업 방해 없음

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

### 릴리스 배포
```bash
# 버전 업데이트 (package.json & tauri.conf.json)
npm version patch|minor|major

# GitHub에 푸시하면 자동으로 GitHub Actions 실행
git add .
git commit -m "Release v0.1.5"
git push origin main

# 자동으로 다음 작업들이 수행됩니다:
# 1. 멀티플랫폼 빌드 (Windows/macOS/Linux)
# 2. 디지털 서명 적용
# 3. GitHub Release 생성  
# 4. 자동 업데이트 매니페스트(latest.json) 생성
```

## 🔧 프로젝트 구조

```
cloudgamesaver/
├── src/                    # React 프론트엔드
├── src-tauri/             # Rust 백엔드
│   ├── src/               # Rust 소스코드
│   ├── icons/             # 앱 아이콘
│   └── tauri.conf.json    # Tauri 설정
├── .github/workflows/     # GitHub Actions CI/CD
└── dist/                  # 빌드 출력 (자동 생성)
```

## 📄 라이선스

이 프로젝트는 **MIT License**를 따릅니다. 상세 내용은 [LICENSE](LICENSE) 파일을 참조해 주세요.

## 📥 다운로드 및 설치

### 최신 버전 다운로드
[![GitHub Release](https://img.shields.io/github/v/release/garoad/cloudgamesaver?style=for-the-badge)](https://github.com/garoad/cloudgamesaver/releases/latest)

| 운영체제 | 다운로드 링크 | 파일 형식 |
|:---:|:---:|:---:|
| **Windows** | [CloudGameSaver.exe](https://github.com/garoad/cloudgamesaver/releases/latest/download/CloudGameSaver.exe) | Portable 실행파일 |
| **macOS** | [CloudGameSaver.app](https://github.com/garoad/cloudgamesaver/releases/latest) | Universal App |
| **Linux** | [CloudGameSaver.AppImage](https://github.com/garoad/cloudgamesaver/releases/latest) | AppImage |

### 사용법
1. **앱 실행** → 위 링크에서 운영체제에 맞는 파일 다운로드 후 실행
2. **드롭박스 연결** → "🔗 드롭박스 연결" 버튼 클릭하여 계정 인증
3. **게임 추가** → "목록 갱신"으로 자동 감지하거나 수동으로 게임 추가
4. **동기화 시작** → 활성화된 게임들의 세이브 파일 자동 동기화

> **Tip**: 앱은 자동으로 최신 버전을 확인하고 업데이트를 제공합니다! 🚀

---

## 🚀 v0.1.5 주요 개선 사항 (Latest)

*   **완전 자동화된 업데이트 시스템**: GitHub Actions와 연동된 자동 빌드/배포 시스템으로 새 버전이 출시되면 앱 내에서 자동으로 감지하고 원클릭 업데이트가 가능합니다.
*   **Portable 실행파일 제공**: 설치 과정 없이 다운로드 즉시 실행 가능한 단일 실행파일로 배포 방식을 변경했습니다.
*   **최적화된 UI/UX**: 업데이트 확인 버튼을 적절한 위치에 배치하고 버튼 겹침 현상을 완전히 해결했습니다.
*   **멀티플랫폼 CI/CD**: GitHub Actions를 통한 Windows/macOS/Linux 동시 빌드 및 자동 릴리스 생성으로 개발 효율성을 크게 향상시켰습니다.
*   **스마트 에러 복구**: 네트워크 오류 발생 시 자동 재시도 로직(최대 3회)과 정기적인 업데이트 확인(30분 간격)을 추가했습니다.
*   **향상된 사용자 피드백**: 업데이트 진행 상황을 실시간으로 표시하고 다운로드 크기, 진행률을 명확하게 제공합니다.

## 🚀 v0.1.4 주요 개선 사항

*   **영구 세션 유지 (Refresh Token)**: 4시간마다 만료되던 기존의 단기 액세스 토큰 방식을 탈피하여 리프레시 토큰 시스템을 도입했습니다. 이제 한 번의 로그인으로 세션이 반영구적으로 유지됩니다.
*   **지능형 양방향 동기화 강화**: 로컬과 클라우드 파일의 해시뿐만 아니라 수정 시간을 정밀하게 비교하여 단 1ms의 차이도 감지하고 최신 파일을 선택합니다.
*   **Dropbox 표준 해시 검증**: 4MB 블록 단위 SHA256 스트리밍 해시 계산을 통해 파일 내용이 실제로 다른지 완벽하게 검증합니다.
*   **실시간 동기화 피드백**: 동기화 진행 상황을 실시간 진행바(ProgressBar)와 현재 처리 중인 파일명을 통해 시각적으로 전달합니다.
*   **안정적인 네트워크 복구**: 동기화 중 토큰 만료(401)가 발생해도 자동으로 토큰을 갱신하고 작업을 즉시 재개합니다.

<div align="center">
  <p>© 2026 CloudGameSaver Project. All rights reserved.</p>
</div>
