//! `tally()` 통합 테스트 — 실제 git 레포 fixture를 만들어 공개 API로 검증한다.
//!
//! fixture는 `git` CLI로 author date를 통제한 커밋을 만든다. `tally`는 그 레포를
//! gix로 읽어 합산하므로, 외부 소비자(예: 미래의 Tauri)와 같은 경로를 탄다.

use std::path::Path;
use std::process::Command;

use jiff::civil::date;
use jiff::Zoned;

/// 고정 tz의 `now`를 만든다. 커밋 author date도 같은 tz 기준으로 두면 결정적.
fn now_utc(y: i16, m: i8, d: i8, h: i8, min: i8) -> Zoned {
    date(y, m, d).at(h, min, 0, 0).in_tz("UTC").unwrap()
}

/// 통제된 author/committer date로 git 명령을 실행한다.
fn git(dir: &Path, args: &[&str], date_rfc3339: Option<&str>) {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir).args(args);
    if let Some(d) = date_rfc3339 {
        cmd.env("GIT_AUTHOR_DATE", d).env("GIT_COMMITTER_DATE", d);
    }
    let out = cmd.output().expect("git 실행 실패");
    assert!(
        out.status.success(),
        "git {:?} 실패:\n{}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

/// 새 temp 레포를 만들고 초기 설정을 마친다.
fn init_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-b", "main"], None);
    git(p, &["config", "user.email", "t@example.com"], None);
    git(p, &["config", "user.name", "Test"], None);
    dir
}

/// 파일에 내용을 쓰고 주어진 author date로 커밋한다.
fn commit_file(dir: &Path, name: &str, contents: &str, date_rfc3339: &str) {
    commit_bytes(dir, name, contents.as_bytes(), date_rfc3339);
}

/// 바이트 내용을 쓰고 커밋한다 (바이너리 파일 테스트용). 중첩 경로 지원.
fn commit_bytes(dir: &Path, name: &str, contents: &[u8], date_rfc3339: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
    git(dir, &["add", name], None);
    git(dir, &["commit", "-m", name], Some(date_rfc3339));
}

#[test]
fn counts_additions_of_a_commit_authored_today() {
    let repo_dir = init_repo();
    let p = repo_dir.path();

    // 3줄짜리 파일을 오늘(2026-06-05) 09:00Z에 커밋.
    commit_file(p, "a.txt", "one\ntwo\nthree\n", "2026-06-05T09:00:00+00:00");

    let repo = gix::open(p).unwrap();
    let result = just_tic::tally(&repo, now_utc(2026, 6, 5, 12, 0)).unwrap();

    assert_eq!(result.additions, 3);
    assert_eq!(result.deletions, 0);
    assert_eq!(result.commits, 1);
}

#[test]
fn excludes_a_commit_authored_yesterday() {
    let repo_dir = init_repo();
    let p = repo_dir.path();

    // 어제 커밋(2줄)과 오늘 커밋(3줄)을 서로 다른 파일에.
    commit_file(p, "y.txt", "a\nb\n", "2026-06-04T09:00:00+00:00");
    commit_file(p, "t.txt", "one\ntwo\nthree\n", "2026-06-05T09:00:00+00:00");

    let repo = gix::open(p).unwrap();
    let result = just_tic::tally(&repo, now_utc(2026, 6, 5, 12, 0)).unwrap();

    // 오늘 커밋만 집계되어야 한다.
    assert_eq!(result.commits, 1);
    assert_eq!(result.additions, 3);
    assert_eq!(result.deletions, 0);
}

#[test]
fn empty_repo_yields_zero() {
    let repo_dir = init_repo(); // 커밋 없음 → unborn HEAD
    let p = repo_dir.path();

    let repo = gix::open(p).unwrap();
    let result = just_tic::tally(&repo, now_utc(2026, 6, 5, 12, 0)).unwrap();

    assert_eq!(result, just_tic::Tally::default());
}

#[test]
fn counts_insertions_and_removals_of_a_modification() {
    let repo_dir = init_repo();
    let p = repo_dir.path();

    // 어제 베이스(집계 제외) → 오늘 수정(집계 대상).
    commit_file(p, "f.txt", "a\nb\nc\n", "2026-06-04T09:00:00+00:00");
    // a,b,c → a,X,c,d : b 삭제·X 추가(수정) + d 추가  → 추가 2, 삭제 1
    commit_file(p, "f.txt", "a\nX\nc\nd\n", "2026-06-05T09:00:00+00:00");

    let repo = gix::open(p).unwrap();
    let result = just_tic::tally(&repo, now_utc(2026, 6, 5, 12, 0)).unwrap();

    assert_eq!(result.commits, 1);
    assert_eq!(result.additions, 2);
    assert_eq!(result.deletions, 1);
}

#[test]
fn counts_files_added_inside_new_subdirectories() {
    let repo_dir = init_repo();
    let p = repo_dir.path();

    // 새 디렉터리를 통째로 추가하는 커밋 — tree 단위 change가 발생한다.
    commit_file(p, "src/main.rs", "fn main() {}\n", "2026-06-05T09:00:00+00:00");

    let repo = gix::open(p).unwrap();
    let result = just_tic::tally(&repo, now_utc(2026, 6, 5, 12, 0)).unwrap();

    assert_eq!(result.commits, 1);
    assert_eq!(result.additions, 1, "중첩 디렉터리 안의 파일도 합산되어야 한다");
    assert_eq!(result.deletions, 0);
}

#[test]
fn binary_file_contributes_zero() {
    let repo_dir = init_repo();
    let p = repo_dir.path();

    // NUL 바이트를 포함한 바이너리 파일 → numstat dash → 0 기여.
    let binary = [0u8, 1, 2, 0, 255, 0, 42, 7];
    commit_bytes(p, "blob.bin", &binary, "2026-06-05T09:00:00+00:00");

    let repo = gix::open(p).unwrap();
    let result = just_tic::tally(&repo, now_utc(2026, 6, 5, 12, 0)).unwrap();

    // 커밋은 오늘이므로 집계되지만, 줄 수 기여는 0.
    assert_eq!(result.commits, 1);
    assert_eq!(result.additions, 0);
    assert_eq!(result.deletions, 0);
}

#[test]
fn sums_today_commits_across_diverged_branches() {
    let repo_dir = init_repo();
    let p = repo_dir.path();

    // 공통 조상(어제, 집계 제외)에서 두 브랜치가 갈라진다.
    commit_file(p, "base.txt", "base\n", "2026-06-04T09:00:00+00:00");
    git(p, &["branch", "feature"], None);

    // main 에만 있는 오늘 커밋 (2줄).
    commit_file(p, "m.txt", "m1\nm2\n", "2026-06-05T09:00:00+00:00");

    // feature 로 옮겨 main 에 없는 오늘 커밋 (3줄). HEAD 는 feature 에 남는다.
    git(p, &["checkout", "feature"], None);
    commit_file(p, "f.txt", "f1\nf2\nf3\n", "2026-06-05T10:00:00+00:00");

    let repo = gix::open(p).unwrap();
    let result = just_tic::tally(&repo, now_utc(2026, 6, 5, 12, 0)).unwrap();

    // HEAD(feature)-only면 main 의 m1 을 놓쳐 3/1 이 된다.
    // 로컬 브랜치 전체를 보면 양쪽 합 = 5/2 여야 한다.
    assert_eq!(result.commits, 2);
    assert_eq!(result.additions, 5);
    assert_eq!(result.deletions, 0);
}

#[test]
fn shared_ancestor_is_counted_once() {
    let repo_dir = init_repo();
    let p = repo_dir.path();

    // 오늘 base 커밋을 두 브랜치가 공유한다.
    commit_file(p, "base.txt", "b1\nb2\n", "2026-06-05T08:00:00+00:00");
    git(p, &["branch", "feature"], None);

    commit_file(p, "m.txt", "m1\nm2\n", "2026-06-05T09:00:00+00:00"); // main 전용
    git(p, &["checkout", "feature"], None);
    commit_file(p, "f.txt", "f1\nf2\n", "2026-06-05T10:00:00+00:00"); // feature 전용

    let repo = gix::open(p).unwrap();
    let result = just_tic::tally(&repo, now_utc(2026, 6, 5, 12, 0)).unwrap();

    // base 가 두 번 세지면 commits 4 / additions 8 이 된다. dedup 되면 3 / 6.
    assert_eq!(result.commits, 3);
    assert_eq!(result.additions, 6);
}

#[test]
fn detached_head_with_no_local_branches_falls_back_to_head() {
    let repo_dir = init_repo();
    let p = repo_dir.path();

    commit_file(p, "c.txt", "c1\nc2\n", "2026-06-05T09:00:00+00:00");

    // HEAD를 현재 커밋에서 분리하고 유일한 로컬 브랜치를 지운다 → refs/heads 비어 있음.
    git(p, &["checkout", "--detach"], None);
    git(p, &["branch", "-D", "main"], None);

    let repo = gix::open(p).unwrap();
    let result = just_tic::tally(&repo, now_utc(2026, 6, 5, 12, 0)).unwrap();

    // 로컬 브랜치가 없어도 HEAD 폴백으로 오늘 커밋을 집계해야 한다.
    assert_eq!(result.commits, 1);
    assert_eq!(result.additions, 2);
}

#[test]
fn excludes_commits_reachable_only_via_remote_tracking_refs() {
    let repo_dir = init_repo();
    let p = repo_dir.path();

    // main: 로컬 오늘 커밋(2줄).
    commit_file(p, "local.txt", "l1\nl2\n", "2026-06-05T09:00:00+00:00");

    // tmp 브랜치에 오늘 커밋(3줄)을 만든 뒤, 그 tip을 remote-tracking ref로 옮기고
    // tmp 로컬 브랜치를 지운다 → 이 커밋은 refs/remotes 로만 닿는다(fetch 흉내).
    git(p, &["checkout", "-b", "tmp"], None);
    commit_file(p, "remote_only.txt", "r1\nr2\nr3\n", "2026-06-05T10:00:00+00:00");
    git(p, &["update-ref", "refs/remotes/origin/feature", "tmp"], None);
    git(p, &["checkout", "main"], None);
    git(p, &["branch", "-D", "tmp"], None);

    let repo = gix::open(p).unwrap();
    let result = just_tic::tally(&repo, now_utc(2026, 6, 5, 12, 0)).unwrap();

    // remote-tracking 커밋은 제외 — main 의 로컬 커밋만 집계.
    assert_eq!(result.commits, 1);
    assert_eq!(result.additions, 2);
}

#[test]
fn merge_commit_does_not_recount_its_branch() {
    let repo_dir = init_repo();
    let p = repo_dir.path();

    // 어제 base(집계 제외)에서 두 브랜치가 갈라진다.
    commit_file(p, "base.txt", "base\n", "2026-06-04T09:00:00+00:00");
    git(p, &["branch", "feature"], None);

    commit_file(p, "m.txt", "m1\nm2\n", "2026-06-05T09:00:00+00:00"); // main: 2줄
    git(p, &["checkout", "feature"], None);
    commit_file(p, "f.txt", "f1\nf2\nf3\n", "2026-06-05T10:00:00+00:00"); // feature: 3줄

    // feature 를 main 에 머지 — --no-ff 로 머지 커밋을 강제 생성(오늘).
    git(p, &["checkout", "main"], None);
    git(
        p,
        &["merge", "--no-ff", "feature", "-m", "merge feature"],
        Some("2026-06-05T11:00:00+00:00"),
    );

    let repo = gix::open(p).unwrap();
    let result = just_tic::tally(&repo, now_utc(2026, 6, 5, 12, 0)).unwrap();

    // 머지 미skip이면 머지 커밋이 f.txt(3줄)를 재카운트 → commits 3 / additions 8.
    // 완전 제외하면 원본 m1·f1 만 → commits 2 / additions 5.
    assert_eq!(result.commits, 2);
    assert_eq!(result.additions, 5);
}

#[test]
fn merge_authored_today_does_not_leak_yesterdays_work() {
    let repo_dir = init_repo();
    let p = repo_dir.path();

    // 모든 원본 작업은 어제. 분기 → 양쪽 어제 커밋.
    commit_file(p, "base.txt", "base\n", "2026-06-03T09:00:00+00:00");
    git(p, &["branch", "feature"], None);
    commit_file(p, "m.txt", "m1\nm2\n", "2026-06-04T09:00:00+00:00"); // main, 어제
    git(p, &["checkout", "feature"], None);
    commit_file(p, "f.txt", "f1\nf2\nf3\n", "2026-06-04T10:00:00+00:00"); // feature, 어제

    // 머지 커밋만 오늘 author date.
    git(p, &["checkout", "main"], None);
    git(
        p,
        &["merge", "--no-ff", "feature", "-m", "merge feature"],
        Some("2026-06-05T11:00:00+00:00"),
    );

    let repo = gix::open(p).unwrap();
    let result = just_tic::tally(&repo, now_utc(2026, 6, 5, 12, 0)).unwrap();

    // 머지(오늘)는 skip, 원본(어제)은 날짜로 제외 → 오늘 합계는 0.
    // 미skip이면 머지가 f.txt(3줄)를 오늘로 끌어와 commits 1 / additions 3 이 된다.
    assert_eq!(result, just_tic::Tally::default());
}
