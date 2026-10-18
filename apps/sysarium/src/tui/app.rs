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
            mode: ViewMode::Split,
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
            title: format!("指標の詳細 · LIVE · #{}", self.analysis.source_sample_id),
            text: crate::output::details(&self.analysis),
            scroll: 0,
        }
    }

    fn help() -> Overlay {
        let text = format!(
            "Sysarium {}\n\nTab: 表示切替（Living → Metrics → Split）\n1・2・3: 各表示を直接選択\ni: ライブ詳細   ?: ヘルプ   Esc: 閉じる\n↑↓ / PgUp / PgDn / Home: 詳細をスクロール\nq / Ctrl-C: 終了\n\nCPU: CPU活動時間の割合（本プロジェクトの近似値）．\nキャラクターの手の動きは活動量に応じて変化します．\n表示用の平滑化を行い，数値自体は変更しません．\n\nCPU pressure: PSI someの区間割合．CPUを待つタスクが少なくとも1つある時間の割合です．\n積まれた書類は競合の強さの比喩で，待機タスク数ではありません．\n段階の境界: 0.5 / 2 / 5 / 10 / 25%（暫定）．\navg10: カーネルの平滑化された傾向．区間値とは異なります．\nRunnable: 実行中も含む参考値．待機数ではありません．\n\nN/A・?: 不明．初回は差分がなく，約1秒後から表示します．\n履歴: 左が過去，右が最新．最大120観測，縦軸0〜100%．\n▁は計測できたゼロ，?は欠測です．\n\n観測は約1Hz，描画は最大10FPS．詳細もライブ更新します．\n通常画面は80列×24行以上．小さい端末ではサイズ案内を表示します．\niで取得元・単位・計算方法・欠測理由を確認できます．\nsystem-view: 可視の/procの範囲で，物理ホスト全体とは限りません．",
            env!("CARGO_PKG_VERSION")
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
                KeyCode::Char('1' | '2' | '3') if self.overlay.is_none() => {
                    self.mode = match key.code {
                        KeyCode::Char('1') => ViewMode::Living,
                        KeyCode::Char('2') => ViewMode::Metrics,
                        _ => ViewMode::Split,
                    };
                }
                _ => {}
            }
        }
        false
    }
}
