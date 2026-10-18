use super::app::{App, ViewMode};
use inside_core::observation::UnavailableReason as Reason;
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
            .title_bottom(" ↑↓ PgUp/PgDn Home · Esc 戻る · q 終了 ")
            .border_style(Style::default().fg(LINE));
        let inner = border.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(border.style(Style::default().bg(BG).fg(INK)), area);
        let lines: Vec<Line<'static>> = overlay
            .wrapped_lines(inner.width)
            .into_iter()
            .map(Line::raw)
            .collect();
        frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
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
                "inside",
                Style::default().fg(INK).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ● LIVE", Style::default().fg(MINT)),
        ])),
        header[0],
    );
    let tabs: Vec<Span> = [ViewMode::Living, ViewMode::Metrics, ViewMode::Split]
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
    frame.render_widget(
        Paragraph::new(Line::from(tabs)).alignment(Alignment::Right),
        header[1],
    );
    match app.mode {
        ViewMode::Living => {
            let pieces =
                Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).split(rows[1]);
            living(frame, pieces[0], app, elapsed);
            let badges = format!(
                "CPU {}     Pressure {}",
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
        ViewMode::Metrics => metrics(frame, rows[1], app),
        ViewMode::Split => {
            let (left, right) = split_areas(rows[1]);
            living(frame, left, app, elapsed);
            metrics(frame, right, app);
        }
    }
    let footer = if outer.width >= 48 {
        "Tab 表示  ·  i 詳細  ·  ? ヘルプ  ·  q 終了"
    } else {
        "Tab 切替 · i 詳細 · ? · q 終了"
    };
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(MUTED)),
        rows[2],
    );
}

pub(super) fn split_areas(area: Rect) -> (Rect, Rect) {
    if area.width >= 74 && area.height >= 16 {
        let cols = Layout::horizontal([
            Constraint::Percentage(58),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);
        (cols[0], cols[2])
    } else {
        let rows = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(area.height.saturating_sub(1).min(9)),
        ])
        .split(area);
        (rows[0], rows[2])
    }
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

/// 形・座標・色はRendererだけが持つ．動作速度だけをSceneから受け取る．
fn living(frame: &mut Frame, area: Rect, app: &App, elapsed: Duration) {
    if area.is_empty() {
        return;
    }
    let rich = area.width >= 39 && area.height >= 15;
    let (width, height) = if rich {
        (39, 15)
    } else {
        (30.min(area.width), 7.min(area.height))
    };
    let stage = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let intensity = &app.scene.character.activity_intensity;
    let phase = match intensity {
        Ok(value) if *value > 0.0 => (elapsed.as_secs_f64() * (0.5 + 5.0 * value)) as u64 % 2 == 1,
        _ => false,
    };
    let eyes = if intensity.is_err() {
        "?    ?"
    } else {
        "●    ●"
    };
    if rich {
        frame.render_widget(
            Block::default().style(Style::default().bg(Color::Rgb(32, 65, 63))),
            Rect::new(stage.x + 4, stage.y + 1, 16, 6),
        );
        frame.render_widget(
            Block::default().style(Style::default().bg(Color::Rgb(23, 43, 46))),
            Rect::new(stage.x + 6, stage.y + 2, 12, 4),
        );
        frame.render_widget(
            Block::default().style(Style::default().bg(CARD)),
            Rect::new(stage.x + 4, stage.y + 12, 14, 1),
        );
        let art = [
            "   ╭────────────────╮".to_owned(),
            "   │ ╭────────────╮ │".into(),
            "   │ │            │ │".into(),
            format!("   │ │   {eyes}   │ │"),
            "   │ │     ◡      │ │".into(),
            "   │ │            │ │".into(),
            "   │ ╰────────────╯ │".into(),
            "   ╰────────────────╯".into(),
            " ╭──╯    ╭──╮    ╰──╮".into(),
            if phase {
                " ╰╮   ╭──┴──┴──╮  ╭─╯"
            } else {
                " ╰─╮  ╭──┴──┴──╮  ╭╯"
            }
            .into(),
            "   ╰╯ ╰────────╯ ╰╯".into(),
            "   ╭──────────────╮".into(),
            if phase {
                "   │ ▪ ▫ ▪ ▫ ▪ ▫  │"
            } else {
                "   │ ▫ ▪ ▫ ▪ ▫ ▪  │"
            }
            .into(),
            "   ╰──────────────╯".into(),
        ];
        for (y, line) in art.iter().enumerate() {
            put(frame, stage, 0, y as u16, line, MINT);
        }
        put(frame, stage, 9, 3, eyes, FACE);
        put(frame, stage, 11, 4, "◡", FACE);
        put(frame, stage, 8, 4, "·", CHEEK);
        put(frame, stage, 16, 4, "·", CHEEK);
        papers(
            frame,
            stage,
            30,
            13,
            &app.scene.task_stack.pressure_level,
            true,
        );
        put(
            frame,
            stage,
            0,
            14,
            "───────────────────────────────────────",
            LINE,
        );
    } else {
        let art = [
            " ╭──────────╮".into(),
            format!(" │  {eyes}  │"),
            " │    ◡     │".into(),
            " ╰──────────╯".into(),
            if phase {
                "  ╯ ╭────╮ ╰"
            } else {
                "  ╰ ╭────╮ ╯"
            }
            .into(),
            "   ╰──────╯".into(),
        ];
        for (y, line) in art.iter().enumerate() {
            put(frame, stage, 0, y as u16, line, MINT);
        }
        papers(
            frame,
            stage,
            21,
            5,
            &app.scene.task_stack.pressure_level,
            false,
        );
        put(frame, stage, 0, 6, "──────────────────────────────", LINE);
    }
    if intensity.is_err() {
        put(frame, area, 0, 0, "CPU N/A", AMBER);
    }
}

fn papers(
    frame: &mut Frame,
    area: Rect,
    x: u16,
    bottom: u16,
    level: &Result<u8, Reason>,
    rich: bool,
) {
    match level {
        Err(_) => put(frame, area, x, bottom.saturating_sub(1), "  ?", AMBER),
        Ok(0) => {}
        Ok(level) => {
            let count = u16::from(*level);
            let top = bottom.saturating_sub(if rich { count * 2 } else { count });
            for y in top..bottom {
                let line = if rich && (y - top) % 2 == 1 {
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
        "CPU",
        &app.analysis.cpu_active_ratio,
        app.history.iter().map(|p| p.activity).collect(),
        MINT,
    );
    card(
        frame,
        cards[1],
        "CPU pressure",
        &app.analysis.cpu_psi_some_interval_ratio,
        app.history.iter().map(|p| p.pressure).collect(),
        AMBER,
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
    let avg = value(&app.analysis.cpu_psi_some_avg10_pct, 1.0);
    let extra = if area.width >= 34 {
        format!("avg10 {avg}     Runnable {runnable}")
    } else {
        format!("avg10 {avg}  Runnable {runnable}")
    };
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
        let gap = u16::from(inner.height >= 5);
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
    value
        .as_ref()
        .map_or("N/A".into(), |value| format!("{:.1}%", value * multiplier))
}
