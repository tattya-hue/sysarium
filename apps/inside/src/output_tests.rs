use crate::{Command, output, run_with};
use inside_core::{
    analysis::analyze,
    observation::{Observation, SYSTEM_VIEW, UnavailableReason as Reason},
};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    path::Path,
    time::{Duration, Instant},
};

pub(crate) fn fixture(id: u64) -> Observation {
    Observation {
        sample_id: id,
        sampled_at_unix_ms: 1000 * id,
        elapsed_since_previous: (id > 1).then_some(Duration::from_secs(1)),
        scope: SYSTEM_VIEW,
        readings: inside_linux::collector::collect_from_proc(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../crates/inside-linux/tests/fixtures/p1")
                .join(if id == 1 { "before" } else { "after" }),
        ),
    }
}

fn render(command: Command, modify: impl Fn(&mut Observation)) -> String {
    let mut out = Vec::new();
    let start = Instant::now();
    let pauses = Cell::new(0);
    run_with(
        command,
        &mut out,
        |id, previous| {
            assert_eq!(previous, if id == 1 { None } else { Some(start) });
            let mut observation = fixture(id);
            modify(&mut observation);
            Ok((observation, start + Duration::from_secs(id - 1)))
        },
        || pauses.set(pauses.get() + 1),
    )
    .unwrap();
    assert_eq!(pauses.get(), 1);
    String::from_utf8(out).unwrap()
}

#[test]
fn cli_json_is_one_document_with_versions_units_and_reproducible_evidence() {
    let text = render(Command::Json, |_| {});
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["sample_id"], 2);
    assert_eq!(value["sampled_at_unix_ms"], 2000);
    assert_eq!(value["scope"], SYSTEM_VIEW);
    assert_eq!(
        value["metrics"]["cpu_active_ratio"],
        json!({"status":"available","unit":"ratio","value":0.8})
    );
    assert_eq!(
        value["metrics"]["cpu_psi_some_interval_ratio"]["value"],
        0.25
    );
    assert_eq!(
        value["metrics"]["cpu_psi_some_avg10_pct"]["unit"],
        "percent"
    );
    assert_eq!(value["metrics"]["procs_running"]["value"], 4);
    assert_eq!(value["metrics"]["procs_running"]["unit"], "runnable_tasks");
    assert_eq!(value["evidence"]["previous"]["sample_id"], 1);
    assert_eq!(value["evidence"]["current"]["sample_id"], 2);
    assert_eq!(
        value["evidence"]["current"]["cpu_times"]["source"],
        "/proc/stat"
    );
    assert_eq!(value["evidence"]["cpu_delta"]["value"]["active_ticks"], 80);
    assert_eq!(value["evidence"]["cpu_delta"]["value"]["total_ticks"], 100);
    assert_eq!(
        value["evidence"]["psi_total_delta_us"]["unit"],
        "microseconds"
    );
    assert_eq!(value["evidence"]["psi_total_delta_us"]["value"], 250000);
    assert_eq!(
        value["evidence"]["elapsed"]["value"],
        json!({"secs":1,"nanos":0})
    );
    assert!(
        value["evidence"]["cpu_rule"]
            .as_str()
            .unwrap()
            .contains("guest")
    );
    assert!(
        value["evidence"]["psi_rule"]
            .as_str()
            .unwrap()
            .contains("monotonic")
    );
    assert_eq!(value["cpu_interpretation"]["activity"], "non_zero");
    assert_eq!(value["warnings"], json!([]));
}

#[test]
fn json_unavailable_is_null_with_nested_reason_and_valid_axis_survives() {
    for cause in [
        Reason::NotFound,
        Reason::PermissionDenied,
        Reason::Invalid {
            field: "avg10",
            value: "bad\n\"value".into(),
        },
    ] {
        let value: Value = serde_json::from_str(&render(Command::Json, |o| {
            if o.sample_id == 2 {
                o.readings.cpu_psi_some.value = Err(cause.clone());
            }
        }))
        .unwrap();
        let psi = &value["metrics"]["cpu_psi_some_interval_ratio"];
        assert_eq!(psi["status"], "unavailable");
        assert!(psi["value"].is_null());
        assert_eq!(psi["reason"]["code"], "input_unavailable");
        assert_eq!(psi["reason"]["sample_id"], 2);
        assert_eq!(psi["reason"]["source"], "/proc/pressure/cpu");
        assert_eq!(psi["reason"]["reason"], output::reason(&cause));
        assert_eq!(value["metrics"]["cpu_active_ratio"]["value"], 0.8);
        assert_eq!(value["cpu_interpretation"]["contention"], "unavailable");
        assert_eq!(value["warnings"].as_array().unwrap().len(), 2);
    }
}

#[test]
fn first_sample_output_does_not_claim_zero_activity() {
    let analysis = analyze(None, &fixture(1));
    let value: Value = serde_json::from_str(&output::json(&analysis).unwrap()).unwrap();
    assert_eq!(
        value["metrics"]["cpu_active_ratio"]["reason"]["code"],
        "first_sample"
    );
    assert_eq!(
        value["metrics"]["cpu_psi_some_interval_ratio"]["reason"]["code"],
        "first_sample"
    );
    assert!(value["metrics"]["cpu_active_ratio"]["value"].is_null());
    let text = output::text(&analysis, false);
    assert!(text.contains("CPU活動量（本プロジェクトの近似値）: N/A"));
    assert!(text.contains("CPU PSI some 区間割合: N/A"));
}

#[test]
fn status_and_inspect_share_metrics_and_inspect_explains_inputs() {
    let status = render(Command::Status, |_| {});
    let inspect = render(Command::Inspect, |_| {});
    assert!(inspect.starts_with(&status));
    assert!(status.contains("80.000 %"));
    assert!(status.contains("25.000 %"));
    assert!(status.contains("2.500 %"));
    assert!(status.contains("実行中を含む参考値"));
    for detail in [
        "previous 観測値",
        "current 観測値",
        "USER_HZ",
        "μs",
        "単調時計",
        "guest",
        "iowait",
        "steal",
        "原子的",
        "/proc/pressure/cpu",
    ] {
        assert!(inspect.contains(detail), "missing {detail}");
    }
    let text = render(Command::Status, |o| {
        if o.sample_id == 2 {
            o.readings.procs_running.value = Err(Reason::PermissionDenied);
        }
    });
    assert!(text.contains("N/A"));
    assert!(text.contains("permission_denied"));
}

#[test]
fn writer_failure_is_returned_and_help_never_samples() {
    for command in [
        Command::Help,
        Command::StatusHelp,
        Command::InspectHelp,
        Command::LiveHelp,
        Command::Version,
    ] {
        let mut out = Vec::new();
        run_with(
            command,
            &mut out,
            |_, _| panic!("help/version must not sample"),
            || panic!("help/version must not sleep"),
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains(if command == Command::Version {
            "inside 0.1.0"
        } else {
            "usage:"
        }));
    }
    let start = Instant::now();
    let error = run_with(
        Command::Json,
        &mut &mut [][..],
        |id, _| Ok((fixture(id), start)),
        || {},
    )
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::WriteZero);
}
