//! Rendering. Every function here only reads `App`.

mod keyboard;
mod keystats;
mod practice;
mod settings;
mod summary;

use crate::app::{App, Screen};
use crate::engine::lesson::LessonKey;
use crate::engine::result::{cpm_to_wpm, time_to_speed};
use crate::engine::textinput::StyledChar;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Padding, Paragraph};

/// Wraps text at spaces into lines of at most `width` characters. A space
/// stays at the end of the line it follows (it must stay visible, as the
/// cursor may sit on it); a word longer than `width` is split.
pub fn wrap_chars(chars: &[StyledChar], width: usize) -> Vec<Vec<StyledChar>> {
    let width = width.max(1);
    let mut lines: Vec<Vec<StyledChar>> = vec![];
    let mut line: Vec<StyledChar> = vec![];
    for word in chars.split_inclusive(|c| c.ch == ' ') {
        if !line.is_empty() && line.len() + word.len() > width {
            lines.push(std::mem::take(&mut line));
        }
        for &c in word {
            if line.len() >= width {
                lines.push(std::mem::take(&mut line));
            }
            line.push(c);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

pub fn draw(frame: &mut Frame, app: &App) {
    let theme = app.theme();
    let area = frame.area();
    frame.render_widget(
        Block::new().style(Style::new().bg(theme.background).fg(theme.text)),
        area,
    );
    match app.screen {
        Screen::Practice => practice::draw(frame, app, area),
        Screen::Summary => summary::draw(frame, app, area),
        Screen::KeyStats => keystats::draw(frame, app, area),
        Screen::Settings => settings::draw(frame, app, area),
    }
}

/// A rectangle of at most `width` x `height`, centred in `area`.
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

/// Words per minute for a time to type in ms.
fn wpm(time_to_type: f64) -> f64 {
    cpm_to_wpm(time_to_speed(time_to_type))
}

/// Colour of a key by its learning state.
fn key_style(theme: &Theme, key: &LessonKey) -> Style {
    if key.focused {
        return Style::new()
            .fg(theme.cursor_fg)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD);
    }
    if !key.included {
        return Style::new().fg(theme.muted);
    }
    match key.confidence {
        Some(c) if c >= 1.0 => Style::new().fg(theme.good),
        Some(_) => Style::new().fg(theme.bad),
        None => Style::new().fg(theme.text),
    }
}

/// A rounded panel with a title in its top border.
fn panel(theme: &Theme, title: &str) -> Block<'static> {
    let border = Style::new().fg(theme.muted);
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Line::from(vec![
            Span::styled("─ ", border),
            Span::styled(
                title.to_string(),
                Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ", border),
        ]))
        .padding(Padding::horizontal(2))
}

/// Text for the right-hand side of a panel's top or bottom border.
fn border_note(theme: &Theme, spans: Vec<Span<'static>>) -> Line<'static> {
    let border = Style::new().fg(theme.muted);
    let mut all = vec![Span::styled(" ", border)];
    all.extend(spans);
    all.push(Span::styled(" ─", border));
    Line::from(all).right_aligned()
}

/// Text for the left-hand side of a panel's bottom border.
fn border_label(theme: &Theme, spans: Vec<Span<'static>>) -> Line<'static> {
    let border = Style::new().fg(theme.muted);
    let mut all = vec![Span::styled("─ ", border)];
    all.extend(spans);
    all.push(Span::styled(" ", border));
    Line::from(all).left_aligned()
}

/// The unlocked keys plus the next locked one, and the focus details.
fn keys_panel(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme();
    let block = panel(theme, "keys");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let included: Vec<&LessonKey> = app.keys.included().collect();
    let next = app.keys.0.iter().find(|k| !k.included);
    // Three cells per key when they fit, so the focused key reads as a block.
    let room = (inner.width as usize).saturating_sub(10);
    let wide = included.len() * 3 <= room;
    let mut spans = vec![];
    for k in &included {
        let text = if wide || k.focused {
            format!(" {} ", k.letter)
        } else {
            format!("{} ", k.letter)
        };
        spans.push(Span::styled(text, key_style(theme, k)));
    }
    if let Some(next) = next {
        spans.push(Span::styled(
            format!("  next {}", next.letter),
            Style::new().fg(theme.muted),
        ));
    }
    frame.render_widget(
        Paragraph::new(vec![Line::from(spans), {
            // Line up with the first key's letter.
            let mut focus = focus_line(app);
            if wide {
                focus.spans.insert(0, Span::raw(" "));
            }
            focus
        }]),
        inner,
    );
}

/// Describes the focused key, or that every key is at target.
fn focus_line(app: &App) -> Line<'static> {
    let theme = app.theme();
    let target = format!("target {} wpm", app.config.lesson.target_wpm);
    let muted = Style::new().fg(theme.muted);
    let Some(key) = app.keys.focused().and_then(|f| app.keys.get(f)) else {
        return Line::from(vec![
            Span::styled("all keys at target", Style::new().fg(theme.good)),
            Span::styled(format!(" · {target}"), muted),
        ]);
    };
    let speed = match (key.time_to_type, key.best_time_to_type) {
        (Some(t), Some(b)) => format!("{:.0} wpm, best {:.0}", wpm(t), wpm(b)),
        _ => "no data yet".to_string(),
    };
    Line::from(vec![
        Span::styled("focus ", muted),
        Span::styled(
            key.letter.to_string(),
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" · {speed} · {target}"), muted),
    ])
}

/// Key hints, centred below the panels.
fn footer(theme: &Theme, text: &str) -> Paragraph<'static> {
    Paragraph::new(Line::styled(text.to_string(), Style::new().fg(theme.muted)).centered())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{app, app_with, type_str};
    use crate::app::{FLASH_MS, Field, Input};
    use crate::config::Config;
    use crate::engine::textinput::Attr;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::Modifier;

    fn plain(text: &str) -> Vec<StyledChar> {
        text.chars()
            .map(|ch| StyledChar {
                ch,
                attr: Attr::Normal,
            })
            .collect()
    }

    fn lines(wrapped: &[Vec<StyledChar>]) -> Vec<String> {
        wrapped
            .iter()
            .map(|l| l.iter().map(|c| c.ch).collect())
            .collect()
    }

    #[test]
    fn wrap_at_spaces() {
        assert_eq!(
            lines(&wrap_chars(&plain("aaa bbb ccc"), 8)),
            ["aaa bbb ", "ccc"]
        );
        assert_eq!(
            lines(&wrap_chars(&plain("aaa bbb ccc"), 7)),
            ["aaa ", "bbb ccc"]
        );
        assert_eq!(
            lines(&wrap_chars(&plain("aaa bbb ccc"), 6)),
            ["aaa ", "bbb ", "ccc"]
        );
        assert_eq!(
            lines(&wrap_chars(&plain("aaa bbb ccc"), 100)),
            ["aaa bbb ccc"]
        );
        assert_eq!(
            lines(&wrap_chars(&plain("abcdefgh ij"), 4)),
            ["abcd", "efgh", " ij"]
        );
        assert!(wrap_chars(&[], 10).is_empty());
    }

    fn render(app: &App, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        terminal.backend().buffer().clone()
    }

    fn screen_text(buf: &Buffer) -> String {
        let area = buf.area;
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Position of the first cell starting `needle` on a row.
    fn find(buf: &Buffer, needle: &str) -> Option<(u16, u16)> {
        let text = screen_text(buf);
        text.lines().enumerate().find_map(|(y, line)| {
            line.find(needle)
                .map(|byte| (line[..byte].chars().count() as u16, y as u16))
        })
    }

    /// The row index of the first line containing `needle`.
    fn row_of(buf: &Buffer, needle: &str) -> Option<usize> {
        screen_text(buf).lines().position(|l| l.contains(needle))
    }

    #[test]
    fn every_screen_uses_framed_panels() {
        let mut app = app();
        for screen in [
            Screen::Practice,
            Screen::Summary,
            Screen::KeyStats,
            Screen::Settings,
        ] {
            app.screen = screen;
            let text = screen_text(&render(&app, 90, 30));
            assert!(
                text.contains('╭') && text.contains('╯'),
                "{screen:?}:\n{text}"
            );
        }
    }

    #[test]
    fn practice_has_lesson_and_keys_panels_with_footer_below() {
        let app = app();
        let buf = render(&app, 90, 30);
        let text = screen_text(&buf);
        let lesson = row_of(&buf, "╭─ guided").expect(&text);
        let keys = row_of(&buf, "╭─ keys").expect(&text);
        let footer = row_of(&buf, "esc menu").expect(&text);
        assert!(lesson < keys && keys < footer, "{text}");
        // The footer is outside every panel.
        let footer_line = text.lines().nth(footer).unwrap();
        assert!(!footer_line.contains('│'), "{footer_line}");
    }

    #[test]
    fn practice_keys_panel_shows_unlocked_and_next_only() {
        let app = app();
        let buf = render(&app, 90, 30);
        let keys_row = row_of(&buf, "╭─ keys").unwrap();
        let text = screen_text(&buf);
        let mut rows = text.lines().skip(keys_row + 1);
        let (keys, focus) = (rows.next().unwrap(), rows.next().unwrap());
        assert!(keys.contains("next t"), "{keys}");
        assert!(!keys.contains(" j "), "locked letters are hidden: {keys}");
        assert!(
            focus.contains("focus e") && focus.contains("target 35 wpm"),
            "{focus}"
        );
    }

    #[test]
    fn panel_titles_per_screen() {
        let mut app = app();
        let lesson: String = app.session.input().text().iter().collect();
        type_str(&mut app, &lesson, 0.0);
        let expected: [(Screen, &[&str]); 3] = [
            (
                Screen::Summary,
                &["╭─ lesson complete", "╭─ progress", "╭─ keys"],
            ),
            (
                Screen::KeyStats,
                &["╭─ keyboard", "╭─ slowest transitions", "╭─ per key"],
            ),
            (Screen::Settings, &["╭─ settings"]),
        ];
        for (screen, titles) in expected {
            app.screen = screen;
            let text = screen_text(&render(&app, 100, 40));
            for t in titles {
                assert!(text.contains(t), "{screen:?} missing {t}:\n{text}");
            }
        }
    }

    fn transition_app() -> App {
        use crate::engine::result::tests::with_bigrams;
        let history = vec![with_bigrams(&[
            ("ea", 20, 0, 500),
            ("rl", 20, 0, 600),
            ("ni", 20, 0, 400),
        ])];
        let mut app = app_with(Config::default(), history);
        app.handle(Input::Esc, 0.0, 0);
        app.handle(Input::Char('t'), 0.0, 0);
        app
    }

    /// The top row of the on-screen keyboard.
    fn keyboard_row(buf: &Buffer) -> Option<u16> {
        row_of(buf, "│ q │").map(|r| r as u16 - 1)
    }

    /// The background behind a key's label on the on-screen keyboard.
    fn key_bg(buf: &Buffer, label: &str) -> ratatui::style::Color {
        let top = keyboard_row(buf).unwrap();
        let p = (top..top + keyboard::HEIGHT)
            .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
            .find(|&p| buf[p].symbol() == label)
            .unwrap();
        buf[p].bg
    }

    #[test]
    fn practice_keyboard_sits_between_keys_and_footer() {
        let app = app();
        let buf = render(&app, 90, 30);
        let text = screen_text(&buf);
        let keys = row_of(&buf, "╭─ keys").expect(&text) as u16;
        let keyboard = keyboard_row(&buf).expect(&text);
        let footer = row_of(&buf, "esc menu").expect(&text) as u16;
        assert!(
            keys < keyboard && keyboard + keyboard::HEIGHT <= footer,
            "{text}"
        );
        // Split by default: boxed keys, halves apart.
        assert!(
            text.contains("│ q │ w │ e │ r │ t │   │ y │ u │ i │ o │ p │"),
            "{text}"
        );
        assert!(text.contains("├───┼───┼"), "{text}");
    }

    #[test]
    fn practice_keyboard_lights_up_typed_keys() {
        let mut app = app();
        app.start_with_text("ab cd");
        // Nothing is lit before typing, not even the next key.
        let buf = render(&app, 90, 30);
        assert_eq!(key_bg(&buf, "a"), app.theme().background);

        type_str(&mut app, "a", 0.0);
        let buf = render(&app, 90, 30);
        assert_eq!(key_bg(&buf, "a"), app.theme().accent);
        assert_eq!(key_bg(&buf, "b"), app.theme().background);

        // A wrong key lights up red.
        type_str(&mut app, "x", 100.0);
        let buf = render(&app, 90, 30);
        assert_eq!(key_bg(&buf, "x"), app.theme().error);
        assert_eq!(app.flash_ends_in(), Some(FLASH_MS));

        // Keys go dark once the flash is over.
        app.now = 100.0 + FLASH_MS;
        let buf = render(&app, 90, 30);
        assert_eq!(key_bg(&buf, "a"), app.theme().background);
        assert_eq!(key_bg(&buf, "x"), app.theme().background);
        assert_eq!(app.flash_ends_in(), None);
    }

    #[test]
    fn practice_keyboard_can_be_hidden() {
        let mut app = app();
        // Too short: the keyboard goes, the rest stays.
        let text = screen_text(&render(&app, 90, 18));
        assert!(!text.contains("│ q │"), "{text}");
        assert!(text.contains("╭─ keys"), "{text}");
        app.config.keyboard = crate::config::KeyboardLayout::Off;
        let text = screen_text(&render(&app, 90, 30));
        assert!(!text.contains("│ q │"), "{text}");
        app.config.keyboard = crate::config::KeyboardLayout::Standard;
        let text = screen_text(&render(&app, 90, 30));
        assert!(text.contains("│ q │ w │ e │ r │ t │ y │"), "{text}");
        assert!(text.contains("╰─┬─┴─┬─"), "staggered rows: {text}");
    }

    #[test]
    fn transition_lesson_shows_title_and_targets() {
        let app = transition_app();
        let text = screen_text(&render(&app, 90, 30));
        assert!(text.contains("╭─ transitions"), "{text}");
        assert!(text.contains("rl · ea · ni"), "{text}");
    }

    #[test]
    fn transition_fallback_shows_notice() {
        let mut app = app();
        app.handle(Input::Esc, 0.0, 0);
        app.handle(Input::Char('t'), 0.0, 0);
        let text = screen_text(&render(&app, 90, 30));
        assert!(text.contains("not enough transition data"), "{text}");
    }

    #[test]
    fn practice_title_names_the_mode() {
        let text = screen_text(&render(&app(), 90, 30));
        assert!(text.contains("╭─ guided"), "{text}");
        assert!(text.contains("esc menu & modes"), "{text}");
    }

    #[test]
    fn summary_mode_panel_marks_current_mode() {
        let mut app = transition_app();
        app.handle(Input::Esc, 0.0, 0);
        let text = screen_text(&render(&app, 90, 30));
        assert!(text.contains("╭─ mode"), "{text}");
        assert!(text.contains("  g guided"), "{text}");
        assert!(
            text.contains("› t transitions  slowest letter pairs: rl · ea · ni"),
            "{text}"
        );
        app.handle(Input::Char('g'), 0.0, 0);
        app.handle(Input::Esc, 0.0, 0);
        let text = screen_text(&render(&app, 90, 30));
        assert!(text.contains("› g guided"), "{text}");
    }

    #[test]
    fn summary_mode_panel_says_when_transitions_need_data() {
        let mut app = app();
        app.handle(Input::Esc, 0.0, 0);
        let text = screen_text(&render(&app, 90, 30));
        assert!(text.contains("needs more lessons first"), "{text}");
    }

    #[test]
    fn settings_clear_progress_dialog() {
        let mut app = transition_app();
        app.screen = Screen::Settings;
        app.settings_cursor = Field::ALL.len() - 1;
        let text = screen_text(&render(&app, 90, 30));
        assert!(
            text.contains("Clear progress") && text.contains("1 lesson "),
            "{text}"
        );
        assert!(!text.contains("clear progress?"), "{text}");
        app.handle(Input::Enter, 0.0, 0);
        let text = screen_text(&render(&app, 90, 30));
        assert!(text.contains("╭─ clear progress?"), "{text}");
        assert!(text.contains("This deletes all 1 lesson of"), "{text}");
        assert!(text.contains("y clear · n or esc cancel"), "{text}");
    }

    #[test]
    fn summary_footer_offers_both_modes() {
        let mut app = app();
        app.handle(Input::Esc, 0.0, 0);
        let text = screen_text(&render(&app, 90, 30));
        assert!(
            text.contains("t transitions") && text.contains("g guided"),
            "{text}"
        );
    }

    #[test]
    fn practice_live_stats_in_lesson_border() {
        let mut app = app();
        let text: String = app.session.input().text().iter().take(5).collect();
        type_str(&mut app, &text, 0.0);
        let buf = render(&app, 90, 30);
        let title_row = row_of(&buf, "╭─ guided").unwrap();
        let line = screen_text(&buf)
            .lines()
            .nth(title_row)
            .unwrap()
            .to_string();
        assert!(line.contains("wpm") && line.contains("100%"), "{line}");
    }

    #[test]
    fn practice_shows_text_keys_and_hints() {
        let app = app();
        let buf = render(&app, 80, 24);
        let text = screen_text(&buf);
        let first_word: String = app
            .session
            .input()
            .text()
            .iter()
            .take_while(|&&c| c != ' ')
            .collect();
        assert!(text.contains(&first_word), "{text}");
        assert!(text.contains("tab"), "{text}");
        // The focused key is highlighted.
        let (x, y) = find(&buf, " e ").expect("key strip");
        let cell = &buf[(x + 1, y)];
        assert_eq!(cell.symbol(), "e");
        assert_eq!(cell.bg, app.theme().accent);
    }

    #[test]
    fn practice_cursor_marks_next_char() {
        let mut app = app();
        app.start_with_text("ab cd");
        type_str(&mut app, "a", 0.0);
        let buf = render(&app, 80, 24);
        let (x, y) = find(&buf, "ab cd").unwrap();
        assert_eq!(buf[(x, y)].fg, app.theme().typed);
        assert_eq!(buf[(x + 1, y)].bg, app.theme().cursor_bg);
        assert_eq!(buf[(x + 3, y)].fg, app.theme().text);
    }

    #[test]
    fn practice_shows_drill_counter() {
        let config = Config {
            drill: crate::config::DrillConfig {
                enabled: true,
                repeat_count: 10,
            },
            ..Config::default()
        };
        let mut app = app_with(config, vec![]);
        app.start_with_text("ab cd");
        type_str(&mut app, "a#b ab ", 0.0);
        let text = screen_text(&render(&app, 80, 24));
        assert!(text.contains("1/10"), "{text}");
        assert!(text.contains("ab"), "{text}");
    }

    #[test]
    fn summary_shows_speed_and_unlocks() {
        let mut app = app();
        let lesson: String = app.session.input().text().iter().collect();
        type_str(&mut app, &lesson, 0.0);
        let text = screen_text(&render(&app, 80, 24));
        assert!(text.contains("wpm"), "{text}");
        assert!(text.contains("accuracy"), "{text}");
        assert!(text.contains("100.0%"), "{text}");
        assert!(text.contains("k stats"), "{text}");
    }

    #[test]
    fn key_stats_lists_letters_and_keyboard() {
        let mut app = app();
        app.handle(Input::Esc, 0.0, 0);
        app.handle(Input::Char('k'), 0.0, 0);
        let text = screen_text(&render(&app, 100, 40));
        assert!(
            text.contains("q w e r t y u i o p") || text.contains("q │ w"),
            "{text}"
        );
        assert!(text.contains("locked"), "{text}");
        assert!(text.contains("focus"), "{text}");
    }

    #[test]
    fn key_stats_shows_transitions_on_narrow_terminals() {
        let mut app = app();
        app.screen = Screen::KeyStats;
        let text = screen_text(&render(&app, 90, 28));
        assert!(text.contains("slowest transitions"), "{text}");
        assert!(text.contains("q w e r t y u i o p"), "{text}");
    }

    #[test]
    fn settings_lists_fields() {
        let mut app = app();
        app.handle(Input::Esc, 0.0, 0);
        app.handle(Input::Char('s'), 0.0, 0);
        let text = screen_text(&render(&app, 80, 24));
        assert!(text.contains("Target speed (wpm)"), "{text}");
        assert!(text.contains("Retype drill"), "{text}");
        assert!(text.contains("terminal"), "{text}");
    }

    #[test]
    fn tiny_terminals_do_not_panic() {
        let mut app = app();
        for screen in [
            Screen::Practice,
            Screen::Summary,
            Screen::KeyStats,
            Screen::Settings,
        ] {
            app.screen = screen;
            for (w, h) in [(1, 1), (10, 3), (20, 5), (40, 10)] {
                render(&app, w, h);
            }
        }
    }

    #[test]
    fn typed_miss_is_error_coloured() {
        let mut app = app();
        app.start_with_text("ab cd");
        type_str(&mut app, "#a", 0.0);
        let buf = render(&app, 80, 24);
        let (x, y) = find(&buf, "ab cd").unwrap();
        assert_eq!(buf[(x, y)].fg, app.theme().error);
        assert!(!buf[(x, y)].modifier.contains(Modifier::CROSSED_OUT));
    }
}
