//! Port of keybr-lesson `guided.ts`, `key.ts` and `target.ts`: decides which
//! letters a lesson uses and which one to focus on.

use super::keystats::KeyStatsMap;
use super::result::speed_to_time;

/// The smallest alphabet a lesson starts with.
pub const MIN_ALPHABET: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LessonSettings {
    /// Target speed in characters per minute.
    pub target_speed: f64,
    /// Fraction (0..=1) of the remaining letters to unlock up front.
    pub alphabet_size: f64,
    /// Judge keys by current rather than best-ever speed.
    pub recover_keys: bool,
}

impl Default for LessonSettings {
    fn default() -> Self {
        Self {
            target_speed: 175.0,
            alphabet_size: 0.0,
            recover_keys: false,
        }
    }
}

/// How close a key's time to type is to the target: 1.0 means on target,
/// above 1.0 is faster than the target.
pub fn confidence(target_speed: f64, time_to_type: Option<f64>) -> Option<f64> {
    time_to_type.map(|t| speed_to_time(target_speed) / t)
}

#[derive(Debug, Clone, PartialEq)]
pub struct LessonKey {
    pub letter: char,
    pub time_to_type: Option<f64>,
    pub best_time_to_type: Option<f64>,
    pub confidence: Option<f64>,
    pub best_confidence: Option<f64>,
    pub included: bool,
    /// Included only because of `alphabet_size`.
    pub forced: bool,
    pub focused: bool,
}

/// All lesson letters in unlock order, with their state.
#[derive(Debug, Clone, PartialEq)]
pub struct LessonKeys(pub Vec<LessonKey>);

impl LessonKeys {
    /// Computes the keys for the next lesson. `letters` must be in unlock
    /// order (most frequent first).
    pub fn update(letters: &[char], stats: &KeyStatsMap, settings: &LessonSettings) -> Self {
        let max_size = MIN_ALPHABET
            + (letters.len().saturating_sub(MIN_ALPHABET) as f64 * settings.alphabet_size).round()
                as usize;
        let mut keys: Vec<LessonKey> = letters
            .iter()
            .map(|&letter| {
                let s = stats.get(letter);
                LessonKey {
                    letter,
                    time_to_type: s.time_to_type,
                    best_time_to_type: s.best_time_to_type,
                    confidence: confidence(settings.target_speed, s.time_to_type),
                    best_confidence: confidence(settings.target_speed, s.best_time_to_type),
                    included: false,
                    forced: false,
                    focused: false,
                }
            })
            .collect();

        let current = |k: &LessonKey| k.confidence.unwrap_or(0.0);
        let best = |k: &LessonKey| k.best_confidence.unwrap_or(0.0);

        for i in 0..keys.len() {
            let included: Vec<&LessonKey> = keys.iter().filter(|k| k.included).collect();
            let key = &keys[i];
            let (include, force) = if included.len() < MIN_ALPHABET {
                (true, false)
            } else if included.len() < max_size {
                (true, true)
            } else if best(key) >= 1.0 {
                // Keys that were ever confident stay unlocked.
                (true, false)
            } else if settings.recover_keys {
                // Unlock a new key only when all included keys are fast now.
                (included.iter().all(|k| current(k) >= 1.0), false)
            } else {
                // Unlock a new key only when all included keys were once fast.
                (included.iter().all(|k| best(k) >= 1.0), false)
            };
            keys[i].included = include;
            keys[i].forced = force;
        }

        let conf = |k: &LessonKey| {
            if settings.recover_keys {
                current(k)
            } else {
                best(k)
            }
        };
        // A stable minimum: on ties the earlier (more frequent) letter wins.
        let weakest = keys
            .iter()
            .enumerate()
            .filter(|(_, k)| k.included && conf(k) < 1.0)
            .min_by(|(_, a), (_, b)| conf(a).total_cmp(&conf(b)))
            .map(|(i, _)| i);
        if let Some(i) = weakest {
            keys[i].focused = true;
        }
        LessonKeys(keys)
    }

    pub fn included(&self) -> impl Iterator<Item = &LessonKey> {
        self.0.iter().filter(|k| k.included)
    }

    pub fn included_letters(&self) -> Vec<char> {
        self.included().map(|k| k.letter).collect()
    }

    pub fn focused(&self) -> Option<char> {
        self.0.iter().find(|k| k.focused).map(|k| k.letter)
    }

    pub fn get(&self, letter: char) -> Option<&LessonKey> {
        self.0.iter().find(|k| k.letter == letter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::keystats::KeyStats;

    const LETTERS: [char; 10] = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J'];

    /// Builds stats with the given (confidence, best confidence) per letter.
    fn fake_stats(
        settings: &LessonSettings,
        conf: [(Option<f64>, Option<f64>); 10],
    ) -> KeyStatsMap {
        let target = speed_to_time(settings.target_speed);
        let mut map = KeyStatsMap::new(&LETTERS);
        for (&letter, (c, b)) in LETTERS.iter().zip(conf) {
            map.insert(KeyStats::with_times(
                letter,
                c.map(|c| target / c),
                b.map(|b| target / b),
            ));
        }
        map
    }

    fn print(keys: &LessonKeys) -> String {
        keys.included()
            .map(|k| {
                let mut s = k.letter.to_string();
                if k.forced {
                    s = format!("!{s}");
                }
                if k.focused {
                    s = format!("[{s}]");
                }
                s
            })
            .collect()
    }

    fn run(
        recover_keys: bool,
        alphabet_size: f64,
        conf: [(Option<f64>, Option<f64>); 10],
    ) -> String {
        let settings = LessonSettings {
            recover_keys,
            alphabet_size,
            ..Default::default()
        };
        print(&LessonKeys::update(
            &LETTERS,
            &fake_stats(&settings, conf),
            &settings,
        ))
    }

    const N: (Option<f64>, Option<f64>) = (None, None);
    const ONE: (Option<f64>, Option<f64>) = (Some(1.0), Some(1.0));
    const ONCE: (Option<f64>, Option<f64>) = (Some(0.9), Some(1.0));
    const LOW: (Option<f64>, Option<f64>) = (Some(0.5), Some(0.5));

    #[test]
    fn initial_state() {
        for recover in [false, true] {
            assert_eq!(run(recover, 0.0, [N; 10]), "[A]BCDEF");
        }
    }

    #[test]
    fn unlocked_key_without_data_when_all_now_fast() {
        let conf = [ONE, ONE, ONE, ONE, ONE, ONE, N, N, N, ONE];
        assert_eq!(run(false, 0.0, conf), "ABCDEF[G]J");
        assert_eq!(run(true, 0.0, conf), "ABCDEF[G]J");
    }

    #[test]
    fn unlocked_key_without_data_when_all_once_fast() {
        let conf = [ONCE, ONCE, ONCE, ONCE, ONCE, ONCE, N, N, N, ONE];
        assert_eq!(run(false, 0.0, conf), "ABCDEF[G]J");
        assert_eq!(run(true, 0.0, conf), "[A]BCDEFJ");
    }

    #[test]
    fn unlocked_key_with_low_confidence_when_all_now_fast() {
        let conf = [ONE, ONE, ONE, ONE, ONE, ONE, LOW, LOW, N, ONE];
        assert_eq!(run(false, 0.0, conf), "ABCDEF[G]J");
        assert_eq!(run(true, 0.0, conf), "ABCDEF[G]J");
    }

    #[test]
    fn unlocked_key_with_low_confidence_when_all_once_fast() {
        let conf = [ONCE, ONCE, ONCE, ONCE, ONCE, ONCE, LOW, LOW, N, ONE];
        assert_eq!(run(false, 0.0, conf), "ABCDEF[G]J");
        assert_eq!(run(true, 0.0, conf), "[A]BCDEFJ");
    }

    #[test]
    fn all_unlocked_some_below_target() {
        let conf = [ONE, ONE, ONE, ONE, ONE, ONE, ONE, ONE, ONE, ONCE];
        assert_eq!(run(false, 0.0, conf), "ABCDEFGHIJ");
        assert_eq!(run(true, 0.0, conf), "ABCDEFGHI[J]");
    }

    #[test]
    fn all_unlocked_all_above_target() {
        for recover in [false, true] {
            assert_eq!(run(recover, 0.0, [ONE; 10]), "ABCDEFGHIJ");
        }
    }

    #[test]
    fn manually_unlock_keys() {
        for recover in [false, true] {
            assert_eq!(run(recover, 1.0, [N; 10]), "[A]BCDEF!G!H!I!J");
        }
    }

    #[test]
    fn focus_is_weakest_key() {
        let conf = [ONE, ONE, LOW, (Some(0.3), Some(0.3)), ONE, ONE, N, N, N, N];
        assert_eq!(run(false, 0.0, conf), "ABC[D]EF");
    }

    #[test]
    fn confidence_relative_to_target() {
        // 175 CPM target = 342.857 ms per key
        assert_eq!(confidence(600.0, Some(100.0)), Some(1.0));
        assert_eq!(confidence(600.0, Some(200.0)), Some(0.5));
        assert_eq!(confidence(600.0, None), None);
    }
}
