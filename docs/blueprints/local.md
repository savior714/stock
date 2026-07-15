# macOS Stock Scanner — Post-`b83eff0` Implementation Blueprint

## 0. 기준 상태

* Repository: `savior714/stock`
* 기준 Branch: `origin/main`
* 기준 Commit: `b83eff0a0cd643765bf3266c396fc14a138894fd`
* 기술 스택:

  * Next.js 16.1.1
  * React 19.2.3
  * Tauri 2
  * Rust
  * SQLite
* 현재 SQLite schema version: `2`

현재 완료된 범위:

1. macOS 전용 Next.js + Tauri scaffold
2. Domain 기본 모델과 구조화된 `AppError`
3. SQLite v1/v2 migration
4. Watchlist persistent CRUD backend
5. Watchlist CRUD UI
6. Scan Preset persistent CRUD backend
7. Scan Preset 조건별 `current/cross` 저장
8. Scan Preset CRUD UI와 6개 고정 조건 카드

현재 `lib.rs`에 등록된 실제 command는 Watchlist와 Scan Preset CRUD뿐이다. Scan 실행, 취소, 결과, 로그 command는 아직 없다.

프로젝트 원래 계획상 남은 핵심 범위는 다음과 같다.

* Phase C: RSI, MFI, Bollinger, signal engine
* Phase D: Yahoo parser, validation, retry/concurrency, incremental persistence
* Phase E: run/progress/cancel/retry, results, logs
* Phase F: legacy preset import

---

# 1. Local LLM 작업 규칙

Qwen3.6 27B에는 **한 번에 아래 Task 하나만** 전달한다.

## 공통 제약

* 한 Task는 한 책임만 가진다.
* 한 Task는 원칙적으로 2~6개 파일만 수정한다.
* 대규모 rename과 디렉터리 재배치는 금지한다.
* 요청하지 않은 UI 개선이나 의존성 추가를 금지한다.
* 기존 migration 파일은 수정하지 않는다.
* 신규 schema 변경은 다음 번호 migration으로 추가한다.
* SQL은 repository 외부에 작성하지 않는다.
* Tauri command에 네트워크, SQL, 지표 계산을 직접 넣지 않는다.
* DB lock을 획득한 상태에서 `await`하지 않는다.
* 계산 함수는 I/O가 없는 순수 함수로 작성한다.
* 모든 오류를 문자열 검색으로 분기하지 않는다.
* 한 Task 완료 후 반드시 한 개 commit으로 정리한다.
* 검증 실패 상태에서는 다음 Task로 넘어가지 않는다.

## 공통 완료 보고 형식

```text
Task ID:
변경 파일:
핵심 구현:
추가 테스트:
npm run lint:
npm run build:
cargo fmt --check:
cargo clippy --all-targets --all-features -- -D warnings:
cargo test:
working tree:
commit SHA:
미해결 사항:
```

## 공통 검증

Repository root:

```bash
npm run lint
npm run build
```

`src-tauri`:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Frontend test runner가 도입된 이후:

```bash
npm test
```

---

# 2. 최우선 안정화 단계

기능 확장 전에 반드시 `S-00`부터 `S-05`까지 완료한다.

---

## Task S-00 — Baseline 재검증

### 목표

`b83eff0`을 수정하지 않은 상태에서 현재 검증 결과를 재확인한다.

### 수정 허용 경로

* 수정 금지

### 수행

```bash
git fetch origin
git checkout main
git reset --hard origin/main
git rev-parse HEAD
git status --short
```

전체 검증 명령을 실행한다.

### Acceptance Criteria

* HEAD가 `b83eff0a0cd643765bf3266c396fc14a138894fd`
* working tree clean
* 기존 lint/build/fmt/clippy/test 결과 기록
* 실패가 있다면 코드를 수정하지 말고 원문 로그만 보고

---

## Task S-01 — Trigger Mode 상태 불일치 수정

### 발견된 문제

`ScanConditionCard`는 `condition.triggerMode`와 별개로 local state를 생성한다.

```ts
const [triggerMode, setTriggerMode] = useState(condition.triggerMode);
```

Preset A에서 Preset B로 전환해도 컴포넌트 key가 동일한 `indicator:side`이므로 local state가 유지될 수 있다. 그 결과 저장된 값과 화면의 활성 버튼이 달라질 수 있다.

### 목표

`ScanConditionCard`를 완전한 controlled component로 변경한다.

### 수정 허용 경로

* `src/features/scan-presets/ScanConditionCard.tsx`

### 구현

1. `useState` import 제거
2. `useRef`와 사용되지 않는 `cardRef` 제거
3. local `triggerMode` state 제거
4. 버튼 활성 상태는 `condition.triggerMode`로 판정
5. 클릭 시 `onChange({ ...condition, triggerMode: mode })`만 호출
6. 카드 root의 `ref` 제거

### 금지

* 부모 Workspace 상태 구조 변경
* CSS 변경
* API 변경

### Acceptance Criteria

* 다른 Preset을 선택하면 저장된 trigger mode가 즉시 표시됨
* 새 Preset 전환 후 기본 `current`가 표시됨
* trigger 버튼 클릭 후 UI와 form state가 일치함

### 권장 Commit

```text
fix(scan-preset): make trigger mode fully controlled
```

---

## Task S-02 — `useBusy` 제거 및 Operation State 명확화

### 발견된 문제

현재 `useBusy`는 callback을 저장하고 이를 timer ID처럼 `clearTimeout`에 전달한다. 이전 unlock timer가 실제로 취소되지 않아 비동기 작업이 겹칠 경우 예상보다 빨리 busy가 풀릴 수 있다.

또한 React batching을 우회하기 위해 별도 timer를 사용하는 설계 자체가 불필요하다.

### 목표

`useBusy`를 제거하고 비동기 작업별 상태를 명시적으로 관리한다.

### 수정 허용 경로

* `src/features/scan-presets/ScanPresetWorkspace.tsx`
* `src/features/scan-presets/useBusy.ts`

### 구현

Workspace에 다음 state를 둔다.

```ts
const [isLoadingDetail, setIsLoadingDetail] = useState(false);
const [isSaving, setIsSaving] = useState(false);
const [isDeleting, setIsDeleting] = useState(false);

const isBusy = isLoadingDetail || isSaving || isDeleting;
```

각 handler는 반드시 `try/catch/finally`를 사용한다.

```ts
setIsSaving(true);

try {
  // operation
} catch (error) {
  // handling
} finally {
  setIsSaving(false);
}
```

추가 조건:

* `새 Preset` 버튼도 `isBusy`일 때 disabled
* Preset 선택 버튼도 `isBusy`일 때 disabled
* 저장 중 삭제 금지
* 삭제 중 저장 금지
* `useBusy.ts` 삭제
* notice timer는 unmount 시 cleanup

### 금지

* `setTimeout`을 사용한 busy 해제
* Boolean state 하나를 여러 operation에서 수동 lock/unlock
* `flushSync` 추가

### Acceptance Criteria

* 성공·실패 여부와 관계없이 busy가 항상 해제됨
* 선택, 저장, 삭제 작업이 동시에 실행되지 않음
* 컴포넌트 unmount 후 notice timer가 남지 않음
* `useBusy.ts`가 제거됨

### 권장 Commit

```text
fix(scan-preset): replace busy timer with explicit operation states
```

---

## Task S-03 — 구조화된 Tauri Error 처리

### 발견된 문제

Rust의 `AppError`는 이미 아래 필드를 직렬화한다.

* `code`
* `message`
* `detail`
* `retryable`

하지만 Scan Preset UI는 duplicate 오류를 찾기 위해 `"duplicate"`, `"conflict"`, `"이름"` 등의 문자열을 검색한다.

### 목표

Frontend가 `AppError.code`를 기준으로 오류를 분기하도록 변경한다.

### 수정 허용 경로

* `src/lib/app-error.ts` 신규
* `src/features/watchlists/api.ts`
* `src/features/scan-presets/ScanPresetWorkspace.tsx`
* 필요한 Watchlist UI 파일 한정

### 구현

```ts
export type AppErrorCode =
  | "validation"
  | "not_found"
  | "conflict"
  | "database"
  | "provider_rate_limited"
  | "provider_unavailable"
  | "invalid_market_data"
  | "insufficient_data"
  | "cancelled"
  | "internal";

export type AppErrorPayload = {
  code: AppErrorCode;
  message: string;
  detail?: string | null;
  retryable: boolean;
};
```

다음 함수를 작성한다.

```ts
export function parseAppError(error: unknown): AppErrorPayload;
export function formatAppError(error: unknown): string;
```

분기 규칙:

* `conflict` → 이름 field error
* 명확한 form validation → 해당 form error
* `not_found` → 목록 refresh 후 전역 error
* 기타 → 전역 error
* 알 수 없는 JS 오류 → `internal` 성격의 fallback

### 금지

* 오류 메시지 substring 검색
* Rust 오류 schema 변경
* 사용자에게 raw JS object 출력

### Acceptance Criteria

* 대소문자 무관 duplicate name이 이름 입력란 아래 표시됨
* DB 오류는 전역 error로 표시됨
* Watchlist와 Scan Preset이 동일한 error utility를 사용함

### 권장 Commit

```text
refactor(frontend): handle structured tauri application errors
```

---

## Task S-04 — Scan Preset Form 순수 로직 분리

### 목표

Workspace 내부의 순수 로직을 UI와 분리해 테스트 가능하게 만든다.

### 수정 허용 경로

* `src/features/scan-presets/model.ts` 신규
* `src/features/scan-presets/ScanPresetWorkspace.tsx`
* `src/features/scan-presets/defaults.ts`
* `src/features/scan-presets/types.ts`

### 이동 대상

* `conditionKey`
* `detailToForm`
* `validateForm`
* form → API input 변환
* condition replacement 함수

### 요구사항

UI component는 렌더링과 event orchestration만 담당한다.

`validateForm` 반환형은 임의 문자열 key map보다 명시적인 type을 사용한다.

예:

```ts
type ScanPresetFormErrors = {
  name?: string;
  conditions?: string;
  conditionErrors: Partial<Record<ConditionKey, {
    period?: string;
    threshold?: string;
    stdDevMultiplier?: string;
  }>>;
};
```

### Acceptance Criteria

* Workspace에 domain validation 세부 구현이 남지 않음
* 기존 동작 유지
* condition별 오류가 period/threshold/multiplier 단위로 구분됨

### 권장 Commit

```text
refactor(scan-preset): extract form mapping and validation
```

---

## Task S-05 — Frontend Unit Test 기반 추가

### 목표

순수 TypeScript 로직을 테스트할 최소 test runner를 추가한다.

### 수정 허용 경로

* `package.json`
* lock file
* `vitest.config.ts`
* `src/features/scan-presets/*.test.ts`
* `src/features/watchlists/*.test.ts`

### 구현 범위

Vitest만 추가한다. 첫 단계에서는 React component test와 Testing Library는 추가하지 않는다.

테스트:

1. `parseSymbols`
2. duplicate symbol 제거
3. `emptyPresetForm`의 6개 slot과 기본값
4. `detailToForm` slot ordering
5. 누락 condition slot 거부
6. 이름 공백/80자 초과
7. 모든 condition 비활성화
8. RSI/MFI threshold 범위
9. Bollinger multiplier 범위
10. form → API input trim

### Acceptance Criteria

```bash
npm test
```

가 PASS한다.

### 권장 Commit

```text
test(frontend): add unit coverage for preset and watchlist models
```

---

# 3. 선행 설계 결정

계산 엔진 전에 문서상 모호성을 제거한다. 이 단계에서는 구현하지 않는다.

---

## Task ADR-01 — Bollinger 판정 가격 기준 확정

### 배경

Indicator specification에는 `high_low`와 `close` 두 판정 방식이 모두 기술되어 있지만, 현재 Scan Preset schema와 UI에는 이를 선택하는 필드가 없다. 현재 condition은 multiplier만 저장한다.

기존 앱은 마지막 adjusted close를 Bollinger lower/upper와 비교했다.

### 권장 MVP 결정

* MVP는 `close` 판정으로 고정
* 기존 앱 동작과 일치
* `parameters_json`에 새로운 field를 지금 추가하지 않음
* `high_low` 선택 기능은 post-MVP로 명시

### 수정 허용 경로

* `docs/indicator-spec.md`
* `docs/architecture.md`
* `docs/adr/0001-bollinger-detection-price.md` 신규

### Acceptance Criteria

* Signal Engine 구현자가 별도 판단 없이 판정 규칙을 구현할 수 있음
* MVP와 post-MVP 범위가 명확함

---

## Task ADR-02 — Scan Run Snapshot, Freshness, Retry 정책

### 확정할 정책

#### Preset snapshot

Scan 실행 후 Preset이 수정되어도 과거 결과를 재현할 수 있어야 한다.

권장:

* `scan_runs.preset_snapshot_json`
* 실행 시점의 이름과 6개 condition 전체 저장

#### Watchlist snapshot

실행 도중 Watchlist가 수정돼도 실행 대상이 바뀌면 안 된다.

권장:

* `scan_runs.symbols_snapshot_json`

#### Freshness

권장:

* 각 result는 실제 계산한 `trade_date` 저장
* run 종료 시 성공 result 중 가장 최신 날짜를 `base_trade_date`로 지정
* 해당 날짜보다 이전인 result는 `data_stale = true`

#### Retry

권장:

* 기존 run을 다시 running으로 변경하지 않음
* retry는 새 run 생성
* `retry_of_run_id`로 원본 run 참조
* 원본 run의 `retryable = true` 오류 symbol만 실행

### 수정 허용 경로

* `docs/architecture.md`
* `docs/database-schema.md`
* `docs/adr/0002-scan-run-lifecycle.md` 신규

---

# 4. Phase C — 순수 Calculation Engine

각 Task는 네트워크·SQLite·Tauri를 사용하지 않는다.

---

## Task C-01 — Indicator Module Scaffold

### 신규 경로

```text
src-tauri/src/indicator/
├─ mod.rs
├─ rsi.rs
├─ mfi.rs
└─ bollinger.rs
```

### 구현

* 공통 입력 validation
* period validation
* warm-up을 `Option`으로 표현하는 output type
* 입력 순서를 변경하지 않음

### Acceptance Criteria

* 빈 입력과 부족한 입력이 panic하지 않음
* warm-up이 `0.0`으로 반환되지 않음

---

## Task C-02 — RSI Wilder 구현

### 규칙

* 첫 average gain/loss: 최초 `period` 변화량의 단순 평균
* 이후 Wilder smoothing
* loss 0, gain 양수 → 100
* gain/loss 모두 0 → 50
* 최초 유효 값 index는 `period`

### 테스트

* 상승만 존재
* 하락만 존재
* 가격 불변
* 정확한 threshold 값
* warm-up 경계
* known fixture

### 권장 Commit

```text
feat(indicator): implement Wilder RSI with golden tests
```

---

## Task C-03 — MFI 구현

### 규칙

* Typical Price = `(high + low + close) / 3`
* Raw Money Flow = `typical_price * volume`
* 동일 typical price는 positive/negative 모두 제외
* negative 0, positive 양수 → 100
* 둘 다 0 → 50

### 테스트

* positive-only
* negative-only
* flat
* zero volume
* warm-up
* slice 길이 불일치 거부

---

## Task C-04 — Bollinger Bands 구현

### 규칙

* Close 사용
* SMA
* population standard deviation
* `ddof = 0`
* output: lower, middle, upper
* constant series에서 세 값 동일

### 테스트

* constant series
* period와 정확히 같은 길이
* multiplier 1.0 / 2.0
* non-finite 입력 거부

---

## Task C-05 — Indicator Snapshot 통합

### 목표

Preset에서 필요한 최대 period를 기준으로 한 번의 시계열 처리로 현재/이전 indicator 값을 제공한다.

### 권장 type

```rust
pub struct IndicatorSnapshot {
    pub trade_date: String,
    pub close: f64,
    pub rsi_by_period: HashMap<u32, Option<f64>>,
    pub mfi_by_period: HashMap<u32, Option<f64>>,
    pub bollinger_by_params: HashMap<BollingerKey, Option<BollingerValue>>,
}
```

과도한 generic abstraction은 금지한다.

---

## Task C-06 — Current Signal Evaluator

### 신규 경로

```text
src-tauri/src/signal/mod.rs
```

### 규칙

* RSI lower: `value <= threshold`
* RSI upper: `value >= threshold`
* MFI 동일
* Bollinger lower: `close <= lower`
* Bollinger upper: `close >= upper`
* 비활성 condition은 평가 결과 집계에서 제외
* warm-up 값이 없으면 `InsufficientData`

### 테스트

* threshold와 정확히 같은 값
* upper/lower 대칭
* 활성/비활성 혼합

---

## Task C-07 — Cross 및 Aggregate Evaluator

### Cross 규칙

Lower:

```text
previous > threshold && current <= threshold
```

Upper:

```text
previous < threshold && current >= threshold
```

Bollinger도 이전 거래일의 band와 close, 현재 거래일의 band와 close를 각각 사용한다.

### Aggregate

* Single: 선택 condition match
* AND: 활성 condition이 하나 이상이고 모두 match
* OR: 활성 condition 중 하나 이상 match
* 활성 condition 0개는 validation 단계에서 거부

### Acceptance Criteria

* current와 cross 결과가 구별됨
* 이전 값이 warm-up이면 cross false가 아니라 `InsufficientData`
* 결과가 condition ID와 연결됨

---

# 5. Persistence 확장

---

## Task P-01 — Schema v3 Migration

### 신규 파일

```text
src-tauri/migrations/0003_scan_run_snapshots.sql
```

### 권장 변경

`scan_runs`에 추가:

* `preset_snapshot_json TEXT NOT NULL DEFAULT '{}'`
* `symbols_snapshot_json TEXT NOT NULL DEFAULT '[]'`
* `retry_of_run_id TEXT NULL`
* self foreign key 또는 애플리케이션 validation
* 필요한 index

`db` migration runner의 latest version을 3으로 변경한다.

### 테스트

* fresh DB → v3
* v2 DB → v3
* 기존 Watchlist/Preset 유지
* snapshot column 존재
* newer schema 거부

---

## Task P-02 — Instrument Repository

### 신규 경로

```text
src-tauri/src/repository/instrument.rs
```

### 기능

* symbol upsert
* provider symbol 저장
* symbol list 조회
* OTC/asset type validation은 별도 정책 없이는 추측하지 않음
* Watchlist 저장 시 생성된 instrument와 호환 유지

---

## Task P-03 — Daily Bar Repository

### 신규 경로

```text
src-tauri/src/repository/daily_bar.rs
```

### 기능

* 여러 bar transaction upsert
* symbol별 earliest/latest date 조회
* inclusive date range 조회
* 오름차순 반환
* 중복 날짜 overwrite
* 다른 `price_basis` 혼합 거부

### 테스트

* insert/upsert
* date range
* ordering
* invalid OHLC
* basis conflict

---

## Task P-04 — Scan Run Repository

### 기능

* pending run 생성
* running 전환
* progress count 갱신
* completed/cancelled/failed 종료
* snapshot 저장
* retry parent 저장
* 최근 run 목록
* run detail 조회

### 상태 전이

```text
pending → running
running → completed
running → cancelled
running → failed
```

그 외 전이는 validation error.

---

## Task P-05 — Scan Result Repository

### 기능

* `(run_id, symbol)` upsert
* run별 result 조회
* AND/OR match filter
* stale flag final update
* signal JSON encode/decode

### 금지

* frontend 필터 조건을 SQL 문자열로 직접 전달

---

## Task P-06 — Scan Error Repository

### 기능

* symbol별 error append
* retryable filter
* attempt 저장
* run별 조회
* retry 대상 symbol 목록 반환

---

# 6. Phase D — Yahoo Data Ingestion

현재 Rust dependency에는 `rusqlite`, `serde`, `serde_json`, `tauri`만 있으며 HTTP/async provider dependency는 아직 없다.

---

## Task D-01 — Provider Interface와 Dependency 추가

### 신규 구조

```text
src-tauri/src/provider/
├─ mod.rs
└─ yahoo/
   ├─ mod.rs
   └─ dto.rs
```

### Trait 예시

```rust
pub trait MarketDataProvider {
    async fn fetch_daily_bars(
        &self,
        symbol: &Symbol,
        range: DateRange,
    ) -> AppResult<Vec<DailyBar>>;
}
```

Rust 버전 `1.77.2` 호환성을 확인하면서 필요한 dependency만 추가한다.

예상 dependency 범위:

* `reqwest`
* `tokio`
* 날짜 처리 crate
* async trait가 반드시 필요할 때만 `async-trait`

---

## Task D-02 — Yahoo DTO Parser와 Fixture Test

### 목표

HTTP 호출 없이 JSON fixture를 domain `DailyBar`로 변환한다.

### 규칙

* timestamp, open, high, low, close, volume index 정렬
* 하나라도 필수 OHLC가 null이면 해당 bar 제외 또는 명확한 오류
* null을 0으로 변환 금지
* raw OHLCV만 사용
* adjusted close와 raw high/low 혼합 금지
* 날짜 오름차순
* 날짜 중복 제거
* invalid timestamp 거부

### Fixture

```text
src-tauri/tests/fixtures/yahoo/
├─ valid_chart.json
├─ null_values.json
├─ empty_result.json
├─ provider_error.json
└─ duplicate_dates.json
```

---

## Task D-03 — Yahoo HTTP Transport

### 구현

* 재사용 `reqwest::Client`
* connect/request timeout
* provider symbol 변환
* non-2xx 분류
* 429 → `ProviderRateLimited`, retryable
* 5xx/network timeout → `ProviderUnavailable`, retryable
* invalid JSON → `InvalidMarketData`, non-retryable 또는 정책 문서화
* 404/no chart result → symbol 단위 오류

### 금지

* random user-agent 목록 복사
* command 내부 HTTP 호출
* 오류를 `String`으로 변환

---

## Task D-04 — Retry와 Concurrency Policy

### 정책

* 최대 동시 요청: 우선 4
* 최대 attempt: 3
* exponential backoff
* bounded jitter
* 429와 transient network만 retry
* invalid data는 retry하지 않음

### 테스트

실제 Yahoo 호출 대신 fake transport 또는 mock response sequence 사용:

* 429 → 성공
* 500 → 500 → 성공
* invalid JSON → 즉시 종료
* 3회 실패 → retryable error 유지

---

## Task D-05 — Incremental Fetch Planner

### 목표

DB에 저장된 날짜 범위를 기준으로 필요한 기간만 요청한다.

### 규칙

* DB 데이터 없음 → 계산에 필요한 충분한 초기 window 요청
* DB 데이터 있음 → 마지막 저장 날짜 이후 요청
* split 또는 basis 변경 감지 기능은 MVP에서 자동 구현하지 않음
* 최신 bar를 포함한 짧은 overlap 구간을 다시 받아 upsert 가능
* 필요한 minimum bars는 최대 period + cross용 이전 bar + buffer

---

# 7. Scan Application Service

정확성을 먼저 검증하기 위해 **단일 종목 → 순차 batch → 동시 batch** 순서로 구현한다.

---

## Task A-01 — Single Symbol Pipeline

### 신규 구조

```text
src-tauri/src/application/
├─ mod.rs
└─ scan_service.rs
```

### 처리 순서

```text
symbol 확인
→ 기존 bar 범위 조회
→ 필요한 데이터 fetch
→ normalize/validate
→ bar upsert
→ 계산용 range load
→ indicators 계산
→ signals 평가
→ ScanResult 생성
```

### 테스트

Fake provider와 in-memory DB로:

* 정상 성공
* provider 실패
* insufficient data
* invalid bar
* current/cross 결과

---

## Task A-02 — Sequential Scan Run

### 목표

동시성 없이 Watchlist 전체를 끝까지 처리한다.

### 흐름

1. Watchlist와 Preset load
2. snapshots 생성
3. pending run 생성
4. running 전환
5. symbol을 순서대로 처리
6. 성공 result 저장
7. 실패 error 저장
8. count 갱신
9. base date/stale 계산
10. completed 처리

### 핵심 Acceptance Criteria

* 한 symbol 실패가 전체 run을 중단하지 않음
* 성공/실패 count 합이 total과 일치
* 모든 symbol 실패 시에도 run record가 남음

---

## Task A-03 — Cancellation Registry

### AppState 확장

현재 AppState는 SQLite `Mutex`만 가진다.

추가 권장:

* 재사용 HTTP client
* run별 cancellation token registry

### 규칙

* DB mutex를 network await 동안 보유하지 않음
* cancel은 token만 변경
* batch 시작 전과 각 symbol 시작 전에 확인
* provider retry sleep 전후 확인
* 취소 시 이미 저장된 결과는 유지
* run status는 `cancelled`

---

## Task A-04 — Bounded Concurrent Scan

### 목표

순차 ScanService 테스트를 유지하면서 symbol 처리만 bounded concurrency로 변경한다.

### 요구사항

* 동시성 4
* join 결과 순서에 의존하지 않음
* DB write는 짧은 critical section
* task panic도 symbol error로 격리
* counter update atomicity 보장
* cancellation 후 신규 symbol task 시작 금지

### 금지

* 500개 symbol을 모두 무제한 spawn
* SQLite lock을 잡은 채 provider await

---

## Task A-05 — Tauri Commands와 Background Run

### 신규 command

* `start_scan`
* `cancel_scan`
* `list_scan_runs`
* `get_scan_run`
* `get_scan_results`
* `get_scan_errors`

`start_scan`은 전체 scan 완료를 기다리지 않고 `runId`를 반환한다.

Background task에서 ScanService를 실행한다.

### Command 원칙

* request DTO parse
* ID validation
* service 호출
* DTO 반환

그 외 로직 금지.

---

## Task A-06 — Progress Events

### Event 이름 권장

```text
scan://started
scan://progress
scan://result
scan://error
scan://completed
scan://cancelled
```

### 모든 Event 공통 필드

```ts
{
  runId: string;
  sequence: number;
}
```

### Progress payload

```ts
{
  runId: string;
  sequence: number;
  completed: number;
  total: number;
  succeeded: number;
  failed: number;
  currentSymbol?: string;
}
```

### 요구사항

* frontend가 event 유실 시 command 조회로 상태 복구 가능
* event는 알림 수단이며 DB가 SSOT
* payload가 Rust/TypeScript camelCase와 일치

---

## Task A-07 — Retry Failed Symbols

### 정책

* 원본 run의 retryable error만 대상
* 새로운 run 생성
* `retry_of_run_id` 저장
* original snapshots 재사용
* 성공한 symbol은 재실행하지 않음
* retry run도 일반 run과 동일한 event 발생

---

# 8. Frontend Scan UX

---

## Task U-01 — Scan API와 Type 정의

### 신규 구조

```text
src/features/scans/
├─ api.ts
├─ types.ts
├─ events.ts
└─ model.ts
```

### 구현

* start/cancel/list/get/results/errors invoke wrapper
* Rust DTO와 정확히 일치하는 TypeScript type
* event payload type
* event subscribe/unsubscribe helper

### 금지

* UI component 안에 직접 `invoke` 호출

---

## Task U-02 — Run Setup 화면

현재 `Results`와 `Logs`는 placeholder다. Scan 실행 설정은 별도 영역 또는 Results 상단에 구현한다.

### 기능

* Watchlist selector
* Scan Preset selector
* 활성 condition 수 표시
* symbol 수 표시
* 시작 버튼
* 입력 누락 validation

### 금지

* Scan Preset 편집 UI 중복 구현
* frontend에서 indicator 계산

---

## Task U-03 — Progress와 Cancel UI

### 표시

* run status
* completed / total
* succeeded / failed
* progress bar
* 현재 처리 symbol
* cancel 버튼

### 동작

* 시작 후 run ID 저장
* event subscription
* 완료 또는 unmount 시 unsubscribe
* 화면 재진입 시 `get_scan_run`으로 복원
* cancel 중복 요청 방지

---

## Task U-04 — Results Table Model

### 순수 함수부터 작성

* Single condition filter
* AND filter
* OR filter
* matched only
* stale 포함/제외
* symbol sort
* RSI/MFI/price sort

### 테스트

* 활성 condition 1개
* 여러 condition
* match 없음
* stale result
* null indicator
* stable sorting

---

## Task U-05 — Results Table UI

### 기본 column

* Symbol
* Trade date
* Price
* RSI
* MFI
* Bollinger lower/middle/upper
* matched condition count
* AND
* OR
* stale

### 표시 규칙

* 계산되지 않은 값은 `—`
* 반올림은 UI에서 수행
* signal 판정에는 반올림 값 사용 금지
* stale 결과 명확히 표시
* 외부 증권 페이지는 Tauri shell/open 방식 검토 후 구현

---

## Task U-06 — Logs와 Failed Symbols UI

### 기능

* run별 오류 목록
* symbol
* code
* message
* retryable
* attempt
* retryable count
* 실패 symbol 재시도 버튼

### 금지

* raw provider response 표시
* DB detail을 기본 화면에 과도하게 노출

---

## Task U-07 — Run History

### 기능

* 최근 run 목록
* 상태, 생성 시각, Watchlist/Preset snapshot 이름
* total/succeeded/failed
* run 선택 시 저장된 Results/Logs 표시
* 앱 재시작 후에도 조회 가능

---

# 9. Legacy Import

---

## Task L-01 — Legacy Fixture 형식 조사

### 목표

`stock_vercel`의 `presets.json` 형식을 문서화한다.

### 수정 범위

* 문서만
* production code 수정 금지

### 산출물

* 입력 JSON schema
* Watchlist name 생성 규칙
* symbol normalization
* duplicate 처리
* 500개 초과 처리
* malformed entry 처리

---

## Task L-02 — Idempotent One-Time Import

### 규칙

* bundled fixture로만 import
* GitHub 네트워크 접근 금지
* 동일 fixture 재실행 시 중복 생성 금지
* import 완료 marker 저장
* 기존 사용자 Watchlist 유지
* 전체 transaction 또는 항목별 실패 정책 문서화

---

# 10. Hardening과 Release

---

## Task H-01 — GitHub Actions CI

### 신규 workflow

macOS runner에서:

```text
npm ci
npm run lint
npm run build
npm test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Tauri bundle build는 별도 job 또는 수동 workflow로 분리한다.

---

## Task H-02 — 500 Symbol 부분 실패 검증

### Test Scenario

* 500 symbol
* 일부 provider rate limit
* 일부 invalid data
* 일부 insufficient data
* 일부 정상
* scan 중 cancel
* 실패 symbol retry

### 완료 조건

* 하나의 실패가 전체 scan을 중단하지 않음
* memory가 symbol 수에 비례해 비정상 증가하지 않음
* 진행률이 감소하지 않음
* succeeded + failed가 completed와 일치
* 취소 후 새로운 provider request가 시작되지 않음

---

## Task H-03 — Tauri macOS Smoke Test

### 검증

* 첫 실행 DB 생성
* 기존 v2 DB migration
* Watchlist CRUD
* Scan Preset CRUD
* 앱 재시작 후 persistence
* scan 시작
* cancel
* 결과/로그 조회
* retry
* `npm run tauri:build`
* 생성된 `.app` 직접 실행

---

# 11. 명시적 제외 범위

아래 항목은 이 blueprint에서 구현하지 않는다.

* 실시간 시세
* 차트
* 자동 실행
* macOS notification
* Android
* Web/Vercel 배포
* GitHub preset 동기화
* Fear & Greed
* VIX/Put-Call 기능
* Broker 계정 연동
* 계정 기반 market-data provider
* 코인
* 옵션 분석
* 자동 주문

---

# 12. 권장 실행 순서

```text
S-00
→ S-01
→ S-02
→ S-03
→ S-04
→ S-05
→ ADR-01
→ ADR-02
→ C-01 ~ C-07
→ P-01 ~ P-06
→ D-01 ~ D-05
→ A-01
→ A-02
→ A-03
→ A-04
→ A-05
→ A-06
→ U-01 ~ U-07
→ A-07
→ L-01 ~ L-02
→ H-01 ~ H-03
```

`A-01`과 `A-02`를 통과하기 전에는 concurrency를 구현하지 않는다.

`A-04`를 통과하기 전에는 frontend 진행률 UI를 구현하지 않는다.

---

# 13. Phase Gate

## Gate 1 — CRUD 안정화

완료 Task:

```text
S-00 ~ S-05
```

통과 조건:

* trigger mode 상태 불일치 없음
* 비동기 operation race 없음
* typed AppError 사용
* frontend 순수 함수 test 존재

## Gate 2 — Calculation Complete

완료 Task:

```text
ADR-01
C-01 ~ C-07
```

통과 조건:

* 네트워크 없이 indicator/signal 전체 검증
* warm-up을 0으로 표현하지 않음
* current/cross golden test 통과

## Gate 3 — Single Symbol End-to-End

완료 Task:

```text
P-01 ~ P-06
D-01 ~ D-05
A-01
```

통과 조건:

* Yahoo fixture → DB → indicator → signal → result
* 실제 HTTP 없이 integration test 가능

## Gate 4 — Batch Scan Backend

완료 Task:

```text
A-02 ~ A-06
```

통과 조건:

* 부분 실패 격리
* cancellation
* progress event
* persistent results/errors

## Gate 5 — MVP UI

완료 Task:

```text
U-01 ~ U-07
A-07
```

통과 조건:

* 앱 재시작 후 과거 run 조회
* 수동 scan/cancel/retry
* Single/AND/OR 결과
* 실패 로그

## Gate 6 — MVP Release Candidate

완료 Task:

```text
L-01 ~ L-02
H-01 ~ H-03
```

통과 조건:

* 500 symbol 부분 실패 검증
* legacy import
* macOS `.app` smoke test
* 전체 CI PASS
