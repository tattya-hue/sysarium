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
    for args in [vec!["--help"], vec!["live", "--help"]] {
        let result = inside().args(args).output().unwrap();
        assert!(result.status.success());
        assert!(String::from_utf8(result.stdout).unwrap().contains("usage:"));
        assert!(result.stderr.is_empty());
    }
}

#[test]
fn process_stream_is_line_delimited_and_broken_pipe_exits_cleanly() {
    use std::{
        io::{BufRead, BufReader},
        process::Stdio,
    };
    let mut child = inside()
        .arg("--json")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    for id in 1..=2 {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["sample_id"], id);
        assert_eq!(value["metrics"]["cpu_active_ratio"]["unit"], "ratio");
        if id == 1 {
            assert!(value["metrics"]["cpu_active_ratio"]["value"].is_null());
        }
    }
    drop(reader);
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    assert!(result.stderr.is_empty());
}

#[test]
fn invalid_options_and_non_tty_live_report_errors_only_on_stderr() {
    for (args, code) in [
        (vec!["status"], 2),
        (vec!["inspect", "cpu"], 2),
        (vec!["--unknown"], 2),
        (vec!["live"], 1),
        (vec![], 1),
    ] {
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
