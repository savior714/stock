# Task Plan

모든 task는 `allowed_paths`, `requirements`, `acceptance`, `forbidden`을 포함한다.

## Milestone 0 — Scaffold

### M0-01 Frontend shell

- Next.js static export 기반 React shell
- Watchlists / Scan Settings / Results / Logs placeholder navigation
- Tauri invoke smoke test button

### M0-02 Rust shell

- Tauri application 시작
- `health_check` command
- macOS-only configuration

### M0-03 Quality gates

- ESLint
- TypeScript strict
- Cargo fmt/clippy/test command

## Milestone 1 — Domain and DB

### M1-01 Domain models

`Symbol`, `DailyBar`, `Watchlist`, `ScanPreset`, `ScanRun`, `ScanResult`, `ScanError`.

### M1-02 Error taxonomy

Retryable network/rate-limit/server errors와 non-retryable validation/symbol/database errors 분리.

### M1-03 SQLite schema

Migration, WAL, foreign keys, repository abstraction.

### M1-04 Watchlist repositories

CRUD, 중복 membership 방지, 여러 Watchlist 소속.

### M1-05 Scan preset repositories

JSON blob가 아닌 typed config와 version field를 우선 검토.

## Milestone 2 — Indicator engine

### M2-01 RSI

Wilder RSI 순수 함수와 golden tests.

### M2-02 MFI

OHLCV 기반 MFI와 경계 tests.

### M2-03 Bollinger Bands

SMA, population stddev, rolling series tests.

### M2-04 Signal engine

상하단, current/cross, Single/AND/OR.

## Milestone 3 — Yahoo and persistence

### M3-01 Yahoo DTO/parser

Fixture 기반 parsing과 결측 데이터 rejection.

### M3-02 Provider client

reqwest client 재사용, timeout, retry, semaphore, jitter.

### M3-03 Incremental update

DB 마지막 날짜 이후 조회, stale 판단, transaction upsert.

### M3-04 Scan service

부분 실패 격리, 취소 token, progress events, result persistence.

## Milestone 4 — UI migration

### M4-01 Watchlist UI

기존 ticker input UX를 새 repository command에 연결.

### M4-02 Scan preset UI

RSI/MFI/BB 파라미터와 current/cross 선택.

### M4-03 Run controls

실행, 중단, 실패 재시도, 진행률.

### M4-04 Results

Single/AND/OR tabs, sorting, filtering, condition badges.

### M4-05 Logs

실행 요약, 종목별 오류, rotating file log 접근.

## Milestone 5 — Legacy import and scale

### M5-01 Preset import

기존 `stock_vercel/presets.json`을 초기 Watchlist로 일회성 import.

### M5-02 500-symbol validation

부분 실패, rate limit, 취소, 재실행, DB 중복 여부 검증.

### M5-03 macOS packaging

개인용 `.app` 빌드와 로컬 데이터 위치 검증.

## 첫 구현 순서

1. M0-01
2. M0-02
3. M1-01
4. M2-01~04
5. M1-03~05
6. M3
7. M4
8. M5

지표 엔진을 DB보다 일찍 구현해 계산 기준을 먼저 고정한다.
