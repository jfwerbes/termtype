//! Application state machine. No I/O: inputs come in with timestamps and
//! side effects go out as `Effect`s for `main` to perform.

use crate::config::Config;
use crate::engine::keystats::KeyStatsMap;
use crate::engine::lesson::LessonKeys;
use crate::engine::phonetic::PhoneticModel;
use crate::engine::result::LessonResult;
use crate::engine::rng::LessonRng;
use crate::engine::session::Session;
use crate::engine::stats::make_stats;
use crate::engine::textgen::{self, Dictionary};
use crate::engine::transitions;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Practice,
    Summary,
    KeyStats,
    Settings,
}

/// What kind of lesson to generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LessonMode {
    /// keybr's method: unlock letters, focus the weakest key.
    Guided,
    /// Words built around the slowest key transitions.
    Transitions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Char(char),
    Backspace,
    ClearWord,
    /// Tab: a fresh lesson.
    Restart,
    Enter,
    Esc,
    Up,
    Down,
    Left,
    Right,
    /// Ctrl-C.
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    SaveResult(LessonResult),
    SaveConfig(Config),
}

/// What happened in the last finished lesson.
#[derive(Debug, Clone, PartialEq)]
pub struct LessonSummary {
    pub result: LessonResult,
    /// Whether the result was good enough to count toward key stats.
    pub counted: bool,
    /// Speed of the previous counted lesson, in CPM.
    pub previous_speed: Option<f64>,
    pub new_keys: Vec<char>,
}

/// Editable settings, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    TargetWpm,
    AlphabetSize,
    RecoverKeys,
    NaturalWords,
    Length,
    RepeatWords,
    StopOnError,
    ForgiveErrors,
    SpaceSkipsWords,
    Drill,
    DrillRepeatCount,
    Theme,
}

impl Field {
    pub const ALL: [Field; 12] = [
        Field::TargetWpm,
        Field::AlphabetSize,
        Field::RecoverKeys,
        Field::NaturalWords,
        Field::Length,
        Field::RepeatWords,
        Field::StopOnError,
        Field::ForgiveErrors,
        Field::SpaceSkipsWords,
        Field::Drill,
        Field::DrillRepeatCount,
        Field::Theme,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Field::TargetWpm => "Target speed (wpm)",
            Field::AlphabetSize => "Unlock letters up front",
            Field::RecoverKeys => "Recover keys (use current speed)",
            Field::NaturalWords => "Natural words",
            Field::Length => "Lesson length",
            Field::RepeatWords => "Repeat each word",
            Field::StopOnError => "Stop on error",
            Field::ForgiveErrors => "Forgive errors",
            Field::SpaceSkipsWords => "Space skips word",
            Field::Drill => "Retype drill",
            Field::DrillRepeatCount => "Drill repetitions",
            Field::Theme => "Theme",
        }
    }
}

pub struct App {
    pub config: Config,
    pub themes: Vec<Theme>,
    pub screen: Screen,
    pub results: Vec<LessonResult>,
    pub key_stats: KeyStatsMap,
    pub keys: LessonKeys,
    pub session: Session,
    pub last_lesson: Option<LessonSummary>,
    pub settings_cursor: usize,
    pub should_quit: bool,
    pub mode: LessonMode,
    /// Transitions the current lesson drills (transition mode only).
    pub targets: Vec<String>,
    /// A message about the current lesson, e.g. why a drill fell back.
    pub notice: Option<String>,
    /// Monotonic ms, updated on every input; used for live speed.
    pub now: f64,
    letters: Vec<char>,
    model: PhoneticModel,
    dictionary: Dictionary,
    rng: LessonRng,
    last_input_at: Option<f64>,
}

impl App {
    /// `themes` must not be empty; `results` is the saved history.
    pub fn new(
        config: Config,
        themes: Vec<Theme>,
        results: Vec<LessonResult>,
        rng: LessonRng,
    ) -> Self {
        assert!(!themes.is_empty(), "at least one theme is required");
        let model = PhoneticModel::english();
        let letters = model.letters();
        let key_stats = KeyStatsMap::from_results(&letters, &results);
        let keys = LessonKeys::update(&letters, &key_stats, &config.lesson_settings());
        let session = Session::new("", config.input_settings(), config.drill_settings());
        let mut app = Self {
            config,
            themes,
            screen: Screen::Practice,
            results,
            key_stats,
            keys,
            session,
            last_lesson: None,
            settings_cursor: 0,
            should_quit: false,
            mode: LessonMode::Guided,
            targets: vec![],
            notice: None,
            now: 0.0,
            letters,
            model,
            dictionary: Dictionary::english(),
            rng,
            last_input_at: None,
        };
        app.new_lesson();
        app
    }

    pub fn theme(&self) -> &Theme {
        self.themes
            .iter()
            .find(|t| t.name == self.config.theme)
            .unwrap_or(&self.themes[0])
    }

    /// Generates a new lesson for the current keys and switches to it.
    fn new_lesson(&mut self) {
        self.targets.clear();
        self.notice = None;
        if self.mode == LessonMode::Transitions {
            self.targets =
                transitions::weak_transitions(&self.results, &self.keys.included_letters());
            if self.targets.is_empty() {
                self.notice = Some("not enough transition data yet · guided lesson instead".into());
            }
        }
        let text = if self.targets.is_empty() {
            textgen::generate(
                &self.model,
                &self.dictionary,
                &self.keys,
                &self.config.text_settings(),
                &mut self.rng,
            )
        } else {
            transitions::generate(
                &self.model,
                &self.dictionary,
                &self.targets,
                &self.keys.included_letters(),
                &self.config.text_settings(),
                &mut self.rng,
            )
        };
        self.session = Session::new(
            &text,
            self.config.input_settings(),
            self.config.drill_settings(),
        );
        self.last_input_at = None;
        self.screen = Screen::Practice;
    }

    fn update_keys(&mut self) {
        self.keys = LessonKeys::update(
            &self.letters,
            &self.key_stats,
            &self.config.lesson_settings(),
        );
    }

    fn finish_lesson(&mut self, unix_ms: i64) -> Vec<Effect> {
        let steps = self.session.input().steps();
        let stats = make_stats(steps);
        let result = LessonResult::from_steps(unix_ms, &stats, steps);
        let counted = result.validate();
        let previous_speed = self.results.last().map(LessonResult::speed);
        let before = self.keys.included_letters();
        let mut effects = vec![];
        if counted {
            self.key_stats.append(&result);
            self.results.push(result.clone());
            self.update_keys();
            effects.push(Effect::SaveResult(result.clone()));
        }
        let new_keys = self
            .keys
            .included_letters()
            .into_iter()
            .filter(|c| !before.contains(c))
            .collect();
        self.last_lesson = Some(LessonSummary {
            result,
            counted,
            previous_speed,
            new_keys,
        });
        self.screen = Screen::Summary;
        effects
    }

    /// Letters in unlock order.
    pub fn letters(&self) -> &[char] {
        &self.letters
    }

    /// Handles one input. `now` is monotonic ms; `unix_ms` is wall-clock
    /// time, stored with results.
    pub fn handle(&mut self, input: Input, now: f64, unix_ms: i64) -> Vec<Effect> {
        self.now = now;
        if input == Input::Quit {
            self.should_quit = true;
            return vec![];
        }
        match self.screen {
            Screen::Practice => self.handle_practice(input, now, unix_ms),
            Screen::Summary => {
                match input {
                    Input::Enter | Input::Char(' ') | Input::Restart => self.new_lesson(),
                    Input::Char('k') => self.screen = Screen::KeyStats,
                    Input::Char('t') => {
                        self.mode = LessonMode::Transitions;
                        self.new_lesson();
                    }
                    Input::Char('g') => {
                        self.mode = LessonMode::Guided;
                        self.new_lesson();
                    }
                    Input::Char('s') => self.screen = Screen::Settings,
                    Input::Char('q') | Input::Esc => self.should_quit = true,
                    _ => {}
                }
                vec![]
            }
            Screen::KeyStats => {
                if matches!(input, Input::Esc | Input::Char('q') | Input::Char('k')) {
                    self.screen = Screen::Summary;
                } else if input == Input::Enter {
                    self.new_lesson();
                }
                vec![]
            }
            Screen::Settings => self.handle_settings(input),
        }
    }

    fn handle_practice(&mut self, input: Input, now: f64, unix_ms: i64) -> Vec<Effect> {
        match input {
            Input::Char(c) => {
                let time_to_type = self.last_input_at.map_or(0.0, |t| now - t);
                self.last_input_at = Some(now);
                self.session.type_char(now, c, time_to_type);
                if self.session.completed() {
                    return self.finish_lesson(unix_ms);
                }
            }
            Input::Backspace => {
                self.session.backspace();
            }
            Input::ClearWord => {
                self.session.clear_word();
            }
            Input::Restart => self.new_lesson(),
            Input::Esc => self.screen = Screen::Summary,
            _ => {}
        }
        vec![]
    }

    fn handle_settings(&mut self, input: Input) -> Vec<Effect> {
        let field = Field::ALL[self.settings_cursor];
        let delta = match input {
            Input::Up => {
                self.settings_cursor = self.settings_cursor.saturating_sub(1);
                return vec![];
            }
            Input::Down => {
                self.settings_cursor = (self.settings_cursor + 1).min(Field::ALL.len() - 1);
                return vec![];
            }
            Input::Esc | Input::Char('q') | Input::Char('s') => {
                self.screen = Screen::Summary;
                return vec![];
            }
            Input::Left => -1,
            Input::Right | Input::Enter | Input::Char(' ') => 1,
            _ => return vec![],
        };
        self.adjust(field, delta);
        self.update_keys();
        self.new_lesson();
        self.screen = Screen::Settings;
        vec![Effect::SaveConfig(self.config.clone())]
    }

    fn adjust(&mut self, field: Field, delta: i32) {
        // Rounded to one decimal so repeated steps don't drift.
        let step = |v: f64, by: f64, min: f64, max: f64| {
            (((v + by * delta as f64) * 10.0).round() / 10.0).clamp(min, max)
        };
        let step_count = |v: usize, min: usize, max: usize| {
            (v as i64 + delta as i64).clamp(min as i64, max as i64) as usize
        };
        let c = &mut self.config;
        match field {
            Field::TargetWpm => c.lesson.target_wpm = step(c.lesson.target_wpm, 5.0, 10.0, 200.0),
            Field::AlphabetSize => {
                c.lesson.alphabet_size = step(c.lesson.alphabet_size, 0.1, 0.0, 1.0)
            }
            Field::Length => c.lesson.length = step(c.lesson.length, 0.1, 0.0, 1.0),
            Field::RecoverKeys => c.lesson.recover_keys = !c.lesson.recover_keys,
            Field::NaturalWords => c.lesson.natural_words = !c.lesson.natural_words,
            Field::StopOnError => c.input.stop_on_error = !c.input.stop_on_error,
            Field::ForgiveErrors => c.input.forgive_errors = !c.input.forgive_errors,
            Field::SpaceSkipsWords => c.input.space_skips_words = !c.input.space_skips_words,
            Field::Drill => c.drill.enabled = !c.drill.enabled,
            Field::RepeatWords => c.lesson.repeat_words = step_count(c.lesson.repeat_words, 1, 10),
            Field::DrillRepeatCount => {
                c.drill.repeat_count = step_count(c.drill.repeat_count, 1, 50)
            }
            Field::Theme => {
                let n = self.themes.len() as i32;
                let current = self
                    .themes
                    .iter()
                    .position(|t| t.name == c.theme)
                    .unwrap_or(0) as i32;
                c.theme = self.themes[(current + delta).rem_euclid(n) as usize]
                    .name
                    .clone();
            }
        }
    }

    /// Live speed of the current lesson in CPM.
    pub fn live_speed(&self) -> f64 {
        let steps = self.session.input().steps();
        match steps.first() {
            Some(first) if self.now > first.time_stamp => {
                steps.len() as f64 / ((self.now - first.time_stamp) / 1000.0) * 60.0
            }
            _ => 0.0,
        }
    }

    /// Current value of a settings field, for display.
    pub fn field_value(&self, field: Field) -> String {
        let c = &self.config;
        let on_off = |b: bool| if b { "on" } else { "off" }.to_string();
        let percent = |v: f64| format!("{:.0}%", v * 100.0);
        match field {
            Field::TargetWpm => format!("{}", c.lesson.target_wpm),
            Field::AlphabetSize => percent(c.lesson.alphabet_size),
            Field::Length => format!("{} chars", 100 + (c.lesson.length * 100.0).round() as usize),
            Field::RecoverKeys => on_off(c.lesson.recover_keys),
            Field::NaturalWords => on_off(c.lesson.natural_words),
            Field::StopOnError => on_off(c.input.stop_on_error),
            Field::ForgiveErrors => on_off(c.input.forgive_errors),
            Field::SpaceSkipsWords => on_off(c.input.space_skips_words),
            Field::Drill => on_off(c.drill.enabled),
            Field::RepeatWords => format!("{}×", c.lesson.repeat_words),
            Field::DrillRepeatCount => format!("{}×", c.drill.repeat_count),
            Field::Theme => c.theme.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn start_with_text(&mut self, text: &str) {
        self.session = Session::new(
            text,
            self.config.input_settings(),
            self.config.drill_settings(),
        );
        self.last_input_at = None;
        self.screen = Screen::Practice;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::engine::result::tests::fake_result;

    pub fn app_with(config: Config, results: Vec<LessonResult>) -> App {
        let themes = Theme::BUILT_IN
            .iter()
            .map(|n| Theme::built_in(n).unwrap())
            .collect();
        App::new(config, themes, results, LessonRng::seeded(1))
    }

    pub fn app() -> App {
        app_with(Config::default(), vec![])
    }

    /// Types `keys` 200 ms apart starting at `start`; returns all effects.
    pub fn type_str(app: &mut App, keys: &str, start: f64) -> Vec<Effect> {
        let mut effects = vec![];
        for (i, c) in keys.chars().enumerate() {
            effects.extend(app.handle(Input::Char(c), start + i as f64 * 200.0, 1_700_000_000_000));
        }
        effects
    }

    fn lesson_text(app: &App) -> String {
        app.session.input().text().iter().collect()
    }

    fn saved_results(effects: &[Effect]) -> usize {
        effects
            .iter()
            .filter(|e| matches!(e, Effect::SaveResult(_)))
            .count()
    }

    fn transition_history() -> Vec<LessonResult> {
        use crate::engine::result::tests::with_bigrams;
        vec![with_bigrams(&[
            ("ea", 20, 0, 500),
            ("rl", 20, 0, 600),
            ("ni", 20, 0, 400),
        ])]
    }

    fn to_menu(app: &mut App) {
        app.handle(Input::Esc, 0.0, 0);
        assert_eq!(app.screen, Screen::Summary);
    }

    #[test]
    fn t_starts_transition_drill() {
        let mut app = app_with(Config::default(), transition_history());
        assert_eq!(app.mode, LessonMode::Guided);
        to_menu(&mut app);
        app.handle(Input::Char('t'), 0.0, 0);
        assert_eq!(app.screen, Screen::Practice);
        assert_eq!(app.mode, LessonMode::Transitions);
        assert_eq!(app.targets, ["rl", "ea", "ni"]);
        assert_eq!(app.notice, None);
        for w in lesson_text(&app).split(' ') {
            assert!(app.targets.iter().any(|t| w.contains(t.as_str())), "{w}");
        }
    }

    #[test]
    fn mode_sticks_across_new_lessons() {
        let mut app = app_with(Config::default(), transition_history());
        to_menu(&mut app);
        app.handle(Input::Char('t'), 0.0, 0);
        app.handle(Input::Restart, 0.0, 0);
        assert_eq!(app.mode, LessonMode::Transitions);
        to_menu(&mut app);
        app.handle(Input::Enter, 0.0, 0);
        assert_eq!(app.mode, LessonMode::Transitions);
        assert!(!app.targets.is_empty());
    }

    #[test]
    fn g_returns_to_guided() {
        let mut app = app_with(Config::default(), transition_history());
        to_menu(&mut app);
        app.handle(Input::Char('t'), 0.0, 0);
        to_menu(&mut app);
        app.handle(Input::Char('g'), 0.0, 0);
        assert_eq!(app.mode, LessonMode::Guided);
        assert!(app.targets.is_empty());
        assert_eq!(app.screen, Screen::Practice);
    }

    #[test]
    fn transition_drill_without_data_falls_back_to_guided_text() {
        let mut app = app();
        to_menu(&mut app);
        app.handle(Input::Char('t'), 0.0, 0);
        assert_eq!(app.mode, LessonMode::Transitions);
        assert!(app.targets.is_empty());
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|n| n.contains("not enough"))
        );
        let text = lesson_text(&app);
        assert!(text.contains('e'), "{text}"); // guided text drills the focus key
    }

    #[test]
    fn transition_lessons_are_saved() {
        let mut app = app_with(Config::default(), transition_history());
        to_menu(&mut app);
        app.handle(Input::Char('t'), 0.0, 0);
        let text = lesson_text(&app);
        let effects = type_str(&mut app, &text, 0.0);
        assert_eq!(saved_results(&effects), 1);
        assert_eq!(app.screen, Screen::Summary);
    }

    #[test]
    fn starts_with_first_six_letters() {
        let app = app();
        assert_eq!(app.screen, Screen::Practice);
        assert_eq!(app.keys.included_letters(), ['e', 'n', 'i', 'a', 'r', 'l']);
        assert_eq!(app.keys.focused(), Some('e'));
        let text = lesson_text(&app);
        assert!(text.len() >= 100);
        assert!(
            text.chars().all(|c| c == ' ' || "eniarl".contains(c)),
            "{text}"
        );
    }

    #[test]
    fn finishing_a_lesson_saves_result_and_updates_stats() {
        let mut app = app();
        let text = lesson_text(&app);
        let effects = type_str(&mut app, &text, 0.0);
        assert_eq!(app.screen, Screen::Summary);
        assert_eq!(saved_results(&effects), 1);
        assert_eq!(app.results.len(), 1);
        let summary = app.last_lesson.as_ref().unwrap();
        assert!(summary.counted);
        // keybr: length / (last - first), so n keys span n - 1 intervals.
        let n = text.chars().count() as f64;
        assert!((summary.result.speed() - 300.0 * n / (n - 1.0)).abs() < 1e-9);
        assert_eq!(summary.result.time_stamp, 1_700_000_000_000);
        assert_eq!(app.key_stats.get('e').time_to_type, Some(200.0));
    }

    #[test]
    fn time_to_type_is_time_since_previous_input() {
        let mut app = app();
        app.start_with_text("abcdefghijklm");
        app.handle(Input::Char('a'), 1000.0, 0);
        app.handle(Input::Char('x'), 1100.0, 0);
        app.handle(Input::Char('b'), 1350.0, 0);
        let steps = app.session.input().steps();
        assert_eq!(steps[0].time_to_type, 0.0);
        assert_eq!(steps[1].time_to_type, 250.0);
    }

    #[test]
    fn invalid_result_is_not_saved() {
        let mut app = app();
        app.start_with_text("ab");
        let effects = type_str(&mut app, "ab", 0.0);
        assert_eq!(app.screen, Screen::Summary);
        assert_eq!(saved_results(&effects), 0);
        assert!(app.results.is_empty());
        assert!(!app.last_lesson.as_ref().unwrap().counted);
    }

    #[test]
    fn fast_history_unlocks_next_key() {
        let fast: Vec<LessonResult> = (0..3)
            .map(|i| {
                fake_result(
                    i,
                    &[
                        ('e', 100),
                        ('n', 100),
                        ('i', 100),
                        ('a', 100),
                        ('r', 100),
                        ('l', 100),
                    ],
                )
            })
            .collect();
        let app = app_with(Config::default(), fast);
        assert_eq!(
            app.keys.included_letters(),
            ['e', 'n', 'i', 'a', 'r', 'l', 't']
        );
        assert_eq!(app.keys.focused(), Some('t'));
    }

    #[test]
    fn summary_reports_newly_unlocked_keys() {
        let almost: Vec<LessonResult> = (0..3)
            .map(|i| {
                fake_result(
                    i,
                    &[('e', 100), ('n', 100), ('i', 100), ('a', 100), ('r', 100)],
                )
            })
            .collect();
        let mut app = app_with(Config::default(), almost);
        // At least 3 distinct characters for a valid result.
        app.start_with_text("lel lel lel lel lel");
        type_str(&mut app, "lel lel lel lel lel", 0.0); // 200 ms per key: fast enough
        assert_eq!(app.last_lesson.as_ref().unwrap().new_keys, ['t']);
    }

    #[test]
    fn navigation() {
        let mut app = app();
        app.handle(Input::Esc, 0.0, 0);
        assert_eq!(app.screen, Screen::Summary);
        app.handle(Input::Char('k'), 0.0, 0);
        assert_eq!(app.screen, Screen::KeyStats);
        app.handle(Input::Esc, 0.0, 0);
        assert_eq!(app.screen, Screen::Summary);
        app.handle(Input::Char('s'), 0.0, 0);
        assert_eq!(app.screen, Screen::Settings);
        app.handle(Input::Esc, 0.0, 0);
        app.handle(Input::Enter, 0.0, 0);
        assert_eq!(app.screen, Screen::Practice);
        app.handle(Input::Esc, 0.0, 0);
        app.handle(Input::Char('q'), 0.0, 0);
        assert!(app.should_quit);
    }

    #[test]
    fn ctrl_c_quits_anywhere() {
        let mut app = app();
        app.handle(Input::Quit, 0.0, 0);
        assert!(app.should_quit);
    }

    #[test]
    fn restart_gives_fresh_lesson() {
        let mut app = app();
        let before = lesson_text(&app);
        type_str(&mut app, &before[..3], 0.0);
        app.handle(Input::Restart, 0.0, 0);
        assert_eq!(app.session.input().pos(), 0);
        assert_ne!(lesson_text(&app), before);
    }

    #[test]
    fn backspace_and_clear_word_reach_session() {
        let mut app = app();
        app.start_with_text("ab cd");
        type_str(&mut app, "ab c", 0.0);
        app.handle(Input::ClearWord, 0.0, 0);
        assert_eq!(app.session.input().pos(), 3);
        app.handle(Input::Backspace, 0.0, 0);
        assert!(app.session.input().has_typo());
    }

    #[test]
    fn settings_edit_saves_config_and_rebuilds_lesson() {
        let mut app = app();
        app.handle(Input::Esc, 0.0, 0);
        app.handle(Input::Char('s'), 0.0, 0);
        assert_eq!(Field::ALL[app.settings_cursor], Field::TargetWpm);
        let effects = app.handle(Input::Right, 0.0, 0);
        assert_eq!(app.config.lesson.target_wpm, 40.0);
        assert_eq!(effects, [Effect::SaveConfig(app.config.clone())]);
        assert_eq!(app.field_value(Field::TargetWpm), "40");
        app.handle(Input::Left, 0.0, 0);
        app.handle(Input::Left, 0.0, 0);
        assert_eq!(app.config.lesson.target_wpm, 30.0);

        // Move to the drill toggle and flip it with Enter.
        while Field::ALL[app.settings_cursor] != Field::Drill {
            app.handle(Input::Down, 0.0, 0);
        }
        app.handle(Input::Enter, 0.0, 0);
        assert!(app.config.drill.enabled);
        assert_eq!(app.field_value(Field::Drill), "on");

        // The cursor stops at the ends.
        for _ in 0..20 {
            app.handle(Input::Down, 0.0, 0);
        }
        assert_eq!(Field::ALL[app.settings_cursor], Field::Theme);
        app.handle(Input::Right, 0.0, 0);
        assert_eq!(app.config.theme, "gruvbox");
        assert_eq!(app.theme().name, "gruvbox");
        app.handle(Input::Left, 0.0, 0);
        app.handle(Input::Left, 0.0, 0);
        assert_eq!(app.config.theme, "nord"); // wraps around

        app.handle(Input::Esc, 0.0, 0);
        app.handle(Input::Enter, 0.0, 0);
        assert!(app.session.drill().is_none());
        app.start_with_text("ab cd");
        type_str(&mut app, "a#b ", 0.0);
        assert!(app.session.drill().is_some());
    }

    #[test]
    fn settings_values_are_clamped() {
        let mut app = app();
        app.screen = Screen::Settings;
        for _ in 0..100 {
            app.handle(Input::Left, 0.0, 0);
        }
        assert_eq!(app.config.lesson.target_wpm, 10.0);
        app.handle(Input::Down, 0.0, 0);
        app.handle(Input::Left, 0.0, 0);
        assert_eq!(app.config.lesson.alphabet_size, 0.0);
        for _ in 0..20 {
            app.handle(Input::Right, 0.0, 0);
        }
        assert_eq!(app.config.lesson.alphabet_size, 1.0);
        assert_eq!(app.field_value(Field::AlphabetSize), "100%");
    }

    #[test]
    fn unknown_theme_falls_back_to_first() {
        let config = Config {
            theme: "missing".into(),
            ..Config::default()
        };
        assert_eq!(app_with(config, vec![]).theme().name, "terminal");
    }

    #[test]
    fn live_speed_counts_from_first_key() {
        let mut app = app();
        app.start_with_text("abcdefghijk");
        assert_eq!(app.live_speed(), 0.0);
        type_str(&mut app, "abcde", 0.0); // 5 chars, last at 800 ms
        app.now = 1000.0;
        assert_eq!(app.live_speed(), 5.0 / 1.0 * 60.0);
    }
}
