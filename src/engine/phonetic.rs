//! Port of keybr-phonetic-model: a Markov chain over letters that generates
//! pronounceable pseudo-words from a restricted alphabet.

use super::rng::{Rng, random_sample, weighted_sample};
use std::collections::{HashMap, HashSet};

const MIN_LENGTH: usize = 3;
const MAX_LENGTH: usize = 10;
const SIGNATURE: &[u8] = b"keybr.com";
/// Generation attempts before giving up on a censored word.
const CENSOR_ATTEMPTS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub ch: char,
    pub frequency: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    BadSignature,
    Truncated,
    BadData,
}

/// Transition frequencies from each context of `order - 1` characters.
#[derive(Debug, Clone)]
pub struct TransitionTable {
    pub order: usize,
    /// `alphabet[0]` is the space, which ends a word.
    pub alphabet: Vec<char>,
    segments: Vec<Vec<Entry>>,
}

impl TransitionTable {
    /// Parses keybr's binary model format (big-endian).
    pub fn load(data: &[u8]) -> Result<Self, LoadError> {
        let mut r = Reader { data, pos: 0 };
        if r.take(SIGNATURE.len())? != SIGNATURE {
            return Err(LoadError::BadSignature);
        }
        let order = r.u8()? as usize;
        let size = r.u8()? as usize;
        if order < 1 || size == 0 {
            return Err(LoadError::BadData);
        }
        let alphabet = (0..size)
            .map(|_| {
                let b = r.take(2)?;
                char::from_u32(u16::from_be_bytes([b[0], b[1]]) as u32).ok_or(LoadError::BadData)
            })
            .collect::<Result<Vec<char>, _>>()?;
        let mut segments = Vec::with_capacity(size.pow(order as u32 - 1));
        for _ in 0..size.pow(order as u32 - 1) {
            let len = r.u8()? as usize;
            if len > size {
                return Err(LoadError::BadData);
            }
            let mut segment = Vec::with_capacity(len);
            for _ in 0..len {
                let index = r.u8()? as usize;
                let frequency = r.u8()?;
                if index >= size || frequency == 0 {
                    return Err(LoadError::BadData);
                }
                segment.push(Entry {
                    ch: alphabet[index],
                    frequency,
                });
            }
            segments.push(segment);
        }
        if r.pos != data.len() {
            return Err(LoadError::BadData);
        }
        Ok(Self {
            order,
            alphabet,
            segments,
        })
    }

    /// Transitions from the last `order - 1` characters of `word`, padded
    /// with spaces at the front.
    pub fn segment(&self, word: &[char]) -> &[Entry] {
        let size = self.alphabet.len();
        let context = self.order - 1;
        let index = (0..context).fold(0, |acc, i| {
            // Position in `word` of the i-th context character, if any.
            let ch = (word.len() + i)
                .checked_sub(context)
                .and_then(|j| word.get(j))
                .copied()
                .unwrap_or(' ');
            acc * size + self.index_of(ch)
        });
        &self.segments[index]
    }

    fn index_of(&self, ch: char) -> usize {
        self.alphabet.iter().position(|&c| c == ch).unwrap_or(0)
    }

    /// Letters (excluding space) with relative frequencies summing to 1,
    /// most frequent first, ties broken by character.
    pub fn letters(&self) -> Vec<(char, f64)> {
        let mut totals: HashMap<char, f64> = HashMap::new();
        for e in self.segments.iter().flatten() {
            *totals.entry(e.ch).or_default() += e.frequency as f64;
        }
        let mut letters: Vec<(char, f64)> = self
            .alphabet
            .iter()
            .filter(|&&c| c != ' ')
            .map(|&c| (c, totals.get(&c).copied().unwrap_or(0.0)))
            .collect();
        let sum: f64 = letters.iter().map(|(_, f)| f).sum();
        if sum > 0.0 {
            letters.iter_mut().for_each(|(_, f)| *f /= sum);
        }
        letters.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        letters
    }
}

/// Restricts generated words to a letter set, optionally requiring a letter.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// `None` allows every letter.
    pub letters: Option<HashSet<char>>,
    pub focused: Option<char>,
}

impl Filter {
    pub fn new(letters: &[char], focused: Option<char>) -> Self {
        Self {
            letters: Some(letters.iter().copied().collect()),
            focused,
        }
    }

    pub fn includes(&self, ch: char) -> bool {
        self.letters.as_ref().is_none_or(|l| l.contains(&ch))
    }
}

/// Reverses keybr's obfuscation of blacklist entries.
pub fn unscramble_word(word: &str) -> String {
    let a: Vec<char> = word.chars().collect();
    (0..a.len()).map(|i| a[(23 * i + 13) % a.len()]).collect()
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], LoadError> {
        let bytes = self
            .data
            .get(self.pos..self.pos + n)
            .ok_or(LoadError::Truncated)?;
        self.pos += n;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, LoadError> {
        Ok(self.take(1)?[0])
    }
}

pub struct PhoneticModel {
    table: TransitionTable,
    /// Word beginnings of 1..=3 letters, keyed by the letter they end with.
    prefixes: HashMap<char, Vec<Vec<char>>>,
    blacklist: HashSet<String>,
}

impl PhoneticModel {
    pub fn new(table: TransitionTable, blacklist: HashSet<String>) -> Self {
        let prefixes = build_prefixes(&table);
        Self {
            table,
            prefixes,
            blacklist,
        }
    }

    /// The bundled English model and blacklist.
    pub fn english() -> Self {
        let table = TransitionTable::load(include_bytes!("../../assets/model-en.data"))
            .expect("bundled model");
        let scrambled: Vec<String> =
            serde_json::from_str(include_str!("../../assets/blacklist-en.json"))
                .expect("bundled blacklist");
        Self::new(
            table,
            scrambled.iter().map(|w| unscramble_word(w)).collect(),
        )
    }

    pub fn table(&self) -> &TransitionTable {
        &self.table
    }

    /// Letters in frequency order: the order in which they are unlocked.
    pub fn letters(&self) -> Vec<char> {
        self.table.letters().into_iter().map(|(c, _)| c).collect()
    }

    /// Generates a word, re-drawing words from the blacklist.
    pub fn next_word(&self, filter: &Filter, rng: &mut impl Rng) -> String {
        let mut word = self.next_word_uncensored(filter, rng);
        for _ in 0..CENSOR_ATTEMPTS {
            if !self.blacklist.contains(&word) {
                break;
            }
            word = self.next_word_uncensored(filter, rng);
        }
        word
    }

    pub fn next_word_uncensored(&self, filter: &Filter, rng: &mut impl Rng) -> String {
        let prefixes = self.find_prefixes(filter);
        let mut word: Vec<char> = vec![];
        let mut attempt = 0;
        // Restarts the word from a random prefix; false after 5 attempts.
        let mut retry = |word: &mut Vec<char>, rng: &mut _| {
            if attempt >= 5 {
                return false;
            }
            attempt += 1;
            word.clear();
            if !prefixes.is_empty() {
                word.extend(random_sample(&prefixes, rng));
            }
            true
        };
        retry(&mut word, rng);
        loop {
            let entries: Vec<(char, f64)> = self
                .table
                .segment(&word)
                .iter()
                .filter(|e| {
                    if e.ch == ' ' {
                        word.len() >= MIN_LENGTH
                    } else {
                        filter.includes(e.ch)
                    }
                })
                .map(|e| {
                    let f = e.frequency as f64;
                    // Boost the space to favour shorter words.
                    (
                        e.ch,
                        if e.ch == ' ' {
                            f * 1.3f64.powi(word.len() as i32)
                        } else {
                            f
                        },
                    )
                })
                .collect();
            if entries.is_empty() {
                if retry(&mut word, rng) {
                    continue;
                }
                break;
            }
            let (ch, _) = *weighted_sample(&entries, |e| e.1, rng);
            if ch == ' ' {
                break;
            }
            if word.len() > MAX_LENGTH {
                if retry(&mut word, rng) {
                    continue;
                }
                break;
            }
            word.push(ch);
        }
        word.into_iter().collect()
    }

    /// Word beginnings that end with the focused letter and fit the filter.
    fn find_prefixes(&self, filter: &Filter) -> Vec<Vec<char>> {
        let Some(focused) = filter.focused else {
            return vec![];
        };
        let found: Vec<Vec<char>> = self
            .prefixes
            .get(&focused)
            .into_iter()
            .flatten()
            .filter(|p| p.iter().all(|&c| filter.includes(c)))
            .cloned()
            .collect();
        if found.is_empty() {
            vec![vec![focused]]
        } else {
            found
        }
    }
}

/// Walks the table from the start of a word, collecting every reachable
/// prefix of up to `MIN_LENGTH` letters.
///
/// keybr pushes each prefix under its *last* letter once per distinct letter
/// it contains (probably meant to index it under each letter). We keep that
/// behaviour so generated text matches keybr's.
fn build_prefixes(table: &TransitionTable) -> HashMap<char, Vec<Vec<char>>> {
    fn walk(
        table: &TransitionTable,
        word: &mut Vec<char>,
        map: &mut HashMap<char, Vec<Vec<char>>>,
    ) {
        for e in table.segment(word).to_vec() {
            if e.ch == ' ' {
                continue;
            }
            word.push(e.ch);
            let distinct = word.iter().collect::<HashSet<_>>().len();
            let list = map.entry(e.ch).or_default();
            list.extend(std::iter::repeat_n(word.clone(), distinct));
            if word.len() < MIN_LENGTH {
                walk(table, word, map);
            }
            word.pop();
        }
    }
    let mut map = HashMap::new();
    walk(table, &mut vec![], &mut map);
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::rng::{Lcg, LessonRng};

    /// Encodes a table in keybr's binary format.
    fn encode(order: u8, alphabet: &[char], segments: &[&[(usize, u8)]]) -> Vec<u8> {
        let mut out = SIGNATURE.to_vec();
        out.push(order);
        out.push(alphabet.len() as u8);
        for &c in alphabet {
            out.extend_from_slice(&(c as u16).to_be_bytes());
        }
        for seg in segments {
            out.push(seg.len() as u8);
            for &(i, f) in *seg {
                out.push(i as u8);
                out.push(f);
            }
        }
        out
    }

    /// Order 2 over {space, a, b}: words are runs of one letter.
    fn runs_table() -> TransitionTable {
        let data = encode(
            2,
            &[' ', 'a', 'b'],
            &[&[(1, 1), (2, 1)], &[(0, 1), (1, 1)], &[(0, 1), (2, 1)]],
        );
        TransitionTable::load(&data).unwrap()
    }

    #[test]
    fn load_english_model() {
        let m = PhoneticModel::english();
        assert_eq!(m.table().order, 4);
        assert_eq!(m.table().alphabet.len(), 27);
        assert_eq!(m.table().alphabet[0], ' ');
    }

    #[test]
    fn load_rejects_bad_input() {
        let good = encode(2, &[' ', 'a'], &[&[(1, 1)], &[(0, 1)]]);
        assert!(TransitionTable::load(&good).is_ok());
        let mut bad_sig = good.clone();
        bad_sig[0] = b'x';
        assert_eq!(
            TransitionTable::load(&bad_sig).unwrap_err(),
            LoadError::BadSignature
        );
        assert_eq!(
            TransitionTable::load(&good[..good.len() - 1]).unwrap_err(),
            LoadError::Truncated
        );
        let mut trailing = good.clone();
        trailing.push(0);
        assert_eq!(
            TransitionTable::load(&trailing).unwrap_err(),
            LoadError::BadData
        );
        let zero_freq = encode(2, &[' ', 'a'], &[&[(1, 0)], &[(0, 1)]]);
        assert_eq!(
            TransitionTable::load(&zero_freq).unwrap_err(),
            LoadError::BadData
        );
        let bad_index = encode(2, &[' ', 'a'], &[&[(5, 1)], &[(0, 1)]]);
        assert_eq!(
            TransitionTable::load(&bad_index).unwrap_err(),
            LoadError::BadData
        );
    }

    #[test]
    fn segment_uses_last_chars_padded_with_spaces() {
        // order 3 over {space, a}: segment index = idx(c1) * 2 + idx(c2)
        let data = encode(
            3,
            &[' ', 'a'],
            &[&[(1, 10)], &[(1, 11)], &[(1, 12)], &[(0, 13)]],
        );
        let t = TransitionTable::load(&data).unwrap();
        assert_eq!(t.segment(&[])[0].frequency, 10); // "  "
        assert_eq!(t.segment(&['a'])[0].frequency, 11); // " a"
        assert_eq!(t.segment(&['a', 'a'])[0].frequency, 13); // "aa"
        assert_eq!(t.segment(&['a', 'a', 'a'])[0].frequency, 13);
    }

    #[test]
    fn english_letters_by_frequency() {
        let letters = PhoneticModel::english().table().letters();
        assert_eq!(letters.len(), 26);
        assert!(!letters.iter().any(|(c, _)| *c == ' '));
        let sum: f64 = letters.iter().map(|(_, f)| f).sum();
        assert!((sum - 1.0).abs() < 1e-9);
        assert!(letters.windows(2).all(|w| w[0].1 >= w[1].1));
        let first: String = letters.iter().take(6).map(|(c, _)| c).collect();
        assert_eq!(first, "eniarl");
    }

    #[test]
    fn words_use_only_filter_letters_and_contain_focus() {
        let m = PhoneticModel::english();
        let filter = Filter::new(&['e', 'n', 'i', 't', 'r', 'l'], Some('l'));
        let mut rng = Lcg::new(42);
        for _ in 0..500 {
            let w = m.next_word(&filter, &mut rng);
            assert!(w.chars().all(|c| filter.includes(c)), "{w}");
            assert!(w.contains('l'), "{w}");
            assert!((1..=MAX_LENGTH + 1).contains(&w.chars().count()), "{w}");
        }
    }

    #[test]
    fn words_without_focus_have_min_length() {
        let m = PhoneticModel::english();
        let filter = Filter::new(&['e', 'n', 'i', 't', 'r', 'l', 's', 'a', 'o'], None);
        let mut rng = LessonRng::seeded(7);
        let words: Vec<String> = (0..300).map(|_| m.next_word(&filter, &mut rng)).collect();
        assert!(
            words.iter().all(|w| w.chars().count() >= MIN_LENGTH),
            "{words:?}"
        );
        let distinct: HashSet<_> = words.iter().collect();
        assert!(
            distinct.len() > 100,
            "{} distinct: {words:?}",
            distinct.len()
        );
    }

    #[test]
    fn empty_table_yields_focus_or_nothing() {
        let data = encode(2, &[' ', 'a', 'b'], &[&[], &[], &[]]);
        let m = PhoneticModel::new(TransitionTable::load(&data).unwrap(), HashSet::new());
        let mut rng = Lcg::new(1);
        assert_eq!(m.next_word(&Filter::default(), &mut rng), "");
        assert_eq!(m.next_word(&Filter::new(&['a'], None), &mut rng), "");
        assert_eq!(m.next_word(&Filter::new(&['a'], Some('a')), &mut rng), "a");
        assert_eq!(
            m.next_word(&Filter::new(&['a', 'b'], Some('a')), &mut rng),
            "a"
        );
    }

    #[test]
    fn prefixes_are_keyed_by_last_letter() {
        let p = build_prefixes(&runs_table());
        assert!(p[&'a'].contains(&vec!['a']));
        assert!(p[&'a'].contains(&vec!['a', 'a', 'a']));
        assert!(p[&'a'].iter().all(|w| w.last() == Some(&'a')));
        assert!(p[&'b'].iter().all(|w| w.iter().all(|&c| c == 'b')));
    }

    #[test]
    fn censor_redraws_blacklisted_words() {
        let raw = PhoneticModel::new(runs_table(), HashSet::new());
        let censored = PhoneticModel::new(runs_table(), HashSet::from(["aaa".to_string()]));
        let filter = Filter::default();
        let mut rng = Lcg::new(3);
        assert!((0..200).any(|_| raw.next_word(&filter, &mut rng) == "aaa"));
        assert!((0..200).all(|_| censored.next_word(&filter, &mut rng) != "aaa"));
    }

    #[test]
    fn unscramble_matches_keybr() {
        assert_eq!(unscramble_word("a"), "a");
        assert_eq!(unscramble_word("ba"), "ab");
        assert_eq!(unscramble_word("bac"), "abc");
        assert_eq!(unscramble_word("bafedc"), "abcdef");
    }
}
