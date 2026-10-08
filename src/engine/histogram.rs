//! Port of keybr-textinput `histogram.ts`: per-character timing for one lesson.

use super::textinput::Step;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    pub hit_count: u32,
    pub miss_count: u32,
    /// Mean ms to type this character, excluding typos; 0 if unknown.
    pub time_to_type: u32,
}

/// Per-character samples, ordered by character.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Histogram(pub BTreeMap<char, Sample>);

impl Histogram {
    pub fn from_steps(steps: &[Step]) -> Self {
        #[derive(Default)]
        struct Acc {
            hits: u32,
            misses: u32,
            time: f64,
            count: u32,
        }
        let mut acc: BTreeMap<char, Acc> = BTreeMap::new();
        for s in steps {
            let a = acc.entry(s.ch).or_default();
            a.hits += 1;
            if s.typo {
                a.misses += 1;
            } else if s.time_to_type > 0.0 {
                a.time += s.time_to_type;
                a.count += 1;
            }
        }
        Histogram(
            acc.into_iter()
                .map(|(ch, a)| {
                    let time_to_type = if a.time > 0.0 && a.count > 0 {
                        (a.time / a.count as f64).round() as u32
                    } else {
                        0
                    };
                    (
                        ch,
                        Sample {
                            hit_count: a.hits,
                            miss_count: a.misses,
                            time_to_type,
                        },
                    )
                })
                .filter(|(_, s)| validate_sample(s))
                .collect(),
        )
    }

    /// Number of distinct characters.
    pub fn complexity(&self) -> usize {
        self.0.len()
    }

    pub fn get(&self, ch: char) -> Option<&Sample> {
        self.0.get(&ch)
    }

    pub fn validate(&self) -> bool {
        self.0.len() >= 3 && self.0.values().all(validate_sample)
    }
}

/// Rejects implausible timings: faster than 1500 CPM or slower than 5 CPM.
pub fn validate_sample(s: &Sample) -> bool {
    s.time_to_type == 0 || (40..=12000).contains(&s.time_to_type)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn step(time_stamp: f64, ch: char, time_to_type: f64, typo: bool) -> Step {
        Step {
            time_stamp,
            ch,
            time_to_type,
            typo,
        }
    }

    fn sample(hit_count: u32, miss_count: u32, time_to_type: u32) -> Sample {
        Sample {
            hit_count,
            miss_count,
            time_to_type,
        }
    }

    fn hist(entries: &[(char, Sample)]) -> Histogram {
        Histogram(entries.iter().copied().collect())
    }

    #[test]
    fn empty() {
        assert_eq!(Histogram::from_steps(&[]).complexity(), 0);
    }

    #[test]
    fn histogram() {
        let h = Histogram::from_steps(&[
            step(100.0, 'a', 100.1, false),
            step(200.0, 'b', 100.1, false),
            step(300.0, 'c', 100.1, false),
            step(600.0, 'a', 300.1, false),
            step(700.0, 'a', 100.1, true),
            step(801.0, 'x', 1.0, false), // invalid
        ]);
        assert_eq!(
            h,
            hist(&[
                ('a', sample(3, 1, 200)),
                ('b', sample(1, 0, 100)),
                ('c', sample(1, 0, 100))
            ])
        );
    }

    #[test]
    fn ignore_typos() {
        let h = Histogram::from_steps(&[
            step(100.0, 'a', 100.1, true),
            step(200.0, 'b', 100.1, true),
            step(300.0, 'c', 100.1, true),
            step(301.0, 'x', 1.0, false),
        ]);
        assert_eq!(
            h,
            hist(&[
                ('a', sample(1, 1, 0)),
                ('b', sample(1, 1, 0)),
                ('c', sample(1, 1, 0))
            ])
        );
    }

    #[test]
    fn validate() {
        assert!(!hist(&[]).validate());
        assert!(!hist(&[('a', sample(10, 0, 100)), ('b', sample(10, 0, 100))]).validate());
        let three = |t| {
            hist(&[
                ('a', sample(10, 0, 100)),
                ('b', sample(10, 0, 100)),
                ('c', sample(10, 0, t)),
            ])
        };
        assert!(!three(12001).validate());
        assert!(!three(39).validate());
        assert!(three(40).validate());
        assert!(three(12000).validate());
        assert!(
            hist(&[
                ('a', sample(10, 10, 0)),
                ('b', sample(10, 10, 0)),
                ('c', sample(10, 10, 0))
            ])
            .validate()
        );
    }
}
