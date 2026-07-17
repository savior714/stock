# 개발 및 검증 가이드

## 개발 모드

### 브라우저 Mock 모드

```bash
npm run dev:mock
```

`NEXT_PUBLIC_STOCK_BACKEND=mock` 환경변수로 시작합니다. Tauri 없이 일반 브라우저에서 전체 UI 탐색이 가능합니다. Watchlist CRUD, Preset CRUD, Scan 실행·취소·이벤트 흐름을 모두 검증할 수 있습니다.

### Tauri 개발 모드

```bash
npm run tauri:dev
```

실제 Tauri 앱을 실행합니다. IPC 호출, SQLite 연동, 실제 Yahoo Finance provider 동작을 검증합니다.

## 검증 명령

| 명령 | 검증 내용 |
|---|---|
| `npm run verify:fast` | lint + 테스트 + 빌드. CSS/UI 수정, IPC API 변경 시 실행 |
| `npm run verify:rust` | Rust fmt/clippy/test. Rust 도메인 변경 시 실행 |
| `npm run verify:tauri` | `tauri build --no-bundle --no-sign`. Tauri 컴파일 검증 |
| `npm run verify:full` | fast + rust + tauri 전체 검증. 마일스톤 완료 시 실행 |

## 권장 실행 기준

| 작업 | 실행 명령 |
|---|---|
| CSS 및 일반 UI 수정 | `verify:fast` |
| IPC API 변경 | `verify:fast` + `verify:rust` |
| Rust 도메인 변경 | `verify:rust` |
| 기능 마일스톤 완료 | `verify:full` |
| DMG 생성 | 릴리스 직전만 |

## Mock fixture

### 제공 Watchlist

- `미국 대형주`: AAPL, MSFT, NVDA, AMZN, GOOGL
- `저점 관찰`: AMD, TSM, AVGO, COST

### 제공 Preset

| ID | 이름 | 설명 |
|---|---|---|
| `preset-1` | 기본 저점 스캔 | 기존 앱의 기본 lower 조건 |
| `preset-2` | `[MOCK] 전체 성공` | 모든 symbol 성공 |
| `preset-3` | `[MOCK] 부분 실패` | 일부 symbol retryable/permanent error |
| `preset-4` | `[MOCK] 느린 실행` | Cancel 검증용 느린 처리 |

### Mock scenario

- **전체 성공**: 모든 symbol이 성공, `succeededSymbols == totalSymbols`
- **부분 실패**: GOOGL은 retryable error, AMD는 permanent error, 나머지는 성공
- **느린 실행**: symbol당 200ms 지연, Cancel 가능

### Mock 데이터 초기화

```bash
# 브라우저 콘솔에서
# (개발 모드에서만 동작)
```

또는 `localStorage`의 `stock.mock.backend.v1` 키를 수동 삭제.

## 검증 경계

### Mock 모드로 검증 가능

- UI 상태
- CRUD 흐름
- Scan event 처리
- 결과와 오류 화면
- 취소 흐름

### 실제 Tauri에서만 검증 가능

- IPC 직렬화
- 실제 SQLite 연결
- 실제 Yahoo provider
- macOS WebView 동작
- 앱 번들링과 서명
