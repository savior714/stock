# Requirements v0.1

## 1. 제품 목적

개인용 macOS 애플리케이션에서 미국 상장 주식과 ETF 약 400~500개를 일봉 기준으로 수동 스캔하고, Bollinger Bands·RSI·MFI 조건을 만족한 종목을 빠르게 선별한다.

## 2. 대상 및 제약

- 미국 주식, ETF, ADR 포함
- OTC 제외
- 일봉만 지원
- 실시간성 불필요
- 앱 배포 계획 없음
- 외부 계정이나 API key가 필요한 Provider 사용 안 함
- Yahoo Finance 데이터 단독 사용

## 3. Watchlist

- 여러 Watchlist 생성·이름 변경·복제·삭제
- 동일 티커는 여러 Watchlist에 포함 가능
- 티커 직접 입력, 쉼표/공백/줄바꿈 기반 일괄 입력
- CSV import/export
- 티커 중복 제거와 대문자 정규화
- `BRK.B`와 같은 Yahoo 형식 변환은 Provider 계층에서 수행
- 기존 `stock_vercel/presets.json` 목록을 초기 Watchlist로 가져올 수 있음

## 4. Scan preset

Watchlist와 별도로 저장한다.

### Bollinger Bands

- 활성화 여부
- 기간
- 표준편차 배수
- 기준 가격: Close
- 이동평균: MVP는 SMA
- 상단/하단 감지 활성화
- 감지 가격: High/Low touch 또는 Close break
- trigger mode: current 또는 cross

### RSI

- 활성화 여부
- 기간
- 계산 방식: Wilder
- 하단/상단 임계값
- 상단/하단 감지 활성화
- trigger mode: current 또는 cross

### MFI

- 활성화 여부
- 기간
- 하단/상단 임계값
- 상단/하단 감지 활성화
- trigger mode: current 또는 cross

## 5. 결과

결과 화면은 다음 세 모드를 제공한다.

1. 단일 조건: 선택한 조건 하나를 만족한 종목
2. AND: 활성화된 조건을 모두 만족한 종목
3. OR: 활성화된 조건 중 하나 이상 만족한 종목

표시 필드:

- 티커
- 기준 거래일
- 종가
- RSI
- MFI
- Bollinger 상단/중앙/하단
- 충족 조건 목록
- 데이터 상태
- 오류 메시지

정렬과 필터를 지원하고, 결과에서 티커를 Watchlist에 추가할 수 있다.

## 6. 실행

- 사용자가 버튼으로 수동 실행
- 진행률 표시
- 실행 중 중단 가능
- 종목별 실패가 전체 실행을 중단하지 않음
- 실패 티커만 재시도 가능
- 동일 실행 중복 시작 방지

## 7. 데이터와 저장

- SQLite 사용
- 일봉 OHLCV 영구 저장
- 마지막 성공 거래일 이후 증분 업데이트
- 최초 조회 시 지표 계산에 충분한 기간 확보
- 실행 이력, 결과, 오류, preset 저장
- 데이터 실패 시 기존 정상 데이터 삭제 금지
- 최신 거래일 미포함 시 stale 상태 표시

## 8. 로그

- 화면에서 현재 실행 로그 확인
- 파일 rotating log
- 네트워크, 파싱, 검증, DB, 계산 오류를 구분
- 재시도 가능 여부를 오류 유형으로 표현

## 9. MVP 제외

- 차트
- 실시간/분봉
- 백테스트
- 자동 스케줄
- macOS 알림
- Android/Web 배포
- Fear & Greed, Put/Call, VIX 위젯
- GitHub 자동 동기화
