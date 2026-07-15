# AGENTS.md

## 목표

개인용 macOS 미국 주식 일봉 스캐너를 Tauri + React + Rust로 개발한다.

## 절대 범위

- macOS 전용이다.
- Android, Capacitor, Vercel, 서버리스 API, GitHub 프리셋 동기화를 추가하지 않는다.
- 시세는 Yahoo Finance 일봉 데이터만 사용한다.
- 차트, 실시간 시세, 자동 실행, 알림은 MVP 범위 밖이다.

## 아키텍처 원칙

1. UI는 표시와 사용자 입력만 담당한다.
2. Tauri command는 얇게 유지하고 application service를 호출한다.
3. 지표 계산과 signal 판정은 순수 함수로 작성한다.
4. Yahoo 응답 모델을 domain 모델과 분리한다.
5. SQLite repository 외부에서 SQL을 직접 실행하지 않는다.
6. 결측 OHLCV를 0으로 대체하지 않는다.
7. 한 종목의 시계열은 정렬·중복 제거·유효성 검사를 통과한 뒤 계산한다.

## 작업 규칙

- 한 task는 하나의 책임과 명확한 acceptance criteria를 가진다.
- 수정 가능한 경로를 task마다 제한한다.
- 요청 없는 의존성 추가와 대규모 리팩터링을 금지한다.
- 신규 domain 로직에는 Rust 단위 테스트를 추가한다.
- 프론트엔드 상태 로직에는 가능한 경우 순수 함수 테스트를 추가한다.
- 비밀키와 개인 데이터는 저장소에 커밋하지 않는다.

## 검증 기준

- `npm run lint`
- `npm run build`
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`

실행 환경에서 검증하지 못한 경우 완료로 표시하지 말고 미검증 항목을 명시한다.
