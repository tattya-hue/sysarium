use inside_core::{
    analysis::{SignalState, analyze},
    observation::{Observation, SYSTEM_VIEW, UnavailableReason as Reason},
};
use inside_linux::{collector::collect_from_proc, cpu::parse_proc_stat, psi::parse_cpu_psi_some};
use std::{path::Path, time::Duration};

fn observation(name: &str, id: u64) -> Observation {
    Observation {
        sample_id: id,
        sampled_at_unix_ms: 10000 + id * 1000,
        elapsed_since_previous: (id > 1).then_some(Duration::from_secs(1)),
        scope: SYSTEM_VIEW,
        readings: collect_from_proc(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/p1")
                .join(name),
        ),
    }
}
fn pair() -> (Observation, Observation) {
    (observation("before", 1), observation("after", 2))
}
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
}

#[test]
fn fixture_formulas_units_and_evidence() {
    let (before, now) = pair();
    let analysis = analyze(Some(&before), &now);
    close(analysis.cpu_active_ratio.unwrap(), 0.8);
    close(analysis.cpu_psi_some_interval_ratio.unwrap(), 0.25);
    close(analysis.cpu_psi_some_avg10_pct.unwrap(), 2.5);
    let delta = analysis.evidence.cpu_delta.unwrap();
    assert_eq!(delta.active_ticks, 80);
    assert_eq!(delta.total_ticks, 100);
    assert_eq!(delta.counters.iowait, 5);
    assert_eq!(delta.counters.steal, 5);
    assert_eq!(analysis.evidence.psi_total_delta_us, Ok(250000));
    assert_eq!(analysis.evidence.elapsed, Ok(Duration::from_secs(1)));
    assert_eq!(analysis.evidence.previous, Some(before));
    assert_eq!(analysis.evidence.current, now);
    assert_eq!(analysis.source_sample_id, 2);
    assert!(analysis.warnings.is_empty());
    assert_eq!(analysis.cpu_interpretation.activity, SignalState::NonZero);
    assert_eq!(analysis.cpu_interpretation.contention, SignalState::NonZero);
}

#[test]
fn first_sample_is_unavailable_but_point_metrics_remain_available() {
    let first = observation("before", 1);
    let analysis = analyze(None, &first);
    assert_eq!(analysis.cpu_active_ratio, Err(Reason::FirstSample));
    assert_eq!(
        analysis.cpu_psi_some_interval_ratio,
        Err(Reason::FirstSample)
    );
    assert_eq!(analysis.cpu_psi_some_avg10_pct, Ok(1.0));
    assert_eq!(
        analysis.cpu_interpretation.activity,
        SignalState::Unavailable
    );
    assert_eq!(analysis.warnings.len(), 2);
}

#[test]
fn monotonic_elapsed_ignores_wall_clock_and_handles_submicroseconds() {
    let (before, mut now) = pair();
    now.sampled_at_unix_ms = 0; // 壁時計が逆行しても区間計算は変化しない．
    now.elapsed_since_previous = Some(Duration::from_secs(2));
    close(
        analyze(Some(&before), &now)
            .cpu_psi_some_interval_ratio
            .unwrap(),
        0.125,
    );
    now.elapsed_since_previous = Some(Duration::from_nanos(1500));
    now.readings.cpu_psi_some.value.as_mut().unwrap().total_us = before
        .readings
        .cpu_psi_some
        .value
        .as_ref()
        .unwrap()
        .total_us
        + 1;
    close(
        analyze(Some(&before), &now)
            .cpu_psi_some_interval_ratio
            .unwrap(),
        2.0 / 3.0,
    );
}

#[test]
fn invalid_intervals_do_not_produce_zero_or_infinity() {
    let (before, mut now) = pair();
    for (elapsed, expected) in [
        (None, Reason::MissingInterval),
        (Some(Duration::ZERO), Reason::ZeroInterval),
    ] {
        now.elapsed_since_previous = elapsed;
        let result = analyze(Some(&before), &now);
        assert_eq!(result.cpu_active_ratio, Err(expected.clone()));
        assert_eq!(result.cpu_psi_some_interval_ratio, Err(expected));
        assert_eq!(result.cpu_psi_some_avg10_pct, Ok(2.5));
    }
}

#[test]
fn no_cpu_delta_is_unavailable_and_does_not_affect_psi() {
    let (before, mut now) = pair();
    now.readings.cpu_times = before.readings.cpu_times.clone();
    let result = analyze(Some(&before), &now);
    assert_eq!(result.cpu_active_ratio, Err(Reason::ZeroCpuDelta));
    assert_eq!(result.cpu_psi_some_interval_ratio, Ok(0.25));
}

#[test]
fn all_cpu_counters_are_checked_for_regression_even_when_total_increases() {
    let (before, now) = pair();
    for index in 0..8 {
        let mut now = now.clone();
        let times = now.readings.cpu_times.value.as_mut().unwrap();
        let old = before.readings.cpu_times.value.as_ref().unwrap();
        let fields = [
            (&mut times.user, old.user, "user"),
            (&mut times.nice, old.nice, "nice"),
            (&mut times.system, old.system, "system"),
            (&mut times.idle, old.idle, "idle"),
            (&mut times.iowait, old.iowait, "iowait"),
            (&mut times.irq, old.irq, "irq"),
            (&mut times.softirq, old.softirq, "softirq"),
            (&mut times.steal, old.steal, "steal"),
        ];
        let mut selected = None;
        for (i, (value, old, field)) in fields.into_iter().enumerate() {
            if i == index {
                *value = old - 1;
                selected = Some(field);
            }
        }
        let analysis = analyze(Some(&before), &now);
        assert_eq!(
            analysis.cpu_active_ratio,
            Err(Reason::CounterRegression {
                field: selected.unwrap()
            })
        );
        assert_eq!(analysis.cpu_psi_some_interval_ratio, Ok(0.25));
    }
}

#[test]
fn psi_regression_and_out_of_range_are_rejected_without_clamping() {
    let (before, mut now) = pair();
    for (total, expected) in [
        (
            999999,
            Reason::CounterRegression {
                field: "some.total_us",
            },
        ),
        (
            2000001,
            Reason::OutOfRange {
                field: "cpu_psi_some_interval_ratio",
            },
        ),
    ] {
        now.readings.cpu_psi_some.value.as_mut().unwrap().total_us = total;
        let result = analyze(Some(&before), &now);
        assert_eq!(result.cpu_psi_some_interval_ratio, Err(expected));
        assert_eq!(result.cpu_active_ratio, Ok(0.8));
        assert_eq!(result.cpu_psi_some_avg10_pct, Ok(2.5));
    }
}

#[test]
fn missing_and_invalid_inputs_preserve_source_sample_and_cause() {
    let (before, now) = pair();
    for previous_fails in [false, true] {
        for invalid in [false, true] {
            for cpu_fails in [false, true] {
                let (mut before, mut now) = (before.clone(), now.clone());
                let failed = if previous_fails {
                    &mut before
                } else {
                    &mut now
                };
                let source = if cpu_fails {
                    "/proc/stat"
                } else {
                    "/proc/pressure/cpu"
                };
                let cause = if invalid {
                    if cpu_fails {
                        let cause =
                            parse_proc_stat(include_str!("fixtures/stat-invalid.txt")).cpu_times;
                        failed.readings.cpu_times.value = cause.clone();
                        cause.unwrap_err()
                    } else {
                        let cause = parse_cpu_psi_some(include_str!("fixtures/psi-invalid.txt"));
                        failed.readings.cpu_psi_some.value = cause.clone();
                        cause.unwrap_err()
                    }
                } else {
                    if cpu_fails {
                        failed.readings.cpu_times.value = Err(Reason::NotFound);
                    } else {
                        failed.readings.cpu_psi_some.value = Err(Reason::NotFound);
                    }
                    Reason::NotFound
                };
                let expected = Reason::InputUnavailable {
                    sample_id: failed.sample_id,
                    source,
                    reason: Box::new(cause),
                };
                let analysis = analyze(Some(&before), &now);
                if cpu_fails {
                    assert_eq!(analysis.cpu_active_ratio, Err(expected));
                    assert_eq!(analysis.cpu_psi_some_interval_ratio, Ok(0.25));
                } else {
                    assert_eq!(analysis.cpu_psi_some_interval_ratio, Err(expected));
                    assert_eq!(analysis.cpu_active_ratio, Ok(0.8));
                    if previous_fails {
                        assert_eq!(analysis.cpu_psi_some_avg10_pct, Ok(2.5));
                    }
                }
            }
        }
    }
}

#[test]
fn missing_previous_sample_is_not_skipped_after_recovery() {
    let (mut before, mut now) = pair();
    before.readings.cpu_psi_some.value = Err(Reason::NotFound);
    assert!(
        analyze(Some(&before), &now)
            .cpu_psi_some_interval_ratio
            .is_err()
    );
    let previous = now.clone();
    now.sample_id = 3;
    now.readings.cpu_psi_some.value.as_mut().unwrap().total_us += 100000;
    assert_eq!(
        analyze(Some(&previous), &now).cpu_psi_some_interval_ratio,
        Ok(0.1)
    );
}

#[test]
fn measured_zero_and_one_are_valid_independent_axes() {
    let (before, mut now) = pair();
    let old = *before.readings.cpu_times.value.as_ref().unwrap();
    now.readings.cpu_times.value = Ok(old);
    now.readings.cpu_times.value.as_mut().unwrap().idle += 100;
    now.readings.cpu_psi_some.value.as_mut().unwrap().total_us += 750000;
    let analysis = analyze(Some(&before), &now);
    assert_eq!(analysis.cpu_active_ratio, Ok(0.0));
    assert_eq!(analysis.cpu_psi_some_interval_ratio, Ok(1.0));
    assert_eq!(analysis.cpu_interpretation.activity, SignalState::Zero);
    now.readings.cpu_times.value = Ok(old);
    now.readings.cpu_times.value.as_mut().unwrap().user += 100;
    now.readings.cpu_psi_some = before.readings.cpu_psi_some.clone();
    let analysis = analyze(Some(&before), &now);
    assert_eq!(analysis.cpu_active_ratio, Ok(1.0));
    assert_eq!(analysis.cpu_psi_some_interval_ratio, Ok(0.0));
    assert_eq!(analysis.cpu_interpretation.contention, SignalState::Zero);
}

#[test]
fn core_validates_nonfinite_psi_and_sum_overflow() {
    let (before, now) = pair();
    for value in [f64::NAN, f64::INFINITY, -1.0, 100.1] {
        let mut now = now.clone();
        now.readings.cpu_psi_some.value.as_mut().unwrap().avg10 = value;
        let result = analyze(Some(&before), &now);
        assert_eq!(
            result.cpu_psi_some_interval_ratio,
            Err(Reason::OutOfRange { field: "avg10" })
        );
        assert_eq!(
            result.cpu_psi_some_avg10_pct,
            Err(Reason::OutOfRange { field: "avg10" })
        );
    }
    let mut now = now;
    let cpu = now.readings.cpu_times.value.as_mut().unwrap();
    cpu.user = u64::MAX;
    cpu.idle = u64::MAX;
    assert_eq!(
        analyze(Some(&before), &now).cpu_active_ratio,
        Err(Reason::ArithmeticOverflow)
    );
}

#[test]
fn metadata_and_source_must_describe_the_same_interval() {
    let (before, now) = pair();
    let mut changed = now.clone();
    changed.sample_id = 1;
    assert_eq!(
        analyze(Some(&before), &changed).cpu_active_ratio,
        Err(Reason::InvalidSampleOrder)
    );
    let mut changed = now.clone();
    changed.scope = "other";
    assert_eq!(
        analyze(Some(&before), &changed).cpu_active_ratio,
        Err(Reason::ScopeChanged)
    );
    let mut changed = now;
    changed.readings.cpu_times.source = "other";
    assert_eq!(
        analyze(Some(&before), &changed).cpu_active_ratio,
        Err(Reason::SourceChanged)
    );
}

#[test]
fn large_absolute_cpu_counters_are_subtracted_before_aggregation() {
    let (mut before, mut now) = pair();
    let mut old = *before.readings.cpu_times.value.as_ref().unwrap();
    old.user = u64::MAX - 100;
    old.idle = u64::MAX - 100;
    let mut new = old;
    new.user += 80;
    new.idle += 20;
    before.readings.cpu_times.value = Ok(old);
    now.readings.cpu_times.value = Ok(new);
    assert_eq!(analyze(Some(&before), &now).cpu_active_ratio, Ok(0.8));
}
