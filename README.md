# Stock

개인용 macOS 미국 주식 일봉 스캐너입니다.

기존 [`savior714/stock_vercel`](https://github.com/savior714/stock_vercel)의 검증된 UI 흐름을 선별 이식하되, Android·Vercel·GitHub 프리셋 동기화·Windows 전용 코드는 제외하고 macOS 전용으로 재구축합니다.

## MVP

- 여러 Watchlist와 티커 추가·삭제·일괄 입력
- Watchlist 프리셋과 Scan 프리셋 분리
- Yahoo Finance 일봉 데이터 직접 조회 및 로컬 SQLite 저장
- Bollinger Bands, RSI, MFI 파라미터 조정
- 현재 조건 충족 / 신규 진입 감지
- 단일 조건 / 모든 조건 AND / 하나 이상 OR 결과
- 수동 스캔, 진행률, 중단, 실패 티커 재시도, 로그

## 기술 스택

- Next.js + React + TypeScript
- Tauri 2
- Rust
- SQLite
- Yahoo Finance chart endpoint

## 제외 범위

- 실시간 시세와 차트
- 자동 실행과 macOS 알림
- Android 및 Web 배포
- 계정이 필요한 시세 Provider
- GitHub 자동 commit/push 기반 프리셋 동기화

설계와 마이그레이션 순서는 `docs/`를 기준으로 합니다.
