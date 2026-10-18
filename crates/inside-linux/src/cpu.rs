use inside_core::observation::{CpuTimes, UnavailableReason};

use crate::{record, unsigned};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcStat {
    pub cpu_times: Result<CpuTimes, UnavailableReason>,
    pub procs_running: Result<u64, UnavailableReason>,
}

/// aggregate cpuのみを読む．cpuN・無関係な行・既知項目より後の値は無視する．
/// 先頭8項目の欠損は0で補完しない．各レコードのエラーは独立して返す．
pub fn parse_proc_stat(input: &str) -> ProcStat {
    ProcStat {
        cpu_times: parse_cpu_times(input),
        procs_running: record(input, "procs_running").and_then(|line| {
            unsigned(
                line.split_whitespace()
                    .nth(1)
                    .ok_or(UnavailableReason::Missing {
                        field: "procs_running",
                    })?,
                "procs_running",
            )
        }),
    }
}

fn parse_cpu_times(input: &str) -> Result<CpuTimes, UnavailableReason> {
    let mut fields = record(input, "cpu")?.split_whitespace().skip(1);
    let mut next = |field| {
        unsigned(
            fields.next().ok_or(UnavailableReason::Missing { field })?,
            field,
        )
    };
    Ok(CpuTimes {
        user: next("user")?,
        nice: next("nice")?,
        system: next("system")?,
        idle: next("idle")?,
        iowait: next("iowait")?,
        irq: next("irq")?,
        softirq: next("softirq")?,
        steal: next("steal")?,
    })
}
