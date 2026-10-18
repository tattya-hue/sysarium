//! レンダラーに依存しない意味モデル．平滑化は表示強度だけに適用する．
use crate::{analysis::Analysis, observation::UnavailableReason as Reason};

/// CPU PSI someの区間割合から比喩の段階を決める暫定しきい値．
/// 運用上の正常・異常の判定基準や待機タスク数ではない．
pub const PRESSURE_THRESHOLDS: [f64; 5] = [0.005, 0.02, 0.05, 0.10, 0.25];
pub const ACTIVITY_SMOOTHING_ALPHA: f64 = 0.5;
/// キャラクター表現の暫定境界．遅延・温度・正常異常の判定ではない．
pub const ACTIVITY_THRESHOLDS: [f64; 2] = [0.70, 0.95];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityLevel {
    Relaxed,
    Busy,
    Strained,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Character {
    pub activity_intensity: Result<f64, Reason>,
}

impl Character {
    /// 平滑化済みの活動強度だけから導出する．PSI・書類の段階は参照しない．
    pub fn activity_level(&self) -> Result<ActivityLevel, Reason> {
        ratio(&self.activity_intensity, "activity_intensity").map(|value| {
            if value >= ACTIVITY_THRESHOLDS[1] {
                ActivityLevel::Strained
            } else if value >= ACTIVITY_THRESHOLDS[0] {
                ActivityLevel::Busy
            } else {
                ActivityLevel::Relaxed
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackRepresents {
    CpuContentionMetaphor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStack {
    pub pressure_level: Result<u8, Reason>,
    pub represents: StackRepresents,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub source_sample_id: u64,
    pub character: Character,
    pub task_stack: TaskStack,
}

impl Scene {
    /// 同じAnalysisと直前Sceneから常に同じSceneを作る純粋関数．
    /// 新しい観測時だけ呼び，描画フレームごとには更新しない．
    /// 欠測時は以前の強度を引き継がず，復帰時も現在値から再開する．
    pub fn from_analysis(analysis: &Analysis, previous: Option<&Scene>) -> Self {
        let activity_intensity = ratio(&analysis.cpu_active_ratio, "cpu_active_ratio").map(|now| {
            let before = previous
                .filter(|s| s.source_sample_id < analysis.source_sample_id)
                .and_then(|s| ratio(&s.character.activity_intensity, "activity_intensity").ok());
            before.map_or(now, |before| {
                ACTIVITY_SMOOTHING_ALPHA * now + (1.0 - ACTIVITY_SMOOTHING_ALPHA) * before
            })
        });
        let pressure_level = ratio(
            &analysis.cpu_psi_some_interval_ratio,
            "cpu_psi_some_interval_ratio",
        )
        .map(|value| {
            PRESSURE_THRESHOLDS
                .iter()
                .filter(|&&threshold| value >= threshold)
                .count() as u8
        });
        Self {
            source_sample_id: analysis.source_sample_id,
            character: Character { activity_intensity },
            task_stack: TaskStack {
                pressure_level,
                represents: StackRepresents::CpuContentionMetaphor,
            },
        }
    }
}

fn ratio(value: &Result<f64, Reason>, field: &'static str) -> Result<f64, Reason> {
    let value = *value.as_ref().map_err(Clone::clone)?;
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(Reason::OutOfRange { field })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analysis::analyze,
        observation::{Observation, RawCpuReadings, SYSTEM_VIEW, Sourced},
    };

    fn analysis(activity: Result<f64, Reason>, pressure: Result<f64, Reason>, id: u64) -> Analysis {
        let observation = Observation {
            sample_id: id,
            sampled_at_unix_ms: 0,
            elapsed_since_previous: None,
            scope: SYSTEM_VIEW,
            readings: RawCpuReadings {
                cpu_times: Sourced {
                    source: "fixture",
                    value: Err(Reason::NotFound),
                },
                cpu_psi_some: Sourced {
                    source: "fixture",
                    value: Err(Reason::NotFound),
                },
                procs_running: Sourced {
                    source: "fixture",
                    value: Err(Reason::NotFound),
                },
            },
        };
        let mut analysis = analyze(None, &observation);
        analysis.cpu_active_ratio = activity;
        analysis.cpu_psi_some_interval_ratio = pressure;
        analysis
    }

    #[test]
    fn character_levels_have_explicit_boundaries_and_preserve_unknown() {
        for (value, expected) in [
            (0.0, ActivityLevel::Relaxed),
            (ACTIVITY_THRESHOLDS[0].next_down(), ActivityLevel::Relaxed),
            (ACTIVITY_THRESHOLDS[0], ActivityLevel::Busy),
            (ACTIVITY_THRESHOLDS[1].next_down(), ActivityLevel::Busy),
            (ACTIVITY_THRESHOLDS[1], ActivityLevel::Strained),
            (1.0, ActivityLevel::Strained),
        ] {
            assert_eq!(
                Character {
                    activity_intensity: Ok(value)
                }
                .activity_level(),
                Ok(expected)
            );
        }
        for value in [f64::NAN, f64::INFINITY, -0.01, 1.01] {
            assert!(
                Character {
                    activity_intensity: Ok(value)
                }
                .activity_level()
                .is_err()
            );
        }
        assert_eq!(
            Character {
                activity_intensity: Err(Reason::FirstSample)
            }
            .activity_level(),
            Err(Reason::FirstSample)
        );
    }

    #[test]
    fn character_levels_follow_smoothing_and_ignore_contention() {
        let mut scene = Scene::from_analysis(&analysis(Ok(0.0), Ok(0.0), 1), None);
        for (id, expected) in [
            ActivityLevel::Relaxed,
            ActivityLevel::Busy,
            ActivityLevel::Busy,
            ActivityLevel::Busy,
            ActivityLevel::Strained,
        ]
        .into_iter()
        .enumerate()
        {
            scene = Scene::from_analysis(
                &analysis(Ok(1.0), Err(Reason::NotFound), id as u64 + 2),
                Some(&scene),
            );
            assert_eq!(scene.character.activity_level(), Ok(expected));
        }
        for pressure in [Ok(0.0), Ok(1.0), Err(Reason::NotFound)] {
            let scene = Scene::from_analysis(&analysis(Ok(0.99), pressure, 10), None);
            assert_eq!(
                scene.character.activity_level(),
                Ok(ActivityLevel::Strained)
            );
        }
    }

    #[test]
    fn every_pressure_boundary_has_defined_inclusive_lower_bound() {
        assert_eq!(
            Scene::from_analysis(&analysis(Ok(0.0), Ok(0.0), 1), None)
                .task_stack
                .pressure_level,
            Ok(0)
        );
        for (index, threshold) in PRESSURE_THRESHOLDS.into_iter().enumerate() {
            for (pressure, expected) in [
                (threshold.next_down(), index as u8),
                (threshold, index as u8 + 1),
                (threshold.next_up(), index as u8 + 1),
            ] {
                let scene = Scene::from_analysis(&analysis(Ok(0.5), Ok(pressure), 2), None);
                assert_eq!(scene.task_stack.pressure_level, Ok(expected));
                assert_eq!(
                    scene.task_stack.represents,
                    StackRepresents::CpuContentionMetaphor
                );
            }
        }
        assert_eq!(
            Scene::from_analysis(&analysis(Ok(1.0), Ok(1.0), 3), None)
                .task_stack
                .pressure_level,
            Ok(5)
        );
    }

    #[test]
    fn mapping_is_deterministic_and_keeps_axes_and_analysis_independent() {
        let source = analysis(Ok(0.93), Ok(0.002), 42);
        let untouched = source.clone();
        let first = Scene::from_analysis(&source, None);
        assert_eq!(first, Scene::from_analysis(&source, None));
        assert_eq!(first.source_sample_id, 42);
        assert_eq!(first.character.activity_intensity, Ok(0.93));
        assert_eq!(first.task_stack.pressure_level, Ok(0));
        let second = Scene::from_analysis(&analysis(Ok(0.92), Ok(0.28), 43), Some(&first));
        assert!((second.character.activity_intensity.unwrap() - 0.925).abs() < 1e-12);
        assert_eq!(second.task_stack.pressure_level, Ok(5));
        assert_eq!(source, untouched);
    }

    #[test]
    fn unknown_does_not_become_zero_and_resets_smoothing() {
        let first = Scene::from_analysis(&analysis(Ok(1.0), Ok(0.5), 1), None);
        let missing = Scene::from_analysis(
            &analysis(Err(Reason::FirstSample), Err(Reason::NotFound), 2),
            Some(&first),
        );
        assert_eq!(
            missing.character.activity_intensity,
            Err(Reason::FirstSample)
        );
        assert_eq!(missing.task_stack.pressure_level, Err(Reason::NotFound));
        let recovered = Scene::from_analysis(&analysis(Ok(0.0), Ok(0.0), 3), Some(&missing));
        assert_eq!(recovered.character.activity_intensity, Ok(0.0));
        assert_eq!(recovered.task_stack.pressure_level, Ok(0));
        let pressure_missing =
            Scene::from_analysis(&analysis(Ok(0.8), Err(Reason::NotFound), 4), None);
        assert_eq!(pressure_missing.character.activity_intensity, Ok(0.8));
        assert_eq!(
            pressure_missing.task_stack.pressure_level,
            Err(Reason::NotFound)
        );
    }

    #[test]
    fn invalid_ratios_are_unavailable_and_never_quantized_or_clamped() {
        for value in [f64::NAN, f64::INFINITY, -0.01, 1.01] {
            let scene = Scene::from_analysis(&analysis(Ok(value), Ok(value), 1), None);
            assert!(scene.character.activity_intensity.is_err());
            assert!(scene.task_stack.pressure_level.is_err());
        }
    }

    #[test]
    fn p3_activity_step_response_preserves_raw_values_and_converges_in_five_samples() {
        let mut scene = Scene::from_analysis(&analysis(Ok(0.0), Ok(0.0), 1), None);
        for step in 1..=5 {
            let source = analysis(Ok(1.0), Ok(0.28), step + 1);
            scene = Scene::from_analysis(&source, Some(&scene));
            let expected = 1.0 - 0.5_f64.powi(step as i32);
            assert_eq!(scene.character.activity_intensity, Ok(expected));
            assert_eq!(source.cpu_active_ratio, Ok(1.0));
            assert_eq!(source.cpu_psi_some_interval_ratio, Ok(0.28));
            assert_eq!(scene.task_stack.pressure_level, Ok(5));
        }
        assert_eq!(scene.character.activity_intensity, Ok(0.96875));
    }

    #[test]
    fn p3_pressure_boundary_sensitivity_remains_explicit() {
        // 暫定ルールにはヒステリシスがなく，境界付近では段階が往復する．
        // その性質を隠して実測値を改変せず，検証報告へ制約として記録する．
        let mut previous = None;
        for (index, (pressure, expected)) in [(0.0049, 0), (0.0051, 1), (0.0049, 0), (0.0051, 1)]
            .into_iter()
            .enumerate()
        {
            let source = analysis(Ok(0.93), Ok(pressure), index as u64 + 1);
            let scene = Scene::from_analysis(&source, previous.as_ref());
            assert_eq!(scene.task_stack.pressure_level, Ok(expected));
            assert_eq!(source.cpu_psi_some_interval_ratio, Ok(pressure));
            previous = Some(scene);
        }
    }
}
