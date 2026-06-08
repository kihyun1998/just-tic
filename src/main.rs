//! `jtic` — 오늘(로컬 타임존 자정~지금) 추가/삭제한 줄 수를 출력하는 얇은 CLI 껍데기.
//!
//! 모든 집계 로직은 코어 lib(`just_tic`)에 있다. main은 시계/로컬 tz를 읽어
//! `tally(repo, now)`에 주입하고 결과를 출력할 뿐이다 (Tauri 재사용 대비).

use std::io::IsTerminal;
use std::process::ExitCode;

use anyhow::Context;
use clap::{CommandFactory, Parser, Subcommand};

/// 오늘 추가/삭제한 줄 수(+/-)를 git 히스토리에서 합산해 보여준다.
#[derive(Parser)]
#[command(name = "jtic", version, about)]
struct Cli {
    /// 기계 판독용 단일 JSON 객체로 출력한다 (상태바·jq 연동).
    #[arg(long)]
    json: bool,

    /// 집계 시작 시점. 날짜 `YYYY-MM-DD`(그 날 자정부터) 또는 기간 `7d`/`24h`/`2w`(지난 N).
    /// 생략하면 오늘(로컬 자정부터). 상한은 항상 현재 시각.
    #[arg(long, value_name = "SINCE")]
    since: Option<String>,

    /// 합산에서 제외할 경로 glob. 반복 지정 가능 (예: `--exclude Cargo.lock --exclude '**/*.min.js'`).
    /// glob은 레포 루트 기준 경로 전체에 매칭되며 `*`는 `/`를 넘지 않는다(중첩은 `**` 사용).
    #[arg(long, value_name = "GLOB")]
    exclude: Vec<String>,

    /// 머지 커밋을 제외하지 않고 first-parent diff로 집계한다(순회도 first-parent만 따름).
    /// 기본은 머지 제외(--no-merges).
    #[arg(long)]
    first_parent: bool,

    /// "오늘" 판정을 author date 대신 committer date로 한다. 기본은 author date.
    #[arg(long)]
    committer_date: bool,

    /// 보조 서브커맨드. 없으면 기본 동작(오늘 합산 출력).
    #[command(subcommand)]
    command: Option<Command>,
}

/// 집계와 무관한 보조 명령(셸 자동완성·man page 생성). git 레포가 필요 없다.
#[derive(Subcommand)]
enum Command {
    /// 셸 자동완성 스크립트를 stdout에 출력한다 (예: `jtic completions bash`).
    Completions {
        /// 대상 셸 (bash, zsh, fish, powershell, elvish).
        shell: clap_complete::Shell,
    },
    /// man page(roff)를 stdout에 출력한다 (예: `jtic man > jtic.1`).
    Man,
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

    // 보조 명령은 git 레포 없이도 동작 — 레포 탐색 전에 처리하고 종료한다.
    if let Some(command) = &cli.command {
        return emit(command);
    }

    // 현재 위치에서 상위로 올라가며 레포를 찾는다.
    let repo = gix::discover(".")
        .context("git 레포를 찾지 못했습니다 (현재 위치나 상위 경로에 .git이 없습니다)")?;

    // 시스템 로컬 타임존의 현재 시각 — 코어에 주입한다.
    let now = jiff::Zoned::now();
    // JSON의 `date`는 리포트 생성 시점(오늘) — Window 하한이 아니라 "as of" 기준.
    let today = now.date();

    // --since가 있으면 그 Window를, 없으면 오늘 Window를 코어에 넘긴다.
    let window = match cli.since.as_deref() {
        Some(spec) => parse_since(spec, &now)?,
        None => just_tic::Window::for_day(now),
    };
    // --exclude glob들을 매처로 컴파일해 코어에 주입 (ADR-0006: 매칭 정책은 CLI 몫).
    let exclude = build_exclude(&cli.exclude)?;
    let tally = just_tic::tally_in(
        &repo,
        &window,
        &just_tic::Options {
            exclude: &exclude,
            first_parent: cli.first_parent,
            committer_date: cli.committer_date,
        },
    )?;
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

/// 보조 명령(셸 자동완성·man page)을 생성해 stdout에 출력한다.
///
/// 생성물은 clap 정의(`Cli`)에서 파생되므로 플래그가 바뀌면 자동으로 동기화된다 — 수동
/// 작성이 아니다. 집계 로직과 무관해 git 레포 없이 동작한다([`Command`] 처리 시점).
fn emit(command: &Command) -> anyhow::Result<()> {
    let mut cmd = Cli::command();
    match command {
        Command::Completions { shell } => {
            clap_complete::generate(*shell, &mut cmd, "jtic", &mut std::io::stdout());
        }
        Command::Man => {
            clap_mangen::Man::new(cmd).render(&mut std::io::stdout())?;
        }
    }
    Ok(())
}

/// `--since` 인자를 [`just_tic::Window`]로 파싱한다.
///
/// 두 형식을 받는다(ADR-0006: 입력 형식은 CLI 관심사라 여기서 해석):
/// - `YYYY-MM-DD` → 그 날 로컬 자정부터 (달력 경계, [`Window::since_local_date`])
/// - `Nd`/`Nh`/`Nw` → 지금부터 N 전까지 (롤링, [`Window::since_ago`])
fn parse_since(spec: &str, now: &jiff::Zoned) -> anyhow::Result<just_tic::Window> {
    if let Some(secs) = parse_ago_seconds(spec) {
        return just_tic::Window::since_ago(jiff::SignedDuration::from_secs(secs), now);
    }
    let date: jiff::civil::Date = spec.parse().map_err(|_| {
        anyhow::anyhow!("--since 형식이 올바르지 않습니다: '{spec}' (YYYY-MM-DD 또는 7d/24h/2w)")
    })?;
    just_tic::Window::since_local_date(date, now)
}

/// `--exclude` glob들을 경로 제외 술어로 컴파일한다.
///
/// 빈 목록이면 아무것도 제외하지 않는다. glob 컴파일은 CLI 관심사라 여기서 처리하고
/// (ADR-0006), 코어엔 `Fn(&str) -> bool` 술어만 넘긴다. 잘못된 glob은 친절한 에러.
fn build_exclude(globs: &[String]) -> anyhow::Result<impl Fn(&str) -> bool> {
    let mut builder = globset::GlobSetBuilder::new();
    for g in globs {
        let glob = globset::Glob::new(g)
            .with_context(|| format!("--exclude: 잘못된 glob '{g}'"))?;
        builder.add(glob);
    }
    let set = builder.build().context("--exclude glob 컴파일 실패")?;
    Ok(move |path: &str| set.is_match(path))
}

/// `Nd`/`Nh`/`Nw` 기간 문자열을 초로 변환한다. 형식이 아니면 `None`(날짜로 재시도).
///
/// 단위: `h`=시간, `d`=일, `w`=주. 절대 기간이라 달력/DST와 무관하다.
fn parse_ago_seconds(spec: &str) -> Option<i64> {
    let (&unit, num) = spec.as_bytes().split_last()?;
    if num.is_empty() || !num.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let n: i64 = spec[..spec.len() - 1].parse().ok()?;
    let unit_secs: i64 = match unit {
        b'h' => 3_600,
        b'd' => 86_400,
        b'w' => 604_800,
        _ => return None,
    };
    n.checked_mul(unit_secs)
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
