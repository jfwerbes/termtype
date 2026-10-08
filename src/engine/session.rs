//! A typing session: a `TextInput` plus termtype's retype drill.
//!
//! With the drill on, any mistake in a word (or the space after it) fails
//! the word. Once the failed word is finished it is inserted `repeat_count`
//! more times, and each copy must be typed cleanly to count.

use super::textinput::{Feedback, Settings, TextInput};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrillSettings {
    pub enabled: bool,
    pub repeat_count: usize,
}

impl Default for DrillSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            repeat_count: 10,
        }
    }
}

/// Progress through the current drill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drill {
    pub word: String,
    /// Clean repetitions so far.
    pub done: usize,
    pub target: usize,
    /// Inserted copies not yet typed.
    pending: usize,
}

#[derive(Debug, Clone)]
pub struct Session {
    input: TextInput,
    drill_settings: DrillSettings,
    word_failed: bool,
    /// End of the last finished word, so a word is never finished twice.
    finished_until: usize,
    drill: Option<Drill>,
}

impl Session {
    /// Drill mode overrides the input settings so the cursor stops on errors.
    pub fn new(text: &str, settings: Settings, drill_settings: DrillSettings) -> Self {
        let settings = if drill_settings.enabled {
            Settings {
                stop_on_error: true,
                forgive_errors: false,
                space_skips_words: false,
            }
        } else {
            settings
        };
        Self {
            input: TextInput::new(text, settings),
            drill_settings,
            word_failed: false,
            finished_until: 0,
            drill: None,
        }
    }

    pub fn input(&self) -> &TextInput {
        &self.input
    }

    pub fn completed(&self) -> bool {
        self.input.completed()
    }

    pub fn drill(&self) -> Option<&Drill> {
        self.drill.as_ref()
    }

    pub fn type_char(&mut self, time_stamp: f64, ch: char, time_to_type: f64) -> Feedback {
        let before = self.input.pos();
        let feedback = self.input.append_char(time_stamp, ch, time_to_type);
        if !self.drill_settings.enabled {
            return feedback;
        }
        if feedback == Feedback::Failed || self.input.steps()[before..].iter().any(|s| s.typo) {
            self.word_failed = true;
        }
        let pos = self.input.pos();
        if pos > before {
            let text = self.input.text();
            let at_space = text[pos - 1] == ' ';
            let word_end = if at_space { pos - 1 } else { pos };
            if (at_space || pos == text.len()) && word_end > self.finished_until {
                self.finish_word(word_end);
            }
        }
        feedback
    }

    fn finish_word(&mut self, word_end: usize) {
        let text = self.input.text();
        let start = text[..word_end]
            .iter()
            .rposition(|&c| c == ' ')
            .map_or(0, |i| i + 1);
        let word: String = text[start..word_end].iter().collect();
        let failed = std::mem::take(&mut self.word_failed);
        self.finished_until = word_end;

        let copies = match &mut self.drill {
            Some(drill) => {
                drill.pending -= 1;
                if failed {
                    // Redo this repetition.
                    drill.pending += 1;
                } else {
                    drill.done += 1;
                }
                if drill.pending == 0 {
                    self.drill = None;
                }
                usize::from(failed)
            }
            None if failed => {
                let target = self.drill_settings.repeat_count;
                self.drill = Some(Drill {
                    word: word.clone(),
                    done: 0,
                    target,
                    pending: target,
                });
                target
            }
            None => 0,
        };
        self.insert_copies(&word, copies);
    }

    /// Inserts copies of `word` right after the cursor.
    fn insert_copies(&mut self, word: &str, copies: usize) {
        if copies == 0 {
            return;
        }
        let pos = self.input.pos();
        let text = if self.input.text()[pos - 1] == ' ' {
            format!("{word} ").repeat(copies)
        } else {
            // At the end of the text: the copies need leading spaces.
            format!(" {word}").repeat(copies)
        };
        self.input.insert(pos, &text);
    }

    pub fn backspace(&mut self) -> Feedback {
        self.input.clear_char()
    }

    pub fn clear_word(&mut self) -> Feedback {
        self.input.clear_word()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drill_session(text: &str, repeat_count: usize) -> Session {
        Session::new(
            text,
            Settings::default(),
            DrillSettings {
                enabled: true,
                repeat_count,
            },
        )
    }

    /// Types `keys`, with `#` standing for a wrong key.
    fn type_keys(s: &mut Session, keys: &str) {
        for ch in keys.chars() {
            let ts = s.input().steps().len() as f64 * 100.0;
            s.type_char(ts, if ch == '#' { '~' } else { ch }, 100.0);
        }
    }

    fn text(s: &Session) -> String {
        s.input().text().iter().collect()
    }

    fn progress(s: &Session) -> Option<(String, usize, usize)> {
        s.drill().map(|d| (d.word.clone(), d.done, d.target))
    }

    #[test]
    fn disabled_drill_changes_nothing() {
        let mut s = Session::new("ab cd", Settings::default(), DrillSettings::default());
        type_keys(&mut s, "a#b ");
        assert_eq!(text(&s), "ab cd");
        assert_eq!(s.drill(), None);
    }

    #[test]
    fn failed_word_is_repeated() {
        let mut s = drill_session("ab cd", 3);
        type_keys(&mut s, "a#b");
        assert_eq!(text(&s), "ab cd"); // not finished yet
        type_keys(&mut s, " ");
        assert_eq!(text(&s), "ab ab ab ab cd");
        assert_eq!(progress(&s), Some(("ab".into(), 0, 3)));
    }

    #[test]
    fn clean_repetitions_count_up_and_finish() {
        let mut s = drill_session("ab cd", 3);
        type_keys(&mut s, "a#b ab ");
        assert_eq!(progress(&s), Some(("ab".into(), 1, 3)));
        type_keys(&mut s, "ab ab ");
        assert_eq!(s.drill(), None);
        type_keys(&mut s, "cd");
        assert!(s.completed());
    }

    #[test]
    fn mistake_during_repetition_does_not_count() {
        let mut s = drill_session("ab cd", 2);
        type_keys(&mut s, "a#b ");
        assert_eq!(text(&s), "ab ab ab cd");
        type_keys(&mut s, "a#b ");
        assert_eq!(progress(&s), Some(("ab".into(), 0, 2)));
        assert_eq!(text(&s), "ab ab ab ab cd");
        type_keys(&mut s, "ab ab ");
        assert_eq!(s.drill(), None);
        assert_eq!(s.input().pos(), "ab ab ab ab ".len());
    }

    #[test]
    fn failed_last_word_is_appended() {
        let mut s = drill_session("ab", 2);
        type_keys(&mut s, "a#b");
        assert_eq!(text(&s), "ab ab ab");
        assert!(!s.completed());
        type_keys(&mut s, " ab ab");
        assert!(s.completed());
        assert_eq!(s.drill(), None);
    }

    #[test]
    fn mistake_on_space_fails_preceding_word() {
        let mut s = drill_session("ab cd", 1);
        type_keys(&mut s, "ab# ");
        assert_eq!(text(&s), "ab ab cd");
        assert_eq!(progress(&s), Some(("ab".into(), 0, 1)));
    }

    #[test]
    fn new_drill_after_previous_one() {
        let mut s = drill_session("ab cd", 1);
        type_keys(&mut s, "a#b ab c#d");
        assert_eq!(text(&s), "ab ab cd cd");
        assert_eq!(progress(&s), Some(("cd".into(), 0, 1)));
    }

    #[test]
    fn drill_forces_stop_on_error() {
        let settings = Settings {
            stop_on_error: false,
            forgive_errors: true,
            space_skips_words: true,
        };
        let mut s = Session::new(
            "ab cd",
            settings,
            DrillSettings {
                enabled: true,
                repeat_count: 1,
            },
        );
        type_keys(&mut s, "#");
        assert_eq!(s.input().pos(), 0);
        assert!(
            s.input()
                .chars()
                .iter()
                .all(|c| c.attr != crate::engine::textinput::Attr::Garbage)
        );
    }

    #[test]
    fn mistakes_still_recorded_as_typos() {
        let mut s = drill_session("ab cd", 1);
        type_keys(&mut s, "a#b ");
        assert!(s.input().steps()[1].typo);
    }
}
