//! Rendering. Every function here only reads `App`.

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
use ratatui::widgets::Block;

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

/// All letters in unlock order, coloured by state. Uses 3, 2 or 1 cells
/// per key depending on the width available.
fn key_strip(app: &App, width: u16) -> Line<'static> {
    let theme = app.theme();
    let n = app.keys.0.len().max(1) as u16;
    let cell = if width >= n * 3 {
        3
    } else if width >= n * 2 {
        2
    } else {
        1
    };
    Line::from(
        app.keys
            .0
            .iter()
            .map(|k| {
                let text = match cell {
                    3 => format!(" {} ", k.letter),
                    2 => format!("{} ", k.letter),
                    _ => k.letter.to_string(),
                };
                Span::styled(text, key_style(theme, k))
            })
            .collect::<Vec<_>>(),
    )
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
        (Some(t), Some(b)) => format!("{:.0} wpm (best {:.0})", wpm(t), wpm(b)),
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

fn hint_line(theme: &Theme, text: &str) -> Line<'static> {
    Line::styled(text.to_string(), Style::new().fg(theme.muted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Input;
    use crate::app::tests::{app, app_with, type_str};
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
        assert!(text.contains("key stats"), "{text}");
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
