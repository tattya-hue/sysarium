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
    Living,
    Metrics,
    Split,
}

impl ViewMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Living => "Living",
            Self::Metrics => "Metrics",
            Self::Split => "Split",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Living => Self::Metrics,
            Self::Metrics => Self::Split,
            Self::Split => Self::Living,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetricPoint {
    pub sample_id: u64,
    pub activity: Option<f64>,
    pub pressure: Option<f64>,
}

/// 詳細だけは開いた時点の表示文字列を保持する．計測・履歴更新は継続する．
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
            mode: ViewMode::Living,
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
        let scene = &self.scene;
        let text = format!(
            "この詳細は開いた時点の観測．背後の計測・履歴は継続します．\n\n{}\n表示用Scene（実測値とは区別）\nsource_sample_id={}\nactivity_intensity={:?}\npressure_level={:?}\nrepresents=cpu_contention_metaphor\n活動EMA alpha={}\n山の境界（割合）={:?}\n山は競合の比喩であり待機タスク数ではない．",
            crate::output::text(&self.analysis, true),
            scene.source_sample_id,
            scene.character.activity_intensity,
            scene.task_stack.pressure_level,
            inside_core::scene::ACTIVITY_SMOOTHING_ALPHA,
            inside_core::scene::PRESSURE_THRESHOLDS
        );
        Overlay {
            kind: OverlayKind::Details,
            title: format!(
                "Details | sample_id={} | 表示固定",
                self.analysis.source_sample_id
            ),
            text,
            scroll: 0,
        }
    }

    fn help() -> Overlay {
        let text = format!(
            "Inside live {}\n\nTab: Living → Metrics → Split\ni: 今の観測の詳細を開く・閉じる\n?: このヘルプを開く・閉じる\nEsc: 詳細・ヘルプを閉じる\n↑↓ / PgUp / PgDn: スクロール\nHome: 先頭へ戻る\nq / Ctrl-C: 終了（詳細表示中も有効）\n\nCPU活動量: 本プロジェクトで定義した活動時間の割合（近似）．\nPCの仕事: CPU活動量を表示用に平滑化した強度．元の数値は変更しない．\nPSI some区間: 単調時計の区間に占める停滞時間の割合．\navg10: 平滑化された傾向で，区間の値とは別．\n仕事の山: 競合の比喩的な0〜5段階．待機タスクの個数ではない．\n山の境界（割合）: {:?}\nprocs_running: 実行中を含むrunnable参考値．\n\nN/A・?: 不明．初回は差分なし．履歴の▁は観測できたゼロ．\n理由が省略される場合はiで同じ観測の根拠を確認できる．\n詳細は開いた時点のsample_idに固定し，閉じると最新表示に戻る．\n\n観測は約1Hz，描画は最大10FPS，履歴は最大120観測．\nsystem-viewは可視の/procの範囲で，物理ホスト全体とは限らない．\n40×24以上では主要情報を表示．小さい画面では簡略表示．",
            env!("CARGO_PKG_VERSION"),
            inside_core::scene::PRESSURE_THRESHOLDS
        );
        Overlay {
            kind: OverlayKind::Help,
            title: "Help | 操作・指標".into(),
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
                _ => {}
            }
        }
        false
    }
}
