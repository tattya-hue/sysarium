use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use inside_core::{
    analysis::{Analysis, analyze},
    observation::Observation,
    scene::Scene,
};
use std::collections::VecDeque;

use ratatui::text::Span;

pub const HISTORY_LIMIT: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Scene,
    Charts,
    Both,
}

impl ViewMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Scene => "Scene",
            Self::Charts => "Charts",
            Self::Both => "Both",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Scene => Self::Charts,
            Self::Charts => Self::Both,
            Self::Both => Self::Scene,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetricPoint {
    pub sample_id: u64,
    pub activity: Option<f64>,
    pub pressure: Option<f64>,
}

/// 詳細も最新のAnalysisに追随する．
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    Help,
    Details,
}

pub struct Overlay {
    pub kind: OverlayKind,
    pub title: String,
    pub text: String,
    pub scroll: u16,
}

impl Overlay {
    pub fn max_scroll(&self, width: u16, height: u16) -> u16 {
        let lines = self.wrapped_lines(width.saturating_sub(2)).len();
        u16::try_from(lines.saturating_sub(usize::from(height.saturating_sub(2))))
            .unwrap_or(u16::MAX)
    }

    /// 全文を表示セル幅で折り返す．制御文字は実行せず空白に置換する．
    pub fn wrapped_lines(&self, width: u16) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }
        let mut lines = Vec::new();
        let mut line = String::new();
        let mut used = 0;
        for c in self.text.chars() {
            if c == '\n' {
                lines.push(std::mem::take(&mut line));
                used = 0;
                continue;
            }
            let c = if c.is_control() { ' ' } else { c };
            let mut glyph = c.to_string();
            let mut size = Span::raw(&glyph).width();
            if size > usize::from(width) {
                glyph = "?".into();
                size = 1;
            }
            if used + size > usize::from(width) {
                lines.push(std::mem::take(&mut line));
                used = 0;
            }
            line.push_str(&glyph);
            used += size;
        }
        lines.push(line);
        lines
    }
}

/// 1つのAnalysisからScene・Metrics・履歴を一括更新する．
pub struct App {
    pub mode: ViewMode,
    pub analysis: Analysis,
    pub scene: Scene,
    pub history: VecDeque<MetricPoint>,
    pub overlay: Option<Overlay>,
    viewport: (u16, u16),
}

impl App {
    pub fn new(observation: Observation) -> Self {
        let analysis = analyze(None, &observation);
        let scene = Scene::from_analysis(&analysis, None);
        let mut app = Self {
            mode: ViewMode::Both,
            analysis,
            scene,
            history: VecDeque::new(),
            overlay: None,
            viewport: (80, 24),
        };
        app.push_history();
        app
    }

    pub fn observe(&mut self, observation: Observation) {
        self.analysis = analyze(Some(&self.analysis.evidence.current), &observation);
        self.scene = Scene::from_analysis(&self.analysis, Some(&self.scene));
        self.push_history();
        if let Some(old) = &self.overlay
            && old.kind == OverlayKind::Details
        {
            let scroll = old.scroll;
            let mut details = self.details();
            details.scroll = scroll.min(details.max_scroll(self.viewport.0, self.viewport.1));
            self.overlay = Some(details);
        }
    }

    fn push_history(&mut self) {
        if self.history.len() == HISTORY_LIMIT {
            self.history.pop_front();
        }
        self.history.push_back(MetricPoint {
            sample_id: self.analysis.source_sample_id,
            activity: self.analysis.cpu_active_ratio.as_ref().ok().copied(),
            pressure: self
                .analysis
                .cpu_psi_some_interval_ratio
                .as_ref()
                .ok()
                .copied(),
        });
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.viewport = (width, height);
        if let Some(overlay) = &mut self.overlay {
            overlay.scroll = overlay.scroll.min(overlay.max_scroll(width, height));
        }
    }

    fn details(&self) -> Overlay {
        Overlay {
            kind: OverlayKind::Details,
            title: format!("Details · LIVE · #{}", self.analysis.source_sample_id),
            text: crate::output::details(&self.analysis),
            scroll: 0,
        }
    }

    fn help() -> Overlay {
        let text = format!(
            "Sysarium {}\n\nTab        Switch view: Scene / Charts / Both\n1 / 2 / 3  Select view\ni          Details\n?          Help\nEsc        Back\n↑↓ / PgUp / PgDn / Home  Scroll\nq / Ctrl-C Quit\n\nCPU usage  Working: CPU time doing work (approx.).\nCPU wait   Time when tasks had to wait for CPU.\nWait time  Milliseconds with any task waiting; not per-task totals.\nUsage and wait can both be high.\nPSI        Linux measures how long tasks wait for CPU.\n10s trend  Recent CPU wait, smoothed over ~10s.\nRunnable   Running + ready to run; not a waiting count.\n\nScreen color and sweat reflect CPU usage.\nPapers reflect CPU wait; not a task count.\nN/A or ? = unknown. First reading needs ~1s.\nCharts: past → now, 0–100%, up to 120 readings.\n▁ = zero / near zero; ? = missing.\n\n80 x 24 minimum. Updates ~1/s; animation up to 10 FPS.\nRaw data and formulas: sysarium --json",
            env!("CARGO_PKG_VERSION")
        );
        Overlay {
            kind: OverlayKind::Help,
            title: "Help".into(),
            text,
            scroll: 0,
        }
    }

    /// trueは終了．Releaseで二重操作せず，Resizeは次の描画で実サイズを使う．
    pub fn handle_event(&mut self, event: Event) -> bool {
        if let Event::Resize(width, height) = event {
            self.resize(width, height);
        }
        if let Event::Key(key) = event
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('q') => return true,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return true;
                }
                KeyCode::Esc => self.overlay = None,
                KeyCode::Char('i') => {
                    self.overlay = if self
                        .overlay
                        .as_ref()
                        .is_some_and(|o| o.kind == OverlayKind::Details)
                    {
                        None
                    } else {
                        Some(self.details())
                    };
                }
                KeyCode::Char('?') => {
                    self.overlay = if self
                        .overlay
                        .as_ref()
                        .is_some_and(|o| o.kind == OverlayKind::Help)
                    {
                        None
                    } else {
                        Some(Self::help())
                    };
                }
                KeyCode::Up
                | KeyCode::Down
                | KeyCode::PageUp
                | KeyCode::PageDown
                | KeyCode::Home => {
                    if let Some(overlay) = &mut self.overlay {
                        let maximum = overlay.max_scroll(self.viewport.0, self.viewport.1);
                        overlay.scroll = match key.code {
                            KeyCode::Up => overlay.scroll.saturating_sub(1),
                            KeyCode::Down => overlay.scroll.saturating_add(1).min(maximum),
                            KeyCode::PageUp => overlay
                                .scroll
                                .saturating_sub(self.viewport.1.saturating_sub(3).max(1)),
                            KeyCode::PageDown => overlay
                                .scroll
                                .saturating_add(self.viewport.1.saturating_sub(3).max(1))
                                .min(maximum),
                            _ => 0,
                        };
                    }
                }
                KeyCode::Tab if self.overlay.is_none() => self.mode = self.mode.next(),
                KeyCode::Char('1' | '2' | '3') if self.overlay.is_none() => {
                    self.mode = match key.code {
                        KeyCode::Char('1') => ViewMode::Scene,
                        KeyCode::Char('2') => ViewMode::Charts,
                        _ => ViewMode::Both,
                    };
                }
                _ => {}
            }
        }
        false
    }
}
