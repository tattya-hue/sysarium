use super::app::{App, ViewMode};
use inside_core::observation::UnavailableReason as Reason;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use std::time::Duration;

const METAPHOR: &str = "山=競合の比喩（タスク数ではない）";

pub fn draw(frame: &mut Frame, app: &App, elapsed: Duration) {
    let area = frame.area();
    if area.is_empty() {
        return;
    }
    if let Some(overlay) = &app.overlay {
        let maximum = overlay.max_scroll(area.width, area.height);
        let scroll = overlay.scroll.min(maximum);
        let border = block(overlay.title.clone())
            .title_bottom(format!("↑↓/PgUp/Dn Home Esc | {scroll}/{maximum}"));
        let inner = border.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(border, area);
        let lines: Vec<Line<'static>> = overlay
            .wrapped_lines(inner.width)
            .into_iter()
            .map(Line::raw)
            .collect();
        frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
        return;
    }
    if area.width < 40 || area.height < 24 {
        compact(frame, area, app, elapsed);
        return;
    }
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .split(area);
    render_lines(
        frame,
        rows[0],
        vec![
            format!(
                "Inside live | {} | sample_id={}",
                app.mode.label(),
                app.analysis.source_sample_id
            ),
            format!(
                "Tab:切替 q:終了 i:詳細 ?:help | system-view | unix_ms={}",
                app.analysis.evidence.current.sampled_at_unix_ms
            ),
        ],
    );
    match app.mode {
        ViewMode::Living => living(frame, rows[1], app, elapsed),
        ViewMode::Metrics => metrics(frame, rows[1], app),
        ViewMode::Split => {
            let (left, right) = split_areas(rows[1]);
            living(frame, left, app, elapsed);
            metrics(frame, right, app);
        }
    }
    render_lines(
        frame,
        rows[2],
        vec![
            METAPHOR.into(),
            "CPU活動量は近似．i:同じ観測の詳細 ?:ヘルプ".into(),
        ],
    );
}

pub(super) fn split_areas(area: Rect) -> (Rect, Rect) {
    let constraints = [Constraint::Percentage(50), Constraint::Percentage(50)];
    let areas = if area.width >= 100 {
        Layout::horizontal(constraints).split(area)
    } else {
        Layout::vertical(constraints).split(area)
    };
    (areas[0], areas[1])
}

fn block(title: String) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::DarkGray))
}

fn living(frame: &mut Frame, area: Rect, app: &App, elapsed: Duration) {
    let border = block(format!("Living | sample_id={}", app.scene.source_sample_id));
    let inner = border.inner(area);
    frame.render_widget(border, area);
    let (face, hands) = pose(&app.scene.character.activity_intensity, elapsed);
    // 8行でキャラクター・山・実測PSI・両軸の理由を残す．
    let mut lines = vec![
        format!(
            "CPUの仕事（近似）: {}",
            value(&app.analysis.cpu_active_ratio, 100.0)
        ),
        format!(
            "表示強度（平滑化）: {}",
            value(&app.scene.character.activity_intensity, 100.0)
        ),
        format!("    .--------.  山: {}", stack(app)),
        format!(
            "    | {face} |  PSI: {}",
            value(&app.analysis.cpu_psi_some_interval_ratio, 100.0)
        ),
        format!("{hands}  競合の比喩"),
        source_or_reason(
            "CPU",
            &app.analysis.cpu_active_ratio,
            app.analysis.evidence.current.readings.cpu_times.source,
        ),
        source_or_reason(
            "PSI",
            &app.analysis.cpu_psi_some_interval_ratio,
            app.analysis.evidence.current.readings.cpu_psi_some.source,
        ),
        "山の段階は待機タスク数ではない".into(),
    ];
    if inner.height >= 15 {
        lines.push(String::new());
        lines.push("仕事の山（0〜5の比喩的な段階）".into());
        match &app.scene.task_stack.pressure_level {
            Ok(level) => {
                for row in (1..=5).rev() {
                    lines.push(if row <= *level { "      [=====]" } else { "" }.into());
                }
                if *level == 0 {
                    lines.push("      （山なし）".into());
                }
            }
            Err(reason) => lines.push(format!("      [?] 不明 [{}]", brief_reason(reason))),
        }
    }
    render_lines(frame, inner, lines);
}

fn source_or_reason(name: &str, metric: &Result<f64, Reason>, source: &str) -> String {
    match metric {
        Ok(_) => format!("{name}: {source}"),
        Err(reason) => format!("{name}不明: {}", brief_reason(reason)),
    }
}

fn stack(app: &App) -> String {
    match &app.scene.task_stack.pressure_level {
        Ok(level) => format!(
            "[{}{}] {level}/5",
            "#".repeat(usize::from(*level)),
            ".".repeat(usize::from(5_u8.saturating_sub(*level)))
        ),
        Err(_) => "[?] 不明".into(),
    }
}

/// Rendererだけがフレーム・ASCIIを持つ．欠測と表示強度0では動かない．
fn pose(intensity: &Result<f64, Reason>, elapsed: Duration) -> (&'static str, &'static str) {
    match intensity {
        Err(_) => (" ?  ? ", "    |  ?     |"),
        Ok(value) if *value == 0.0 => (" -  - ", "    |  CPU   |"),
        Ok(value) => {
            let phase = (elapsed.as_secs_f64() * (0.5 + 5.0 * value)) as u64 % 2;
            if phase == 0 {
                (" o  o ", "  / |  CPU   | \\")
            } else {
                (" ^  ^ ", "  \\ |  CPU   | /")
            }
        }
    }
}

fn metrics(frame: &mut Frame, area: Rect, app: &App) {
    let border = block(format!(
        "Metrics | sample_id={}",
        app.analysis.source_sample_id
    ))
    .title_bottom("/proc/stat + /proc/pressure/cpu");
    let inner = border.inner(area);
    frame.render_widget(border, area);
    let mut lines = metric_lines(app);
    lines.extend(history_lines(app, inner.width));
    let interval = match &app.analysis.evidence.elapsed {
        Ok(duration) => format!("区間: {:.6}s（単調時計）", duration.as_secs_f64()),
        Err(reason) => format!("区間: N/A [{}]", brief_reason(reason)),
    };
    lines.push(interval);
    if inner.height >= 10 {
        lines.push("avg10: 平滑化された傾向".into());
        lines.push("runnable: 実行中を含む参考値".into());
    }
    render_lines(frame, inner, lines);
}

fn metric_lines(app: &App) -> Vec<String> {
    let a = &app.analysis;
    let runnable = match &a.evidence.current.readings.procs_running.value {
        Ok(value) => format!("{value}（実行中含む参考値）"),
        Err(reason) => format!("N/A [{}]", brief_reason(reason)),
    };
    vec![
        metric("CPU活動(近似)", &a.cpu_active_ratio, 100.0),
        metric("PSI区間", &a.cpu_psi_some_interval_ratio, 100.0),
        metric("avg10(平滑化)", &a.cpu_psi_some_avg10_pct, 1.0),
        format!("procs_running: {runnable}"),
    ]
}

fn history_lines(app: &App, width: u16) -> Vec<String> {
    let count = usize::from(width.saturating_sub(5)).min(app.history.len());
    let points: Vec<_> = app.history.iter().skip(app.history.len() - count).collect();
    let activity = series(points.iter().map(|p| p.activity));
    let pressure = series(points.iter().map(|p| p.pressure));
    let ids = points
        .first()
        .zip(points.last())
        .map_or("-".into(), |(first, last)| {
            format!("{}..{}", first.sample_id, last.sample_id)
        });
    vec![
        format!("CPU  {activity}"),
        format!("PSI  {pressure}"),
        format!("履歴0〜100% ?:不明 #{ids}"),
    ]
}

pub(super) fn series(values: impl Iterator<Item = Option<f64>>) -> String {
    const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    values
        .map(|value| match value {
            Some(value) if value.is_finite() && (0.0..=1.0).contains(&value) => {
                BARS[(value * 7.0).round() as usize]
            }
            _ => '?',
        })
        .collect()
}

fn compact(frame: &mut Frame, area: Rect, app: &App, elapsed: Duration) {
    let mut lines = vec![format!(
        "q:終了 Tab:切替 i:詳細 ?:help {} #{}",
        app.mode.label(),
        app.analysis.source_sample_id
    )];
    if app.mode != ViewMode::Metrics {
        let (face, _) = pose(&app.scene.character.activity_intensity, elapsed);
        lines.push(format!("Living sample_id={}", app.scene.source_sample_id));
        lines.push(format!(
            "PC [{face}] 仕事:{}",
            value(&app.analysis.cpu_active_ratio, 100.0)
        ));
        lines.push(format!("山:{}（競合の比喩）", stack(app)));
        if let Err(reason) = &app.scene.task_stack.pressure_level {
            lines.push(format!("PSI: N/A [{}]", brief_reason(reason)));
        }
        if let Err(reason) = &app.scene.character.activity_intensity {
            lines.push(format!("CPU: N/A [{}]", brief_reason(reason)));
        }
    }
    if app.mode != ViewMode::Living {
        lines.push(format!(
            "Metrics sample_id={}",
            app.analysis.source_sample_id
        ));
        lines.extend(metric_lines(app));
        lines.extend(history_lines(app, area.width));
    }
    lines.push("山は待機数ではない / ?:不明".into());
    render_lines(frame, area, lines);
}

fn value(value: &Result<f64, Reason>, multiplier: f64) -> String {
    match value {
        Ok(value) => format!("{:.2}%", value * multiplier),
        Err(_) => "N/A 不明".into(),
    }
}

fn metric(name: &str, result: &Result<f64, Reason>, multiplier: f64) -> String {
    match result {
        Ok(_) => format!("{name}: {}", value(result, multiplier)),
        Err(reason) => format!("{name}: N/A [{}]", brief_reason(reason)),
    }
}

/// 画面内の短い理由．元の観測・原因の全文はinspect cpuとJSONが保持する．
fn brief_reason(reason: &Reason) -> String {
    match reason {
        Reason::InputUnavailable {
            sample_id, reason, ..
        } => format!("{}@#{sample_id}", brief_reason(reason)),
        Reason::Missing { field } => format!("missing:{field}"),
        Reason::Invalid { field, .. } => format!("invalid:{field}"),
        Reason::Duplicate { field } => format!("duplicate:{field}"),
        Reason::CounterRegression { field } => format!("counter_regression:{field}"),
        _ => crate::output::reason(reason)["code"]
            .as_str()
            .unwrap_or("unavailable")
            .into(),
    }
}

/// 折返しで後続の必須情報を押し出さず，表示セル幅で明示的に省略する．
fn render_lines(frame: &mut Frame, area: Rect, lines: Vec<String>) {
    let lines: Vec<Line<'static>> = lines
        .into_iter()
        .map(|line| {
            let color = if line.contains("N/A") || line.contains("不明:") {
                Color::Yellow
            } else {
                Color::Reset
            };
            Line::styled(fit(&line, area.width), Style::default().fg(color))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn fit(text: &str, width: u16) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if Span::raw(&cleaned).width() <= usize::from(width) {
        return cleaned;
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in cleaned.chars() {
        let size = Span::raw(c.to_string()).width();
        if used + size >= usize::from(width) {
            break;
        }
        out.push(c);
        used += size;
    }
    out.push('…');
    out
}
