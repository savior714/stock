# Migration Plan

Source: `savior714/stock_vercel`
Target: `savior714/stock`

## 1. 이식 원칙

기존 파일을 통째로 복사하지 않는다. UX와 검증된 실행 패턴을 먼저 문서화한 뒤 새 계층에 맞게 재작성한다.

## 2. 이식 대상

### Frontend UX

- 티커 입력·삭제
- 분석 시작, 일시정지 또는 중단
- 진행률 표시
- 실패 티커 집계와 재시도
- 설정 모달의 RSI/MFI/BB 섹션
- 결과 테이블과 탭 패턴
- 결과 행에서 외부 증권 페이지 열기

참고 후보:

- `src/app/page.tsx`
- `src/hooks/useAnalysis.ts`
- `src/hooks/useTickers.ts`
- `src/hooks/useSettings.ts`
- `src/components/SettingsModal.tsx`
- `src/components/ResultTable.tsx`

### Rust 패턴

- 재사용 가능한 reqwest client
- semaphore 동시성 제한
- 종목별 오류 격리
- 429/네트워크 오류 재시도
- symbol `.` → `-` 변환

참고 후보:

- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/stock.rs`

### 데이터

- `presets.json`을 초기 Watchlist import fixture로 사용

## 3. 재작성 대상

### 지표 엔진

기존 `src-tauri/src/analysis.rs`는 직접 이식하지 않는다.

이유:

- warm-up 실패를 0으로 표현
- 결측 OHLCV를 0으로 변환하는 upstream 구조
- adjusted close와 raw high/low/volume 혼합
- Bollinger 판정이 close 기준 하단에 고정
- 상단 감지와 cross mode 미지원
- Triple Signal이 하드코딩됨

새 구현은 순수 함수와 golden test부터 작성한다.

### 저장 구조

다음을 제거한다.

- localStorage를 SSOT로 사용하는 ticker/result/settings 저장
- AppLocalData JSON preset
- GitHub raw preset download
- 자동 git commit/push

SQLite를 유일한 영구 저장소로 사용한다.

## 4. 폐기 대상

- `android/`
- Capacitor 관련 package와 platform branch
- Vercel/Web API 및 배포 설정
- GitHub sync repository
- Windows WebView2와 click-through/overlay 코드
- Fear & Greed, Put/Call, VIX 기능
- Windows 전용 `.bat`, PowerShell 개발 스크립트

## 5. 단계

### Phase A — scaffold

- macOS-only Tauri/Next 구조
- 문서, lint/build/test command
- 빈 UI shell과 Rust command 연결

### Phase B — domain foundation

- domain models
- error taxonomy
- SQLite schema와 migrations
- repository tests

### Phase C — calculation

- RSI
- MFI
- Bollinger Bands
- current/cross signal engine
- golden tests

### Phase D — data ingestion

- Yahoo parser
- validation
- retry/concurrency
- incremental persistence

### Phase E — feature migration

- Watchlist UI
- Scan preset UI
- run/progress/cancel/retry
- single/AND/OR results
- logs

### Phase F — legacy preset import

- 기존 `presets.json`을 초기 Watchlist로 일회성 import
- import 후 원본 GitHub 연결 없음

## 6. 완료 조건

- Android, Vercel, GitHub sync, Windows-only dependency가 target 저장소에 없음
- 500개 티커에서 부분 실패가 전체 scan을 중단하지 않음
- 동일 fixture에 대한 지표 결과가 반복 실행 시 동일함
- 모든 결과에 기준 거래일과 계산 파라미터가 추적됨
- 기존 앱의 티커 관리·진행률·재시도 UX가 유지됨
