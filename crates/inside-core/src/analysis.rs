use std::time::Duration;

use crate::observation::{CpuTimes, Observation, Sourced, UnavailableReason as Reason};

pub const CPU_RULE: &str = "delta(user+nice+system+irq+softirq) / delta(user+nice+system+idle+iowait+irq+softirq+steal); guest/guest_nice excluded";
pub const PSI_RULE: &str = "delta(some.total_us) / monotonic_elapsed_us; no clamping";
pub const SCOPE_WARNING: &str =
    "system-viewは可視の/procの範囲．コンテナやVMでは物理ホスト全体と一致するとは限らない．";
pub const METRIC_WARNING: &str = "CPU活動量は本プロジェクトの近似値．PSIは停滞時間割合，procs_runningは実行中を含むrunnable参考値であり待機タスク数ではない．";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuDelta {
    pub counters: CpuTimes,
    pub active_ticks: u64,
    pub total_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalState {
    Unavailable,
    Zero,
    NonZero,
}

/// 活動と競合の独立した2軸．運用上の正常・異常しきい値は設けない．
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuInterpretation {
    pub activity: SignalState,
    pub contention: SignalState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub metric: &'static str,
    pub reason: Reason,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Evidence {
    pub previous: Option<Observation>,
    pub current: Observation,
    pub elapsed: Result<Duration, Reason>,
    pub cpu_delta: Result<CpuDelta, Reason>,
    pub psi_total_delta_us: Result<u64, Reason>,
    pub cpu_rule: &'static str,
    pub psi_rule: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    pub source_sample_id: u64,
    pub cpu_active_ratio: Result<f64, Reason>,
    pub cpu_psi_some_interval_ratio: Result<f64, Reason>,
    pub cpu_psi_some_avg10_pct: Result<f64, Reason>,
    pub cpu_interpretation: CpuInterpretation,
    pub warnings: Vec<Warning>,
    pub evidence: Evidence,
}

/// 純粋な2観測の解析．欠測後は直前サンプルを使い，古い正常値まで遡らない．
pub fn analyze(previous: Option<&Observation>, current: &Observation) -> Analysis {
    let elapsed = interval(previous, current);
    let cpu_delta = cpu_delta(previous, current, &elapsed);
    let cpu_active_ratio = cpu_delta
        .as_ref()
        .map(|d| d.active_ticks as f64 / d.total_ticks as f64)
        .map_err(Clone::clone);
    let psi_total_delta_us = psi_delta(previous, current, &elapsed);
    let cpu_psi_some_interval_ratio = psi_total_delta_us.clone().and_then(|delta| {
        let elapsed = elapsed.as_ref().map_err(Clone::clone)?;
        // 整数のnsへ揃えてから浮動小数点化し，境界の1も丸め込みなしで保持する．
        Ok((u128::from(delta) * 1000) as f64 / elapsed.as_nanos() as f64)
    });
    let cpu_psi_some_avg10_pct = input(&current.readings.cpu_psi_some, current.sample_id)
        .and_then(|psi| percentage(psi.avg10, "avg10"));
    let mut warnings = Vec::new();
    for (metric, value) in [
        ("cpu_active_ratio", &cpu_active_ratio),
        ("cpu_psi_some_interval_ratio", &cpu_psi_some_interval_ratio),
        ("cpu_psi_some_avg10_pct", &cpu_psi_some_avg10_pct),
    ] {
        if let Err(reason) = value {
            warnings.push(Warning {
                metric,
                reason: reason.clone(),
            });
        }
    }
    if let Err(reason) = input(&current.readings.procs_running, current.sample_id) {
        warnings.push(Warning {
            metric: "procs_running",
            reason,
        });
    }
    Analysis {
        source_sample_id: current.sample_id,
        cpu_interpretation: CpuInterpretation {
            activity: signal(&cpu_active_ratio),
            contention: signal(&cpu_psi_some_interval_ratio),
        },
        cpu_active_ratio,
        cpu_psi_some_interval_ratio,
        cpu_psi_some_avg10_pct,
        warnings,
        evidence: Evidence {
            previous: previous.cloned(),
            current: current.clone(),
            elapsed,
            cpu_delta,
            psi_total_delta_us,
            cpu_rule: CPU_RULE,
            psi_rule: PSI_RULE,
        },
    }
}

fn signal(value: &Result<f64, Reason>) -> SignalState {
    match value {
        Ok(value) if *value == 0.0 => SignalState::Zero,
        Ok(_) => SignalState::NonZero,
        Err(_) => SignalState::Unavailable,
    }
}

fn input<T>(value: &Sourced<T>, sample_id: u64) -> Result<&T, Reason> {
    value
        .value
        .as_ref()
        .map_err(|reason| Reason::InputUnavailable {
            sample_id,
            source: value.source,
            reason: Box::new(reason.clone()),
        })
}

fn pair<'a, T>(
    previous: Option<&'a Observation>,
    current: &'a Observation,
    select: impl Fn(&'a Observation) -> &'a Sourced<T>,
) -> Result<(&'a T, &'a T), Reason> {
    let now = input(select(current), current.sample_id)?;
    let previous = previous.ok_or(Reason::FirstSample)?;
    let before = input(select(previous), previous.sample_id)?;
    if select(previous).source != select(current).source {
        return Err(Reason::SourceChanged);
    }
    Ok((before, now))
}

fn interval(previous: Option<&Observation>, current: &Observation) -> Result<Duration, Reason> {
    let previous = previous.ok_or(Reason::FirstSample)?;
    if current.sample_id <= previous.sample_id {
        return Err(Reason::InvalidSampleOrder);
    }
    if current.scope != previous.scope {
        return Err(Reason::ScopeChanged);
    }
    let elapsed = current
        .elapsed_since_previous
        .ok_or(Reason::MissingInterval)?;
    if elapsed.is_zero() {
        return Err(Reason::ZeroInterval);
    }
    Ok(elapsed)
}

fn cpu_delta(
    previous: Option<&Observation>,
    current: &Observation,
    elapsed: &Result<Duration, Reason>,
) -> Result<CpuDelta, Reason> {
    let (before, now) = pair(previous, current, |o| &o.readings.cpu_times)?;
    elapsed.as_ref().map_err(Clone::clone)?;
    let delta = |now: u64, before: u64, field| {
        now.checked_sub(before)
            .ok_or(Reason::CounterRegression { field })
    };
    let counters = CpuTimes {
        user: delta(now.user, before.user, "user")?,
        nice: delta(now.nice, before.nice, "nice")?,
        system: delta(now.system, before.system, "system")?,
        idle: delta(now.idle, before.idle, "idle")?,
        iowait: delta(now.iowait, before.iowait, "iowait")?,
        irq: delta(now.irq, before.irq, "irq")?,
        softirq: delta(now.softirq, before.softirq, "softirq")?,
        steal: delta(now.steal, before.steal, "steal")?,
    };
    let active_ticks = sum(&[
        counters.user,
        counters.nice,
        counters.system,
        counters.irq,
        counters.softirq,
    ])?;
    let total_ticks = sum(&[active_ticks, counters.idle, counters.iowait, counters.steal])?;
    if total_ticks == 0 {
        return Err(Reason::ZeroCpuDelta);
    }
    Ok(CpuDelta {
        counters,
        active_ticks,
        total_ticks,
    })
}

fn sum(values: &[u64]) -> Result<u64, Reason> {
    values.iter().try_fold(0_u64, |acc, value| {
        acc.checked_add(*value).ok_or(Reason::ArithmeticOverflow)
    })
}

fn psi_delta(
    previous: Option<&Observation>,
    current: &Observation,
    elapsed: &Result<Duration, Reason>,
) -> Result<u64, Reason> {
    let (before, now) = pair(previous, current, |o| &o.readings.cpu_psi_some)?;
    // Core APIへの直接入力も検証し，パーサだけに正当性を委ねない．
    for psi in [before, now] {
        percentage(psi.avg10, "avg10")?;
        percentage(psi.avg60, "avg60")?;
        percentage(psi.avg300, "avg300")?;
    }
    let elapsed = elapsed.as_ref().map_err(Clone::clone)?;
    let delta = now
        .total_us
        .checked_sub(before.total_us)
        .ok_or(Reason::CounterRegression {
            field: "some.total_us",
        })?;
    if u128::from(delta) * 1000 > elapsed.as_nanos() {
        return Err(Reason::OutOfRange {
            field: "cpu_psi_some_interval_ratio",
        });
    }
    Ok(delta)
}

fn percentage(value: f64, field: &'static str) -> Result<f64, Reason> {
    if !value.is_finite() || !(0.0..=100.0).contains(&value) {
        return Err(Reason::OutOfRange { field });
    }
    Ok(value)
}
