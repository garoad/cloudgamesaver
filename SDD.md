# 📘 CloudGameSaver 소프트웨어 설계서 (SDD)

## 1. 시스템 아키텍처 (System Architecture)

본 시스템은 **Tauri v2** 프레임워크를 기반으로 하며, 사용자 인터페이스(Frontend)와 시스템 제어(Backend)가 분리된 구조를 가집니다.

*   **Frontend (UI Layer)**: React 19와 TypeScript를 사용하여 사용자 입력을 처리하고 동기화 상태를 시각화합니다.
*   **Backend (Core Layer)**: Rust를 사용하여 파일 시스템 접근, Dropbox API 통신, 고성능 동기화 로직을 수행합니다.
*   **Communication**: Tauri의 `invoke`를 통한 RPC(Remote Procedure Call)와 `emit`을 통한 비동기 이벤트 시스템을 사용하여 두 계층이 통신합니다.

## 2. 구성 요소 설계 (Component Design)

### 2.1 Backend (Rust)
*   **Main Entry (`run`)**: Tauri 빌더를 초기화하고 플러그인(Updater, Dialog, Opener) 및 상태(AppState)를 등록합니다.
*   **Sync Engine (`sync_folders`)**: 비동기 런타임(Tokio) 상에서 로컬과 클라우드 파일 목록을 비교하고 병렬 작업을 관리합니다.
*   **Auth Module**: OAuth2 흐름을 관리하며 리프레시 토큰을 통해 세션을 유지합니다.
*   **File Utility**: Dropbox 전용 해시(`compute_dropbox_hash`) 계산 및 재귀적 파일 스캔을 담당합니다.

### 2.2 Frontend (React)
*   **App Component**: 메인 상태(items, token) 관리 및 주요 비즈니스 로직(동기화 호출, 업데이트 확인)을 제어합니다.
*   **Sync List**: 관리 중인 게임 항목을 렌더링하고 활성화 상태를 관리하는 UI 컴포넌트입니다.
*   **Loading Overlay**: 진행률(`progress`) 및 현재 작업 파일(`currentFile`)을 실시간으로 표시하는 모달 계층입니다.

## 3. 인터페이스 설계 (Interface Design)

### 3.1 IPC Commands (Frontend → Backend)
| Command | Parameter | Description |
| :--- | :--- | :--- |
| `open_auth_url` | - | 브라우저를 열어 Dropbox 인증 페이지로 리다이렉트 |
| `exchange_code_for_token` | `code: String` | 인증 코드를 Access/Refresh Token으로 교환 |
| `sync_folders` | `items: Vec<SyncItem>` | 선택된 항목들에 대해 양방향 동기화 수행 |
| `cancel_sync` | - | 현재 진행 중인 동기화 작업 중단 명령 |
| `list_dropbox_folders` | `token: String` | 클라우드 루트의 폴더 목록 조회 |
| `pick_folder_dialog` | - | 운영체제 네이티브 폴더 선택 창 오픈 |

### 3.2 Events (Backend → Frontend)
| Event | Payload | Description |
| :--- | :--- | :--- |
| `dropbox-code-received` | `String` (Code) | 인증 성공 시 서버로부터 받은 코드 전달 |
| `sync-progress` | `{current_file, progress}` | 실시간 동기화 진행 상황 전달 |
| `sync-complete` | `{success, message}` | 동기화 최종 완료 또는 실패 알림 |
| `tokens-updated` | `Vec<(index, token)>` | 갱신된 토큰 정보를 프론트에 반영 요청 |

## 4. 데이터 설계 (Data Design)

### 4.1 데이터 모델
```rust
struct SyncItem {
    name: String,           // 게임 표시 명칭
    local_path: String,     // PC 내 세이브 폴더 경로
    cloud_path: String,     // Dropbox 내 대상 경로
    token: String,          // 현재 Access Token
    refresh_token: Option,  // 세션 갱신용 Refresh Token
    enabled: bool           // 동기화 활성화 여부
}
```

### 4.2 영구 저장 (Persistence)
*   **LocalStorage**: `sync-items`, `dropbox-token`, `dropbox-refresh-token` 정보를 브라우저 저장소에 보관하여 앱 재시작 시 복구합니다.

## 5. 핵심 알고리즘 상세 설계 (Detailed Design)

### 5.1 양방향 동기화 로직 (Bi-directional Sync)
1.  **목록 수집**: 로컬 파일 시스템과 클라우드 API를 호출하여 전체 파일 메타데이터를 가져옵니다.
2.  **비교 알고리즘**:
    *   **신규 파일**: 한쪽에만 존재하는 파일은 반대편으로 복사 (업로드/다운로드).
    *   **변경 파일**: 해시가 다를 경우, 수정 시간(`Modified Time`)을 비교하여 더 최신 파일을 원본으로 선택합니다.
3.  **작업 큐**: 결정된 모든 작업을 `total_tasks` 리스트에 적재합니다.
4.  **병렬 실행**: `FuturesUnordered`와 제한된 워커(Limit: 5)를 사용하여 네트워크 대역폭을 효율적으로 사용하며 작업을 처리합니다.

### 5.2 Dropbox 해시 계산
*   Dropbox는 파일을 4MB 블록으로 나누어 각 블록의 SHA256을 계산한 뒤, 해당 결과들을 다시 SHA256으로 해싱하는 특수한 방식을 사용합니다.
*   본 시스템은 `sha2` 라이브러리를 사용하여 서버와 동일한 해시값을 로컬에서 생성하여 불필요한 전송을 방지합니다.

## 6. 보안 및 예외 처리
*   **토큰 갱신**: 동기화 중 `401 Unauthorized` 발생 시 즉시 `refresh_token`을 사용하여 토큰을 갱신하고 작업을 재개합니다.
*   **취소 안전성**: `AtomicBool` 플래그를 사용하여 동기화 중단 요청 시 현재 진행 중인 파일 처리를 안전하게 마치고 종료합니다.

---
*Last Updated: 2026-04-03*
