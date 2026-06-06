//! just-tic core: 오늘(로컬 타임존 자정~지금) author date 커밋들의 numstat 합산.
//!
//! 코어는 시계를 직접 읽지 않는다 — 호출자가 `now`를 주입한다. 그래야 테스트가
//! 고정된 시각으로 자정 경계·DST를 결정적으로 검증할 수 있다.

use jiff::civil::Date;
use jiff::{Timestamp, Zoned};
use serde::Serialize;

/// 오늘 합산 결과: 추가/삭제 줄 수와 집계에 포함된 커밋 수.
///
/// `Serialize` 파생은 `--json` 출력(#5)을 위한 것이자, 코어 반환 타입을 직렬화
/// 가능한 깨끗한 구조체로 강제한다(미래 Tauri 재사용에도 이득).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Tally {
    pub additions: u64,
    pub deletions: u64,
    pub commits: u64,
}

impl Tally {
    /// 사람이 읽는 plain 한 줄 출력(ANSI 색 없음). non-TTY/파이프·`NO_COLOR`용.
    /// 색 적용 여부는 CLI(#6)가 환경을 보고 판단해 [`Self::to_human_line_colored`]와 고른다.
    ///
    /// 오늘 커밋이 없으면 빈 stdout 대신 명시적 메시지를 낸다(스크립트 안전).
    pub fn to_human_line(&self) -> String {
        if self.commits == 0 {
            return "+0 -0 · no commits today".to_string();
        }
        format!(
            "+{} -{} · {} commits",
            self.additions, self.deletions, self.commits
        )
    }

    /// 색을 입힌 휴먼 한 줄. `+N`은 초록, `-N`은 빨강(ANSI). TTY일 때만 쓰인다.
    ///
    /// 색 적용 여부(TTY·`NO_COLOR`)는 호출자가 판단한다 — 코어는 환경을 모른다.
    /// 0커밋 메시지는 카운트가 아닌 상태 문구라 색 없이 그대로 둔다([`Self::to_human_line`]와 동일).
    pub fn to_human_line_colored(&self) -> String {
        if self.commits == 0 {
            return "+0 -0 · no commits today".to_string();
        }
        // \x1b[32m=초록, \x1b[31m=빨강, \x1b[0m=리셋. 각 카운트만 감싸고 나머지는 plain.
        format!(
            "\x1b[32m+{}\x1b[0m \x1b[31m-{}\x1b[0m · {} commits",
            self.additions, self.deletions, self.commits
        )
    }

    /// `--json` 출력 한 줄. `date`(로컬 "오늘" 날짜) + 합산 필드를 단일 객체로 낸다:
    /// `{"date":"2026-06-05","additions":127,"deletions":34,"commits":5}`.
    ///
    /// 날짜는 `Tally`에 없으므로(코어는 시계를 모름) 호출자가 로컬 날짜를 주입한다.
    /// 0커밋이어도 유효한 객체(필드 전부 0)를 낸다 — 스크립트/`jq` 안전.
    pub fn to_json_line(&self, date: Date) -> String {
        // 날짜를 먼저 둔 한 줄 객체를 만들기 위해 Tally를 flatten한 뷰를 직렬화한다.
        #[derive(Serialize)]
        struct JsonView<'a> {
            date: String,
            #[serde(flatten)]
            tally: &'a Tally,
        }
        let view = JsonView {
            date: date.to_string(),
            tally: self,
        };
        // 직렬화 대상이 String·u64뿐이라 실패하지 않는다.
        serde_json::to_string(&view).expect("Tally JSON 직렬화는 실패하지 않는다")
    }
}

/// 현재 레포에서 `now` 기준 "오늘"(로컬 자정~now) author date 커밋들의 numstat을 합산한다.
///
/// 시계는 호출자가 `now`로 주입한다 — 코어는 시계를 읽지 않는다.
///
/// 커밋 집합은 로컬 브랜치(`refs/heads/*`) 전체에서 reachable한 커밋을 commit id로
/// dedup해 모은다(`refs/remotes/*` 제외). 머지 커밋(부모 2+)은 완전 제외한다
/// (--no-merges). rename 미감지(#5).
pub fn tally(repo: &gix::Repository, now: Zoned) -> anyhow::Result<Tally> {
    let window = Window::for_day(now);
    let mut total = Tally::default();

    // 로컬 브랜치(refs/heads/*) tip 전부를 순회 시작점으로 모은다.
    // refs/remotes/*는 제외 — fetch로 들어온 커밋이 숫자를 부풀리지 못하게.
    let mut tips: Vec<gix::ObjectId> = Vec::new();
    for branch in repo.references()?.local_branches()? {
        // local_branches 이터레이터의 에러는 Box<dyn Error>라 anyhow로 감싼다.
        let branch = branch.map_err(anyhow::Error::msg)?;
        tips.push(branch.into_fully_peeled_id()?.detach());
    }

    // 로컬 브랜치가 없으면(detached HEAD 등) HEAD로 폴백. HEAD도 없으면(빈 레포) 0/0/0.
    if tips.is_empty() {
        match repo.head_commit() {
            Ok(commit) => tips.push(commit.id().detach()),
            Err(_) => return Ok(total),
        }
    }

    // 블롭 라인 diff용 캐시 — 트리 순회 캐시와 별개로 한 번 만들어 재사용한다.
    let mut diff_cache = repo.diff_resource_cache_for_tree_diff()?;

    // 모든 tip에서 단일 multi-tip revwalk — 공유 커밋은 commit id로 한 번만 방문(dedup).
    for info in repo.rev_walk(tips).all()? {
        let commit = repo.find_commit(info?.id)?;

        // 머지 커밋(부모 2개 이상)은 완전 제외 — numstat·commits 둘 다 빼고 건너뛴다.
        // git log --no-merges와 일치. 머지가 가져온 줄은 원본 커밋에서 이미 세므로
        // 재카운트를 막는다. (충돌 해결로 머지에만 있는 줄은 집계 안 됨 — 수용된 한계.)
        if commit.parent_ids().take(2).count() > 1 {
            continue;
        }

        // author date(UTC 인스턴트)가 오늘 구간에 속하지 않으면 건너뛴다.
        let authored = Timestamp::from_second(commit.author()?.time()?.seconds)?;
        if !window.contains(authored) {
            continue;
        }
        total.commits += 1;

        // #2 범위: 머지 커밋도 포함(첫 부모 대비 diff). 머지 skip은 #4.
        let (additions, deletions) =
            numstat_against_first_parent(repo, &commit, &mut diff_cache)?;
        total.additions += additions;
        total.deletions += deletions;
    }

    Ok(total)
}

/// 한 커밋을 첫 부모(루트면 빈 트리)와 비교해 추가/삭제 줄 수를 합산한다.
///
/// gix의 tree diff는 디렉터리 추가/삭제 시 잎 blob을 따로 yield하면서 부모 tree
/// 단위 change도 함께 낸다. tree는 blob diff가 불가하고 잎에서 이미 세므로 건너뛴다
/// (이중 카운트 방지). 바이너리면 `line_counts()`가 `None` → 0 기여(자동 처리).
///
/// 이 슬라이스(#2)는 rename을 감지하지 않는다(gix 기본). rename 감지는 #5.
fn numstat_against_first_parent(
    repo: &gix::Repository,
    commit: &gix::Commit<'_>,
    diff_cache: &mut gix::diff::blob::Platform,
) -> anyhow::Result<(u64, u64)> {
    let new_tree = commit.tree()?;
    let old_tree = match commit.parent_ids().next() {
        Some(parent) => repo.find_commit(parent.detach())?.tree()?,
        None => repo.empty_tree(),
    };

    let mut additions = 0u64;
    let mut deletions = 0u64;
    old_tree
        .changes()?
        .for_each_to_obtain_tree(&new_tree, |change| {
            let cont = std::ops::ControlFlow::Continue(());
            if change.entry_mode().is_tree() {
                return Ok::<_, Box<dyn std::error::Error + Send + Sync>>(cont);
            }
            if let Some(stats) = change.diff(diff_cache)?.line_counts()? {
                additions += u64::from(stats.insertions);
                deletions += u64::from(stats.removals);
            }
            Ok(cont)
        })?;
    diff_cache.clear_resource_cache();

    Ok((additions, deletions))
}

/// 절대(UTC) 인스턴트로 표현한 반열림 구간 `[start, end)`.
///
/// "오늘" 멤버십은 각 커밋의 UTC 인스턴트를 이 구간과 비교해 판정한다. 커밋에
/// 기록된 타임존 offset은 멤버십에 무관하다 — 비교는 절대시각으로만 이뤄진다.
pub struct Window {
    start: Timestamp,
    end: Timestamp,
}

impl Window {
    /// 로컬 기준 "오늘"을 덮는 구간 `[로컬 자정, now)`을 만든다.
    pub fn for_day(now: Zoned) -> Self {
        let end = now.timestamp();
        let start = now
            .start_of_day()
            .expect("local day start within representable range")
            .timestamp();
        Window { start, end }
    }

    /// `instant`가 `[start, end)`에 속하는가? (하한 닫힘, 상한 열림)
    pub fn contains(&self, instant: Timestamp) -> bool {
        self.start <= instant && instant < self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn human_line_shows_additions_deletions_and_commit_count() {
        let tally = Tally {
            additions: 127,
            deletions: 34,
            commits: 5,
        };
        assert_eq!(tally.to_human_line(), "+127 -34 · 5 commits");
    }

    #[test]
    fn human_line_for_no_commits_is_explicit() {
        // 빈 stdout 대신 명시적 메시지(스크립트 안전).
        assert_eq!(Tally::default().to_human_line(), "+0 -0 · no commits today");
    }

    #[test]
    fn colored_line_wraps_counts_in_green_and_red() {
        let tally = Tally {
            additions: 127,
            deletions: 34,
            commits: 5,
        };
        // +N=초록(32), -N=빨강(31), 커밋 수는 plain.
        assert_eq!(
            tally.to_human_line_colored(),
            "\x1b[32m+127\x1b[0m \x1b[31m-34\x1b[0m · 5 commits"
        );
    }

    #[test]
    fn colored_line_for_no_commits_has_no_ansi() {
        // 0커밋 상태 문구는 색 없이 plain — 휴먼 plain과 동일.
        let line = Tally::default().to_human_line_colored();
        assert_eq!(line, "+0 -0 · no commits today");
        assert!(!line.contains('\x1b'), "0커밋 메시지엔 ANSI가 없어야 한다");
    }

    #[test]
    fn json_line_is_a_single_object_with_date_first() {
        // 출력 계약 스냅샷 — 키·순서·형식을 고정해 회귀를 막는다.
        let tally = Tally {
            additions: 127,
            deletions: 34,
            commits: 5,
        };
        assert_eq!(
            tally.to_json_line(date(2026, 6, 5)),
            r#"{"date":"2026-06-05","additions":127,"deletions":34,"commits":5}"#
        );
    }

    #[test]
    fn json_line_for_no_commits_is_still_a_valid_object() {
        // 0커밋이어도 빈 출력이 아니라 필드 전부 0인 유효 객체(jq 안전).
        assert_eq!(
            Tally::default().to_json_line(date(2026, 6, 5)),
            r#"{"date":"2026-06-05","additions":0,"deletions":0,"commits":0}"#
        );
    }

    #[test]
    fn instant_within_today_is_contained() {
        let now = date(2026, 6, 5)
            .at(12, 0, 0, 0)
            .in_tz("America/New_York")
            .unwrap();
        let window = Window::for_day(now);

        let commit = date(2026, 6, 5)
            .at(9, 0, 0, 0)
            .in_tz("America/New_York")
            .unwrap()
            .timestamp();

        assert!(window.contains(commit));
    }

    #[test]
    fn midnight_is_included() {
        let now = date(2026, 6, 5)
            .at(12, 0, 0, 0)
            .in_tz("America/New_York")
            .unwrap();
        let window = Window::for_day(now);

        let midnight = date(2026, 6, 5)
            .at(0, 0, 0, 0)
            .in_tz("America/New_York")
            .unwrap()
            .timestamp();

        assert!(window.contains(midnight), "하한(자정)은 닫혀 있어야 한다");
    }

    #[test]
    fn now_is_excluded() {
        let now = date(2026, 6, 5)
            .at(12, 0, 0, 0)
            .in_tz("America/New_York")
            .unwrap();
        let now_ts = now.timestamp();
        let window = Window::for_day(now);

        assert!(!window.contains(now_ts), "상한(now)은 열려 있어야 한다");
    }

    #[test]
    fn yesterday_is_excluded() {
        let now = date(2026, 6, 5)
            .at(12, 0, 0, 0)
            .in_tz("America/New_York")
            .unwrap();
        let window = Window::for_day(now);

        let yesterday = date(2026, 6, 4)
            .at(23, 0, 0, 0)
            .in_tz("America/New_York")
            .unwrap()
            .timestamp();

        assert!(!window.contains(yesterday));
    }

    #[test]
    fn dst_spring_forward_day_membership() {
        // 2026-03-08 America/New_York: 02:00 EST → 03:00 EDT (하루가 23시간).
        let now = date(2026, 3, 8)
            .at(12, 0, 0, 0)
            .in_tz("America/New_York")
            .unwrap();
        let window = Window::for_day(now);

        // 그날 자정(EST)은 포함.
        let midnight = date(2026, 3, 8)
            .at(0, 0, 0, 0)
            .in_tz("America/New_York")
            .unwrap()
            .timestamp();
        assert!(window.contains(midnight), "DST 날 자정도 포함");

        // 봄철 점프 직후(EDT)의 인스턴트도 포함.
        let after_gap = date(2026, 3, 8)
            .at(4, 0, 0, 0)
            .in_tz("America/New_York")
            .unwrap()
            .timestamp();
        assert!(window.contains(after_gap), "DST gap 이후 인스턴트도 포함");

        // 전날 밤은 제외.
        let yesterday = date(2026, 3, 7)
            .at(23, 30, 0, 0)
            .in_tz("America/New_York")
            .unwrap()
            .timestamp();
        assert!(!window.contains(yesterday));
    }
}
