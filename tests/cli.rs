//! `jtic` 바이너리 end-to-end 테스트 — 빌드된 실행 파일을 실제로 구동한다.

use std::path::Path;
use std::process::Command;

/// 빌드된 jtic 바이너리 경로 (cargo가 주입).
fn jtic() -> Command {
    Command::new(env!("CARGO_BIN_EXE_jtic"))
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git 실행 실패");
    assert!(
        out.status.success(),
        "git {:?} 실패:\n{}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn errors_with_nonzero_exit_outside_a_repo() {
    let dir = tempfile::tempdir().unwrap();

    let out = jtic().current_dir(dir.path()).output().unwrap();

    assert!(!out.status.success(), "레포 밖에서는 비0 종료여야 한다");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("jtic:"), "친절한 에러 메시지가 필요하다: {stderr}");
}

#[test]
fn prints_count_and_exits_zero_for_a_commit_today() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-b", "main"]);
    git(p, &["config", "user.email", "t@example.com"]);
    git(p, &["config", "user.name", "Test"]);

    // author date를 주지 않으면 현재 시각(=오늘)으로 커밋된다.
    std::fs::write(p.join("x.txt"), "x\ny\n").unwrap();
    git(p, &["add", "x.txt"]);
    git(p, &["commit", "-m", "today"]);

    let out = jtic().current_dir(p).output().unwrap();

    assert!(out.status.success(), "성공 경로는 종료코드 0이어야 한다");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.starts_with('+'), "출력은 +로 시작해야 한다: {stdout}");
    assert!(stdout.contains("· 1 commits"), "오늘 커밋 1개 표시: {stdout}");
}

#[test]
fn discovers_repo_from_a_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-b", "main"]);
    git(p, &["config", "user.email", "t@example.com"]);
    git(p, &["config", "user.name", "Test"]);
    std::fs::write(p.join("x.txt"), "x\n").unwrap();
    git(p, &["add", "x.txt"]);
    git(p, &["commit", "-m", "today"]);

    // 레포 루트가 아닌 하위 디렉터리에서 실행해도 .git을 상위로 찾아야 한다.
    let sub = p.join("deep/nested");
    std::fs::create_dir_all(&sub).unwrap();
    let out = jtic().current_dir(&sub).output().unwrap();

    assert!(out.status.success(), "하위 디렉터리에서도 동작해야 한다");
    assert!(String::from_utf8_lossy(&out.stdout).starts_with('+'));
}

#[test]
fn json_flag_prints_a_single_object_with_expected_keys() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-b", "main"]);
    git(p, &["config", "user.email", "t@example.com"]);
    git(p, &["config", "user.name", "Test"]);

    // author date 미지정 → 오늘 커밋. 2줄 추가.
    std::fs::write(p.join("x.txt"), "a\nb\n").unwrap();
    git(p, &["add", "x.txt"]);
    git(p, &["commit", "-m", "today"]);

    let out = jtic().arg("--json").current_dir(p).output().unwrap();

    assert!(out.status.success(), "--json도 성공 경로는 종료코드 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("유효한 JSON이어야 한다");

    assert!(v.is_object(), "단일 객체여야 한다: {stdout}");
    assert!(
        v.get("date").and_then(|d| d.as_str()).is_some(),
        "date 키 필요: {stdout}"
    );
    assert_eq!(v["additions"], 2);
    assert_eq!(v["deletions"], 0);
    assert_eq!(v["commits"], 1);
}

#[test]
fn json_flag_on_empty_repo_is_a_valid_zero_object() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-b", "main"]); // 커밋 없음

    let out = jtic().arg("--json").current_dir(p).output().unwrap();

    assert!(out.status.success(), "오늘 작업 없음은 에러가 아니다 (exit 0)");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("0커밋도 유효한 JSON");

    assert_eq!(v["additions"], 0);
    assert_eq!(v["deletions"], 0);
    assert_eq!(v["commits"], 0);
}

#[test]
fn empty_repo_prints_no_commits_message_and_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-b", "main"]); // 커밋 없음

    let out = jtic().current_dir(p).output().unwrap();

    assert!(out.status.success(), "오늘 작업 없음은 에러가 아니다 (exit 0)");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.trim_end(), "+0 -0 · no commits today");
}
