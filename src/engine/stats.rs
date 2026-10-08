//! Port of keybr-textinput `stats.ts`: summary of one lesson's steps.

use super::histogram::Histogram;
use super::textinput::Step;

#[derive(Debug, Clone, PartialEq)]
pub struct Stats {
    /// Milliseconds from the first to the last step.
    pub time: f64,
    /// Characters per minute.
    pub speed: f64,
    pub length: usize,
    pub errors: usize,
    pub accuracy: f64,
    pub histogram: Histogram,
}

pub fn make_stats(steps: &[Step]) -> Stats {
    match steps {
        [first, .., last] => {
            let length = steps.len();
            let time = (last.time_stamp - first.time_stamp).round();
            let errors = steps.iter().filter(|s| s.typo).count();
            Stats {
                time,
                speed: compute_speed(length, time),
                length,
                errors,
                accuracy: (length - errors) as f64 / length as f64,
                // The trigger step is ignored.
                histogram: Histogram::from_steps(&steps[1..]),
            }
        }
        _ => Stats {
            time: 0.0,
            speed: 0.0,
            length: 0,
            errors: 0,
            accuracy: 0.0,
            histogram: Histogram::default(),
        },
    }
}

/// Characters per minute.
pub fn compute_speed(length: usize, time_ms: f64) -> f64 {
    if time_ms > 0.0 {
        length as f64 / (time_ms / 1000.0) * 60.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::histogram::Sample;
    use crate::engine::histogram::tests::step;

    #[test]
    fn compute_stats() {
        let s = make_stats(&[
            step(100.1, 'x', 900.1, false), // trigger is ignored
            step(200.1, 'a', 100.1, false),
            step(300.1, 'b', 100.1, false),
            step(400.1, 'c', 100.1, false),
            step(500.2, ' ', 100.1, true),
        ]);
        assert_eq!(s.time, 400.0);
        assert_eq!(s.speed, 750.0);
        assert_eq!((s.length, s.errors), (5, 1));
        assert_eq!(s.accuracy, 0.8);
        let s100 = Sample {
            hit_count: 1,
            miss_count: 0,
            time_to_type: 100,
        };
        let expected: Vec<(char, Sample)> = vec![
            (
                ' ',
                Sample {
                    hit_count: 1,
                    miss_count: 1,
                    time_to_type: 0,
                },
            ),
            ('a', s100),
            ('b', s100),
            ('c', s100),
        ];
        assert_eq!(s.histogram.0.into_iter().collect::<Vec<_>>(), expected);
    }

    #[test]
    fn compute_accuracy() {
        assert_eq!(make_stats(&[]).accuracy, 0.0);
        let run = |typos: [bool; 4]| {
            make_stats(
                &typos
                    .iter()
                    .enumerate()
                    .map(|(i, &t)| step(100.0 * (i + 1) as f64, 'a', 100.0, t))
                    .collect::<Vec<_>>(),
            )
            .accuracy
        };
        assert_eq!(run([true; 4]), 0.0);
        assert_eq!(run([false, false, true, true]), 0.5);
        assert_eq!(run([false; 4]), 1.0);
    }

    #[test]
    fn single_step_is_empty() {
        let s = make_stats(&[step(100.0, 'a', 100.0, false)]);
        assert_eq!((s.length, s.speed, s.time), (0, 0.0, 0.0));
    }
}
