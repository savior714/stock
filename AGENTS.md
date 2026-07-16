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

## 세션 종료 및 자동 커밋 규칙

- 구현 또는 수정 작업이 완료되면 최종 완료 보고 전에 `git status --short`와 `git diff --check`로 변경 상태를 확인한다.
- 해당 task에 적용 가능한 검증 명령을 실행하고 acceptance criteria 충족 여부를 확인한다.
- 검증을 통과했고 이번 task에서 생성하거나 수정한 변경이 있으면, 사용자에게 매번 별도 확인을 요청하지 말고 해당 변경만 스테이징하여 커밋한다.
- 커밋 메시지는 변경 목적이 드러나는 간결한 명령형 문장으로 작성한다. 가능하면 `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:` 접두사를 사용한다.
- 작업 시작 전부터 존재하던 변경, 다른 task의 변경, `agents/` 같은 로컬 백업, 생성물, 비밀키, 개인 데이터는 스테이징하거나 커밋하지 않는다.
- 관련 변경과 무관한 변경을 안전하게 분리할 수 없으면 자동 커밋하지 말고 최종 보고에 그 이유와 남은 변경 파일을 명시한다.
- 검증 실패, 미완성 작업, 해결되지 않은 오류가 있으면 정상 완료 커밋을 만들지 않는다. 사용자가 명시적으로 요청한 경우에만 checkpoint/WIP 커밋을 허용한다.
- 변경이 없으면 빈 커밋을 만들지 않는다.
- 기존 커밋을 `--amend`하거나 rebase, reset, force push로 기록을 변경하지 않는다. 사용자가 명시적으로 요청한 경우에만 예외로 한다.
- 자동 커밋 후 최종 보고에는 커밋 SHA와 메시지, 실행한 검증 결과, 남은 working tree 상태를 포함한다.
- `git push`는 task 또는 사용자가 명시적으로 요청한 경우에만 수행한다.
