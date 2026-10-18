mod output;
mod shutdown;
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
    Live,
    Json,
    Help,
    Version,
}

fn command(args: &[String]) -> Result<Command, &'static str> {
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | ["live"] => Ok(Command::Live),
        ["--json"] | ["live", "--json"] => Ok(Command::Json),
        ["--help" | "-h"] | ["live", "--help" | "-h"] => Ok(Command::Help),
        ["--version" | "-V"] => Ok(Command::Version),
        _ => Err("usage: inside [live] [--json] | inside --help | inside --version"),
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
    match command {
        Command::Version => writeln!(out, "inside {}", env!("CARGO_PKG_VERSION")),
        Command::Help => writeln!(
            out,
            "usage: inside [live] [--json]\n\n引数なしでライブ監視を開始します．\nTab / 1・2・3: 表示切替  i: ライブ詳細  ?: ヘルプ  q: 終了\n--json: 約1秒ごとのNDJSONストリーム（Ctrl-Cで終了）．\n対象: Linuxのsystem-view．TUIはstdin/stdoutに端末が必要です．"
        ),
        Command::Live => tui::run(),
        Command::Json => {
            let shutdown = shutdown::Shutdown::install()?;
            stream(
                out,
                sample,
                || thread::sleep(Duration::from_millis(100)),
                || shutdown.requested(),
            )
        }
    }
}

// TUIと同じ観測・解析経路．初回も欠測理由付きで出力する．
fn stream(
    out: &mut impl Write,
    mut collect: impl FnMut(u64, Option<Instant>) -> io::Result<(Observation, Instant)>,
    mut pause: impl FnMut(),
    mut stopped: impl FnMut() -> bool,
) -> io::Result<()> {
    let mut previous = None;
    let mut stamp = None;
    let mut id = 1_u64;
    while !stopped() {
        if stamp.is_some_and(|t: Instant| t.elapsed() < Duration::from_secs(1)) {
            pause();
            continue;
        }
        let (current, now) = collect(id, stamp)?;
        writeln!(
            out,
            "{}",
            output::json(&analyze(previous.as_ref(), &current))?
        )?;
        out.flush()?;
        previous = Some(current);
        stamp = Some(now);
        id = id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("sample_id overflow"))?;
    }
    Ok(())
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
    let result = run(command, &mut io::stdout().lock());
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("inside: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod output_tests;
