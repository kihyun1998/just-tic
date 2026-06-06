//! `jtic` — 오늘(로컬 타임존 자정~지금) 추가/삭제한 줄 수를 출력하는 얇은 CLI 껍데기.
//!
//! 모든 집계 로직은 코어 lib(`just_tic`)에 있다. main은 시계/로컬 tz를 읽어
//! `tally(repo, now)`에 주입하고 결과를 출력할 뿐이다 (Tauri 재사용 대비).

use std::io::IsTerminal;
use std::process::ExitCode;

use anyhow::Context;
use clap::Parser;

/// 오늘 추가/삭제한 줄 수(+/-)를 git 히스토리에서 합산해 보여준다.
#[derive(Parser)]
#[command(name = "jtic", version, about)]
struct Cli {
    /// 기계 판독용 단일 JSON 객체로 출력한다 (상태바·jq 연동).
    #[arg(long)]
    json: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("jtic: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // 현재 위치에서 상위로 올라가며 레포를 찾는다.
    let repo = gix::discover(".")
        .context("git 레포를 찾지 못했습니다 (현재 위치나 상위 경로에 .git이 없습니다)")?;

    // 시스템 로컬 타임존의 현재 시각 — 코어에 주입한다.
    let now = jiff::Zoned::now();
    // JSON의 `date`는 로컬 "오늘" 날짜 — Window 하한과 같은 기준.
    let today = now.date();

    let tally = just_tic::tally(&repo, now)?;
    if cli.json {
        // JSON은 항상 색 없음 — 기계 판독·jq 안전.
        println!("{}", tally.to_json_line(today));
    } else if should_colorize(std::io::stdout().is_terminal(), no_color_set()) {
        println!("{}", tally.to_human_line_colored());
    } else {
        println!("{}", tally.to_human_line());
    }
    Ok(())
}

/// 휴먼 출력에 ANSI 색을 입힐지 결정한다(순수 함수 — 테스트 가능).
///
/// stdout이 TTY이고 `NO_COLOR`가 설정돼 있지 않을 때만 색을 켠다. 파이프/리다이렉트
/// (non-TTY)거나 `NO_COLOR`이 설정돼 있으면 plain. (`--json`은 이 분기 전에 처리.)
fn should_colorize(stdout_is_terminal: bool, no_color_set: bool) -> bool {
    stdout_is_terminal && !no_color_set
}

/// `NO_COLOR` 환경변수가 설정돼 있는가. https://no-color.org 관례: 값과 무관하게 존재만으로 색 끔.
fn no_color_set() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

#[cfg(test)]
mod tests {
    use super::should_colorize;

    #[test]
    fn colorize_only_on_tty_without_no_color() {
        assert!(should_colorize(true, false), "TTY + NO_COLOR 없음 → 색");
        assert!(!should_colorize(false, false), "파이프(non-TTY) → 색 끔");
        assert!(!should_colorize(true, true), "NO_COLOR 설정 → TTY여도 색 끔");
        assert!(!should_colorize(false, true), "파이프 + NO_COLOR → 색 끔");
    }
}
