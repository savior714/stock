# Database Schema v1

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

지표 설정 묶음의 상위 엔터티다. `trigger_mode`는 `current` 또는 `cross`다.

### `scan_preset_conditions`

각 preset에 속하는 방향성 조건을 저장한다.

- `indicator`: `bollinger`, `rsi`, `mfi`
- `side`: `lower`, `upper`
- `period`
- `threshold`
- `parameters_json`: 표준편차, 가격 기준 등 지표별 확장 파라미터

동일 preset에서 같은 indicator/side 조합은 하나만 허용한다.

### `daily_bars`

분석에 사용하는 canonical OHLCV다.

- 기본키: `(symbol, trade_date)`
- `price_basis`: `raw` 또는 `split_adjusted`
- OHLC 양수 및 high/low 관계를 CHECK constraint로 검증
- `provider`: MVP에서는 `yahoo`

가격 보정 정책을 바꾸는 경우 기존 행과 혼합하지 말고 전체 구간을 같은 기준으로 다시 적재한다.

### `scan_runs`

한 번의 수동 실행을 표현한다. Watchlist와 Scan preset을 참조하고 진행 상태 및 성공/실패 수를 저장한다.

### `scan_results`

종목별 지표 값과 조건 결과를 저장한다.

- `signal_flags_json`: 단일 조건별 결과
- `all_conditions_matched`: AND 결과
- `any_condition_matched`: OR 결과
- `data_stale`: 최신 거래일 누락 여부

### `scan_errors`

종목별 실패 원인, 재시도 가능 여부와 시도 횟수를 저장한다.

## 이후 migration

기존 migration 파일은 수정하지 않는다. 변경은 다음 순번 파일로 추가한다.

```text
src-tauri/migrations/
├─ 0001_initial.sql
├─ 0002_....sql
└─ 0003_....sql
```
