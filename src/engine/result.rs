//! Port of keybr-result `result.ts`: the stored record of one lesson.

use super::histogram::{Histogram, Sample};
use super::stats::Stats;
use super::textinput::Step;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Timing of key transitions, keyed by the two-character string `"ab"`.
/// Not used by keybr's algorithm; recorded for weak-transition analysis.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bigrams(pub BTreeMap<String, Sample>);

impl Bigrams {
    /// Each step after the first is attributed to the transition from the
    /// previous character. Implausible individual timings are not averaged.
    pub fn from_steps(steps: &[Step]) -> Self {
        let mut acc: BTreeMap<String, (Sample, f64, u32)> = BTreeMap::new();
        for w in steps.windows(2) {
            let key: String = [w[0].ch, w[1].ch].iter().collect();
            let (sample, time, count) = acc.entry(key).or_insert((
                Sample {
                    hit_count: 0,
                    miss_count: 0,
                    time_to_type: 0,
                },
                0.0,
                0,
            ));
            sample.hit_count += 1;
            let ttt = w[1].time_to_type;
            if w[1].typo {
                sample.miss_count += 1;
            } else if (40.0..=12000.0).contains(&ttt) {
                *time += ttt;
                *count += 1;
            }
        }
        Bigrams(
            acc.into_iter()
                .map(|(k, (mut s, time, count))| {
                    if count > 0 {
                        s.time_to_type = (time / count as f64).round() as u32;
                    }
                    (k, s)
                })
                .collect(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LessonResult {
    /// Unix time in milliseconds when the lesson finished.
    pub time_stamp: i64,
    pub length: usize,
    /// Lesson duration in milliseconds.
    pub time: f64,
    pub errors: usize,
    pub histogram: Histogram,
    #[serde(default)]
    pub bigrams: Bigrams,
}

impl LessonResult {
    pub fn from_steps(time_stamp: i64, stats: &Stats, steps: &[Step]) -> Self {
        Self {
            time_stamp,
            length: stats.length,
            time: stats.time,
            errors: stats.errors,
            histogram: stats.histogram.clone(),
            bigrams: Bigrams::from_steps(steps),
        }
    }

    pub fn complexity(&self) -> usize {
        self.histogram.complexity()
    }

    fn measurable(&self) -> bool {
        self.length > 0 && self.time > 0.0 && self.complexity() > 0
    }

    /// Characters per minute.
    pub fn speed(&self) -> f64 {
        if self.measurable() {
            self.length as f64 / (self.time / 1000.0) * 60.0
        } else {
            0.0
        }
    }

    pub fn accuracy(&self) -> f64 {
        if self.measurable() {
            (self.length - self.errors) as f64 / self.length as f64
        } else {
            0.0
        }
    }

    pub fn score(&self) -> f64 {
        if !self.measurable() {
            return 0.0;
        }
        self.speed() * self.complexity() as f64 / (self.errors + 1) as f64
            * (self.length as f64 / 50.0)
    }

    /// Whether the result is good enough to feed into key statistics.
    pub fn validate(&self) -> bool {
        self.length >= 10
            && self.time >= 1000.0
            && self.complexity() >= 1
            && self.speed() >= 1.0
            && self.histogram.validate()
    }
}

/// Milliseconds per character to characters per minute.
pub fn time_to_speed(ms: f64) -> f64 {
    60_000.0 / ms
}

/// Characters per minute to milliseconds per character.
pub fn speed_to_time(cpm: f64) -> f64 {
    60_000.0 / cpm
}

/// Characters per minute to words per minute (5 characters per word).
pub fn cpm_to_wpm(cpm: f64) -> f64 {
    cpm / 5.0
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::engine::histogram::tests::step;

    pub fn fake_result(index: i64, entries: &[(char, u32)]) -> LessonResult {
        LessonResult {
            time_stamp: 1_000_000 + index * 1000,
            length: 100,
            time: 10_000.0,
            errors: 0,
            histogram: Histogram(
                entries
                    .iter()
                    .map(|&(c, t)| {
                        (
                            c,
                            Sample {
                                hit_count: 1,
                                miss_count: 0,
                                time_to_type: t,
                            },
                        )
                    })
                    .collect(),
            ),
            bigrams: Bigrams::default(),
        }
    }

    fn sample(ttt: u32) -> Sample {
        Sample {
            hit_count: 10,
            miss_count: 0,
            time_to_type: ttt,
        }
    }

    fn result(length: usize, time: f64, errors: usize, n_chars: usize) -> LessonResult {
        LessonResult {
            time_stamp: 0,
            length,
            time,
            errors,
            histogram: Histogram(('a'..).take(n_chars).map(|c| (c, sample(100))).collect()),
            bigrams: Bigrams::default(),
        }
    }

    #[test]
    fn derived_metrics() {
        let r = result(100, 30_000.0, 4, 5);
        assert_eq!(r.speed(), 200.0);
        assert_eq!(r.accuracy(), 0.96);
        assert_eq!(r.score(), 200.0 * 5.0 / 5.0 * 2.0);
    }

    #[test]
    fn zero_metrics_when_unmeasurable() {
        let r = result(0, 0.0, 0, 0);
        assert_eq!((r.speed(), r.accuracy(), r.score()), (0.0, 0.0, 0.0));
    }

    #[test]
    fn validate_thresholds() {
        assert!(result(10, 1000.0, 0, 3).validate());
        assert!(!result(9, 1000.0, 0, 3).validate()); // too short
        assert!(!result(10, 999.0, 0, 3).validate()); // too quick
        assert!(!result(10, 1000.0, 0, 2).validate()); // histogram too small
    }

    #[test]
    fn speed_conversions() {
        assert_eq!(time_to_speed(100.0), 600.0);
        assert_eq!(speed_to_time(600.0), 100.0);
        assert_eq!(cpm_to_wpm(175.0), 35.0);
    }

    #[test]
    fn bigrams_from_steps() {
        let b = Bigrams::from_steps(&[
            step(0.0, 'a', 900.0, false),
            step(100.0, 'b', 100.0, false),
            step(400.0, 'a', 300.0, false),
            step(500.0, 'b', 200.0, true),
            step(501.0, 'a', 1.0, false), // implausible timing: counted, not timed
        ]);
        let get = |k: &str| b.0.get(k).copied();
        assert_eq!(
            get("ab"),
            Some(Sample {
                hit_count: 2,
                miss_count: 1,
                time_to_type: 100
            })
        );
        assert_eq!(
            get("ba"),
            Some(Sample {
                hit_count: 2,
                miss_count: 0,
                time_to_type: 300
            })
        );
        assert_eq!(b.0.len(), 2);
    }

    #[test]
    fn json_round_trip() {
        let r = result(100, 30_000.0, 4, 5);
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(serde_json::from_str::<LessonResult>(&s).unwrap(), r);
    }
}
