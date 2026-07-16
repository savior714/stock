# Smoke Test Report — Task H-03: Tauri macOS Smoke Test

**Date:** 2026-07-16
**Environment:** macOS (Apple Silicon / aarch64)
**Tauri CLI:** 2.11.4
**Tauri Runtime:** 2.11.5
**Frontend:** Next.js 16.1.1 (Turbopack)
**Rust:** dev profile (debug) / release profile

---

## 1. 첫 실행 DB 생성

**결과: PASS**

- `npm run tauri:dev` 실행 시 SQLite DB 파일이 `~/Library/Application Support/com.savior714.stock/stock.sqlite3` 경로에 생성됨
- 스키마 버전 0에서 시작하여 v1 → v2 → v3 → v4 마이그레이션이 순차적으로 적용됨
- 로그에 migration 오류 없음
- 생성된 DB 테이블: `watchlists`, `scan_presets`, `scan_runs`, `scan_results`, `scan_errors`, `watchlist_symbols`, `daily_bars`, `instruments`, `scan_preset_conditions`

## 2. 기존 v2 DB migration

**결과: PASS**

- 기존 v2 스키마 버전 DB를 복원하고 앱 재시작
- v2 → v3 (scan_run_snapshots) → v4 (legacy_import) 마이그레이션이 정상 적용됨
- 기존 데이터(watchlist 2건, scan preset 2건)가 모두 보존됨
- 스키마 버전이 4로 정상 업그레이드됨
- 로그에 migration 오류 없음

## 3. Watchlist CRUD

**결과: 부분 PASS (UI 렌더링 확인됨, Tauri IPC 미사용)**

- Watchlists 페이지가 Next.js 프론트엔드에서 정상 렌더링됨
- "새 목록" 버튼, 이름/설명/티커 입력 폼, 저장된 목록 섹션이 모두 표시됨
- **제한사항:** Tauri IPC는 웹뷰 내부에서만 동작하므로, 브라우저에서 직접 접근 시 IPC 호출 불가. 따라서 실제 DB 생성/수정/삭제 테스트는 Tauri 앱 내 웹뷰에서 수행 필요.
- "알 수 없는 오류가 발생했습니다." 메시지는 IPC unavailable으로 인한 예상 동작

## 4. Scan Preset CRUD

**결과: 부분 PASS (UI 렌더링 확인됨, Tauri IPC 미사용)**

- Scan Settings 페이지가 정상 렌더링됨
- Presets 목록 섹션과 "새 Preset" 버튼 표시
- New Preset 폼: 이름 입력, 6개 condition (RSI Lower/Upper, MFI Lower/Upper, Bollinger Lower/Upper) 활성화 체크박스, Period/Threshold/Trigger 설정, 생성 버튼
- **제한사항:** 동일하게 Tauri IPC 미사용으로 실제 CRUD 동작 검증 불가

## 5. Scan 시작

**결과: 부분 PASS (UI 렌더링 확인됨, Tauri IPC 미사용)**

- Scan 페이지가 정상 렌더링됨
- Watchlist dropdown, Scan Preset dropdown, "Start Scan" 버튼 표시
- Watchlist과 Preset 미선택 시 "Watchlist과 Preset을 모두 선택하십시오." 메시지 표시
- Start Scan 버튼은 미선택 시 disabled 상태
- **제한사항:** Tauri IPC 미사용으로 실제 스캔 시작 동작 검증 불가

## 6. Cancel

**결과: 미검증**

- UI에 취소 버튼이 있는지 확인 불가 (Tauri IPC 미사용)
- Rust 코드에는 `cancel_scan` 명령과 `CancellationRegistry`가 구현되어 있음
- 실제 앱 내 웹뷰에서 테스트 필요

## 7. 결과/로그 조회

**결과: 부분 PASS (UI 렌더링 확인됨, Tauri IPC 미사용)**

- Results 페이지: "알 수 없는 오류가 발생했습니다." 메시지 표시 (IPC unavailable)
- Logs 페이지: 동일하게 오류 메시지 표시
- **제한사항:** 실제 결과/로그 조회는 Tauri 앱 내 웹뷰에서 테스트 필요

## 8. Retry

**결과: 미검증**

- UI에 retry 버튼이 있는지 확인 불가
- Rust 코드에 `retry_of_run_id` 컬럼이 있음 (migration v4에서 추가)
- 실제 앱 내 웹뷰에서 테스트 필요

## 9. Tauri Build

**결과: PASS**

- `npm run tauri:build` 성공
- Next.js 프로덕션 빌드 완료 (Static pages 3개)
- Rust release 빌드 완료 (1m 39s)
- `.app` 번들 생성: `src-tauri/target/release/bundle/macos/Stock.app`
- `.dmg` 번들 생성: `src-tauri/target/release/bundle/dmg/Stock_0.1.0_aarch64.dmg`
- 빌드 로그에 오류 없음

## 10. 생성된 .app 직접 실행

**결과: PASS**

- `open Stock.app`으로 실행 성공
- 앱 프로세스 정상 실행 (`/target/release/bundle/macos/Stock.app/Contents/MacOS/stock`)
- 기존 DB를 정상적으로 읽고 스키마 버전 4 확인
- 윈도우가 생성됨 (accessibility API로 확인)
- 바이너리에 프론트엔드가 `tauri://localhost` 프로토콜로 임베딩됨 (custom-protocol feature)

---

## 요약

| # | 항목 | 결과 | 비고 |
|---|------|------|------|
| 1 | 첫 실행 DB 생성 | PASS | v1→v2→v3→v4 마이그레이션 정상 |
| 2 | 기존 v2 DB migration | PASS | 기존 데이터 보존, 스키마 업그레이드 정상 |
| 3 | Watchlist CRUD | 부분 PASS | UI 렌더링 확인, IPC 미사용으로 실제 동작 미검증 |
| 4 | Scan Preset CRUD | 부분 PASS | UI 렌더링 확인, IPC 미사용으로 실제 동작 미검증 |
| 5 | Scan 시작 | 부분 PASS | UI 렌더링 확인, IPC 미사용으로 실제 동작 미검증 |
| 6 | Cancel | 미검증 | Tauri 앱 내 웹뷰에서 테스트 필요 |
| 7 | 결과/로그 조회 | 부분 PASS | UI 렌더링 확인, IPC 미사용으로 실제 동작 미검증 |
| 8 | Retry | 미검증 | Tauri 앱 내 웹뷰에서 테스트 필요 |
| 9 | Tauri Build | PASS | .app, .dmg 번들 생성 성공 |
| 10 | .app 직접 실행 | PASS | 정상 시작, 기존 DB 연동 확인 |

## 미해결 사항

1. **Tauri IPC 미사용으로 인한 CRUD/CANCEL/RETRY 미검증**: Tauri 웹뷰는 커맨드라인에서 직접 상호작용이 불가능하여, 실제 Watchlist/Scan Preset CRUD, Scan 시작, Cancel, Retry 동작은 수동으로 Tauri 앱 내에서 테스트 필요.
2. **UI 상호작용 테스트**: AppleScript UI scripting은 웹뷰 기반 앱의 내부 DOM에 접근하지 못함. 실제 상호작용 테스트는 사람이 직접 Tauri 앱 창에서 수행해야 함.
3. **인pector 활성화**: Tauri inspector(Cmd+Option+I)가 정상 작동하는지 확인하지 못함. inspector가 활성화되면 웹뷰 디버깅이 가능해짐.

## 로컬 에이전트 후속 실행 프롬프트

```bash
# 1. Tauri 앱 실행
cd /Users/seungjulee/Desktop/Dev/stock
npm run tauri:dev

# 2. 앱 창에서 다음을 수동으로 테스트:
#    - Watchlists 탭: 새 목록 생성, 이름 수정, 삭제
#    - Scan Settings 탭: 새 Preset 생성, 수정, 삭제
#    - Scan 탭: Watchlist + Preset 선택 후 Scan 시작
#    - 진행 중 Cancel 버튼 클릭
#    - Results 탭: 완료된 scan 결과 확인
#    - Logs 탭: 로그 확인
#    - 실패한 symbol의 Retry 버튼 클릭 (있다면)
#    - 앱 재시작 후 데이터 persistence 확인

# 3. Tauri inspector 활성화 (앱 실행 중 Cmd+Option+I)

# 4. 빌드 검증
npm run tauri:build
open src-tauri/target/release/bundle/macos/Stock.app
```
