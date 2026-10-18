mod output;
mod tui;

use inside_core::{
    analysis::analyze,
    observation::{Observation, SYSTEM_VIEW},
};
use std::{
    env,
    io::{self, Write},
    process::ExitCode,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Status,
    Json,
    Inspect,
    Live,
    Help,
    StatusHelp,
    InspectHelp,
    LiveHelp,
    Version,
}

fn command(args: &[String]) -> Result<Command, &'static str> {
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["status"] => Ok(Command::Status),
        ["status", "--json"] => Ok(Command::Json),
        ["inspect", "cpu"] => Ok(Command::Inspect),
        ["live"] => Ok(Command::Live),
        ["--help"] | ["-h"] => Ok(Command::Help),
        ["status", "--help" | "-h"] => Ok(Command::StatusHelp),
        ["inspect", "--help" | "-h"] | ["inspect", "cpu", "--help" | "-h"] => {
            Ok(Command::InspectHelp)
        }
        ["live", "--help" | "-h"] => Ok(Command::LiveHelp),
        ["--version"] | ["-V"] => Ok(Command::Version),
        _ => {
            Err("usage: inside status [--json] | inside inspect cpu | inside live | inside --help")
        }
    }
}

fn sample(sample_id: u64, previous: Option<Instant>) -> io::Result<(Observation, Instant)> {
    // 各読込開始時の同じ基準点で壁時計と単調時計を記録する．
    let monotonic = Instant::now();
    let wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?;
    let sampled_at_unix_ms = u64::try_from(wall.as_millis()).map_err(io::Error::other)?;
    Ok((
        Observation {
            sample_id,
            sampled_at_unix_ms,
            elapsed_since_previous: previous
                .and_then(|before| monotonic.checked_duration_since(before)),
            scope: SYSTEM_VIEW,
            readings: inside_linux::collector::collect(),
        },
        monotonic,
    ))
}

fn run(command: Command, out: &mut impl Write) -> io::Result<()> {
    run_with(command, out, sample, || {
        thread::sleep(Duration::from_secs(1))
    })
}

// 時計・取得・待機の境界を注入し，同じCLI経路をfixtureで検証する．
fn run_with(
    command: Command,
    out: &mut impl Write,
    mut sample: impl FnMut(u64, Option<Instant>) -> io::Result<(Observation, Instant)>,
    pause: impl FnOnce(),
) -> io::Result<()> {
    let help = match command {
        Command::Help => Some(
            "usage: inside status [--json] | inside inspect cpu | inside live\ninside --version | <command> --help\nstatus/inspect: 約1秒間隔の2観測．live: 1Hz観測・最大10FPS描画．\n対象: Linuxのsystem-view．PSI未対応時は理由付きN/A．",
        ),
        Command::StatusHelp => Some(
            "usage: inside status [--json]\n約1秒間隔の2観測でCPU活動量・CPU PSI some・runnable参考値を表示．\n--json: stdoutはバージョン・単位・欠測理由付きのJSONのみ．",
        ),
        Command::InspectHelp => Some(
            "usage: inside inspect cpu\n約1秒間隔の2観測で，元の値・取得元・単位・差分・計算規則・欠測理由を表示．",
        ),
        Command::LiveHelp => Some(
            "usage: inside live\nTab: Living/Metrics/Split切替，i: 同じ観測の詳細，?: ヘルプ．\n詳細・ヘルプ: ↑↓/PgUp/PgDn/Homeでスクロール，Escで閉じる．\nq/Ctrl-C: 終了．stdin/stdoutが端末である必要があります．",
        ),
        Command::Version => return writeln!(out, "inside {}", env!("CARGO_PKG_VERSION")),
        _ => None,
    };
    if let Some(help) = help {
        return writeln!(out, "{help}");
    }
    let (previous, stamp) = sample(1, None)?;
    pause();
    let (current, _) = sample(2, Some(stamp))?;
    let analysis = analyze(Some(&previous), &current);
    if command == Command::Json {
        // 先に完全なJSON文字列を構築し，診断をstdoutへ混在させない．
        let text = output::json(&analysis)?;
        writeln!(out, "{text}")
    } else {
        write!(
            out,
            "{}",
            output::text(&analysis, command == Command::Inspect)
        )
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = match env::args_os()
        .skip(1)
        .map(|value| value.into_string())
        .collect()
    {
        Ok(args) => args,
        Err(_) => {
            eprintln!("inside: arguments must be valid UTF-8");
            return ExitCode::from(2);
        }
    };
    let command = match command(&args) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let result = if command == Command::Live {
        tui::run()
    } else {
        run(command, &mut io::stdout().lock())
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("inside: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_p1_and_p2_commands() {
        for (args, expected) in [
            (vec!["status"], Command::Status),
            (vec!["status", "--json"], Command::Json),
            (vec!["inspect", "cpu"], Command::Inspect),
            (vec!["live"], Command::Live),
            (vec!["--help"], Command::Help),
        ] {
            assert_eq!(
                command(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()),
                Ok(expected)
            );
        }
        for args in [
            vec![],
            vec!["live", "--json"],
            vec!["status", "--unknown"],
            vec!["inspect", "cpu", "--json"],
        ] {
            assert!(command(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
    }
}

#[cfg(test)]
mod output_tests;
