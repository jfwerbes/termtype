//! Port of keybr-lesson `dictionary.ts`, `text/words.ts`, `text/fragment.ts`
//! and `GuidedLesson.generate`: builds the text of a guided lesson.

use super::lesson::LessonKeys;
use super::phonetic::{Filter, PhoneticModel};
use super::rng::{Rng, random_sample};

/// Real words to practice with, most frequent first.
#[derive(Debug, Clone, Default)]
pub struct Dictionary {
    words: Vec<String>,
}

impl Dictionary {
    /// Keeps words longer than two characters.
    pub fn new(words: impl IntoIterator<Item = String>) -> Self {
        Self {
            words: words
                .into_iter()
                .filter(|w| w.chars().count() > 2)
                .collect(),
        }
    }

    /// The bundled English word list.
    pub fn english() -> Self {
        let words: Vec<String> = serde_json::from_str(include_str!("../../assets/words-en.json"))
            .expect("bundled word list");
        Self::new(words)
    }

    /// Words made only of the filter's letters and containing its focused
    /// letter, in frequency order.
    pub fn find(&self, filter: &Filter) -> Vec<&str> {
        self.words
            .iter()
            .filter(|w| filter.focused.is_none_or(|f| w.contains(f)))
            .filter(|w| w.chars().all(|c| filter.includes(c)))
            .map(String::as_str)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSettings {
    /// 0..=1, scales the lesson from 100 to 200 characters.
    pub length: f64,
    /// Prefer real words over pseudo-words.
    pub natural_words: bool,
    /// How many times each word is repeated in a row.
    pub repeat_words: usize,
}

impl Default for TextSettings {
    fn default() -> Self {
        Self {
            length: 0.0,
            natural_words: true,
            repeat_words: 1,
        }
    }
}

/// Joins generated words until their letters reach `100 + length * 100`.
/// An exhausted generator yields `?` placeholders.
pub fn fragment(
    mut next_word: impl FnMut() -> Option<String>,
    length: f64,
    repeat_words: usize,
) -> String {
    let target = 100 + (length * 100.0).round() as usize;
    let mut words: Vec<String> = vec![];
    let mut total = 0;
    loop {
        let word = next_word()
            .filter(|w| !w.is_empty())
            .unwrap_or_else(|| "?".to_string());
        for _ in 0..repeat_words.max(1) {
            total += word.chars().count();
            words.push(word.clone());
            if total >= target {
                return words.join(" ");
            }
        }
    }
}

/// Wraps a generator to avoid the same word twice in a row (up to 3 tries).
pub fn unique_words(
    mut next_word: impl FnMut() -> Option<String>,
) -> impl FnMut() -> Option<String> {
    let mut last = String::new();
    move || {
        let mut word = None;
        for _ in 0..3 {
            let w = next_word().filter(|w| !w.is_empty())?;
            if w != last {
                last.clone_from(&w);
                return Some(w);
            }
            word = Some(w);
        }
        word
    }
}

/// Generates the text of a guided lesson for the given keys.
pub fn generate(
    model: &PhoneticModel,
    dictionary: &Dictionary,
    keys: &LessonKeys,
    settings: &TextSettings,
    rng: &mut impl Rng,
) -> String {
    let filter = Filter::new(&keys.included_letters(), keys.focused());
    let words = if settings.natural_words {
        let mut words: Vec<String> = dictionary
            .find(&filter)
            .into_iter()
            .take(1000)
            .map(String::from)
            .collect();
        while words.len() < 15 {
            let w = model.next_word(&filter, rng);
            if w.is_empty() {
                break;
            }
            words.push(w);
        }
        if words.is_empty() {
            words.push("?".to_string());
        }
        Some(words)
    } else {
        None
    };
    let next_word = || match &words {
        Some(words) => Some(random_sample(words, rng).clone()),
        None => Some(model.next_word(&filter, rng)).filter(|w| !w.is_empty()),
    };
    fragment(
        unique_words(next_word),
        settings.length,
        settings.repeat_words,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::keystats::KeyStatsMap;
    use crate::engine::lesson::LessonSettings;
    use crate::engine::rng::LessonRng;

    fn dict(words: &[&str]) -> Dictionary {
        Dictionary::new(words.iter().map(|w| w.to_string()))
    }

    fn from_list(words: &[&str]) -> impl FnMut() -> Option<String> {
        let mut it = words
            .iter()
            .map(|w| w.to_string())
            .collect::<Vec<_>>()
            .into_iter()
            .cycle();
        move || it.next()
    }

    #[test]
    fn dictionary_find_filters_and_requires_focus() {
        let d = dict(&["the", "tree", "net", "ten", "it", "tin", "rent", "zebra"]);
        let letters = ['e', 'n', 'i', 't', 'r'];
        assert_eq!(d.find(&Filter::new(&letters, Some('r'))), ["tree", "rent"]);
        assert_eq!(
            d.find(&Filter::new(&letters, None)),
            ["tree", "net", "ten", "tin", "rent"]
        );
        assert_eq!(d.find(&Filter::default()).len(), 7); // "it" is too short
    }

    #[test]
    fn fragment_stops_at_target_length() {
        let text = fragment(from_list(&["abcd"]), 0.0, 1);
        // 25 words * 4 letters = 100
        assert_eq!(text.split(' ').count(), 25);
        let text = fragment(from_list(&["abc"]), 0.5, 1);
        // ceil(150 / 3) = 50 words
        assert_eq!(text.split(' ').count(), 50);
    }

    #[test]
    fn fragment_repeats_words() {
        let text = fragment(from_list(&["one", "two"]), 0.0, 3);
        assert!(text.starts_with("one one one two two two one"));
    }

    #[test]
    fn fragment_placeholder_when_exhausted() {
        let text = fragment(|| None, 0.0, 1);
        assert_eq!(text.split(' ').count(), 100);
        assert!(text.split(' ').all(|w| w == "?"));
    }

    #[test]
    fn unique_words_skips_repeats() {
        let mut g = unique_words(from_list(&["a", "a", "b"]));
        assert_eq!(g().as_deref(), Some("a"));
        assert_eq!(g().as_deref(), Some("b")); // skipped the second "a"
        assert_eq!(g().as_deref(), Some("a"));
        let mut only = unique_words(from_list(&["a"]));
        assert_eq!(only().as_deref(), Some("a"));
        assert_eq!(only().as_deref(), Some("a")); // gives up after 3 tries
    }

    fn first_lesson_keys(model: &PhoneticModel) -> LessonKeys {
        let letters = model.letters();
        LessonKeys::update(
            &letters,
            &KeyStatsMap::new(&letters),
            &LessonSettings::default(),
        )
    }

    #[test]
    fn generate_natural_words() {
        let model = PhoneticModel::english();
        let keys = first_lesson_keys(&model);
        let included = keys.included_letters();
        let focus = keys.focused().unwrap();
        let text = generate(
            &model,
            &Dictionary::english(),
            &keys,
            &TextSettings::default(),
            &mut LessonRng::seeded(1),
        );
        assert!(text.chars().count() >= 100);
        for w in text.split(' ') {
            assert!(w.chars().all(|c| included.contains(&c)), "{w}");
            assert!(w.contains(focus), "{w}");
        }
    }

    #[test]
    fn generate_pseudo_words_only() {
        let model = PhoneticModel::english();
        let keys = first_lesson_keys(&model);
        let settings = TextSettings {
            natural_words: false,
            ..Default::default()
        };
        let text = generate(
            &model,
            &dict(&[]),
            &keys,
            &settings,
            &mut LessonRng::seeded(2),
        );
        let included = keys.included_letters();
        assert!(
            text.chars().all(|c| c == ' ' || included.contains(&c)),
            "{text}"
        );
    }

    #[test]
    fn natural_words_topped_up_with_pseudo_words() {
        let model = PhoneticModel::english();
        let keys = first_lesson_keys(&model); // e n i a r l, focus e
        let text = generate(
            &model,
            &dict(&["linear"]),
            &keys,
            &TextSettings::default(),
            &mut LessonRng::seeded(3),
        );
        assert!(text.split(' ').any(|w| w == "linear"));
        assert!(text.split(' ').any(|w| w != "linear"));
    }
}
