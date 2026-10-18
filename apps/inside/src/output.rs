use std::{fmt::Write, io};

use inside_core::{
    analysis::{Analysis, METRIC_WARNING, SCOPE_WARNING, SignalState},
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
        "limitations":[SCOPE_WARNING, METRIC_WARNING],
    });
    serde_json::to_string(&value).map_err(io::Error::other)
}

fn display(value: &Result<f64, Reason>, multiplier: f64, unit: &str) -> String {
    match value {
        Ok(value) => format!("{:.3} {unit}", value * multiplier),
        Err(error) => format!("N/A ({})", reason(error)),
    }
}

pub fn text(analysis: &Analysis, inspect: bool) -> String {
    let evidence = &analysis.evidence;
    let current = &evidence.current;
    let mut out = format!(
        "Inside {} | sample_id={} | sampled_at_unix_ms={}\nscope: {}\n",
        env!("CARGO_PKG_VERSION"),
        analysis.source_sample_id,
        current.sampled_at_unix_ms,
        current.scope
    );
    writeln!(
        out,
        "CPU活動量（本プロジェクトの近似値）: {} | source={}",
        display(&analysis.cpu_active_ratio, 100.0, "%"),
        current.readings.cpu_times.source
    )
    .unwrap();
    writeln!(
        out,
        "CPU PSI some 区間割合: {} | source={}",
        display(&analysis.cpu_psi_some_interval_ratio, 100.0, "%"),
        current.readings.cpu_psi_some.source
    )
    .unwrap();
    writeln!(
        out,
        "CPU PSI some avg10（平滑化傾向）: {}",
        display(&analysis.cpu_psi_some_avg10_pct, 1.0, "%")
    )
    .unwrap();
    let runnable = match &current.readings.procs_running.value {
        Ok(value) => format!("{value} runnable tasks"),
        Err(error) => format!("N/A ({})", reason(error)),
    };
    writeln!(
        out,
        "procs_running（実行中を含む参考値）: {runnable} | source={}",
        current.readings.procs_running.source
    )
    .unwrap();
    match &evidence.elapsed {
        Ok(elapsed) => {
            writeln!(out, "観測区間（単調時計）: {:.9} s", elapsed.as_secs_f64()).unwrap()
        }
        Err(error) => writeln!(out, "観測区間: N/A ({})", reason(error)).unwrap(),
    }
    writeln!(out, "活動と競合は独立した2軸．0は観測できたゼロ，N/Aは取得・解析不能．\n{SCOPE_WARNING}\n{METRIC_WARNING}").unwrap();
    for warning in &analysis.warnings {
        writeln!(out, "警告 {}: {}", warning.metric, reason(&warning.reason)).unwrap();
    }
    if inspect {
        writeln!(
            out,
            "\n計算根拠 CPU: {}\n計算根拠 PSI: {}",
            evidence.cpu_rule, evidence.psi_rule
        )
        .unwrap();
        writeln!(out, "CPU活動量はiowait・stealを活動に含めず，guest・guest_niceを加算しない．\nPSI avg10は10秒窓の単純平均ではなく平滑化傾向．system-level fullは使用しない．\n各ファイルは順番に読み，時刻は読込開始時点．原子的なスナップショットではない．").unwrap();
        writeln!(
            out,
            "CPU差分（USER_HZ ticks）: {:?}\nPSI total差分（μs）: {:?}",
            evidence.cpu_delta, evidence.psi_total_delta_us
        )
        .unwrap();
        for (name, observation) in [
            ("previous", evidence.previous.as_ref()),
            ("current", Some(current)),
        ] {
            if let Some(observation) = observation {
                writeln!(
                    out,
                    "{name} 観測値:\n{}",
                    serde_json::to_string_pretty(&self::observation(observation))
                        .expect("finite parser/core values")
                )
                .unwrap();
            }
        }
    }
    out
}

fn signal(state: SignalState) -> &'static str {
    match state {
        SignalState::Zero => "zero",
        SignalState::NonZero => "non_zero",
        SignalState::Unavailable => "unavailable",
    }
}
