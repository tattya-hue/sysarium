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

#[test]
fn tab_cycles_three_modes_q_and_control_c_exit_and_resize_keeps_state() {
    let mut app = app();
    let original = app.analysis.clone();
    for mode in [ViewMode::Metrics, ViewMode::Split, ViewMode::Living] {
        assert!(!app.handle_event(Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))));
        assert_eq!(app.mode, mode);
    }
    let mut release = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    release.kind = KeyEventKind::Release;
    assert!(!app.handle_event(Event::Key(release)));
    assert_eq!(app.mode, ViewMode::Living);
    assert!(!app.handle_event(Event::Resize(30, 10)));
    assert_eq!(app.analysis, original);
    assert!(app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('q'),
        KeyModifiers::NONE
    ))));
    assert!(app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL
    ))));
}

#[test]
fn living_and_metrics_render_the_same_sample_and_values() {
    let mut app = app();
    app.mode = ViewMode::Split;
    let out = render(&app, 140, 34);
    assert!(out.contains("Living | sample_id=2"));
    assert!(out.contains("Metrics | sample_id=2"));
    assert!(out.contains("80.00%"));
    assert!(out.contains("25.00%"));
    assert!(out.contains("2.50%"));
    assert!(out.contains("5/5"));
    assert!(out.contains(".--------."));
    assert!(out.contains("[=====]"));
    assert!(out.contains("/proc/pressure/cpu"));
    assert_eq!(app.scene.source_sample_id, app.analysis.source_sample_id);
    assert_eq!(
        app.history.back().unwrap().sample_id,
        app.analysis.source_sample_id
    );
}

#[test]
fn first_sample_and_missing_psi_have_visible_unknown_and_reason() {
    let mut app = App::new(fixture(1));
    app.mode = ViewMode::Split;
    let out = render(&app, 140, 34);
    assert!(out.contains("N/A"));
    assert!(out.contains('?'));
    assert!(!out.contains("[=====]"));
    assert!(!out.contains("0.00%"));
    let mut next = fixture(2);
    next.readings.cpu_psi_some.value = Err(Reason::NotFound);
    app.observe(next);
    let out = render(&app, 140, 34);
    assert!(out.contains("80.00%"));
    assert!(out.contains("N/A"));
    assert!(out.contains("not_found"));
    assert!(!out.contains("[=====]"));
}

#[test]
fn history_is_bounded_and_keeps_unavailable_as_gaps() {
    let mut app = app();
    assert_eq!(app.history.front().unwrap().activity, None);
    assert_eq!(app.history.back().unwrap().activity, Some(0.8));
    for id in 3..=140 {
        app.observe(fixture(id));
    }
    assert_eq!(app.history.len(), HISTORY_LIMIT);
    assert_eq!(app.history.front().unwrap().sample_id, 21);
    assert_eq!(app.history.back().unwrap().sample_id, 140);
    assert_eq!(
        renderer::series([None, Some(0.0), Some(1.0), Some(f64::NAN)].into_iter()),
        "?▁█?"
    );
}

#[test]
fn drawing_never_updates_observations_scene_or_history() {
    let mut app = app();
    let original = app.analysis.clone();
    let scene = app.scene.clone();
    let history = app.history.clone();
    let mut terminal = Terminal::new(TestBackend::new(140, 34)).unwrap();
    for step in 0..30 {
        app.mode = [ViewMode::Living, ViewMode::Metrics, ViewMode::Split][step % 3];
        terminal
            .draw(|frame| renderer::draw(frame, &app, Duration::from_millis(step as u64 * 100)))
            .unwrap();
    }
    assert_eq!(app.analysis, original);
    assert_eq!(app.scene, scene);
    assert_eq!(app.history, history);
}

#[test]
fn resizing_same_terminal_reflows_split_and_handles_tiny_areas() {
    let mut app = app();
    app.mode = ViewMode::Split;
    let mut terminal = Terminal::new(TestBackend::new(140, 34)).unwrap();
    for (width, height) in [
        (140, 34),
        (72, 40),
        (40, 24),
        (28, 9),
        (1, 1),
        (0, 0),
        (140, 34),
    ] {
        terminal.backend_mut().resize(width, height);
        terminal.resize(Rect::new(0, 0, width, height)).unwrap();
        terminal
            .draw(|frame| renderer::draw(frame, &app, Duration::ZERO))
            .unwrap();
        assert_eq!(
            terminal.backend().buffer().area,
            Rect::new(0, 0, width, height)
        );
        if width >= 40 {
            let out = text(&terminal);
            assert!(out.contains("Living | sample_id=2"));
            assert!(out.contains("Metrics | sample_id=2"));
        }
    }
    let wide = renderer::split_areas(Rect::new(0, 0, 140, 30));
    assert_eq!(wide.0.y, wide.1.y);
    assert!(wide.0.x < wide.1.x);
    let narrow = renderer::split_areas(Rect::new(0, 0, 72, 36));
    assert_eq!(narrow.0.x, narrow.1.x);
    assert!(narrow.0.y < narrow.1.y);
}

#[test]
fn only_measured_activity_animates_and_pressure_stack_stays_fixed() {
    let mut app = app();
    let first = render_at(&app, 140, 34, Duration::ZERO);
    let later = render_at(&app, 140, 34, Duration::from_millis(300));
    assert_ne!(first, later);
    assert_eq!(
        first.matches("[=====]").count(),
        later.matches("[=====]").count()
    );
    for activity in [Ok(0.0), Err(Reason::NotFound)] {
        app.scene.character.activity_intensity = activity;
        assert_eq!(
            render_at(&app, 140, 34, Duration::ZERO),
            render_at(&app, 140, 34, Duration::from_secs(3))
        );
    }
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
fn p3_cases_distinguish_activity_and_contention_through_the_entire_pipeline() {
    for (case, cpu, psi, level) in [
        ("a", 0.93, Some(0.002), Some(0)),
        ("b", 0.92, Some(0.28), Some(5)),
        ("c", 0.08, Some(0.001), Some(0)),
        ("d", 0.85, None, None),
    ] {
        let mut app = p3_app(case);
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
        assert_eq!(app.scene.source_sample_id, 2);
        assert_eq!(app.history.back().unwrap().sample_id, 2);
        app.mode = ViewMode::Split;
        let out = render(&app, 80, 24);
        assert!(out.contains(&format!("{:.2}%", cpu * 100.0)));
        match psi {
            Some(psi) => assert!(out.contains(&format!("{:.2}%", psi * 100.0))),
            None => {
                assert!(out.contains("[?] 不明"));
                assert!(out.contains("not_found"));
            }
        }
        if let Some(level) = level {
            assert!(out.contains(&format!("{level}/5")));
        }
    }
    // runnableが同じでもPSI由来の山は異なる．待機数の推定には使っていない．
    assert_ne!(p3_app("a").scene.task_stack, p3_app("b").scene.task_stack);
}

#[test]
fn p3_supported_sizes_keep_character_stack_metrics_history_and_meaning_visible() {
    for (width, height) in [
        (140, 34),
        (120, 32),
        (100, 24),
        (80, 24),
        (60, 24),
        (40, 24),
    ] {
        for mode in [ViewMode::Living, ViewMode::Metrics, ViewMode::Split] {
            let mut app = p3_app("b");
            app.mode = mode;
            let out = render(&app, width, height);
            assert!(
                out.contains("28.00%"),
                "PSI absent at {width}x{height} {mode:?}"
            );
            assert!(
                out.contains("山=競合の比喩（タスク数ではない）")
                    || out.contains("山の段階は待機タスク数ではない")
            );
            if mode != ViewMode::Metrics {
                for label in [
                    "Living | sample_id=2",
                    ".--------.",
                    "[#####] 5/5",
                    "92.00%",
                    "平滑化",
                ] {
                    assert!(
                        out.contains(label),
                        "{label} absent at {width}x{height} {mode:?}\n{out}"
                    );
                }
            }
            if mode != ViewMode::Living {
                for label in [
                    "Metrics | sample_id=2",
                    "CPU活動(近似)",
                    "PSI区間",
                    "avg10(平滑化)",
                    "procs_running: 2",
                    "CPU  ?",
                    "PSI  ?",
                    "履歴0〜100% ?:不明",
                    "単調時計",
                ] {
                    assert!(
                        out.contains(label),
                        "{label} absent at {width}x{height} {mode:?}\n{out}"
                    );
                }
            }
        }
    }
}

#[test]
fn p3_missing_and_long_errors_do_not_push_other_metrics_out_of_view() {
    for (width, height) in [(140, 34), (100, 24), (80, 24), (40, 24)] {
        let mut app = p3_app("d");
        app.mode = ViewMode::Split;
        let out = render(&app, width, height);
        for label in [
            "85.00%",
            "[?] 不明",
            "not_found",
            "procs_running: 2",
            "CPU  ?",
            "PSI  ??",
        ] {
            assert!(
                out.contains(label),
                "{label} absent at {width}x{height}\n{out}"
            );
        }
        let mut observation = p3_observation("b", 3);
        let cause = Reason::Io {
            message: "long error context ".repeat(100),
        };
        observation.readings.cpu_times.value = Err(cause.clone());
        observation.readings.cpu_psi_some.value = Err(cause);
        observation.readings.procs_running.value = Err(Reason::PermissionDenied);
        app.observe(observation);
        let out = render(&app, width, height);
        for label in ["N/A", "io@#3", "permission_denied", "CPU  ?", "PSI  ??"] {
            assert!(
                out.contains(label),
                "{label} absent at {width}x{height}\n{out}"
            );
        }
        assert!(!out.contains("0.00%"));
        let json = crate::output::json(&app.analysis).unwrap();
        assert!(json.contains(&"long error context ".repeat(100)));
    }
}

#[test]
fn p3_resize_from_high_pressure_to_missing_clears_the_old_stack() {
    let mut app = p3_app("b");
    app.mode = ViewMode::Split;
    let mut terminal = Terminal::new(TestBackend::new(140, 34)).unwrap();
    terminal
        .draw(|frame| renderer::draw(frame, &app, Duration::ZERO))
        .unwrap();
    assert!(text(&terminal).contains("[#####] 5/5"));
    app.observe(p3_observation("d", 3));
    for (width, height) in [(80, 24), (40, 24), (140, 34)] {
        terminal.backend_mut().resize(width, height);
        terminal.resize(Rect::new(0, 0, width, height)).unwrap();
        terminal
            .draw(|frame| renderer::draw(frame, &app, Duration::ZERO))
            .unwrap();
        let out = text(&terminal);
        assert!(out.contains("[?] 不明"));
        assert!(out.contains("not_found"));
        assert!(!out.contains("[#####]"));
        assert!(!out.contains("[=====]"));
        assert_eq!(app.scene.source_sample_id, 3);
    }
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
        app.mode = ViewMode::Split;
        for (width, height) in [(140, 34), (100, 24), (80, 24), (40, 24)] {
            let snapshot = render(&app, width, height)
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            std::fs::write(
                root.join(format!("synthetic-{case}-{width}x{height}.txt")),
                snapshot,
            )
            .unwrap();
        }
    }
    for (name, key, width, height) in [("details-d", 'i', 40, 24), ("help", '?', 80, 24)] {
        let mut app = p3_app("d");
        press(&mut app, KeyCode::Char(key));
        let snapshot = render(&app, width, height)
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
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
fn details_preserve_the_selected_sample_and_full_reasons_while_collection_continues() {
    let mut app = p3_app("b");
    let mut failed = p3_observation("b", 3);
    failed.readings.cpu_psi_some.value = Err(Reason::Io {
        message: "specific error context ".repeat(100),
    });
    app.observe(failed);
    assert!(!press(&mut app, KeyCode::Char('i')));
    let details = app.overlay.as_ref().unwrap();
    assert_eq!(details.kind, super::app::OverlayKind::Details);
    assert!(details.title.contains("sample_id=3"));
    assert!(
        details
            .text
            .contains(&"specific error context ".repeat(100))
    );
    assert!(details.text.contains("USER_HZ"));
    assert!(details.text.contains("cpu_contention_metaphor"));
    let text = details.text.clone();
    let history_size = app.history.len();
    app.observe(p3_observation("b", 4));
    assert_eq!(app.analysis.source_sample_id, 4);
    assert_eq!(app.history.len(), history_size + 1);
    assert_eq!(app.overlay.as_ref().unwrap().text, text);
    let out = render(&app, 80, 24);
    assert!(out.contains("Details | sample_id=3"));
    assert!(!out.contains("sample_id=4"));
    assert!(!press(&mut app, KeyCode::Esc));
    assert!(app.overlay.is_none());
    assert!(render(&app, 80, 24).contains("sample_id=4"));
}

#[test]
fn help_and_details_scroll_to_both_ends_and_resize_without_losing_access() {
    let mut app = p3_app("b");
    for key in ['?', 'i'] {
        app.overlay = None;
        assert!(!press(&mut app, KeyCode::Char(key)));
        for (width, height) in [(80, 24), (40, 24), (28, 9), (1, 1), (0, 0), (140, 34)] {
            app.resize(width, height);
            render(&app, width, height);
            for _ in 0..200 {
                press(&mut app, KeyCode::PageDown);
            }
            let overlay = app.overlay.as_ref().unwrap();
            assert_eq!(overlay.scroll, overlay.max_scroll(width, height));
            if width >= 40 {
                let last = render(&app, width, height);
                let visible: String = last
                    .chars()
                    .filter(|c| !c.is_whitespace() && *c != '│')
                    .collect();
                assert!(
                    visible.contains(if key == 'i' {
                        "タスク数ではない"
                    } else {
                        "小さい画面では簡略表示"
                    }),
                    "missing tail: {key} {width}x{height}\n{last}"
                );
            }
            press(&mut app, KeyCode::Up);
            let max = app.overlay.as_ref().unwrap().max_scroll(width, height);
            assert_eq!(app.overlay.as_ref().unwrap().scroll, max.saturating_sub(1));
            press(&mut app, KeyCode::Home);
            assert_eq!(app.overlay.as_ref().unwrap().scroll, 0);
        }
        // 同じキーで閉じる．
        press(&mut app, KeyCode::Char(key));
        assert!(app.overlay.is_none());
    }
}

#[test]
fn overlays_explain_metrics_and_keep_quit_available() {
    let mut app = p3_app("b");
    press(&mut app, KeyCode::Char('?'));
    let help = &app.overlay.as_ref().unwrap().text;
    for label in [
        "平滑化",
        "初回",
        "タスクの個数ではない",
        "system-view",
        "procs_running",
        "1Hz",
    ] {
        assert!(help.contains(label));
    }
    let mode = app.mode;
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.mode, mode);
    assert!(press(&mut app, KeyCode::Char('q')));
    press(&mut app, KeyCode::Char('i'));
    assert_eq!(
        app.overlay.as_ref().unwrap().kind,
        super::app::OverlayKind::Details
    );
    assert!(press(&mut app, KeyCode::Char('q')));
    assert!(app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL
    ))));
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
