# 6. 시계 주입 경계 — 코어/CLI 분리

- Status: Accepted
- Date: 2026-06-08
- Source: PRD (#1), Implementation Decisions · User Story 20 · 21

## Context

자정 경계·DST를 결정적으로 검증하려면 테스트가 시각을 통제할 수 있어야 한다. 또한 미래의
Tauri 프런트엔드가 CLI 없이 같은 집계 로직을 재사용할 수 있어야 한다.

## Decision

- 코어의 공개 API는 **함수 하나**: `tally(repo, now) -> Result<Tally>`.
- **코어는 시계를 직접 읽지 않는다** — 호출자가 `now`(로컬 tz 인지 시각)를 주입한다.
- 실제 시계·로컬 tz·환경(TTY/`NO_COLOR`)은 **`main.rs`(CLI)만** 읽는다.
- 반환 타입 `Tally`는 `serde::Serialize` 파생 — JSON 출력이자, 코어 반환을 깨끗한 직렬화
  가능 구조체로 강제한다.

## Consequences

- 테스트가 고정된 `now`로 자정 경계·DST·미래시각 제외를 결정적으로 검증한다.
- 같은 코어 lib(`just_tic`)를 GUI(Tauri 등)에서 그대로 재사용할 수 있다.
- CLI는 "시계 읽기 → 코어 호출 → 출력"만 하는 얇은 껍데기로 유지된다.
- 이 경계가 곧 테스트성과 재사용의 핵심 — 깨면 두 이점이 동시에 무너진다.
