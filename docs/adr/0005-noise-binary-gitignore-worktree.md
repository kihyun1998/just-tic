# 5. 잡음 처리 — binary · gitignore · 워킹트리

- Status: Accepted
- Date: 2026-06-08
- Source: PRD (#1), Q6 · Q7

## Context

줄 수 합산에 끼면 안 되는 잡음원이 여럿이다: 바이너리 파일(이미지/PDF), 추적하지 않는
산출물(gitignore), 그리고 author date가 없는 미커밋 워킹트리 변경.

## Decision

- **추적(tracked) 파일의 오늘 커밋 줄을 전부 카운트.** 별도 denylist/`.gitattributes` 필터 없음.
- **바이너리**: numstat이 dash(`-`)를 주므로 "dash면 0" 처리로 자동 0 기여.
- **gitignore**: untracked라 커밋 numstat에 애초에 안 나타나므로 자동 제외(처리 코드 불필요).
- **워킹트리**: **커밋만** 집계. staged/unstaged 미커밋 변경은 v1에서 제외 — 워킹트리 변경엔
  author date가 없어 "오늘" 정의에 안 맞는다.

## Consequences

- 이미지/PDF 커밋이 줄 수를 오염시키지 않는다.
- **수용된 한계**: 추적되던 파일을 나중에 gitignore해도 git이 계속 추적하면 여전히 집계된다.
  결과적으로 추적되는 `Cargo.lock`을 regen한 날은 숫자가 그대로 튄다.
- 향후 `--exclude <glob>` 또는 `.gitattributes linguist-generated` 존중, `--include-uncommitted`는
  별도 결정 사항(현재 범위 밖).
