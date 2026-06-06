//! `jtic` — 오늘(로컬 타임존 자정~지금) 추가/삭제한 줄 수를 출력하는 얇은 CLI 껍데기.
//!
//! 모든 집계 로직은 코어 lib(`just_tic`)에 있다. main은 시계/로컬 tz를 읽어
//! `tally(repo, now)`에 주입하고 결과를 출력할 뿐이다 (Tauri 재사용 대비).

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
        println!("{}", tally.to_json_line(today));
    } else {
        println!("{}", tally.to_human_line());
    }
    Ok(())
}
