# 2. 커밋 집합 — 로컬 브랜치 multi-tip revwalk + dedup

- Status: Accepted
- Date: 2026-06-08
- Source: PRD (#1), Q4

## Context

지금 HEAD가 어디 있든 "오늘 총량"은 정확해야 한다. 오전엔 feature 브랜치, 오후엔 master에
커밋한 날 둘 다 잡혀야 하고, 여러 브랜치가 공유하는 커밋(공통 조상)이 중복으로 세지면 안 된다.
`fetch`로 들어온 원격 커밋이 내 작업량을 부풀려서도 안 된다.

## Decision

- 순회 시작점(tip)은 **로컬 브랜치 전체**(`refs/heads/*`)에서 모은다.
- `refs/remotes/*`(원격 추적 ref)는 **제외**한다.
- 모든 tip에서 **단일 multi-tip revwalk**로 순회하고, 방문 표시(commit id)로 **dedup**한다.
- 로컬 브랜치가 하나도 없으면(detached HEAD 등) **HEAD를 tip으로 폴백**, HEAD도 없으면(빈 레포)
  `0/0/0`.

## Consequences

- HEAD 위치와 무관하게 오늘 작업이 빠짐없이 합산된다.
- 공통 조상 커밋은 commit id로 한 번만 세진다(순회의 성질로 달성 — 별도 dedup 코드 불필요).
- `fetch`가 숫자를 부풀리지 못한다(원격 추적 ref 제외).
- 성능: author-date 필터가 오래된 tip을 자연 prune한다. 단 커밋 날짜가 위상순서와 항상
  일치하진 않으므로 "자정보다 오래되면 순회 중단" 같은 **순진한 조기 종료는 금지**(놓침 위험).
