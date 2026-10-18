use std::process::Command;

fn inside() -> Command {
    Command::new(env!("CARGO_BIN_EXE_inside"))
}

#[test]
fn version_and_all_help_paths_work_without_a_terminal() {
    let version = inside().arg("--version").output().unwrap();
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout).unwrap(),
        format!("inside {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(version.stderr.is_empty());
    for args in [
        vec!["--help"],
        vec!["status", "--help"],
        vec!["inspect", "--help"],
        vec!["inspect", "cpu", "-h"],
        vec!["live", "--help"],
    ] {
        let result = inside().args(args).output().unwrap();
        assert!(result.status.success());
        assert!(String::from_utf8(result.stdout).unwrap().contains("usage:"));
        assert!(result.stderr.is_empty());
    }
}

#[test]
fn process_stdout_contains_exactly_one_json_document() {
    let result = inside().args(["status", "--json"]).output().unwrap();
    assert!(result.status.success());
    assert!(result.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["sample_id"], 2);
    assert_eq!(value["metrics"]["cpu_active_ratio"]["unit"], "ratio");
    for name in ["cpu_active_ratio", "cpu_psi_some_interval_ratio"] {
        let metric = &value["metrics"][name];
        if metric["status"] == "available" {
            assert!((0.0..=1.0).contains(&metric["value"].as_f64().unwrap()));
        } else {
            assert_eq!(metric["status"], "unavailable");
            assert!(metric["value"].is_null());
            assert!(metric["reason"].is_object());
        }
    }
}

#[test]
fn invalid_options_and_non_tty_live_report_errors_only_on_stderr() {
    for (args, code) in [(vec!["status", "--unknown"], 2), (vec!["live"], 1)] {
        let result = inside().args(args).output().unwrap();
        assert_eq!(result.status.code(), Some(code));
        assert!(result.stdout.is_empty());
        assert!(!result.stderr.is_empty());
    }
}

#[cfg(unix)]
#[test]
fn invalid_utf8_arguments_are_a_usage_error_instead_of_a_panic() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let result = inside()
        .arg(OsString::from_vec(vec![0xff]))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    let error = String::from_utf8(result.stderr).unwrap();
    assert!(error.contains("UTF-8"));
    assert!(!error.contains("panicked"));
}
