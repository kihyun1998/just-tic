# 4. rename 감지 ON

- Status: Accepted
- Date: 2026-06-08
- Source: PRD (#1), Q8 (diff)

## Context

파일/폴더를 옮긴 날 숫자가 튀면 안 된다. rename을 감지하지 않으면 이동이 "전부 삭제 +
전부 추가"(`+500 -500`)로 잡혀 실제 작업량을 왜곡한다.

## Decision

- 커밋별 부모 대비 diff에서 **rename(유사도) 감지를 명시적으로 ON**으로 둔다.
- 기본값은 git 기본과 같은 50% 유사도·복사 미추적(`git log --numstat` 기본 동작과 일치).
- gix 기본은 ambient git config(`diff.renames`)를 따르므로, 사용자가 그걸 꺼 두면 숫자가
  어긋난다 → 코드에서 명시적으로 켜서 환경 의존을 제거한다.

## Consequences

- 순수 이동은 단일 Rewrite로 합쳐져 `0/0`, 이동+수정은 변경분만 잡힌다.
- 디렉터리 이동이 숫자를 부풀리지 않는다.
- 결과가 사용자의 로컬 `diff.renames` 설정과 무관하게 일관된다.
