use super::app::{App, ViewMode};
use inside_core::{observation::UnavailableReason as Reason, scene::ActivityLevel};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use std::time::Duration;

const BG: Color = Color::Rgb(19, 25, 38);
const CARD: Color = Color::Rgb(26, 34, 49);
const INK: Color = Color::Rgb(226, 232, 237);
const MUTED: Color = Color::Rgb(140, 156, 177);
const MINT: Color = Color::Rgb(126, 220, 185);
const AMBER: Color = Color::Rgb(239, 190, 113);
const FACE: Color = Color::Rgb(247, 229, 201);
const CHEEK: Color = Color::Rgb(226, 145, 139);
const SWEAT: Color = Color::Rgb(139, 211, 255);
const SWEAT_FADED: Color = Color::Rgb(79, 115, 141);
const LINE: Color = Color::Rgb(56, 72, 89);

pub fn draw(frame: &mut Frame, app: &App, elapsed: Duration) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(BG).fg(INK)),
        area,
    );
    if area.is_empty() {
        return;
    }
    if let Some(overlay) = &app.overlay {
        let scroll = overlay
            .scroll
            .min(overlay.max_scroll(area.width, area.height));
        let border = Block::bordered()
            .title(format!(" {} ", overlay.title))
            .title_bottom(" ↑↓ scroll · Esc back · q quit ")
            .border_style(Style::default().fg(LINE));
        let inner = border.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(border.style(Style::default().bg(BG).fg(INK)), area);
        let lines: Vec<Line<'static>> = overlay
            .wrapped_lines(inner.width)
            .into_iter()
            .map(|text| {
                let color = if text.starts_with("CPU usage ") {
                    MINT
                } else if text.starts_with("CPU wait ") {
                    AMBER
                } else {
                    INK
                };
                let heading = text.starts_with("CPU usage ") || text.starts_with("CPU wait ");
                Line::styled(
                    text,
                    Style::default().fg(color).add_modifier(if heading {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
                )
            })
            .collect();
        frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
        return;
    }
    if area.width < 80 || area.height < 24 {
        small_terminal(frame, area, app);
        return;
    }
    let outer = area.inner(Margin::new(
        if area.width >= 60 { 2 } else { 1 },
        u16::from(area.height >= 16),
    ));
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(outer);
    let header = Layout::horizontal([
        Constraint::Min(12),
        Constraint::Length(if outer.width >= 64 { 35 } else { 0 }),
    ])
    .split(rows[0]);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "sysarium",
                Style::default().fg(INK).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ● LIVE", Style::default().fg(MINT)),
        ])),
        header[0],
    );
    let mut tabs: Vec<Span> = [ViewMode::Animation, ViewMode::Charts, ViewMode::Both]
        .into_iter()
        .flat_map(|mode| {
            [
                Span::styled(
                    format!(" {} ", mode.label()),
                    Style::default()
                        .fg(if app.mode == mode { MINT } else { MUTED })
                        .add_modifier(if app.mode == mode {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::raw(" "),
            ]
        })
        .collect();
    tabs.insert(0, Span::styled("View: ", Style::default().fg(MUTED)));
    frame.render_widget(
        Paragraph::new(Line::from(tabs)).alignment(Alignment::Right),
        header[1],
    );
    match app.mode {
        ViewMode::Animation => {
            let pieces =
                Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(rows[1]);
            living(frame, pieces[0], app, elapsed);
            let badges = format!(
                "CPU usage {}     CPU wait {}",
                value(&app.analysis.cpu_active_ratio, 100.0),
                value(&app.analysis.cpu_psi_some_interval_ratio, 100.0)
            );
            frame.render_widget(
                Paragraph::new(badges)
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(MUTED)),
                pieces[1],
            );
        }
        ViewMode::Charts => metrics(frame, rows[1], app),
        ViewMode::Both => {
            let (left, right) = split_areas(rows[1]);
            living(frame, left, app, elapsed);
            metrics(frame, right, app);
        }
    }
    let footer = "Tab view  ·  i details  ·  ? help  ·  q quit";
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(MUTED)),
        rows[2],
    );
}

pub(super) fn split_areas(area: Rect) -> (Rect, Rect) {
    let cols = Layout::horizontal([
        Constraint::Percentage(58),
        Constraint::Length(3),
        Constraint::Min(0),
    ])
    .split(area);
    (cols[0], cols[2])
}

fn put(frame: &mut Frame, area: Rect, x: u16, y: u16, text: &str, color: Color) {
    if x < area.width && y < area.height {
        frame.render_widget(
            Paragraph::new(text).style(Style::default().fg(color)),
            Rect::new(
                area.x + x,
                area.y + y,
                (Span::raw(text).width() as u16).min(area.width - x),
                1,
            ),
        );
    }
}

/// 形・座標・色・動作周期はRendererが持ち，Sceneの表示強度を見た目へ写す．
fn living(frame: &mut Frame, area: Rect, app: &App, elapsed: Duration) {
    if area.is_empty() {
        return;
    }
    // 完全な姿を置けない領域には描かず，欠けた描画や縮小版を作らない．
    if area.width < 39 || area.height < 15 {
        return;
    }
    let stage = Rect::new(
        area.x + (area.width - 39) / 2,
        area.y + (area.height - 15) / 2,
        39,
        15,
    );
    let pc = Rect::new(stage.x, stage.y + 3, 24, 11);
    let level = app.scene.character.activity_level();
    let busy = matches!(level, Ok(ActivityLevel::Busy | ActivityLevel::Strained));
    let mouth = match level {
        Err(Reason::FirstSample) => "──",
        Ok(ActivityLevel::Relaxed) | Err(_) => "╰╯",
        Ok(ActivityLevel::Busy) => "╭╮",
        Ok(ActivityLevel::Strained) => "~~",
    };
    let eyes = match level {
        Err(Reason::FirstSample) => "·    ·",
        Err(_) => "?    ?",
        Ok(ActivityLevel::Strained) => ">    <",
        _ => "●    ●",
    };
    let (case, screen, rim) = match level {
        Ok(ActivityLevel::Relaxed) => (Color::Rgb(32, 65, 63), Color::Rgb(23, 43, 46), MINT),
        Ok(ActivityLevel::Busy) => (Color::Rgb(58, 55, 33), Color::Rgb(68, 51, 26), AMBER),
        Ok(ActivityLevel::Strained) => (
            Color::Rgb(83, 39, 47),
            Color::Rgb(58, 28, 39),
            Color::Rgb(246, 123, 132),
        ),
        Err(_) => (CARD, BG, MUTED),
    };
    frame.render_widget(
        Block::default().style(Style::default().bg(case)),
        Rect::new(pc.x + 4, pc.y + 1, 16, 6),
    );
    frame.render_widget(
        Block::default().style(Style::default().bg(screen)),
        Rect::new(pc.x + 6, pc.y + 2, 12, 4),
    );
    let head = [
        "   ╭────────────────╮".to_owned(),
        "   │ ╭────────────╮ │".into(),
        "   │ │            │ │".into(),
        format!("   │ │   {eyes}   │ │"),
        format!("   │ │     {mouth}     │ │"),
        "   │ │            │ │".into(),
        "   │ ╰────────────╯ │".into(),
        "   ╰────────────────╯".into(),
    ];
    for (y, line) in head.iter().enumerate() {
        put(frame, pc, 0, y as u16, line, rim);
    }
    put(frame, pc, 9, 3, eyes, FACE);
    put(frame, pc, 11, 4, mouth, FACE);
    put(frame, pc, 8, 4, "·", CHEEK);
    put(frame, pc, 15, 4, "·", CHEEK);
    put(frame, pc, 10, 8, "│  │", rim);
    put(frame, pc, 7, 9, "╭──┴──┴──╮", rim);
    put(frame, pc, 7, 10, "╰────────╯", rim);
    // 汗はCPU活動だけを参照し，PSIや待機数には結び付けない．
    if busy {
        put(frame, pc, 9, 2, "╲", FACE);
        put(frame, pc, 14, 2, "╱", FACE);
        // 頭の斜め上へ丸い2粒を短く飛ばす．線・三角・軌跡は付けない．
        // 使用率の更新で位相が飛ばないよう，時計だけで周期を決める．
        let tick = elapsed.as_millis() / 100;
        let splash = match tick % 24 {
            0 => Some(([(2, 2), (4, 1)], SWEAT)),
            1 => Some(([(1, 1), (3, 0)], SWEAT)),
            2 => Some(([(0, 1), (2, 0)], SWEAT_FADED)),
            12 => Some(([(21, 2), (19, 1)], SWEAT)),
            13 => Some(([(22, 1), (20, 0)], SWEAT)),
            14 => Some(([(23, 1), (21, 0)], SWEAT_FADED)),
            _ => None,
        };
        if let Some((drops, color)) = splash {
            for (x, y) in drops {
                put(frame, stage, x, y, "•", color);
            }
        }
    }
    papers(frame, stage, 30, 13, &app.scene.task_stack.pressure_level);
    put(
        frame,
        stage,
        0,
        14,
        "───────────────────────────────────────",
        LINE,
    );
    match level {
        Err(Reason::FirstSample) => put(frame, area, 0, 0, "Starting…", MUTED),
        Err(_) => put(frame, area, 0, 0, "CPU usage N/A", AMBER),
        Ok(_) => {}
    }
}

fn papers(frame: &mut Frame, area: Rect, x: u16, bottom: u16, level: &Result<u8, Reason>) {
    match level {
        Err(Reason::FirstSample) => put(frame, area, x, bottom.saturating_sub(1), "  …", MUTED),
        Err(_) => put(frame, area, x, bottom.saturating_sub(1), "  ?", AMBER),
        Ok(0) => {}
        Ok(level) => {
            let count = u16::from(*level);
            let top = bottom.saturating_sub(count * 2);
            for y in top..bottom {
                let line = if (y - top) % 2 == 1 {
                    "│ ──── │"
                } else {
                    "╭──────╮"
                };
                put(frame, area, x, y, line, AMBER);
            }
            put(frame, area, x, bottom, "╰──────╯", AMBER);
        }
    }
}

fn metrics(frame: &mut Frame, area: Rect, app: &App) {
    let content = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(if area.height >= 8 { 2 } else { 0 }),
    ])
    .split(area);
    let cards = if content[0].width >= 70 {
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .spacing(2)
            .split(content[0])
    } else {
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)])
            .spacing(1)
            .split(content[0])
    };
    card(
        frame,
        cards[0],
        "CPU usage",
        &app.analysis.cpu_active_ratio,
        app.history.iter().map(|p| p.activity).collect(),
        MINT,
        &["Working".into()],
    );
    card(
        frame,
        cards[1],
        "CPU wait",
        &app.analysis.cpu_psi_some_interval_ratio,
        app.history.iter().map(|p| p.pressure).collect(),
        AMBER,
        &[
            "Waiting for CPU".into(),
            format!(
                "10s trend {}",
                value(&app.analysis.cpu_psi_some_avg10_pct, 1.0)
            ),
        ],
    );
    let runnable = app
        .analysis
        .evidence
        .current
        .readings
        .procs_running
        .value
        .as_ref()
        .map_or("N/A".into(), |v| v.to_string());
    let extra = format!("Runnable {runnable}");
    frame.render_widget(
        Paragraph::new(extra).style(Style::default().fg(MUTED)),
        content[1],
    );
}

fn card(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    metric: &Result<f64, Reason>,
    history: Vec<Option<f64>>,
    color: Color,
    notes: &[String],
) {
    if area.is_empty() {
        return;
    }
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(LINE))
        .style(Style::default().bg(CARD));
    let inner = block
        .inner(area)
        .inner(Margin::new(u16::from(area.width >= 18), 0));
    frame.render_widget(block, area);
    let heading = Line::from(vec![
        Span::styled(format!("{label}  "), Style::default().fg(MUTED)),
        Span::styled(
            value(metric, 100.0),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(heading),
        Rect::new(inner.x, inner.y, inner.width, inner.height.min(1)),
    );
    if inner.height >= 2 {
        let gap = (notes.len() as u16).min(inner.height.saturating_sub(1));
        for (row, note) in notes.iter().take(usize::from(gap)).enumerate() {
            frame.render_widget(
                Paragraph::new(note.as_str()).style(Style::default().fg(MUTED)),
                Rect::new(inner.x, inner.y + 1 + row as u16, inner.width, 1),
            );
        }
        let graph = Rect::new(
            inner.x,
            inner.y + 1 + gap,
            inner.width,
            inner.height.saturating_sub(1 + gap),
        );
        chart(frame, graph, &history, color);
    }
}

/// 履歴は生の解析値で描く．空白の過去を埋めず，欠測は?を残す．
fn chart(frame: &mut Frame, area: Rect, history: &[Option<f64>], color: Color) {
    const BARS: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    let n = history.len().min(usize::from(area.width));
    for (i, value) in history[history.len() - n..].iter().enumerate() {
        let x = area.width - n as u16 + i as u16;
        match value {
            Some(value) if value.is_finite() && (0.0..=1.0).contains(value) => {
                let units = (value * f64::from(area.height) * 8.0).round() as u32;
                for row in 0..area.height {
                    let amount = units
                        .saturating_sub(u32::from(area.height - 1 - row) * 8)
                        .min(8);
                    let glyph = if row == area.height - 1 && amount == 0 {
                        "▁"
                    } else {
                        BARS[amount as usize]
                    };
                    put(frame, area, x, row, glyph, color);
                }
            }
            _ if area.height > 0 => put(frame, area, x, area.height - 1, "?", MUTED),
            _ => {}
        }
    }
}

fn value(value: &Result<f64, Reason>, multiplier: f64) -> String {
    match value {
        Err(Reason::FirstSample) => "…".into(),
        Err(_) => "N/A".into(),
        Ok(value) => format!("{:.1}%", value * multiplier),
    }
}

fn small_terminal(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines = vec![
        Line::styled(
            "sysarium",
            Style::default().fg(MINT).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::raw("More room needed"),
        Line::raw("Resize to 80 x 24 or larger"),
        Line::styled(
            format!("Current: {} x {}", area.width, area.height),
            Style::default().fg(MUTED),
        ),
    ];
    if area.width >= 26 && area.height >= 10 {
        lines.extend([
            Line::raw(""),
            Line::raw(format!(
                "CPU usage {}",
                value(&app.analysis.cpu_active_ratio, 100.0)
            )),
            Line::raw(format!(
                "CPU wait {}",
                value(&app.analysis.cpu_psi_some_interval_ratio, 100.0)
            )),
        ]);
    }
    if area.width >= 20 && area.height >= 7 {
        lines.push(Line::raw(""));
        lines.push(Line::styled("q quit · ? help", Style::default().fg(MUTED)));
    }
    let height = (lines.len() as u16).min(area.height);
    let rect = Rect::new(
        area.x,
        area.y + (area.height - height) / 2,
        area.width,
        height,
    );
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), rect);
}
