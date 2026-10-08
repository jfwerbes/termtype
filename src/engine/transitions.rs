//! Weak-transition drills: lessons built around the slowest key pairs.
//! Not part of keybr; uses the bigram timings termtype records.

use super::phonetic::{Filter, PhoneticModel};
use super::result::{LessonResult, slowest_bigrams};
use super::rng::{Rng, random_sample};
use super::textgen::{Dictionary, TextSettings, fragment};

/// Lessons of history to judge transitions by.
pub const RECENT_LESSONS: usize = 50;
/// Occurrences needed before a transition's timing is trusted.
pub const MIN_HITS: u32 = 10;
/// How many transitions a drill targets at once.
pub const TARGETS: usize = 3;

/// Pseudo-words generated while looking for ones containing a target.
const PSEUDO_ATTEMPTS: usize = 300;
/// Candidate words wanted per target before giving up on more.
const WORDS_PER_TARGET: usize = 30;

/// The slowest letter-to-letter transitions over recent lessons, using only
/// `letters`, slowest first. Transitions involving a space are left out.
pub fn weak_transitions(results: &[LessonResult], letters: &[char]) -> Vec<String> {
    let recent = &results[results.len().saturating_sub(RECENT_LESSONS)..];
    slowest_bigrams(recent, letters, MIN_HITS, usize::MAX)
        .into_iter()
        .map(|b| b.bigram)
        .filter(|b| !b.contains(' '))
        .take(TARGETS)
        .collect()
}

/// Lesson text whose words take turns containing each target transition.
/// Real words come first, topped up with pseudo-words; a target that fits
/// no word is drilled on its own.
pub fn generate(
    model: &PhoneticModel,
    dictionary: &Dictionary,
    targets: &[String],
    letters: &[char],
    settings: &TextSettings,
    rng: &mut impl Rng,
) -> String {
    let all = Filter::new(letters, None);
    let pools: Vec<Vec<String>> = targets
        .iter()
        .map(|target| {
            let mut words: Vec<String> = dictionary
                .find(&all)
                .into_iter()
                .filter(|w| w.contains(target.as_str()))
                .take(1000)
                .map(String::from)
                .collect();
            // Pseudo-words starting near the pair make it more likely to appear.
            let first = target.chars().next().filter(|c| letters.contains(c));
            let filter = Filter::new(letters, first);
            for _ in 0..PSEUDO_ATTEMPTS {
                if words.len() >= WORDS_PER_TARGET {
                    break;
                }
                let w = model.next_word(&filter, rng);
                if w.contains(target.as_str()) && !words.contains(&w) {
                    words.push(w);
                }
            }
            if words.is_empty() {
                words.push(target.clone());
            }
            words
        })
        .collect();
    if pools.is_empty() {
        return String::new();
    }

    let mut turn = 0;
    let mut last: Vec<Option<String>> = vec![None; pools.len()];
    let next_word = || {
        let i = turn % pools.len();
        turn += 1;
        // Avoid repeating this target's previous word when there is a choice.
        let mut word = random_sample(&pools[i], rng).clone();
        for _ in 0..3 {
            if last[i].as_ref() != Some(&word) {
                break;
            }
            word = random_sample(&pools[i], rng).clone();
        }
        last[i] = Some(word.clone());
        Some(word)
    };
    fragment(next_word, settings.length, settings.repeat_words)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::result::tests::with_bigrams;
    use crate::engine::rng::LessonRng;

    const LETTERS: [char; 7] = ['e', 'n', 'i', 'a', 'r', 'l', 't'];

    #[test]
    fn weak_transitions_ranks_letter_pairs_only() {
        let results = vec![with_bigrams(&[
            ("ea", 20, 0, 300),
            ("nt", 20, 0, 500),
            ("e ", 20, 0, 900), // space: excluded
            ("rl", 20, 0, 400),
            ("ti", 20, 0, 200),
            ("qu", 20, 0, 999), // locked letters: excluded
            ("la", 5, 0, 800),  // too few hits
        ])];
        assert_eq!(weak_transitions(&results, &LETTERS), ["nt", "rl", "ea"]);
    }

    #[test]
    fn weak_transitions_uses_recent_lessons_only() {
        let mut results = vec![with_bigrams(&[("nt", 20, 0, 900)])];
        results.extend((0..RECENT_LESSONS).map(|_| with_bigrams(&[("ea", 20, 0, 300)])));
        assert_eq!(weak_transitions(&results, &LETTERS), ["ea"]);
    }

    #[test]
    fn weak_transitions_empty_without_data() {
        assert!(weak_transitions(&[], &LETTERS).is_empty());
    }

    fn targets(t: &[&str]) -> Vec<String> {
        t.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn every_word_contains_a_target_and_uses_only_letters() {
        let model = PhoneticModel::english();
        let dict = Dictionary::english();
        let t = targets(&["nt", "rl", "ea"]);
        let text = generate(
            &model,
            &dict,
            &t,
            &LETTERS,
            &TextSettings::default(),
            &mut LessonRng::seeded(1),
        );
        assert!(text.chars().count() >= 100);
        for w in text.split(' ') {
            assert!(t.iter().any(|b| w.contains(b.as_str())), "{w} in {text}");
            assert!(w.chars().all(|c| LETTERS.contains(&c)), "{w}");
        }
    }

    #[test]
    fn targets_take_turns() {
        let model = PhoneticModel::english();
        let dict = Dictionary::english();
        let t = targets(&["nt", "rl", "ea"]);
        let text = generate(
            &model,
            &dict,
            &t,
            &LETTERS,
            &TextSettings::default(),
            &mut LessonRng::seeded(2),
        );
        let words: Vec<&str> = text.split(' ').collect();
        for (i, w) in words.iter().enumerate() {
            assert!(
                w.contains(t[i % 3].as_str()),
                "word {i} {w} should drill {}",
                t[i % 3]
            );
        }
    }

    #[test]
    fn pseudo_words_fill_in_without_dictionary() {
        let model = PhoneticModel::english();
        let t = targets(&["nt"]);
        let text = generate(
            &model,
            &Dictionary::default(),
            &t,
            &LETTERS,
            &TextSettings::default(),
            &mut LessonRng::seeded(3),
        );
        let words: Vec<&str> = text.split(' ').collect();
        assert!(words.iter().all(|w| w.contains("nt")), "{text}");
        assert!(words.iter().any(|w| *w != "nt"), "{text}");
    }

    #[test]
    fn impossible_target_is_drilled_alone() {
        let model = PhoneticModel::english();
        let letters = ['x', 'z'];
        let text = generate(
            &model,
            &Dictionary::default(),
            &targets(&["xz"]),
            &letters,
            &TextSettings::default(),
            &mut LessonRng::seeded(4),
        );
        assert!(text.split(' ').all(|w| w.contains("xz")), "{text}");
    }
}
