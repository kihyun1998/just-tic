# CONTEXT — just-tic (`jtic`)

`jtic`은 현재 git 레포에서 **오늘(로컬 타임존 자정~지금) author date 커밋들**의 numstat을
합산해 추가/삭제 줄 수를 한 줄로 보여주는 CLI다. 이 문서는 코드 탐색·이슈 작성 전에 읽는
도메인 용어집이며, 결정의 근거는 [`docs/adr/`](docs/adr/)에 기록한다.

## 용어집 (Glossary)

이 프로젝트의 산출물(이슈 제목, 리팩터 제안, 테스트 이름)은 아래 용어를 그대로 쓴다.

| 용어 | 정의 |
|------|------|
| **Tally** | 합산 결과 구조체 — `additions`, `deletions`, `commits`. 코어의 공개 반환 타입(serde 직렬화 가능). |
| **`tally(repo, now)`** | 코어의 **단일 공개 API**. 레포와 *주입된* 현재 시각을 받아 `Tally`를 반환한다. |
| **Window** | "오늘"을 나타내는 **반열림** 시간 구간 `[로컬 자정, now)`. 멤버십은 UTC 인스턴트로 판정. |
| **author date** | 커밋 저자가 작성한 시각. "오늘" 판정의 기준이며 **committer date가 아니다**. |
| **numstat** | 파일별 추가/삭제 줄 수(`git log --numstat` 관례). 바이너리는 dash(`-`) → 0 기여. |
| **tip** | 브랜치가 가리키는 최신 커밋. revwalk의 시작점. |
| **multi-tip revwalk** | 여러 tip에서 reachable한 커밋을 단일 순회로 방문. 방문 표시가 곧 dedup. |
| **dedup** | 여러 브랜치에 공유된 커밋을 commit id 기준 **한 번만** 세는 것. |
| **merge commit** | 부모가 2개 이상인 커밋. 집계에서 완전히 제외(skip). |
| **rename 감지** | diff에서 파일/폴더 이동을 유사도로 인식 — 순수 이동은 `0/0`. |
| **clock injection** | 코어가 시계를 직접 읽지 않고 `now`를 인자로 받는 경계. 테스트성·재사용의 핵심. |

## 핵심 경계 (Architecture)

- **코어 (lib `just_tic`)** — 순수 집계. 시계도 환경도 직접 읽지 않는다.
- **CLI (`jtic`, `src/main.rs`)** — 얇은 껍데기. 시계·로컬 tz·환경(TTY/`NO_COLOR`)을 읽어 코어에
  주입하고 결과를 출력한다. 미래의 Tauri 프런트엔드도 같은 코어를 재사용한다.

## 결정 기록 (ADR)

| ADR | 주제 |
|-----|------|
| [0001](docs/adr/0001-today-by-author-date-half-open-window.md) | "오늘" 판정 — author date + 반열림 로컬 구간 |
| [0002](docs/adr/0002-commit-set-local-branches-dedup.md) | 커밋 집합 — 로컬 브랜치 multi-tip revwalk + dedup |
| [0003](docs/adr/0003-skip-merge-commits.md) | 머지 커밋 skip |
| [0004](docs/adr/0004-rename-detection-on.md) | rename 감지 ON |
| [0005](docs/adr/0005-noise-binary-gitignore-worktree.md) | 잡음 처리 — binary·gitignore·워킹트리 |
| [0006](docs/adr/0006-clock-injection-core-cli-split.md) | 시계 주입 경계 — 코어/CLI 분리 |
