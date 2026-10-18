use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::{self, IsTerminal};

pub type LiveTerminal = Terminal<CrosstermBackend<io::Stdout>>;

struct Restore {
    active: bool,
}

impl Restore {
    fn finish(&mut self) -> io::Result<()> {
        // raw modeの解除に失敗しても，画面とカーソルの復旧は試みる．
        let raw = disable_raw_mode();
        let screen = execute!(io::stdout(), LeaveAlternateScreen, Show);
        let result = raw.and(screen);
        self.active = result.is_err();
        result
    }
}

impl Drop for Restore {
    fn drop(&mut self) {
        if self.active {
            let _ = self.finish();
        }
    }
}

/// 初期化途中の失敗，通常終了，エラー，panicのunwindで復旧する．
pub fn with_terminal(run: impl FnOnce(&mut LiveTerminal) -> io::Result<()>) -> io::Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(io::Error::other(
            "sysarium live requires a terminal on stdin and stdout",
        ));
    }
    let mut restore = Restore { active: true };
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, Hide)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let result = run(&mut terminal);
    let restored = restore.finish();
    match (result, restored) {
        (Err(error), Err(cleanup)) => Err(io::Error::other(format!(
            "{error}; terminal restore failed: {cleanup}"
        ))),
        (Err(error), _) => Err(error),
        (_, result) => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 実PTYが必要．tests/verify_terminal.pyから個別に実行する．
    #[test]
    #[ignore = "requires a PTY; run python3 apps/sysarium/tests/verify_terminal.py"]
    fn restores_after_error() {
        let error = with_terminal(|_| {
            assert!(crossterm::terminal::is_raw_mode_enabled()?);
            Err(io::Error::other("injected loop error"))
        })
        .unwrap_err();
        assert!(error.to_string().contains("injected loop error"));
        assert!(!crossterm::terminal::is_raw_mode_enabled().unwrap());
    }

    #[test]
    #[ignore = "requires a PTY; run python3 apps/sysarium/tests/verify_terminal.py"]
    fn restores_after_panic() {
        let result = std::panic::catch_unwind(|| {
            let _ = with_terminal(|_| panic!("injected panic"));
        });
        assert!(result.is_err());
        assert!(!crossterm::terminal::is_raw_mode_enabled().unwrap());
    }
}
