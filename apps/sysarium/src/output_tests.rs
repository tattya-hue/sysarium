use crate::output;
use inside_core::{
    analysis::analyze,
    observation::{Observation, SYSTEM_VIEW, UnavailableReason as Reason},
};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

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

fn render(inspect: bool, modify: impl Fn(&mut Observation)) -> String {
    let mut previous = fixture(1);
    let mut current = fixture(2);
    modify(&mut previous);
    modify(&mut current);
    let analysis = analyze(Some(&previous), &current);
    if inspect {
        output::details(&analysis)
    } else {
        output::json(&analysis).unwrap()
    }
}

#[test]
fn each_json_record_has_versions_units_and_reproducible_evidence() {
    let text = render(false, |_| {});
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
        let value: Value = serde_json::from_str(&render(false, |o| {
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
    let text = output::details(&analysis);
    assert!(text.contains("CPU usage    N/A"));
    assert!(text.contains("CPU wait     N/A"));
}

#[test]
fn live_details_explain_values_units_and_inputs() {
    let inspect = render(true, |_| {});
    assert!(inspect.contains("80.0%"));
    assert!(inspect.contains("25.0%"));
    assert!(inspect.contains("2.5%"));
    assert!(inspect.contains("Running + ready to run"));
    for detail in [
        "CPU usage",
        "CPU wait",
        "PSI: Linux's CPU-wait measure",
        "Time when tasks had to wait for CPU",
        "250.0 ms of the last 1.000 s",
        "Updated      Every 1.000 s",
        "/proc/pressure/cpu",
    ] {
        assert!(inspect.contains(detail), "missing {detail}");
    }
    for jargon in ["USER_HZ", "guest", "active_ticks", "total_ticks", "delta("] {
        assert!(!inspect.contains(jargon));
    }
    assert!(inspect.lines().count() <= 20);
    let text = render(true, |o| {
        if o.sample_id == 2 {
            o.readings.procs_running.value = Err(Reason::PermissionDenied);
        }
    });
    assert!(text.contains("N/A"));
    assert!(text.contains("Permission denied"));
    assert!(!text.contains("permission_denied"));
    for raw in ["source_sample_id", "total_us", "\"value\"", "Ok(", "Err("] {
        assert!(!text.contains(raw));
    }
}

#[test]
fn stream_emits_flushed_sequential_documents_and_propagates_writer_errors() {
    use std::{cell::Cell, io, time::Instant};
    let count = Cell::new(0);
    let origin = Instant::now() - Duration::from_secs(10);
    let mut output = Vec::new();
    crate::stream(
        &mut output,
        |id, previous| {
            assert_eq!(
                previous,
                (id > 1).then(|| origin + Duration::from_secs(id - 2))
            );
            count.set(count.get() + 1);
            Ok((fixture(id), origin + Duration::from_secs(id - 1)))
        },
        || panic!("fixture must not wait"),
        || count.get() == 3,
    )
    .unwrap();
    let text = String::from_utf8(output).unwrap();
    let values: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(values.len(), 3);
    for (i, value) in values.iter().enumerate() {
        assert_eq!(value["sample_id"], i + 1);
    }
    assert!(values[0]["metrics"]["cpu_active_ratio"]["value"].is_null());
    assert_eq!(values[1]["metrics"]["cpu_active_ratio"]["value"], 0.8);
    let error = crate::stream(
        &mut &mut [][..],
        |id, _| Ok((fixture(id), origin)),
        || {},
        || false,
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::WriteZero);
}

#[test]
fn wait_duration_uses_the_observed_window_and_never_fills_missing_with_zero() {
    let text = render(true, |o| {
        if o.sample_id == 2 {
            o.elapsed_since_previous = Some(Duration::from_millis(500));
        }
    });
    assert!(text.contains("CPU wait     50.0%"));
    assert!(text.contains("250.0 ms of the last 0.500 s"));
    let baseline = fixture(1).readings.cpu_psi_some.value.unwrap().total_us;
    let zero = render(true, |o| {
        o.readings.cpu_psi_some.value.as_mut().unwrap().total_us = baseline
    });
    assert!(zero.contains("Wait time  0.0 ms"));
    let first = output::details(&analyze(None, &fixture(1)));
    assert!(!first.contains("Wait time"));
    for cause in [Reason::NotFound, Reason::PermissionDenied] {
        let missing = render(true, |o| {
            if o.sample_id == 2 {
                o.readings.cpu_psi_some.value = Err(cause.clone());
            }
        });
        assert!(missing.contains("CPU wait     N/A"));
        assert!(!missing.contains("Wait time"));
    }
    let invalid = render(true, |o| {
        if o.sample_id == 2 {
            o.readings.cpu_psi_some.value.as_mut().unwrap().total_us = baseline + 2_000_000;
        }
    });
    assert!(invalid.contains("CPU wait     N/A"));
    assert!(!invalid.contains("Wait time"));
}
