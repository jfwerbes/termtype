//! Port of keybr-textinput `textinput.ts`: turns keystrokes into steps.

/// The outcome of one input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feedback {
    Succeeded,
    Recovered,
    Failed,
}

/// One position of the text that has been typed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    pub time_stamp: f64,
    pub ch: char,
    /// Milliseconds since the previous keystroke; 0 for synthetic steps.
    pub time_to_type: f64,
    /// Whether any mistake was made at this position.
    pub typo: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub stop_on_error: bool,
    pub forgive_errors: bool,
    pub space_skips_words: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            stop_on_error: true,
            forgive_errors: true,
            space_skips_words: false,
        }
    }
}

/// How a character should be displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attr {
    /// Not typed yet.
    Normal,
    /// Typed correctly the first time.
    Hit,
    /// Typed after one or more mistakes.
    Miss,
    /// A wrong key, shown only when `stop_on_error` is off.
    Garbage,
    /// The next character to type.
    Cursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyledChar {
    pub ch: char,
    pub attr: Attr,
}

const RECOVER_BUFFER_LENGTH: usize = 3;
const GARBAGE_BUFFER_LENGTH: usize = 10;

#[derive(Debug, Clone)]
pub struct TextInput {
    pub settings: Settings,
    text: Vec<char>,
    steps: Vec<Step>,
    garbage: Vec<Step>,
    typo: bool,
}

impl TextInput {
    pub fn new(text: &str, settings: Settings) -> Self {
        Self {
            settings,
            text: text.chars().collect(),
            steps: vec![],
            garbage: vec![],
            typo: false,
        }
    }

    pub fn text(&self) -> &[char] {
        &self.text
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn pos(&self) -> usize {
        self.steps.len()
    }

    pub fn completed(&self) -> bool {
        self.pos() == self.len()
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// Whether a mistake has been made at the current position.
    pub fn has_typo(&self) -> bool {
        self.typo
    }

    pub fn chars(&self) -> Vec<StyledChar> {
        let mut out: Vec<StyledChar> = self
            .steps
            .iter()
            .zip(&self.text)
            .map(|(s, &ch)| StyledChar {
                ch,
                attr: if s.typo { Attr::Miss } else { Attr::Hit },
            })
            .collect();
        if !self.settings.stop_on_error {
            out.extend(self.garbage.iter().map(|g| StyledChar {
                ch: g.ch,
                attr: Attr::Garbage,
            }));
        }
        let mut rest = self.text[self.pos()..].iter();
        if let Some(&ch) = rest.next() {
            out.push(StyledChar {
                ch,
                attr: Attr::Cursor,
            });
        }
        out.extend(rest.map(|&ch| StyledChar {
            ch,
            attr: Attr::Normal,
        }));
        out
    }

    pub fn clear_char(&mut self) -> Feedback {
        self.garbage.pop();
        self.typo = true;
        Feedback::Succeeded
    }

    pub fn clear_word(&mut self) -> Feedback {
        self.garbage.clear();
        while self.pos() > 0 && self.text[self.pos() - 1] != ' ' {
            self.steps.pop();
        }
        self.typo = true;
        Feedback::Succeeded
    }

    /// Processes a typed character. Input after completion is ignored
    /// (keybr throws here).
    pub fn append_char(&mut self, time_stamp: f64, ch: char, time_to_type: f64) -> Feedback {
        if self.completed() {
            return Feedback::Failed;
        }
        let expected = self.text[self.pos()];

        if expected != ' ' && ch == ' ' {
            if self.settings.space_skips_words
                && ((self.pos() > 0 && self.text[self.pos() - 1] != ' ') || self.typo)
            {
                self.skip_word(time_stamp);
                return Feedback::Recovered;
            }
            if self.garbage.is_empty() && !self.typo {
                return Feedback::Succeeded;
            }
        }

        if (expected == ch || normalize(expected) == ch)
            && (self.settings.forgive_errors || self.garbage.is_empty())
        {
            let typo = self.typo;
            self.steps.push(Step {
                time_stamp,
                ch: expected,
                time_to_type,
                typo,
            });
            self.garbage.clear();
            self.typo = false;
            return if typo {
                Feedback::Recovered
            } else {
                Feedback::Succeeded
            };
        }

        self.typo = true;
        if (!self.settings.stop_on_error || self.settings.forgive_errors)
            && self.garbage.len() < GARBAGE_BUFFER_LENGTH
        {
            self.garbage.push(Step {
                time_stamp,
                ch,
                time_to_type,
                typo: false,
            });
        }
        if self.settings.forgive_errors
            && (self.handle_replaced_character() || self.handle_skipped_character())
        {
            Feedback::Recovered
        } else {
            Feedback::Failed
        }
    }

    fn skip_word(&mut self, time_stamp: f64) {
        // Skip the rest of the word, then the space after it.
        while self.pos() < self.len() && self.text[self.pos()] != ' ' {
            let ch = self.text[self.pos()];
            self.steps.push(Step {
                time_stamp,
                ch,
                time_to_type: 0.0,
                typo: true,
            });
        }
        if self.pos() < self.len() {
            self.steps.push(Step {
                time_stamp,
                ch: ' ',
                time_to_type: 0.0,
                typo: false,
            });
        }
        self.garbage.clear();
        self.typo = false;
    }

    /// text `abcd`, garbage `xbcd`: the first character was mistyped.
    fn handle_replaced_character(&mut self) -> bool {
        self.recover(1)
    }

    /// text `abcd`, garbage `bcd`: the first character was skipped.
    fn handle_skipped_character(&mut self) -> bool {
        self.recover(0)
    }

    /// Recovers when the garbage, from `offset`, matches the text following
    /// the current position.
    fn recover(&mut self, offset: usize) -> bool {
        let pos = self.pos();
        if pos + RECOVER_BUFFER_LENGTH + 1 > self.len()
            || self.garbage.len() < RECOVER_BUFFER_LENGTH + offset
        {
            return false;
        }
        if (0..RECOVER_BUFFER_LENGTH).any(|i| self.text[pos + i + 1] != self.garbage[i + offset].ch)
        {
            return false;
        }
        self.steps.push(Step {
            time_stamp: self.garbage[0].time_stamp,
            ch: self.text[pos],
            time_to_type: 0.0,
            typo: true,
        });
        let garbage = std::mem::take(&mut self.garbage);
        self.steps.extend_from_slice(&garbage[offset..]);
        self.typo = false;
        true
    }
}

/// Lets plain ASCII stand in for typographic characters.
pub fn normalize(ch: char) -> char {
    match ch {
        '‘' | '’' => '\'',
        '“' | '”' | '«' | '»' => '"',
        '¿' => '?',
        '¡' => '!',
        _ => ch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Feedback::*;

    fn input(
        text: &str,
        stop_on_error: bool,
        forgive_errors: bool,
        space_skips_words: bool,
    ) -> TextInput {
        TextInput::new(
            text,
            Settings {
                stop_on_error,
                forgive_errors,
                space_skips_words,
            },
        )
    }

    fn show_steps(t: &TextInput) -> String {
        t.steps()
            .iter()
            .map(|s| {
                let v = format!("{},{},{}", s.ch, s.time_stamp, s.time_to_type);
                if s.typo { format!("!{v}") } else { v }
            })
            .collect::<Vec<_>>()
            .join("|")
    }

    fn show_chars(t: &TextInput) -> String {
        t.chars()
            .iter()
            .map(|c| match c.attr {
                Attr::Miss => format!("!{}", c.ch),
                Attr::Garbage => format!("*{}", c.ch),
                Attr::Cursor => format!("[{}]", c.ch),
                Attr::Normal | Attr::Hit => c.ch.to_string(),
            })
            .collect::<Vec<_>>()
            .join("|")
    }

    #[test]
    fn empty_text() {
        let mut t = input("", true, true, true);
        assert_eq!((t.len(), t.pos(), t.completed()), (0, 0, true));
        assert_eq!(t.append_char(100.0, 'a', 100.0), Failed);
        assert_eq!(t.pos(), 0);
    }

    #[test]
    fn advance_to_completion() {
        let mut t = input("abcd", true, true, true);
        assert_eq!(show_chars(&t), "[a]|b|c|d");
        assert_eq!(t.append_char(100.0, 'a', 101.0), Succeeded);
        assert_eq!(show_steps(&t), "a,100,101");
        assert_eq!(show_chars(&t), "a|[b]|c|d");
        assert_eq!(t.append_char(200.0, 'b', 102.0), Succeeded);
        assert_eq!(t.append_char(300.0, 'c', 103.0), Succeeded);
        assert!(!t.completed());
        assert_eq!(t.append_char(400.0, 'd', 104.0), Succeeded);
        assert_eq!(show_steps(&t), "a,100,101|b,200,102|c,300,103|d,400,104");
        assert_eq!(show_chars(&t), "a|b|c|d");
        assert!(t.completed());
    }

    #[test]
    fn hit_and_miss_attrs() {
        let mut t = input("ab", true, false, false);
        t.append_char(100.0, 'a', 100.0);
        assert_eq!(t.chars()[0].attr, Attr::Hit);
        assert_eq!(t.append_char(200.0, 'x', 100.0), Failed);
        assert_eq!(t.append_char(300.0, 'b', 100.0), Recovered);
        assert_eq!(t.chars()[1].attr, Attr::Miss);
    }

    #[test]
    fn accumulate_and_delete_garbage() {
        let mut t = input("abc", false, false, true);
        assert_eq!(t.append_char(100.0, 'x', 100.0), Failed);
        assert_eq!(show_chars(&t), "*x|[a]|b|c");
        assert_eq!(t.append_char(200.0, 'a', 100.0), Failed);
        assert_eq!(show_steps(&t), "");
        assert_eq!(show_chars(&t), "*x|*a|[a]|b|c");
        assert_eq!(t.clear_char(), Succeeded);
        assert_eq!(show_chars(&t), "*x|[a]|b|c");
        assert_eq!(t.clear_char(), Succeeded);
        assert_eq!(show_chars(&t), "[a]|b|c");
        assert_eq!(t.append_char(500.0, 'a', 91.0), Recovered);
        assert_eq!(show_steps(&t), "!a,500,91");
        assert_eq!(show_chars(&t), "!a|[b]|c");
    }

    #[test]
    fn limit_garbage_length() {
        let mut t = input("abc", false, false, true);
        for i in 1..=100 {
            assert_eq!(t.append_char(i as f64 * 100.0, 'x', 100.0), Failed);
        }
        assert_eq!(show_chars(&t), "*x|*x|*x|*x|*x|*x|*x|*x|*x|*x|[a]|b|c");
    }

    #[test]
    fn backspace_at_start_of_word() {
        let mut t = input("abc", false, false, true);
        t.append_char(100.0, 'x', 100.0);
        t.clear_char();
        t.clear_char();
        assert_eq!(show_chars(&t), "[a]|b|c");
        assert_eq!(t.append_char(400.0, 'a', 101.0), Recovered);
        assert_eq!(show_steps(&t), "!a,400,101");
    }

    #[test]
    fn backspace_in_middle_of_word() {
        let mut t = input("abc", false, false, true);
        assert_eq!(t.append_char(100.0, 'a', 101.0), Succeeded);
        assert_eq!(t.append_char(200.0, 'x', 100.0), Failed);
        assert_eq!(show_chars(&t), "a|*x|[b]|c");
        t.clear_char();
        t.clear_char();
        assert_eq!(show_chars(&t), "a|[b]|c");
        assert_eq!(t.append_char(500.0, 'b', 102.0), Recovered);
        assert_eq!(show_steps(&t), "a,100,101|!b,500,102");
        assert_eq!(show_chars(&t), "a|!b|[c]");
    }

    #[test]
    fn forgive_inserted_character() {
        let mut t = input("abc", true, true, true);
        assert_eq!(t.append_char(100.0, 'x', 100.0), Failed);
        assert_eq!(show_chars(&t), "[a]|b|c");
        assert_eq!(t.append_char(200.0, 'a', 101.0), Recovered);
        assert_eq!(t.append_char(300.0, 'b', 102.0), Succeeded);
        assert_eq!(t.append_char(400.0, 'c', 103.0), Succeeded);
        assert_eq!(show_steps(&t), "!a,200,101|b,300,102|c,400,103");
        assert!(t.completed());
    }

    #[test]
    fn forgive_skipped_character() {
        let mut t = input("abcd", true, true, true);
        assert_eq!(t.append_char(100.0, 'b', 101.0), Failed);
        assert_eq!(t.append_char(200.0, 'c', 102.0), Failed);
        assert_eq!(show_chars(&t), "[a]|b|c|d");
        assert_eq!(t.append_char(300.0, 'd', 103.0), Recovered);
        assert_eq!(show_steps(&t), "!a,100,0|b,100,101|c,200,102|d,300,103");
        assert_eq!(show_chars(&t), "!a|b|c|d");
        assert!(t.completed());
    }

    #[test]
    fn forgive_replaced_character() {
        let mut t = input("abcd", true, true, true);
        assert_eq!(t.append_char(100.0, 'x', 101.0), Failed);
        assert_eq!(t.append_char(200.0, 'b', 102.0), Failed);
        assert_eq!(t.append_char(300.0, 'c', 103.0), Failed);
        assert_eq!(show_steps(&t), "");
        assert_eq!(t.append_char(400.0, 'd', 104.0), Recovered);
        assert_eq!(show_steps(&t), "!a,100,0|b,200,102|c,300,103|d,400,104");
        assert!(t.completed());
    }

    #[test]
    fn ignore_whitespace_key() {
        let mut t = input("abc", true, true, false);
        assert_eq!(t.append_char(100.0, ' ', 100.0), Succeeded);
        assert_eq!(show_chars(&t), "[a]|b|c");
        assert_eq!(t.append_char(200.0, 'a', 101.0), Succeeded);
        assert_eq!(t.append_char(300.0, ' ', 100.0), Succeeded);
        assert_eq!(t.append_char(400.0, 'b', 102.0), Succeeded);
        assert_eq!(show_steps(&t), "a,200,101|b,400,102");
    }

    #[test]
    fn space_in_garbage() {
        let mut t = input("abc", false, false, false);
        assert_eq!(t.append_char(100.0, 'x', 100.0), Failed);
        assert_eq!(t.append_char(200.0, ' ', 100.0), Failed);
        assert_eq!(show_chars(&t), "*x|* |[a]|b|c");
        t.clear_char();
        t.clear_char();
        assert_eq!(t.append_char(500.0, 'a', 101.0), Recovered);
        assert_eq!(show_steps(&t), "!a,500,101");
    }

    #[test]
    fn space_skips_words_ignored_at_word_start() {
        let mut t = input("abc", true, true, true);
        assert_eq!(t.append_char(100.0, ' ', 100.0), Succeeded);
        assert_eq!(show_steps(&t), "");
        let mut t = input("x abc", true, true, true);
        t.append_char(100.0, 'x', 101.0);
        t.append_char(200.0, ' ', 102.0);
        assert_eq!(t.append_char(300.0, ' ', 103.0), Succeeded);
        assert_eq!(show_steps(&t), "x,100,101| ,200,102");
        assert_eq!(show_chars(&t), "x| |[a]|b|c");
    }

    #[test]
    fn space_skips_word_after_error() {
        let mut t = input("abc", true, true, true);
        assert_eq!(t.append_char(100.0, 'x', 101.0), Failed);
        assert_eq!(t.append_char(200.0, ' ', 102.0), Recovered);
        assert_eq!(show_steps(&t), "!a,200,0|!b,200,0|!c,200,0");
        assert_eq!(show_chars(&t), "!a|!b|!c");
        assert!(t.completed());
    }

    #[test]
    fn space_skips_word_mid_word() {
        let mut t = input("x abc", true, true, true);
        t.append_char(100.0, 'x', 101.0);
        t.append_char(200.0, ' ', 102.0);
        t.append_char(300.0, 'a', 103.0);
        assert_eq!(t.append_char(400.0, ' ', 104.0), Recovered);
        assert_eq!(
            show_steps(&t),
            "x,100,101| ,200,102|a,300,103|!b,400,0|!c,400,0"
        );
        assert!(t.completed());
    }

    #[test]
    fn space_skips_word_removes_garbage() {
        let mut t = input("x abc", false, true, true);
        t.append_char(100.0, 'x', 101.0);
        t.append_char(200.0, ' ', 102.0);
        t.append_char(300.0, 'a', 103.0);
        assert_eq!(t.append_char(300.0, 'x', 104.0), Failed);
        assert_eq!(show_chars(&t), "x| |a|*x|[b]|c");
        assert_eq!(t.append_char(400.0, ' ', 105.0), Recovered);
        assert_eq!(show_chars(&t), "x| |a|!b|!c");
    }

    #[test]
    fn clear_word_removes_steps_back_to_space() {
        let mut t = input("ab cd", true, false, false);
        for (i, c) in "ab c".chars().enumerate() {
            t.append_char(i as f64 * 100.0, c, 100.0);
        }
        assert_eq!(t.pos(), 4);
        assert_eq!(t.clear_word(), Succeeded);
        assert_eq!(t.pos(), 3);
        assert!(t.has_typo());
    }

    #[test]
    fn normalize_characters() {
        for (text, typed) in [
            ("‘’", "''"),
            ("“”", "\"\""),
            ("«»", "\"\""),
            ("¿?¡!", "??!!"),
            ("¿?¡!", "¿?¡!"),
        ] {
            let mut t = input(text, true, false, false);
            for (i, c) in typed.chars().enumerate() {
                assert_eq!(
                    t.append_char(i as f64 * 100.0, c, 100.0),
                    Succeeded,
                    "{text} {typed}"
                );
            }
            assert!(t.completed());
        }
    }

    #[test]
    fn trailing_whitespace() {
        let mut t = input("a  ", true, false, false);
        assert_eq!(t.append_char(100.0, 'a', 100.0), Succeeded);
        assert_eq!(t.append_char(200.0, ' ', 100.0), Succeeded);
        assert_eq!(t.append_char(300.0, ' ', 100.0), Succeeded);
        assert!(t.completed());
    }

    #[test]
    fn emoji() {
        let mut t = input("🍬🍭", true, true, true);
        assert_eq!(show_chars(&t), "[🍬]|🍭");
        assert_eq!(t.append_char(100.0, '🍬', 101.0), Succeeded);
        assert_eq!(t.append_char(200.0, '🍭', 102.0), Succeeded);
        assert!(t.completed());
    }
}
