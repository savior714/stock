# Database Schema v2

## 목적

SQLite는 Watchlist, Scan preset, 일봉 캐시, 실행 결과와 오류 이력을 영구 저장한다.

## 핵심 규칙

- `PRAGMA foreign_keys = ON`
- `PRAGMA journal_mode = WAL`
- schema version은 `PRAGMA user_version`으로 관리
- ticker symbol은 앱 내부 표기(`BRK.B`)와 provider 표기(`BRK-B`)를 분리
- 일봉의 OHLCV는 한 행 안에서 동일한 `price_basis`를 사용
- Yahoo adjusted close만 raw high/low와 혼합하지 않음
- Watchlist와 Scan preset은 독립 엔터티
- Scan 실패는 실행 전체를 롤백하지 않고 `scan_errors`에 종목별로 기록
- Scan preset의 trigger mode는 조건별로 저장한다.

## 테이블

### `instruments`

지원 종목의 기준 정보다.

- `symbol`: 사용자에게 표시하는 정규화 ticker
- `provider_symbol`: Yahoo 요청용 ticker
- `asset_type`: `stock`, `etf`, `adr`
- OTC는 등록 단계에서 거부한다.

### `watchlists`

여러 관심종목 목록을 저장한다.

### `watchlist_symbols`

Watchlist와 instrument의 다대다 관계다. 동일 종목을 여러 Watchlist에 넣을 수 있다.

### `scan_presets`

지표 설정 묶음의 상위 엔터티다.

v1의 `trigger_mode` 컬럼은 migration 호환성을 위해 유지하지만, v2 이후 실제 signal 판정은 `scan_preset_conditions.trigger_mode`를 사용한다. 신규 저장 시 상위 컬럼은 `current`로 기록한다.

### `scan_preset_conditions`

각 preset에 속하는 방향성 조건을 저장한다.

- `indicator`: `bollinger`, `rsi`, `mfi`
- `side`: `lower`, `upper`
- `period`
- `threshold`: RSI와 MFI에서 사용
- `parameters_json`: Bollinger 표준편차 배수 등 지표별 확장 파라미터
- `trigger_mode`: 조건별 `current` 또는 `cross`
- `is_enabled`: 고정 슬롯의 활성화 여부

동일 preset에서 같은 indicator/side 조합은 하나만 허용한다. 애플리케이션 계층은 아래 6개 슬롯을 모두 저장하도록 강제한다.

1. RSI lower
2. RSI upper
3. MFI lower
4. MFI upper
5. Bollinger lower
6. Bollinger upper

최소 하나의 조건은 활성화되어야 한다.

### 기본 Scan preset

v2 migration 시 Scan preset이 하나도 없으면 `기존 앱 기본값`을 생성한다.

- RSI lower: 14 / 30 / current / 활성화
- RSI upper: 14 / 70 / current / 비활성화
- MFI lower: 14 / 30 / current / 활성화
- MFI upper: 14 / 70 / current / 비활성화
- Bollinger lower: 20 / 1.0σ / current / 활성화
- Bollinger upper: 20 / 1.0σ / current / 비활성화

### `daily_bars`

분석에 사용하는 canonical OHLCV다.

- 기본키: `(symbol, trade_date)`
- `price_basis`: `raw` 또는 `split_adjusted`
- OHLC 양수 및 high/low 관계를 CHECK constraint로 검증
- `provider`: MVP에서는 `yahoo`

가격 보정 정책을 바꾸는 경우 기존 행과 혼합하지 말고 전체 구간을 같은 기준으로 다시 적재한다.

### `scan_runs`

한 번의 수동 실행을 표현한다. Watchlist와 Scan preset을 참조하고 진행 상태 및 성공/실패 수를 저장한다.

- `preset_snapshot_json`: 실행 시점의 Preset 이름과 6개 condition 전체를 JSON으로 저장 (Preset 수정 후에도 과거 결과 재현 가능, ADR-0002)
- `symbols_snapshot_json`: 실행 시점의 symbol 목록을 JSON 배열로 저장 (Watchlist 변경 후에도 실행 대상 고정, ADR-0002)
- `retry_of_run_id`: retry 실행일 경우 원본 run의 ID를 참조 (self FK, ADR-0002)
- `base_trade_date`: 성공 result 중 가장 최신 `trade_date` (freshness 기준, ADR-0002)

### `scan_results`

종목별 지표 값과 조건 결과를 저장한다.

- `signal_flags_json`: 단일 조건별 결과
- `all_conditions_matched`: AND 결과
- `any_condition_matched`: OR 결과
- `trade_date`: 실제 지표 계산에 사용된 최신 일봉 날짜 (freshness 판정 기준, ADR-0002)
- `data_stale`: `base_trade_date`보다 이전인 경우 true (최신 거래일 누락, ADR-0002)

### `scan_errors`

종목별 실패 원인, 재시도 가능 여부와 시도 횟수를 저장한다.

- `retryable`: 재시도 가능 여부 (네트워크 오류, 일시적 실패 시 true; 데이터 부족, 유효성 검사 실패 시 false, ADR-0002)

## Migration 순서

기존 migration 파일은 수정하지 않는다.

```text
src-tauri/migrations/
├─ 0001_initial.sql
├─ 0002_condition_trigger_modes.sql
└─ 0003_....sql
```
