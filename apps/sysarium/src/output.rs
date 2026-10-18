use std::{fmt::Write, io};

use inside_core::{
    analysis::{Analysis, SignalState},
    observation::{CpuTimes, Observation, Sourced, UnavailableReason as Reason},
};
use serde_json::{Value, json};

pub fn reason(reason: &Reason) -> Value {
    match reason {
        Reason::FirstSample => json!({"code":"first_sample"}),
        Reason::MissingInterval => json!({"code":"missing_interval"}),
        Reason::ZeroInterval => json!({"code":"zero_interval"}),
        Reason::ZeroCpuDelta => json!({"code":"zero_cpu_delta"}),
        Reason::CounterRegression { field } => json!({"code":"counter_regression","field":field}),
        Reason::ArithmeticOverflow => json!({"code":"arithmetic_overflow"}),
        Reason::OutOfRange { field } => json!({"code":"out_of_range","field":field}),
        Reason::InvalidSampleOrder => json!({"code":"invalid_sample_order"}),
        Reason::ScopeChanged => json!({"code":"scope_changed"}),
        Reason::SourceChanged => json!({"code":"source_changed"}),
        Reason::InputUnavailable {
            sample_id,
            source,
            reason: inner,
        } => json!({
            "code":"input_unavailable", "sample_id":sample_id, "source":source, "reason":self::reason(inner)
        }),
        Reason::NotFound => json!({"code":"not_found"}),
        Reason::PermissionDenied => json!({"code":"permission_denied"}),
        Reason::Io { message } => json!({"code":"io","message":message}),
        Reason::Missing { field } => json!({"code":"missing","field":field}),
        Reason::Invalid { field, value } => json!({"code":"invalid","field":field,"value":value}),
        Reason::Duplicate { field } => json!({"code":"duplicate","field":field}),
    }
}

fn metric<T>(value: &Result<T, Reason>, unit: &str, convert: impl FnOnce(&T) -> Value) -> Value {
    match value {
        Ok(value) => json!({"status":"available", "value":convert(value), "unit":unit}),
        Err(error) => {
            json!({"status":"unavailable", "value":null, "unit":unit, "reason":reason(error)})
        }
    }
}

fn sourced<T>(value: &Sourced<T>, unit: &str, convert: impl FnOnce(&T) -> Value) -> Value {
    let mut metric = metric(&value.value, unit, convert);
    metric["source"] = json!(value.source);
    metric
}

fn cpu(value: &CpuTimes) -> Value {
    json!({"user":value.user, "nice":value.nice, "system":value.system, "idle":value.idle,
        "iowait":value.iowait, "irq":value.irq, "softirq":value.softirq, "steal":value.steal})
}

fn observation(value: &Observation) -> Value {
    json!({
        "sample_id":value.sample_id, "sampled_at_unix_ms":value.sampled_at_unix_ms,
        "scope":value.scope,
        "elapsed_since_previous": value.elapsed_since_previous.map(|d| json!({"secs":d.as_secs(), "nanos":d.subsec_nanos()})),
        "cpu_times":sourced(&value.readings.cpu_times, "USER_HZ_ticks", cpu),
        "cpu_psi_some":sourced(&value.readings.cpu_psi_some, "avg:percent,total_us:microseconds", |p| json!({"avg10":p.avg10,"avg60":p.avg60,"avg300":p.avg300,"total_us":p.total_us})),
        "procs_running":sourced(&value.readings.procs_running, "runnable_tasks", |v| json!(v)),
    })
}

/// 安定したv1 JSONエンベロープ．Coreに表示・シリアライズの依存を持ち込まない．
pub fn json(analysis: &Analysis) -> io::Result<String> {
    let evidence = &analysis.evidence;
    let value = json!({
        "schema_version":1, "version":env!("CARGO_PKG_VERSION"),
        "sample_id":analysis.source_sample_id,
        "sampled_at_unix_ms":evidence.current.sampled_at_unix_ms,
        "scope":evidence.current.scope,
        "metrics": {
            "cpu_active_ratio":metric(&analysis.cpu_active_ratio, "ratio", |v| json!(v)),
            "cpu_psi_some_interval_ratio":metric(&analysis.cpu_psi_some_interval_ratio, "ratio", |v| json!(v)),
            "cpu_psi_some_avg10_pct":metric(&analysis.cpu_psi_some_avg10_pct, "percent", |v| json!(v)),
            "procs_running":sourced(&evidence.current.readings.procs_running, "runnable_tasks", |v| json!(v)),
        },
        "cpu_interpretation": {
            "activity":signal(analysis.cpu_interpretation.activity),
            "contention":signal(analysis.cpu_interpretation.contention),
            "rule":"independent axes: zero / non_zero / unavailable; no health thresholds",
        },
        "evidence": {
            "previous":evidence.previous.as_ref().map(observation), "current":observation(&evidence.current),
            "elapsed":metric(&evidence.elapsed, "seconds_and_nanoseconds", |d| json!({"secs":d.as_secs(),"nanos":d.subsec_nanos()})),
            "cpu_delta":metric(&evidence.cpu_delta, "USER_HZ_ticks", |d| json!({"counters":cpu(&d.counters),"active_ticks":d.active_ticks,"total_ticks":d.total_ticks})),
            "psi_total_delta_us":metric(&evidence.psi_total_delta_us, "microseconds", |d| json!(d)),
            "cpu_rule":evidence.cpu_rule, "psi_rule":evidence.psi_rule,
            "clock":"monotonic; wall clock is display only",
            "sampling":"sequential proc reads, timestamps at collection start; not an atomic snapshot",
        },
        "warnings":analysis.warnings.iter().map(|w| json!({"metric":w.metric,"reason":reason(&w.reason)})).collect::<Vec<_>>(),
        "limitations":["Visible /proc scope; containers may differ from the physical host.", "CPU usage excludes I/O wait and steal. CPU wait is PSI some time, not a task count."],
    });
    serde_json::to_string(&value).map_err(io::Error::other)
}

fn display(value: &Result<f64, Reason>, multiplier: f64) -> String {
    value
        .as_ref()
        .map_or("N/A".into(), |v| format!("{:.1}%", v * multiplier))
}

/// 人が読む詳細．生のJSONやRustの内部型はNDJSON出力に残す．
pub fn details(a: &Analysis) -> String {
    let e = &a.evidence;
    let mut out = format!(
        "CPU usage    {}\n  CPU time doing work (approx.).\n",
        display(&a.cpu_active_ratio, 100.0)
    );
    if let Err(reason) = &a.cpu_active_ratio {
        writeln!(out, "  {}", explain(reason)).unwrap();
    }
    writeln!(out,"\nCPU wait     {}\n  Time with any task waiting for CPU.\n  PSI = Pressure Stall Information.\n  Usage and wait can both be high.\n  10s trend  {}  (smoothed wait)",display(&a.cpu_psi_some_interval_ratio,100.0),display(&a.cpu_psi_some_avg10_pct,1.0)).unwrap();
    if let Err(reason) = &a.cpu_psi_some_interval_ratio {
        writeln!(out, "  {}", explain(reason)).unwrap();
    }
    if let Err(reason) = &a.cpu_psi_some_avg10_pct
        && a.cpu_psi_some_interval_ratio.as_ref().err() != Some(reason)
    {
        writeln!(out, "  Trend: {}", explain(reason)).unwrap();
    }
    match &e.current.readings.procs_running.value {
        Ok(v) => writeln!(out, "\nRunnable     {v}\n  Running + ready tasks.").unwrap(),
        Err(reason) => writeln!(out, "\nRunnable     N/A\n  {}", explain(reason)).unwrap(),
    }
    match &e.elapsed {
        Ok(d) => writeln!(
            out,
            "\nWindow       {:.3} s · Time between the last two readings.",
            d.as_secs_f64()
        )
        .unwrap(),
        Err(reason) => writeln!(out, "\nWindow       N/A · {}", explain(reason)).unwrap(),
    }
    writeln!(
        out,
        "Sources      {} · {}\nRaw data     sysarium --json",
        e.current.readings.cpu_times.source, e.current.readings.cpu_psi_some.source
    )
    .unwrap();
    out
}

fn explain(reason: &Reason) -> String {
    match reason {
        Reason::FirstSample => "Warming up; waiting for the next reading.".into(),
        Reason::MissingInterval => "Time between readings is unavailable.".into(),
        Reason::ZeroInterval => "Readings are too close together.".into(),
        Reason::ZeroCpuDelta => "CPU counters have not advanced.".into(),
        Reason::InputUnavailable {
            sample_id, reason, ..
        } => format!("Sample #{sample_id}: {}", explain(reason)),
        Reason::NotFound => "Source missing; this metric may be unsupported.".into(),
        Reason::PermissionDenied => "Permission denied reading the source.".into(),
        Reason::Io { message } => {
            let short: String = message
                .chars()
                .take(72)
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect();
            format!(
                "Read failed: {short}{}",
                if message.chars().count() > 72 {
                    "… (full error: --json)"
                } else {
                    ""
                }
            )
        }
        Reason::Missing { field } => format!("Missing field: {field}."),
        Reason::Invalid { field, .. } => format!("Invalid value: {field}."),
        Reason::Duplicate { field } => format!("Duplicate field: {field}."),
        Reason::CounterRegression { field } => {
            format!("Counter reset: {field}; waiting for fresh readings.")
        }
        Reason::ArithmeticOverflow => "Counter overflow; cannot calculate this reading.".into(),
        Reason::OutOfRange { field } => format!("Value out of range: {field}."),
        Reason::InvalidSampleOrder => "Readings are out of order.".into(),
        Reason::ScopeChanged => "Scope changed; waiting for fresh readings.".into(),
        Reason::SourceChanged => "Source changed; waiting for fresh readings.".into(),
    }
}

fn signal(state: SignalState) -> &'static str {
    match state {
        SignalState::Zero => "zero",
        SignalState::NonZero => "non_zero",
        SignalState::Unavailable => "unavailable",
    }
}
