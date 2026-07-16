# ADR-0002: Scan Run Snapshot, Freshness, Retry 정책

**Status**: Accepted
**Date**: 2026-07-15

## Context

Scan 실행은 Watchlist(종목 목록)와 Scan Preset(조건 설정)을 조합하여 실행한다. 실행 중 또는 실행 후 사용자가 Preset이나 Watchlist를 수정할 수 있으므로, 과거 실행 결과를 재현 가능하게 보존하는 정책이 필요하다. 또한 Yahoo API 호출 실패, 데이터 부족 등 부분 실패 시 재시도 정책과 결과의 최신성(freshness) 판정 기준도 명확해야 한다.

해결해야 하는 문제:

1. **Preset 변경 후 결과 재현 불가** — 실행 후 Preset을 수정하면 과거 결과가 어떤 조건으로 계산되었는지 알 수 없음
2. **Watchlist 변경 후 실행 대상 불명확** — 실행 도중 Watchlist에 종목을 추가/삭제하면 어떤 종목이 실행되었는지 추적 불가
3. **Freshness 기준 부재** — 어떤 결과가 최신 거래일 데이터로 계산되었는지 판정할 수 없음
4. **Retry 정책 부재** — 실패한 종목만 재실행할 때 원본 실행과 어떻게 연결할지, 어떤 오류를 재시도할지 정의되지 않음

## Decision

### 1. Preset snapshot

`scan_runs` 테이블에 `preset_snapshot_json TEXT NOT NULL DEFAULT '{}'` 필드를 추가한다.

- 실행 시점의 Preset 이름과 6개 condition(RSI lower/upper, MFI lower/upper, Bollinger lower/upper) 전체를 JSON으로 저장
- Preset이 나중에 수정되어도 과거 실행의 조건을 재현 가능
- JSON 구조: `{ "name": "...", "conditions": [ {...}, ... ] }` — `scan_preset_conditions` 행의 필드를 그대로 매핑

### 2. Watchlist snapshot

`scan_runs` 테이블에 `symbols_snapshot_json TEXT NOT NULL DEFAULT '[]'` 필드를 추가한다.

- 실행 시점의 symbol 목록(`["AAPL", "MSFT", ...]`)을 JSON 배열로 저장
- 실행 도중 Watchlist가 수정되어도 실행 대상이 바뀌지 않음
- ScanService는 실행 시작 시 Watchlist의 symbol을 snapshot으로 고정 후 처리

### 3. Freshness

`scan_runs` 테이블에 `base_trade_date TEXT` 필드를 추가하고, `scan_results` 테이블에 `trade_date TEXT`와 `data_stale INTEGER NOT NULL DEFAULT 0` 필드를 명시한다.

- 각 result는 실제 지표 계산에 사용된 `trade_date`(최신 일봉 날짜)를 저장
- run 종료 시 성공 result 중 가장 최신 `trade_date`를 `base_trade_date`로 지정
- `base_trade_date`보다 이전인 result는 `data_stale = 1`(true)로 표시
- UI는 `data_stale` result를 시각적으로 구분(예: 흐리게 표시)

### 4. Retry

`scan_runs` 테이블에 `retry_of_run_id INTEGER` 필드를 추가하고, `scan_errors` 테이블에 `retryable INTEGER NOT NULL DEFAULT 1` 필드를 명시한다.

- 기존 run을 다시 `running` 상태로 변경하지 않음
- Retry 요청 시 새 run을 생성하고 `retry_of_run_id`로 원본 run의 ID를 참조
- 원본 run에서 `retryable = 1`인 오류 symbol만 실행 대상에 포함
- `retry_of_run_id`는 self-referencing FK(`REFERENCES scan_runs(id)`) 또는 애플리케이션 계층 validation으로 구현

## Consequences

- `scan_runs` 테이블에 4개 필드 추가 필요 (migration 0003 또는 별도 migration)
- ScanService는 실행 시작 시 snapshot 생성 로직 추가
- Retry command(`retry_failed_symbols`)는 새 run 생성 + 원본 참조 로직 구현
- Freshness 판정은 run 종료 시 batch update로 처리 가능
- `preset_snapshot_json`과 `symbols_snapshot_json`은 읽기 전용이며 실행 후 수정하지 않음
- UI는 `data_stale` 결과에 대한 표시와 `retryable` 오류에 대한 재시도 버튼 구현 필요
