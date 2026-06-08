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
    // output()은 stdout을 파이프로 캡처 → non-TTY → ANSI 색 코드가 없어야 한다.
    assert!(
        !stdout.contains('\x1b'),
        "파이프 출력엔 ANSI 색이 없어야 한다: {stdout:?}"
    );
}

#[test]
fn piped_human_output_has_no_ansi_even_with_no_color_unset() {
    // 파이프(non-TTY)면 NO_COLOR 설정과 무관하게 plain. 색은 TTY일 때만.
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-b", "main"]);
    git(p, &["config", "user.email", "t@example.com"]);
    git(p, &["config", "user.name", "Test"]);
    std::fs::write(p.join("x.txt"), "a\nb\nc\n").unwrap();
    git(p, &["add", "x.txt"]);
    git(p, &["commit", "-m", "today"]);

    // NO_COLOR를 명시적으로 제거해, plain의 원인이 NO_COLOR가 아니라 non-TTY임을 분명히 한다.
    let out = jtic().env_remove("NO_COLOR").current_dir(p).output().unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains('\x1b'), "파이프 출력은 plain: {stdout:?}");
    assert!(stdout.starts_with("+3 -0 · 1 commits"), "plain 포맷: {stdout}");
}

#[test]
fn json_output_never_contains_ansi() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-b", "main"]);
    git(p, &["config", "user.email", "t@example.com"]);
    git(p, &["config", "user.name", "Test"]);
    std::fs::write(p.join("x.txt"), "a\nb\n").unwrap();
    git(p, &["add", "x.txt"]);
    git(p, &["commit", "-m", "today"]);

    let out = jtic().arg("--json").current_dir(p).output().unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains('\x1b'), "--json엔 색이 절대 없어야 한다: {stdout:?}");
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

#[test]
fn completions_print_a_script_and_exit_zero_without_a_repo() {
    // 보조 명령은 git 레포가 없어도 동작해야 한다 — temp(레포 아님)에서 실행.
    let dir = tempfile::tempdir().unwrap();

    let out = jtic()
        .current_dir(dir.path())
        .args(["completions", "bash"])
        .output()
        .unwrap();

    assert!(out.status.success(), "completions는 레포 밖에서도 exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("_jtic"), "bash 자동완성 함수가 있어야 한다: {stdout}");
}

#[test]
fn man_prints_roff_and_exits_zero_without_a_repo() {
    let dir = tempfile::tempdir().unwrap();

    let out = jtic().current_dir(dir.path()).arg("man").output().unwrap();

    assert!(out.status.success(), "man은 레포 밖에서도 exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // roff man page는 .TH 헤더로 시작하고 바이너리 이름을 담는다.
    assert!(stdout.contains(".TH jtic"), "roff man page여야 한다: {stdout}");
}

#[test]
fn since_includes_older_commits_outside_today() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-b", "main"]);
    git(p, &["config", "user.email", "t@example.com"]);
    git(p, &["config", "user.name", "Test"]);

    // author date가 과거인 커밋(2줄) — "오늘"엔 안 잡힌다.
    std::fs::write(p.join("old.txt"), "a\nb\n").unwrap();
    git(p, &["add", "old.txt"]);
    git(p, &["commit", "-m", "old", "--date=2001-06-15T12:00:00"]);

    // 오늘 커밋(3줄).
    std::fs::write(p.join("new.txt"), "x\ny\nz\n").unwrap();
    git(p, &["add", "new.txt"]);
    git(p, &["commit", "-m", "today"]);

    // 기본: 오늘 것만 (1 commit).
    let default = jtic().current_dir(p).output().unwrap();
    let d = String::from_utf8_lossy(&default.stdout);
    assert!(d.contains("· 1 commits"), "기본은 오늘 1커밋이어야: {d}");

    // --since 2000-01-01: 과거 커밋도 포함 (2 commits).
    let since = jtic()
        .current_dir(p)
        .args(["--since", "2000-01-01"])
        .output()
        .unwrap();
    assert!(since.status.success());
    let s = String::from_utf8_lossy(&since.stdout);
    assert!(s.contains("· 2 commits"), "--since는 과거 커밋도 포함해야: {s}");
}

#[test]
fn invalid_since_errors_with_nonzero_exit() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-b", "main"]);
    git(p, &["config", "user.email", "t@example.com"]);
    git(p, &["config", "user.name", "Test"]);
    std::fs::write(p.join("x.txt"), "x\n").unwrap();
    git(p, &["add", "x.txt"]);
    git(p, &["commit", "-m", "c"]);

    let out = jtic()
        .current_dir(p)
        .args(["--since", "nope"])
        .output()
        .unwrap();

    assert!(!out.status.success(), "잘못된 --since는 비0 종료여야 한다");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--since"), "에러에 --since 언급 필요: {stderr}");
}
