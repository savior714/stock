---
situation: 커밋
# trigger: /git  ← catalog metadata only; Read this file before executing (error_patterns/detail/workflow.md#161)
level: Mandatory
description: Git Commit & Push - mainline 직접 반영·임시 브랜치 즉시 정리
execution: sequential  # 단일 선형 경로. "실행" 지시 시 질문 없이 모든 단계 순차 수행
version: 1.8.0
last_updated: 2026-07-15
scope: workflow
domain: workflow
---
<!-- Language: ko -->

# Git Commit & Push (`/git`)

세션 WIP를 SSOT에 반영한 뒤 **슬라이스별 커밋** → **main push 1회**로 마무리. **트리거 시 본 문서 1회 Read.**

**실행 규칙**: 사용자가 "실행"을 지시하면 질문 없이 branch entry(§0) → hard gate → soft gate → WIP → 슬라이스 루프(§5.1) → Push·정리(§5.2)를 순차 수행한다.

**검증 레이어 SSOT**: [verification.md](../core/verification.md) — Scope L1/L2/L3.

---

## Branch Topology (MUST)

- **영구 브랜치**: `main` 하나만 사용한다.
- **기본 경로**: 단일 세션·비충돌 작업은 `main`에서 직접 작업하고 `main`으로 push한다. 별도 feature 브랜치를 자동 생성하지 않는다.
- **임시 브랜치 허용 조건**: 여러 세션이 동시에 write하거나 동일 파일 충돌 가능성이 있을 때만 `agent/<short-scope>`를 사용한다.
- **임시 브랜치 수명**: 작업 완료 → 최신 `main`으로 rebase → `main`에 fast-forward 반영 → push → local·remote 임시 브랜치 삭제를 같은 작업 주기에서 끝낸다.
- **PR**: 기본 수행하지 않는다. DB migration, public API contract, 임상 정책, 대규모 리팩터링, 여러 세션 결과 통합에만 선택적으로 사용한다.
- **금지**: `develop`, `release/*`, 장기 `feature/*`, stacked branch 운영을 기본 전략으로 만들지 않는다.

정책 SSOT: [PROJECT_RULES.md §2.3](../../PROJECT_RULES.md#23-git-mainline-policy-must).

---

## 금지

| 규칙 | 이유 |
| :--- | :--- |
| `git add .` (일괄 스테이징) | 도메인 혼합·비밀 파일 유입 |
| `git push --force` / `--force-with-lease` | 원격 히스토리 파괴 — 단, [PROJECT_RULES.md §3.4](../../PROJECT_RULES.md#34-사용자-승인-필수-human-in-the-loop-must) HITL(사용자 명시적 승인) 시 예외 허용 |
| 기본 `--no-verify` | hard 게이트(dotenv·비밀) 우회 — PR·`just lint`에서 재차단. **단**, escape hatch는 [PROJECT_RULES.md §3.7](../../PROJECT_RULES.md#37-게이트-우회-금지-gate-bypass-prohibition-must--zero-tolerance) 참조. |
| 슬라이스 미완료 상태에서 push | unstaged WIP 잔존 시 §5.1 재실행 |
| NEVER_STAGE 경로 커밋 | 로컬·비밀·아티팩트 — §5.0 표 |
| 사유 없는 임시 브랜치 생성 | 1인 개발 mainline 정책 위반·관리 복잡도 증가 |
| `main` 반영 후 임시 브랜치 방치 | stale branch·잘못된 후속 작업 기준 생성 |

---

## CLI (기본 main 경로)

| 단계 | 명령 |
| :--- | :--- |
| fetch | `git fetch origin` |
| main 진입·원격 동기화 | `git switch main && git pull --ff-only origin main` |
| 작업 조건 확인 | `git status --short` — clean && origin/main 동기화 확인 |
| hard 선제 검증 | `just commit-gate-hard` (빠름, 보안 관련) |
| soft 선제 검증 | `just commit-gate-soft` (lint-fix + FE `typecheck` strict) |
| 배포 동등 검증 (push 전 권장) | `just renderer-ship-gate` (= Vercel `build:cloud` SSOT) |
| WIP 보존 | `just wip "pre-commit-$(date +%Y%m%d_%H%M)"` |
| 슬라이스 커밋 | `git add <paths>` → `git commit -m "type(scope): 요약"` (§5.1 루프) |
| push 전 동기화 | `git pull --rebase origin main` |
| push | `git push origin main` |

임시 브랜치 작업 중에는 현재 `agent/<scope>`에서 커밋하되, §5.2 통합·삭제 절차를 거쳐 최종 push 대상은 `main`으로 한다.

**commit-gate 층**: hard(`env-lint`, `staged_secret_gate`) — `--no-verify` **금지** · soft(`lint-fix`, FE strict `typecheck`, archive) — 예외 가능하나 후속 통과 필요.

**배포 게이트 (SSOT)**: `just renderer-ship-gate` = `apps/renderer/vercel.json`의 `buildCommand`와 동일 (`EMR_UI_TARGET=cloud next build --webpack`). **commit 통과 ≠ 배포 가능** — main push 시 GitHub CI `renderer-ship-gate` job이 필수(required). 로컬 push 전에도 동일 명령 실행을 권장.

| 검증 레이어 | 명령 | FE TypeScript | Cloud build |
| :--- | :--- | :---: | :---: |
| commit (pre-commit) | `just commit-gate` | strict (0 허용) | — |
| push / CI | `renderer-ship-gate` job | (build 내장) | ✅ Vercel 동일 |
| 세션 종료 | `just sync-turn-end` | strict | (선택) |

### 게이트 실패 시 원인 분류

`just commit-gate` 실패 시 다음 순서로 원인 분류:

```bash
just commit-gate-hard   # 1. hard 게이트 (보안) — 먼저 확인
just commit-gate-soft   # 2. soft 게이트 (lint/ty) — hard 통과 시 확인
```

**hard 게이트 실패** → 변경사항 수정 후 재시도 (우회 금지)
**soft 게이트 실패** → §6.1 ty 에러 pre-existing 확인. 한시 escape hatch는 [PROJECT_RULES.md §3.7](../../PROJECT_RULES.md#37-게이트-우회-금지-gate-bypass-prohibition-must--zero-tolerance) SSOT — 3단계: ① `just commit-gate-hard` 통과 ② Surgical 예외로 에러 최소 수정 후 재시도 ③ 수정 불가 시 `--no-verify` 1회. `just sync-turn-end`·CI에서 재차단, main push 전 해결 필수.

### §6.1 ARG001 미사용 인자 처리

```bash
# 미사용 인자 발견 시:
# 1. 정말 미사용이면 함수 시그니처에서 제거
# 2. API 계약상 필요하면 _prefix 패턴 사용 (의도적 미사용 명시)
# 3. 아니면 # noqa: ARG001 주석 추가
```

**규칙**: `_prefix`, `_env_name` 등 underscore prefix는 "의도적 미사용"을 명시하는 좋은 관행

### §6.2 unstaged 변경사항 정리

push 전 rebase 실패 방지:

```bash
# unstaged 변경사항 확인 (tracked 파일만)
git status --short | grep -v "^??"

# unstaged가 많으면 stash 후 push
git stash push -m "pre-push-wip"
git push origin main
git stash pop
```

**stash pathspec 구문**: `git stash push -m "message" -- path/to/file` 형태로 `--` 뒤에 경로를 명시해야 함. `git stash push -m "message" path/to/file` (dash 없이)는 구문 오류.

### §6.3 stash rebase 패턴

rebase 전 unstaged 변경사항 처리:

```bash
# rebase 전 stash
git stash push -m "pre-rebase-wip"

# rebase 실행
git pull --rebase origin main

# stash 복원
git stash pop
```

**주의**: `git stash`만 실행하면 전체 unstaged가 stash됨. 특정 파일만 stash하려면 `git stash push -m "message" -- path/to/file` 사용.

---

## §0 Branch Entry & Pull-First

### Branch Representation (MUST)

- **원격 기준 브랜치**: `origin/main`
- **실제 작업 브랜치**: `origin/main`을 tracking하는 로컬 `main`
- **commit 수행**: 로컬 `main`에서 수행
- **push 대상**: `origin/main`
- **금지**: `origin/main`을 직접 checkout하여 detached HEAD 상태에서 작업하지 않는다.

### 기본: main 직접 작업 — 세션 시작 게이트

```text
1. git fetch origin
2. git switch main
3. git pull --ff-only origin main
4. git status --short
```

**작업 시작 조건 (MUST)**: working tree가 clean하고 로컬 main이 origin/main과 동기화된 상태여야만 작업을 시작한다. 조건 미달 시 원인 해결 후 재시도.

`--ff-only`가 실패하면 임의 merge commit을 만들지 않는다. local-only commit이 있으면 원인을 확인하고 `git pull --rebase origin main`으로 선형화한다.

### 예외: 병렬 write 임시 브랜치

```text
1. 최신 main 확인
2. git switch -c agent/<short-scope>
3. 해당 세션의 명시된 파일만 수정
4. main 반영 전 §5.2 실행
```

단순 조사·read-only 작업은 브랜치를 만들지 않는다.

---

## 게이트 — §5.0~§5.2 슬라이스 루프

**완료 조건**: `git status --short`에 커밋 대상 없음 (NEVER_STAGE·제외 untracked만 잔존 가능).

### §5.0 NEVER_STAGE (스테이징 금지)

| 패턴 | 사유 |
|------|------|
| `agents/route/session-manifest.json`, `.agent/route/**` | 세션 route 매니페스트 |
| `test-results/**`, `playwright-report/**` | E2E 아티팩트 |
| `.env`, `.env.*`, `*.pem`, `*.key`, `*.db` | 비밀·로컬 DB |

### §5.1 슬라이스 루프 (Mandatory)

```text
WHILE 커밋 대상(M/N/??) 존재:
  1. §5.0.1 휴리스틱으로 다음 슬라이스 S 선택
  2. git add <path1> <path2> ...  (S만)
  3. Scope 검증 (해당 paths 기준 최소 1회)
  4. git commit -m "type(scope): [ID] 요약" (+ 본문 Verify 1줄)
  5. commit-gate 실패 → 해당 슬라이스만 수정 → 3부터 (최대 3회)
END WHILE
```

- **혼합 금지**: 서로 다른 scope를 한 커밋에 넣지 않는다.
- **push 거부** → rebase 후 재시도 (`force push` 금지).

**§5.0.1 슬라이스 휴리스틱** (동일 prefix·Blueprint·`git diff` 주제 → 한 슬라이스):

| 경로 prefix | type(scope) 예 |
|-------------|----------------|
| `agents/**`, `AGENTS.md`, `scripts/agent/**` | `docs(agent)` |
| `docs/blueprints/**`, `docs/discussions/**` | `docs(plans)` / `docs(...)` |
| `apps/renderer/**` | `feat(renderer)` / `fix(renderer)` |
| `src/**`, `tests/**` | `fix(backend)` / `test(...)` |
| `Justfile`, `scripts/verify/**` | `chore(tools)` |

### §5.2 Main Push & Temporary Branch Cleanup

#### main 직접 작업

```bash
git pull --rebase origin main
git push origin main
```

#### 임시 브랜치 작업

```bash
# 임시 브랜치에서 검증·커밋 완료 후
git fetch origin
git rebase origin/main

git switch main
git pull --ff-only origin main
git merge --ff-only agent/<short-scope>
git push origin main

# 같은 작업 주기에서 정리
git branch -d agent/<short-scope>
git push origin --delete agent/<short-scope>  # remote에 push된 경우만
```

fast-forward가 불가능하면 다른 세션의 최신 변경과 충돌한 것이다. 임의 merge commit을 만들지 말고 임시 브랜치를 최신 `origin/main`에 다시 rebase한 뒤 재검증한다.

### §5.3 최종 보고

작업 완료 후 최종 보고에는 다음을 명시한다:

```text
- local main commit SHA: <abbreviated SHA>
- origin/main push 성공 여부: <성공/실패>
```

### Push 명령

```bash
git add <의도한 파일>
git commit -m "<명확한 커밋 메시지>"
git push origin main
```

**에이전트 종료점**: `main` push 성공과 사용 완료된 임시 브랜치 삭제까지. PR 생성은 선택 사항이며 기본 수행하지 않는다.

---

## lazy (필요 시 Read)

| 주제 | SSOT |
| :--- | :--- |
| 브랜치 정책 | `PROJECT_RULES.md §2.3` · 본 문서 `Branch Topology` |
| 슬라이스 경계 모호 | `git diff` 의미·Blueprint 참조 — **더 좁은 scope**로 분리 |
| 불확실 상황 대응 | [verification.md](../core/verification.md) |
| SSOT 문서 갱신 (0~1단계) | `PROJECT_RULES.md` §6 · `docs/specs/` |
| Scope L1/L2/L3 | [verification.md](../core/verification.md) |
| 커밋 메시지 형식 | `type(scope): 요약` — **본 문서 §5.1** (커밋 메시지 SSOT) |
| Feature-split commits | AGENTS.md §5 pointer → **본 문서 §5.1** |
| Verify Report | [`verification.md`](../core/verification.md) §1산출물 |
