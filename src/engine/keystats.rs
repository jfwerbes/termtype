//! Port of keybr-result `keystats.ts`: per-key learning progress.

use super::math::Filter;
use super::result::LessonResult;
use std::collections::BTreeMap;

const ALPHA: f64 = 0.1;

#[derive(Debug, Clone, PartialEq)]
pub struct KeySample {
    /// Lesson number (counts every result, not only those with this key).
    pub index: usize,
    pub time_stamp: i64,
    pub hit_count: u32,
    pub miss_count: u32,
    pub time_to_type: f64,
    pub filtered_time_to_type: f64,
}

#[derive(Debug, Clone)]
pub struct KeyStats {
    pub letter: char,
    pub samples: Vec<KeySample>,
    /// Smoothed time to type, in ms.
    pub time_to_type: Option<f64>,
    /// Lowest smoothed time to type ever reached.
    pub best_time_to_type: Option<f64>,
    filter: Filter,
    index: usize,
}

impl KeyStats {
    pub fn new(letter: char) -> Self {
        Self {
            letter,
            samples: vec![],
            time_to_type: None,
            best_time_to_type: None,
            filter: Filter::new(ALPHA),
            index: 0,
        }
    }

    pub fn append(&mut self, result: &LessonResult) {
        if let Some(s) = result.histogram.get(self.letter)
            && s.time_to_type > 0
        {
            let time_to_type = s.time_to_type as f64;
            let filtered = self.filter.add(time_to_type);
            self.samples.push(KeySample {
                index: self.index,
                time_stamp: result.time_stamp,
                hit_count: s.hit_count,
                miss_count: s.miss_count,
                time_to_type,
                filtered_time_to_type: filtered,
            });
            self.time_to_type = Some(filtered);
            self.best_time_to_type =
                Some(self.best_time_to_type.map_or(filtered, |b| b.min(filtered)));
        }
        self.index += 1;
    }
}

/// Key statistics for a fixed set of letters.
#[derive(Debug, Clone, Default)]
pub struct KeyStatsMap {
    stats: BTreeMap<char, KeyStats>,
}

impl KeyStatsMap {
    pub fn new(letters: &[char]) -> Self {
        Self {
            stats: letters.iter().map(|&c| (c, KeyStats::new(c))).collect(),
        }
    }

    pub fn from_results<'a>(
        letters: &[char],
        results: impl IntoIterator<Item = &'a LessonResult>,
    ) -> Self {
        let mut map = Self::new(letters);
        for r in results {
            map.append(r);
        }
        map
    }

    pub fn append(&mut self, result: &LessonResult) {
        for s in self.stats.values_mut() {
            s.append(result);
        }
    }

    /// Returns stats for `letter`; unknown letters have no data.
    pub fn get(&self, letter: char) -> KeyStats {
        self.stats
            .get(&letter)
            .cloned()
            .unwrap_or_else(|| KeyStats::new(letter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::result::tests::fake_result;

    #[test]
    fn compute_key_stats() {
        let r1 = fake_result(1, &[('a', 500)]);
        let r2 = fake_result(2, &[('b', 200)]);
        let r3 = fake_result(3, &[('a', 100)]);
        let mut map = KeyStatsMap::new(&['a', 'b']);

        let a = map.get('a');
        assert!(a.samples.is_empty());
        assert_eq!((a.time_to_type, a.best_time_to_type), (None, None));

        map.append(&r1);
        let a = map.get('a');
        assert_eq!(
            a.samples,
            [KeySample {
                index: 0,
                time_stamp: r1.time_stamp,
                hit_count: 1,
                miss_count: 0,
                time_to_type: 500.0,
                filtered_time_to_type: 500.0
            }]
        );
        assert_eq!(
            (a.time_to_type, a.best_time_to_type),
            (Some(500.0), Some(500.0))
        );

        map.append(&r2);
        map.append(&r3);
        let a = map.get('a');
        assert_eq!(a.samples.len(), 2);
        assert_eq!(a.samples[1].index, 2);
        assert_eq!(a.samples[1].time_stamp, r3.time_stamp);
        assert_eq!(a.samples[1].filtered_time_to_type, 460.0);
        assert_eq!(
            (a.time_to_type, a.best_time_to_type),
            (Some(460.0), Some(460.0))
        );

        let b = map.get('b');
        assert_eq!(b.samples.len(), 1);
        assert_eq!(b.samples[0].index, 1);
        assert_eq!(
            (b.time_to_type, b.best_time_to_type),
            (Some(200.0), Some(200.0))
        );
    }

    #[test]
    fn best_is_min_of_filtered_values() {
        let results: Vec<_> = [100, 1000]
            .iter()
            .enumerate()
            .map(|(i, &t)| fake_result(i as i64, &[('a', t)]))
            .collect();
        let a = KeyStatsMap::from_results(&['a'], &results).get('a');
        assert_eq!(a.best_time_to_type, Some(100.0));
        assert_eq!(a.time_to_type, Some(190.0));
    }

    #[test]
    fn zero_time_samples_are_ignored() {
        let map = KeyStatsMap::from_results(&['a'], &[fake_result(0, &[('a', 0)])]);
        assert!(map.get('a').samples.is_empty());
    }
}
