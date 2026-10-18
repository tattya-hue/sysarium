use inside_core::observation::{CpuPsiSome, CpuTimes, UnavailableReason as Reason};
use inside_linux::{collector::collect_from_proc, cpu::parse_proc_stat, psi::parse_cpu_psi_some};
use std::path::Path;

const STAT: &str = include_str!("fixtures/stat.txt");
const PSI: &str = include_str!("fixtures/psi.txt");

#[test]
fn aggregate_cpu_and_runnable_with_extra_fields() {
    let expected = CpuTimes {
        user: 100,
        nice: 2,
        system: 30,
        idle: 400,
        iowait: 5,
        irq: 6,
        softirq: 7,
        steal: 8,
    };
    for input in [STAT, include_str!("fixtures/stat-extra.txt")] {
        let parsed = parse_proc_stat(input);
        assert_eq!(parsed.cpu_times, Ok(expected));
        assert_eq!(parsed.procs_running, Ok(3));
    }
}

#[test]
fn stat_missing_short_and_invalid_are_independent() {
    let parsed = parse_proc_stat(include_str!("fixtures/stat-missing.txt"));
    assert_eq!(parsed.cpu_times, Err(Reason::Missing { field: "cpu" }));
    assert_eq!(
        parsed.procs_running,
        Err(Reason::Missing {
            field: "procs_running"
        })
    );
    let parsed = parse_proc_stat(include_str!("fixtures/stat-short.txt"));
    assert_eq!(parsed.cpu_times, Err(Reason::Missing { field: "steal" }));
    assert_eq!(parsed.procs_running, Ok(3));
    let parsed = parse_proc_stat(include_str!("fixtures/stat-invalid.txt"));
    assert_eq!(
        parsed.cpu_times,
        Err(Reason::Invalid {
            field: "system",
            value: "broken".into()
        })
    );
    assert_eq!(parsed.procs_running, Ok(3));
    for line in ["procs_running", "procs_running nope"] {
        let parsed = parse_proc_stat(&format!("{STAT}\n{line}").replace("procs_running 3\n", ""));
        assert!(parsed.cpu_times.is_ok());
        assert!(parsed.procs_running.is_err());
    }
}

#[test]
fn stat_rejects_negative_signed_overflow_and_invalid_integers() {
    for value in ["-1", "+1", "18446744073709551616", "1.5", "NaN"] {
        let parsed = parse_proc_stat(&format!("cpu {value} 0 0 0 0 0 0 0\nprocs_running {value}"));
        assert_eq!(
            parsed.cpu_times,
            Err(Reason::Invalid {
                field: "user",
                value: value.into()
            })
        );
        assert_eq!(
            parsed.procs_running,
            Err(Reason::Invalid {
                field: "procs_running",
                value: value.into()
            })
        );
    }
    let parsed = parse_proc_stat("cpu 18446744073709551615 0 0 0 0 0 0 0\nprocs_running 0");
    assert_eq!(parsed.cpu_times.unwrap().user, u64::MAX);
    assert_eq!(parsed.procs_running, Ok(0));
}

#[test]
fn duplicate_records_are_not_silently_selected() {
    let parsed = parse_proc_stat(&format!("{STAT}{STAT}"));
    assert_eq!(parsed.cpu_times, Err(Reason::Duplicate { field: "cpu" }));
    assert_eq!(
        parsed.procs_running,
        Err(Reason::Duplicate {
            field: "procs_running"
        })
    );
    assert_eq!(
        parse_cpu_psi_some(&format!("{PSI}{PSI}")),
        Err(Reason::Duplicate { field: "some" })
    );
}

#[test]
fn psi_some_supports_reordering_and_extra_fields() {
    let expected = CpuPsiSome {
        avg10: 1.25,
        avg60: 2.5,
        avg300: 3.75,
        total_us: 123456,
    };
    for input in [PSI, include_str!("fixtures/psi-extra.txt")] {
        assert_eq!(parse_cpu_psi_some(input), Ok(expected));
    }
}

#[test]
fn psi_missing_short_and_invalid() {
    assert_eq!(
        parse_cpu_psi_some(include_str!("fixtures/psi-missing.txt")),
        Err(Reason::Missing { field: "some" })
    );
    assert_eq!(
        parse_cpu_psi_some(include_str!("fixtures/psi-short.txt")),
        Err(Reason::Missing { field: "avg300" })
    );
    assert_eq!(
        parse_cpu_psi_some(include_str!("fixtures/psi-invalid.txt")),
        Err(Reason::Invalid {
            field: "avg10",
            value: "broken".into()
        })
    );
    for field in ["avg10", "avg60", "avg300", "total"] {
        let input = PSI
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .filter(|token| !token.starts_with(&format!("{field}=")))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(parse_cpu_psi_some(&input), Err(Reason::Missing { field }));
        assert_eq!(
            parse_cpu_psi_some(&format!(
                "some {field}=1 {}",
                PSI.trim_start_matches("some ")
            )),
            Err(Reason::Duplicate { field })
        );
    }
}

#[test]
fn psi_rejects_nonfinite_out_of_range_and_malformed_values() {
    for field in ["avg10", "avg60", "avg300"] {
        for value in ["NaN", "inf", "-0.1", "100.01", "1e1", "+1", "1.2.3"] {
            let input = "some avg10=0 avg60=0 avg300=0 total=0"
                .to_owned()
                .replace(&format!("{field}=0"), &format!("{field}={value}"));
            assert_eq!(
                parse_cpu_psi_some(&input),
                Err(Reason::Invalid {
                    field,
                    value: value.into()
                })
            );
        }
    }
    for value in ["-1", "+1", "1.5", "18446744073709551616"] {
        assert_eq!(
            parse_cpu_psi_some(&format!("some avg10=0 avg60=0 avg300=0 total={value}")),
            Err(Reason::Invalid {
                field: "total",
                value: value.into()
            })
        );
    }
    for token in ["garbage", "=1", "future=", "future=a=b"] {
        assert_eq!(
            parse_cpu_psi_some(&format!("some avg10=0 avg60=0 avg300=0 total=0 {token}")),
            Err(Reason::Invalid {
                field: "some",
                value: token.into()
            })
        );
    }
}

#[test]
fn zero_and_boundary_values_remain_available() {
    let parsed =
        parse_cpu_psi_some("some avg10=0.00 avg60=100.00 avg300=0 total=18446744073709551615")
            .unwrap();
    assert_eq!(parsed.avg10, 0.0);
    assert_eq!(parsed.avg60, 100.0);
    assert_eq!(parsed.total_us, u64::MAX);
    assert_eq!(
        parse_proc_stat("cpu 0 0 0 0 0 0 0 0\nprocs_running 0").procs_running,
        Ok(0)
    );
}

#[test]
fn empty_files_are_missing_not_zero() {
    for input in ["", " \n\t"] {
        assert_eq!(
            parse_proc_stat(input).cpu_times,
            Err(Reason::Missing { field: "cpu" })
        );
        assert_eq!(
            parse_proc_stat(input).procs_running,
            Err(Reason::Missing {
                field: "procs_running"
            })
        );
        assert_eq!(
            parse_cpu_psi_some(input),
            Err(Reason::Missing { field: "some" })
        );
    }
}

#[test]
fn missing_psi_file_does_not_discard_stat() {
    let result = collect_from_proc(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/proc-without-psi"),
    );
    assert!(result.cpu_times.value.is_ok());
    assert_eq!(result.procs_running.value, Ok(3));
    assert_eq!(result.cpu_psi_some.value, Err(Reason::NotFound));
}
