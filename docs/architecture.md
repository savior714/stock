# Architecture

## 1. 구조

```text
React UI
  ↓ invoke
Tauri Commands
  ↓
Application Services
  ├─ WatchlistService
  ├─ PresetService
  └─ ScanService
       ├─ MarketDataProvider
       ├─ IndicatorEngine
       ├─ SignalEngine
       └─ Repositories
            ↓
       SQLite / File Logs
```

## 2. Frontend

역할:

- Watchlist와 Scan preset 입력
- 스캔 시작·중단·재시도
- 진행률 및 로그 표시
- 단일/AND/OR 결과 필터

금지:

- 지표 계산
- Yahoo 응답 파싱
- SQL 실행
- 재시도 정책 결정

프론트엔드는 Tauri event를 통해 진행률과 종목별 결과를 점진적으로 수신한다.

## 3. Tauri command

Command는 입력 검증 후 application service를 호출하고 DTO를 반환한다. 네트워크, SQL, 지표 계산을 command 함수에 직접 작성하지 않는다.

초기 command 후보:

- `list_watchlists`
- `save_watchlist`
- `delete_watchlist`
- `list_scan_presets`
- `save_scan_preset`
- `run_scan`
- `cancel_scan`
- `retry_failed_symbols`
- `get_scan_results`
- `get_run_logs`

## 4. Domain

### 주요 모델

- `Symbol`
- `DailyBar`
- `Watchlist`
- `ScanPreset`
- `IndicatorValues`
- `SignalCondition`
- `SignalMatch`
- `ScanRun`
- `ScanError`

Domain 모델은 Yahoo 및 SQLite 구조에 의존하지 않는다.

## 5. Infrastructure

### Yahoo provider

- Yahoo chart endpoint 직접 호출
- `.`을 `-`로 바꾸는 등 Provider 전용 symbol 변환
- timeout, 429, 5xx 재시도
- 동시성 제한과 jitter
- raw response를 domain `DailyBar`로 변환
- 결측값을 0으로 대체하지 않음

### SQLite

Repository가 다음 데이터를 관리한다.

- symbols
- watchlists
- watchlist_symbols
- scan_presets
- daily_bars
- scan_runs
- scan_results
- scan_errors

WAL mode와 foreign key를 활성화한다.

## 6. 데이터 처리 흐름

```text
Watchlist + ScanPreset 선택
→ DB에서 기존 bar 범위 확인
→ 필요한 날짜만 Yahoo 조회
→ 응답 검증 및 정규화
→ SQLite upsert
→ 계산에 필요한 연속 시계열 로드
→ 지표 계산
→ signal 평가
→ 결과·오류 저장
→ UI event 전송
```

## 7. 동시성과 취소

- 종목 다운로드 동시성은 작은 고정값으로 시작한다.
- 하나의 종목 실패는 다른 task에 전파하지 않는다.
- 취소 token을 batch와 종목 처리 사이에서 확인한다.
- DB write는 transaction 단위로 짧게 유지한다.
- 동일 Watchlist에 대한 중복 scan 실행을 방지한다.

## 8. 데이터 유효성

각 종목은 계산 전에 다음을 통과해야 한다.

- 날짜 오름차순
- 거래일 중복 제거
- OHLC 필수값 존재
- 가격은 양수
- `high >= max(open, close, low)`
- `low <= min(open, close, high)`
- volume은 음수가 아님
- 계산에 필요한 최소 길이 확보

Adjusted OHLC를 일관되게 만들 수 없는 경우 raw OHLCV를 동일 체계로 사용한다. adjusted close만 raw high/low와 혼합하지 않는다.

## 9. 테스트

- Indicator 순수 함수 golden test
- Signal current/cross 경계 test
- Yahoo fixture parsing test
- SQLite repository integration test
- ScanService 부분 실패·취소 test
- UI 필터 순수 함수 test
