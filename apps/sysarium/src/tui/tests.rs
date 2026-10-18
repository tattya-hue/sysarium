use super::{
    app::{App, HISTORY_LIMIT, ViewMode},
    renderer,
};
use crate::output_tests::fixture;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use inside_core::observation::UnavailableReason as Reason;
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::time::Duration;

fn app() -> App {
    let mut app = App::new(fixture(1));
    app.observe(fixture(2));
    app
}

fn render(app: &App, width: u16, height: u16) -> String {
    render_at(app, width, height, Duration::ZERO)
}

fn render_at(app: &App, width: u16, height: u16, elapsed: Duration) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| renderer::draw(frame, app, elapsed))
        .unwrap();
    text(&terminal)
}

fn text(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let mut out = String::new();
    for y in buffer.area.top()..buffer.area.bottom() {
        let mut x = buffer.area.left();
        while x < buffer.area.right() {
            let symbol = buffer[(x, y)].symbol();
            out.push_str(symbol);
            x += ratatui::text::Span::raw(symbol).width().max(1) as u16;
        }
        out.push('\n');
    }
    out
}

fn p3_observation(case: &str, id: u64) -> inside_core::observation::Observation {
    inside_core::observation::Observation {
        sample_id: id,
        sampled_at_unix_ms: 1000 * id,
        elapsed_since_previous: (id > 1).then_some(Duration::from_secs(1)),
        scope: inside_core::observation::SYSTEM_VIEW,
        readings: inside_linux::collector::collect_from_proc(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/p3")
                .join(case),
        ),
    }
}

fn p3_app(case: &str) -> App {
    let mut app = App::new(p3_observation("before", 1));
    app.observe(p3_observation(case, 2));
    app
}

#[test]
#[ignore = "explicit synthetic screen export; requires INSIDE_REVIEW_SNAPSHOTS"]
fn p3_export_review_snapshots() {
    // 明示指定時のみ，負荷生成なしで合成fixtureの画面を保存する．
    let root = std::env::var_os("INSIDE_REVIEW_SNAPSHOTS")
        .expect("set INSIDE_REVIEW_SNAPSHOTS to the export directory");
    let root = std::path::PathBuf::from(root);
    std::fs::create_dir_all(&root).unwrap();
    for case in ["a", "b", "c", "d"] {
        let mut app = p3_app(case);
        app.mode = ViewMode::Both;
        for (width, height) in [(140, 34), (100, 24), (80, 24), (40, 24)] {
            if width == 80 {
                export_svg(
                    &app,
                    width,
                    height,
                    Duration::ZERO,
                    &root.join(format!("synthetic-{case}-{width}x{height}.svg")),
                );
            }
            let snapshot = render(&app, width, height)
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n")
                .trim_end()
                .to_owned()
                + "\n";
            std::fs::write(
                root.join(format!("synthetic-{case}-{width}x{height}.txt")),
                snapshot,
            )
            .unwrap();
        }
    }
    if let Some(frames_root) = std::env::var_os("SYSARIUM_PREVIEW_FRAMES") {
        for case in ["a", "b", "c"] {
            let app = p3_app(case);
            let frames = std::path::PathBuf::from(&frames_root).join(case);
            std::fs::create_dir_all(&frames).unwrap();
            for step in 0..24 {
                export_svg(
                    &app,
                    80,
                    24,
                    Duration::from_millis(step * 100),
                    &frames.join(format!("{step:02}.svg")),
                );
            }
        }
    }
    for (name, key, width, height) in [
        ("details-d", 'i', 40, 24),
        ("details-b", 'i', 80, 24),
        ("help", '?', 80, 24),
    ] {
        let mut app = p3_app(if name == "details-b" { "b" } else { "d" });
        press(&mut app, KeyCode::Char(key));
        let snapshot = render(&app, width, height)
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end()
            .to_owned()
            + "\n";
        std::fs::write(
            root.join(format!("synthetic-{name}-{width}x{height}.txt")),
            snapshot,
        )
        .unwrap();
    }
}

fn press(app: &mut App, code: KeyCode) -> bool {
    app.handle_event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
}

#[test]
fn diagnostic_wrapping_keeps_unicode_and_sanitizes_terminal_control_characters() {
    let overlay = super::app::Overlay {
        kind: super::app::OverlayKind::Details,
        title: "fixture".into(),
        text: "日本語abcdef\nnext\u{1b}[31m\rline".into(),
        scroll: 0,
    };
    for width in [1, 2, 7, 40] {
        let lines = overlay.wrapped_lines(width);
        for line in &lines {
            assert!(ratatui::text::Span::raw(line).width() <= usize::from(width));
            assert!(!line.chars().any(char::is_control));
        }
        if width >= 2 {
            assert!(lines.join("").contains("日本語abcdef"));
        }
    }
    assert!(overlay.wrapped_lines(0).is_empty());
}

#[test]
fn live_defaults_to_both_and_keys_switch_without_resampling() {
    let mut app = app();
    assert_eq!(app.mode, ViewMode::Both);
    let original = app.analysis.clone();
    let header = render(&app, 80, 24);
    assert!(header.contains("View:") && header.contains("Scene"));
    assert!(!header.contains("Computer"));
    press(&mut app, KeyCode::Char('?'));
    let help = &app.overlay.as_ref().unwrap().text;
    assert!(help.contains("Scene / Charts / Both"));
    assert!(!help.contains("Computer"));
    press(&mut app, KeyCode::Esc);
    for mode in [ViewMode::Scene, ViewMode::Charts, ViewMode::Both] {
        assert!(!press(&mut app, KeyCode::Tab));
        assert_eq!(app.mode, mode);
    }
    for (key, mode) in [
        ('1', ViewMode::Scene),
        ('2', ViewMode::Charts),
        ('3', ViewMode::Both),
    ] {
        press(&mut app, KeyCode::Char(key));
        assert_eq!(app.mode, mode);
    }
    let mut release = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    release.kind = KeyEventKind::Release;
    app.handle_event(Event::Key(release));
    assert_eq!(app.mode, ViewMode::Both);
    app.handle_event(Event::Resize(30, 10));
    assert_eq!(app.analysis, original);
    assert!(press(&mut app, KeyCode::Char('q')));
    assert!(app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL
    ))));
}

#[test]
fn fixture_pipeline_retains_both_axes_and_shared_observation() {
    for (case, cpu, psi, level) in [
        ("a", 0.93, Some(0.002), Some(0)),
        ("b", 0.92, Some(0.28), Some(5)),
        ("c", 0.08, Some(0.001), Some(0)),
        ("d", 0.85, None, None),
    ] {
        let app = p3_app(case);
        assert_eq!(app.analysis.cpu_active_ratio, Ok(cpu));
        assert_eq!(
            app.analysis
                .cpu_psi_some_interval_ratio
                .as_ref()
                .ok()
                .copied(),
            psi
        );
        assert_eq!(
            app.scene.task_stack.pressure_level.as_ref().ok().copied(),
            level
        );
        assert_eq!(app.scene.character.activity_intensity, Ok(cpu));
        assert_eq!(
            app.analysis.evidence.current.readings.procs_running.value,
            Ok(2)
        );
        assert_eq!(app.scene.source_sample_id, app.analysis.source_sample_id);
        assert_eq!(
            app.history.back().unwrap().sample_id,
            app.scene.source_sample_id
        );
        let out = render(&app, 80, 24);
        assert!(out.contains(&format!("{:.1}%", cpu * 100.0)));
        if let Some(psi) = psi {
            assert!(out.contains(&format!("{:.1}%", psi * 100.0)));
        } else {
            assert!(out.contains("N/A"));
            assert!(out.contains('?'));
        }
    }
    assert_ne!(p3_app("a").scene.task_stack, p3_app("b").scene.task_stack);
}

#[test]
fn supported_sizes_preserve_primary_values_without_technical_clutter() {
    for (width, height) in [(140, 34), (120, 32), (100, 24), (80, 24)] {
        for mode in [ViewMode::Scene, ViewMode::Charts, ViewMode::Both] {
            let mut app = p3_app("b");
            app.mode = mode;
            let out = render(&app, width, height);
            for label in ["92.0%", "28.0%", "i details", "q quit"] {
                assert!(
                    out.contains(label),
                    "{label}: {width}x{height} {mode:?}\n{out}"
                );
            }
            for clutter in ["sample_id", "/proc/", "EMA", "山", "比喩", "USER_HZ"] {
                assert!(!out.contains(clutter));
            }
            if mode != ViewMode::Charts {
                assert!(
                    out.contains("·  ╭╮  ·"),
                    "missing busy face: {width}x{height}\n{out}"
                );
                assert!(out.contains("│ ──── │") || out.contains("╭──────╮"));
            }
            if mode != ViewMode::Scene {
                assert!(out.contains("CPU wait"));
                assert!(out.contains("10s trend"));
                assert!(out.contains("?")); // first observation remains a history gap
            }
        }
    }
}

#[test]
fn unavailable_has_readable_live_details_and_lossless_json() {
    let mut app = App::new(fixture(1));
    let out = render(&app, 80, 24);
    assert!(out.contains("N/A"));
    assert!(out.contains('?'));
    assert!(!out.contains("0.0%"));
    assert!(!out.contains("│ ──── │"));
    let mut next = fixture(2);
    next.readings.cpu_psi_some.value = Err(Reason::Io {
        message: "specific context ".repeat(100),
    });
    app.observe(next);
    assert!(render(&app, 80, 24).contains("80.0%"));
    press(&mut app, KeyCode::Char('i'));
    assert!(app.overlay.as_ref().unwrap().text.contains("Read failed"));
    assert!(
        crate::output::json(&app.analysis)
            .unwrap()
            .contains(&"specific context ".repeat(100))
    );
    assert!(
        !app.overlay
            .as_ref()
            .unwrap()
            .text
            .contains(&"specific context ".repeat(100))
    );
    press(&mut app, KeyCode::PageDown);
    let offset = app.overlay.as_ref().unwrap().scroll;
    app.observe(fixture(3));
    let details = app.overlay.as_ref().unwrap();
    assert!(details.title.contains("#3"));
    assert!(!details.text.contains("source_sample_id"));
    assert_eq!(details.scroll, offset.min(details.max_scroll(80, 24)));
    app.observe(fixture(4)); // both samples healthy again
    assert!(app.overlay.as_ref().unwrap().title.contains("#4"));
    assert!(
        !app.overlay
            .as_ref()
            .unwrap()
            .text
            .contains("specific context")
    );
}

#[test]
fn history_is_bounded_and_keeps_zero_distinct_from_missing() {
    let mut app = app();
    assert_eq!(app.history.front().unwrap().activity, None);
    assert_eq!(app.history.back().unwrap().activity, Some(0.8));
    for id in 3..=140 {
        app.observe(fixture(id));
    }
    assert_eq!(app.history.len(), HISTORY_LIMIT);
    assert_eq!(app.history.front().unwrap().sample_id, 21);
    assert_eq!(app.history.back().unwrap().sample_id, 140);
    app.mode = ViewMode::Charts;
    app.history.clear();
    app.history.push_back(super::app::MetricPoint {
        sample_id: 140,
        activity: Some(0.0),
        pressure: None,
    });
    let out = render(&app, 80, 24);
    assert!(out.contains('▁'));
    assert!(out.contains('?'));
}

#[test]
fn rendering_is_pure_animation_requires_activity_and_does_not_change_pressure() {
    let mut app = app();
    let analysis = app.analysis.clone();
    let scene = app.scene.clone();
    let history = app.history.clone();
    let before = render_at(&app, 140, 34, Duration::ZERO);
    let after = render_at(&app, 140, 34, Duration::from_millis(300));
    assert_ne!(before, after);
    assert_eq!(
        before.matches("│ ──── │").count(),
        after.matches("│ ──── │").count()
    );
    assert_eq!(app.analysis, analysis);
    assert_eq!(app.scene, scene);
    assert_eq!(app.history, history);
    for activity in [Ok(0.0), Err(Reason::NotFound)] {
        app.scene.character.activity_intensity = activity;
        assert_eq!(
            render_at(&app, 140, 34, Duration::ZERO),
            render_at(&app, 140, 34, Duration::from_secs(3))
        );
    }
}

#[test]
fn resize_clears_old_pressure_and_restores_full_scene() {
    let mut app = p3_app("b");
    let mut terminal = Terminal::new(TestBackend::new(140, 34)).unwrap();
    terminal
        .draw(|frame| renderer::draw(frame, &app, Duration::ZERO))
        .unwrap();
    assert!(text(&terminal).contains("│ ──── │"));
    app.observe(p3_observation("d", 3));
    for (width, height) in [(80, 24), (40, 24), (28, 9), (1, 1), (0, 0), (140, 34)] {
        terminal.backend_mut().resize(width, height);
        terminal.resize(Rect::new(0, 0, width, height)).unwrap();
        terminal
            .draw(|frame| renderer::draw(frame, &app, Duration::ZERO))
            .unwrap();
        assert_eq!(
            terminal.backend().buffer().area,
            Rect::new(0, 0, width, height)
        );
        let out = text(&terminal);
        assert!(!out.contains("│ ──── │"));
        if width >= 80 {
            assert!(out.contains("N/A"));
            assert!(out.contains('?'));
        }
    }
    let (a, b) = renderer::split_areas(Rect::new(0, 0, 76, 19));
    assert_eq!(a.y, b.y);
    assert!(a.right() < b.x);
}

#[test]
fn overlay_scroll_resize_help_and_quit_remain_accessible() {
    let mut app = p3_app("b");
    for key in ['?', 'i'] {
        app.overlay = None;
        press(&mut app, KeyCode::Char(key));
        for (width, height) in [(80, 24), (40, 24), (28, 9), (1, 1), (0, 0), (140, 34)] {
            app.resize(width, height);
            render(&app, width, height);
            for _ in 0..300 {
                press(&mut app, KeyCode::PageDown);
            }
            let overlay = app.overlay.as_ref().unwrap();
            assert_eq!(overlay.scroll, overlay.max_scroll(width, height));
            press(&mut app, KeyCode::Home);
            assert_eq!(app.overlay.as_ref().unwrap().scroll, 0);
        }
        let mode = app.mode;
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.mode, mode);
        assert!(press(&mut app, KeyCode::Char('q')));
        press(&mut app, KeyCode::Char(key));
        assert!(app.overlay.is_none());
    }
    press(&mut app, KeyCode::Char('?'));
    for term in [
        "CPU usage",
        "CPU wait",
        "not a task count",
        "Linux measures how long tasks wait",
        "Runnable",
        "~1/s",
        "0–100%",
    ] {
        assert!(app.overlay.as_ref().unwrap().text.contains(term));
    }
}

// レビュー用の静的成果物．本体の描画バッファを色も含めてそのまま保存する．
fn export_svg(app: &App, width: u16, height: u16, elapsed: Duration, path: &std::path::Path) {
    use ratatui::style::Color;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| renderer::draw(frame, app, elapsed))
        .unwrap();
    let color = |c: Color| match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => "#e2e8ed".into(),
    };
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\"><title>Sysarium · synthetic fixture · {width}×{height}</title>",
        width * 10,
        height * 20,
        width * 10,
        height * 20
    );
    svg.push_str(&format!(
        "<rect width=\"{}\" height=\"{}\" fill=\"#131926\"/>\n",
        width * 10,
        height * 20
    ));
    for y in 0..height {
        let mut x = 0;
        while x < width {
            let bg = terminal.backend().buffer()[(x, y)].bg;
            let start = x;
            while x < width && terminal.backend().buffer()[(x, y)].bg == bg {
                x += 1;
            }
            if bg != Color::Rgb(19, 25, 38) && bg != Color::Reset {
                svg.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"20\" fill=\"{}\"/>\n",
                    start * 10,
                    y * 20,
                    (x - start) * 10,
                    color(bg)
                ));
            }
        }
    }
    for y in 0..height {
        let mut x = 0;
        while x < width {
            let cell = &terminal.backend().buffer()[(x, y)];
            let symbol = cell.symbol();
            let size = ratatui::text::Span::raw(symbol).width().max(1) as u16;
            if symbol != " " {
                let escaped = symbol
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");
                svg.push_str(&format!("<text x=\"{}\" y=\"{}\" fill=\"{}\" font-family=\"DejaVu Sans Mono,DejaVu Sans,Noto Sans CJK JP,monospace\" font-size=\"16\">{escaped}</text>\n",x*10,y*20+16,color(cell.fg)));
            }
            x += size;
        }
    }
    svg.push_str("</svg>\n");
    std::fs::write(path, svg).unwrap();
}

#[test]
fn face_monitor_and_stand_stay_symmetric_while_sweat_moves() {
    let reflect = |c| match c {
        '╭' => '╮',
        '╮' => '╭',
        '╰' => '╯',
        '╯' => '╰',
        '╱' => '╲',
        '╲' => '╱',
        '├' => '┤',
        '┤' => '├',
        c => c,
    };
    for elapsed in [Duration::ZERO, Duration::from_millis(300)] {
        let out = render_at(&app(), 80, 24, elapsed);
        let lines: Vec<_> = out.lines().collect();
        let top = lines
            .iter()
            .position(|line| line.contains("╭────────────────╮"))
            .unwrap();
        let x = lines[top].chars().position(|c| c == '╭').unwrap() - 3;
        for line in &lines[top..=top + 10] {
            // 汗は非対称に流す．PC本体と顔だけは対称性を保つ．
            let body: Vec<_> = line.chars().skip(x + 3).take(18).collect();
            let mirrored: Vec<_> = body.iter().rev().copied().map(reflect).collect();
            assert_eq!(body, mirrored, "unbalanced computer: {line}");
        }
    }
}

#[test]
fn sweat_splashes_outward_one_side_at_a_time_with_pauses() {
    let mut app = p3_app("a");
    app.scene.character.activity_intensity = Ok(1.0);
    let drops = |elapsed| {
        let out = render_at(&app, 80, 24, elapsed);
        out.lines()
            .enumerate()
            .flat_map(|(y, line)| {
                line.chars()
                    .enumerate()
                    .filter_map(move |(x, c)| (c == '•').then_some((x, y)))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let left = drops(Duration::ZERO);
    let left_outward = drops(Duration::from_millis(300));
    let right = drops(Duration::from_secs(1));
    let right_outward = drops(Duration::from_millis(1300));
    assert_eq!(left.len(), 1);
    assert_eq!(left_outward.len(), 1);
    assert_eq!(right.len(), 1);
    assert_eq!(right_outward.len(), 1);
    assert!(left_outward[0].0 < left[0].0 && left_outward[0].1 < left[0].1);
    assert!(right_outward[0].0 > right[0].0 && right_outward[0].1 < right[0].1);
    assert!(drops(Duration::from_millis(750)).is_empty());
    assert!(drops(Duration::from_millis(1750)).is_empty());
    for step in 0..24 {
        assert!(drops(Duration::from_millis(step * 100)).len() <= 1);
    }
}

#[test]
fn undersized_terminal_shows_guidance_without_cutting_or_shrinking_character() {
    let mut app = p3_app("b");
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    for (width, height) in [
        (80, 24),
        (79, 24),
        (80, 23),
        (40, 24),
        (28, 9),
        (1, 1),
        (0, 0),
        (80, 24),
    ] {
        terminal.backend_mut().resize(width, height);
        terminal.resize(Rect::new(0, 0, width, height)).unwrap();
        terminal
            .draw(|frame| renderer::draw(frame, &app, Duration::ZERO))
            .unwrap();
        let out = text(&terminal);
        if width >= 80 && height >= 24 {
            assert!(out.contains("·  ╭╮  ·"));
            assert!(out.contains("╰────────╯"));
            assert!(out.contains("Tab view"));
        } else {
            assert!(!out.contains("·  ╭╮  ·"));
            assert!(!out.contains("╰────────╯"));
            if width >= 40 && height >= 10 {
                assert!(out.contains("80 x 24"));
                assert!(out.contains("92.0%"));
                assert!(out.contains("28.0%"));
            }
        }
    }
    app.resize(40, 24);
    app.observe(p3_observation("d", 3));
    assert_eq!(app.analysis.source_sample_id, 3);
    assert!(render(&app, 40, 24).contains("CPU wait N/A"));
    assert!(press(&mut app, KeyCode::Char('q')));
}

#[test]
fn all_views_overlays_and_unavailable_states_use_english_copy() {
    let no_cjk = |text: &str| {
        assert!(
            !text
                .chars()
                .any(|c| matches!(c, '\u{3040}'..='\u{30ff}' | '\u{4e00}'..='\u{9fff}')),
            "non-English UI: {text}"
        )
    };
    for mut app in [App::new(fixture(1)), p3_app("b"), p3_app("d")] {
        for mode in [ViewMode::Scene, ViewMode::Charts, ViewMode::Both] {
            app.mode = mode;
            no_cjk(&render(&app, 80, 24));
        }
        no_cjk(&render(&app, 40, 24));
        for key in ['i', '?'] {
            app.overlay = None;
            press(&mut app, KeyCode::Char(key));
            no_cjk(&app.overlay.as_ref().unwrap().text);
            no_cjk(&render(&app, 80, 24));
        }
    }
}

#[test]
fn computer_activity_is_visible_without_keyboard_and_independent_of_wait() {
    let pc = |app: &App| {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| renderer::draw(frame, app, Duration::ZERO))
            .unwrap();
        let out = text(&terminal);
        let lines: Vec<_> = out.lines().collect();
        let top = lines
            .iter()
            .position(|line| line.contains("╭────────────────╮"))
            .unwrap();
        let left = lines[top].chars().position(|c| c == '╭').unwrap() - 3;
        let buffer = terminal.backend().buffer();
        let mut cells = Vec::new();
        for y in top..top + 11 {
            for x in left..left + 24 {
                cells.push(buffer[(x as u16, y as u16)].clone());
            }
        }
        cells
    };
    let relaxed = p3_app("c");
    assert!(!render(&relaxed, 80, 24).contains('•'));
    assert!(render(&relaxed, 80, 24).contains("·  ╰╯  ·"));
    let mut busy = p3_app("a");
    busy.scene.character.activity_intensity = Ok(0.9);
    assert!(render(&busy, 80, 24).contains('•'));
    assert!(render(&busy, 80, 24).contains("·  ╭╮  ·"));
    assert!(!render(&busy, 80, 24).contains("·  ╰╯  ·"));
    assert_ne!(pc(&relaxed), pc(&busy)); // Includes screen and case colours.
    for case in ["b", "d"] {
        let mut other = p3_app(case);
        other.scene.character.activity_intensity = Ok(0.9);
        assert_eq!(pc(&busy), pc(&other)); // Waiting/PSI missing must not drive sweating.
    }
    busy.scene.character.activity_intensity = Err(Reason::NotFound);
    assert!(!render(&busy, 80, 24).contains('•'));
    assert_ne!(pc(&relaxed), pc(&busy)); // Unknown must not look idle.
    for app in [&relaxed, &busy] {
        assert!(!render(app, 80, 24).contains('▪'));
    }
}
