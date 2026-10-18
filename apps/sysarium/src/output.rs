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

fn display(value: &Result<f64, Reason>, multiplier: f64) -> String {
    value
        .as_ref()
        .map_or("N/A".into(), |v| format!("{:.1}%", v * multiplier))
}

/// 人が読む詳細．生のJSONやRustの内部型はNDJSON出力に残す．
pub fn details(a: &Analysis) -> String {
    let e = &a.evidence;
    let readings = &e.current.readings;
    let mut out = format!(
        "CPU\n  活動量    {}\n  CPUが仕事をしていた時間の割合（近似）．\n",
        display(&a.cpu_active_ratio, 100.0)
    );
    if let Err(reason) = &a.cpu_active_ratio {
        writeln!(out, "  {}", explain(reason)).unwrap();
    }
    writeln!(
        out,
        "\nCPU pressure\n  区間      {}\n  CPUを待つタスクがある時間の割合．",
        display(&a.cpu_psi_some_interval_ratio, 100.0)
    )
    .unwrap();
    if let Err(reason) = &a.cpu_psi_some_interval_ratio {
        writeln!(out, "  {}", explain(reason)).unwrap();
    }
    writeln!(
        out,
        "  avg10     {}  （平滑化された傾向）",
        display(&a.cpu_psi_some_avg10_pct, 1.0)
    )
    .unwrap();
    if let Err(reason) = &a.cpu_psi_some_avg10_pct
        && a.cpu_psi_some_interval_ratio.as_ref().err() != Some(reason)
    {
        writeln!(out, "  avg10: {}", explain(reason)).unwrap();
    }
    writeln!(out, "\nRunnable").unwrap();
    match &readings.procs_running.value {
        Ok(v) => writeln!(out, "  {v} タスク  （実行中を含む．待機数ではない）").unwrap(),
        Err(reason) => writeln!(out, "  N/A · {}", explain(reason)).unwrap(),
    }
    writeln!(out, "\n観測").unwrap();
    match &e.elapsed {
        Ok(d) => writeln!(
            out,
            "  区間 {:.3} s · 単調時計 · 約1秒ごとに更新",
            d.as_secs_f64()
        )
        .unwrap(),
        Err(reason) => writeln!(out, "  区間 N/A · {}", explain(reason)).unwrap(),
    }
    writeln!(out, "  対象 {} · 可視の/procの範囲", e.current.scope).unwrap();
    writeln!(out, "\n計算と取得元").unwrap();
    if let Ok(delta) = &e.cpu_delta {
        writeln!(
            out,
            "  CPU: 活動差分 {} / 総差分 {} ticks（USER_HZ）",
            delta.active_ticks, delta.total_ticks
        )
        .unwrap();
    }
    writeln!(out,"  活動: user + nice + system + irq + softirq\n  総時間: 活動 + idle + iowait + steal\n  guestは重複加算しない．取得元 {}",readings.cpu_times.source).unwrap();
    if let (Ok(delta), Ok(elapsed)) = (&e.psi_total_delta_us, &e.elapsed) {
        writeln!(
            out,
            "  PSI: 停滞差分 {delta} μs / 経過 {:.3} μs",
            elapsed.as_secs_f64() * 1_000_000.0
        )
        .unwrap();
    }
    writeln!(out,"  取得元 {} · some.totalの差分\n\n読み方\n  活動量と競合は別の軸．高活動だけで異常とは判断しない．\n  書類は競合の強さの比喩で，待機タスク数ではない．\n  N/Aは不明．0%は計測できたゼロ．\n  各ファイルは順次読むため，原子的な観測ではない．\n  コンテナやVMでは物理ホスト全体と一致するとは限らない．\n\n生データの継続出力: sysarium --json",readings.cpu_psi_some.source).unwrap();
    out
}

fn explain(reason: &Reason) -> String {
    match reason {
        Reason::FirstSample => "初回のため，次の観測を待っています".into(),
        Reason::MissingInterval => "観測間隔を取得できません".into(),
        Reason::ZeroInterval => "観測間隔が短すぎるため計算できません".into(),
        Reason::ZeroCpuDelta => "CPU時間に差分がないため計算できません".into(),
        Reason::InputUnavailable {
            sample_id, reason, ..
        } => format!("観測 #{sample_id}: {}", explain(reason)),
        Reason::NotFound => "計測ファイルがありません（未対応の可能性）".into(),
        Reason::PermissionDenied => "計測ファイルを読む権限がありません".into(),
        Reason::Io { message } => {
            let short: String = message
                .chars()
                .take(96)
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect();
            format!(
                "読み取りエラー: {short}{}",
                if message.chars().count() > 96 {
                    "…（全文は--json）"
                } else {
                    ""
                }
            )
        }
        Reason::Missing { field } => format!("必要な項目がありません（{field}）"),
        Reason::Invalid { field, .. } => format!("値の形式が不正です（{field}）"),
        Reason::Duplicate { field } => format!("項目が重複しています（{field}）"),
        Reason::CounterRegression { field } => {
            format!("累積値が減少したため計算できません（{field}）")
        }
        Reason::ArithmeticOverflow => "計算可能な数値範囲を超えました".into(),
        Reason::OutOfRange { field } => format!("値が有効範囲外です（{field}）"),
        Reason::InvalidSampleOrder => "観測の順序が不正です".into(),
        Reason::ScopeChanged => "観測対象が変わったため次の観測を待っています".into(),
        Reason::SourceChanged => "取得元が変わったため次の観測を待っています".into(),
    }
}

fn signal(state: SignalState) -> &'static str {
    match state {
        SignalState::Zero => "zero",
        SignalState::NonZero => "non_zero",
        SignalState::Unavailable => "unavailable",
    }
}
