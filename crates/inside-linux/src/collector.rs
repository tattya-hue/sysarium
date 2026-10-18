use std::{fs, io, path::Path};

use inside_core::observation::{RawCpuReadings, Sourced, UnavailableReason};

use crate::{cpu::parse_proc_stat, psi::parse_cpu_psi_some};

pub const STAT_SOURCE: &str = "/proc/stat";
pub const PSI_SOURCE: &str = "/proc/pressure/cpu";

/// システムから生データを1回取得する．時刻付与や状態計算は行わない．
pub fn collect() -> RawCpuReadings {
    collect_from_proc(Path::new("/proc"))
}

/// fixture用にprocルートを差し替えられる最小の読込API．
/// 一方のファイルの失敗によって他方の値を失わない．
/// sourceはproc内の論理的な取得元を示す．
pub fn collect_from_proc(proc_root: &Path) -> RawCpuReadings {
    collect_with(|relative| fs::read_to_string(proc_root.join(relative)))
}

fn collect_with(mut read: impl FnMut(&str) -> io::Result<String>) -> RawCpuReadings {
    let (cpu_times, procs_running) = match read("stat") {
        Ok(input) => {
            let stat = parse_proc_stat(&input);
            (stat.cpu_times, stat.procs_running)
        }
        Err(error) => {
            let reason = read_error(error);
            (Err(reason.clone()), Err(reason))
        }
    };
    let cpu_psi_some = read("pressure/cpu")
        .map_err(read_error)
        .and_then(|input| parse_cpu_psi_some(&input));
    RawCpuReadings {
        cpu_times: Sourced {
            source: STAT_SOURCE,
            value: cpu_times,
        },
        procs_running: Sourced {
            source: STAT_SOURCE,
            value: procs_running,
        },
        cpu_psi_some: Sourced {
            source: PSI_SOURCE,
            value: cpu_psi_some,
        },
    }
}

fn read_error(error: io::Error) -> UnavailableReason {
    match error.kind() {
        io::ErrorKind::NotFound => UnavailableReason::NotFound,
        io::ErrorKind::PermissionDenied => UnavailableReason::PermissionDenied,
        _ => UnavailableReason::Io {
            message: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_failures_are_explicit_and_independent() {
        for (kind, expected) in [
            (io::ErrorKind::NotFound, UnavailableReason::NotFound),
            (
                io::ErrorKind::PermissionDenied,
                UnavailableReason::PermissionDenied,
            ),
            (
                io::ErrorKind::Other,
                UnavailableReason::Io {
                    message: "fixture failure".into(),
                },
            ),
            (
                io::ErrorKind::InvalidData,
                UnavailableReason::Io {
                    message: "fixture failure".into(),
                },
            ),
        ] {
            for failed_file in ["stat", "pressure/cpu"] {
                let result = collect_with(|path| {
                    if path == failed_file {
                        Err(io::Error::new(kind, "fixture failure"))
                    } else if path == "stat" {
                        Ok(include_str!("../tests/fixtures/stat.txt").into())
                    } else {
                        Ok(include_str!("../tests/fixtures/psi.txt").into())
                    }
                });
                assert_eq!(result.cpu_times.source, STAT_SOURCE);
                assert_eq!(result.procs_running.source, STAT_SOURCE);
                assert_eq!(result.cpu_psi_some.source, PSI_SOURCE);
                if failed_file == "stat" {
                    assert_eq!(result.cpu_times.value, Err(expected.clone()));
                    assert_eq!(result.procs_running.value, Err(expected.clone()));
                    assert!(result.cpu_psi_some.value.is_ok());
                } else {
                    assert!(result.cpu_times.value.is_ok());
                    assert!(result.procs_running.value.is_ok());
                    assert_eq!(result.cpu_psi_some.value, Err(expected.clone()));
                }
            }
        }
    }
}
