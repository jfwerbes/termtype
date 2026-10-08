use super::{centered, focus_line, hint_line, key_strip, wrap_chars};
use crate::app::App;
use crate::engine::result::cpm_to_wpm;
use crate::engine::textinput::{Attr, StyledChar};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const MAX_WIDTH: u16 = 76;
const TEXT_LINES: usize = 5;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme();
    let width = area.width.min(MAX_WIDTH);
    let input = app.session.input();
    let chars = input.chars();
    let wrapped = wrap_chars(&chars, width as usize);

    // Keep the cursor's line in view, with one line of context above.
    let cursor_line = wrapped
        .iter()
        .position(|l| l.iter().any(|c| c.attr == Attr::Cursor))
        .unwrap_or(wrapped.len().saturating_sub(1));
    let visible = wrapped.len().min(TEXT_LINES);
    let start = cursor_line.saturating_sub(1).min(wrapped.len() - visible);

    let char_style = |c: &StyledChar| -> (String, Style) {
        let shown = |blank: &str| {
            if c.ch == ' ' {
                blank.to_string()
            } else {
                c.ch.to_string()
            }
        };
        match c.attr {
            Attr::Normal => (shown(" "), Style::new().fg(theme.text)),
            Attr::Hit => (shown(" "), Style::new().fg(theme.typed)),
            Attr::Miss => (shown("·"), Style::new().fg(theme.error)),
            Attr::Garbage => (
                shown("·"),
                Style::new()
                    .fg(theme.error)
                    .add_modifier(Modifier::CROSSED_OUT),
            ),
            Attr::Cursor => {
                let bg = if input.has_typo() {
                    theme.error
                } else {
                    theme.cursor_bg
                };
                (shown(" "), Style::new().fg(theme.cursor_fg).bg(bg))
            }
        }
    };

    let mut lines = vec![key_strip(app, width), focus_line(app), Line::default()];
    for line in &wrapped[start..start + visible] {
        let spans: Vec<Span> = line
            .iter()
            .map(|c| {
                let (s, style) = char_style(c);
                Span::styled(s, style)
            })
            .collect();
        lines.push(Line::from(spans).alignment(Alignment::Left));
    }
    lines.push(Line::default());

    lines.push(match app.session.drill() {
        Some(d) => Line::from(vec![
            Span::styled("retype ", Style::new().fg(theme.muted)),
            Span::styled(
                d.word.clone(),
                Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {}/{}", d.done, d.target),
                Style::new().fg(theme.accent),
            ),
        ]),
        None => Line::default(),
    });

    let steps = input.steps();
    lines.push(if steps.is_empty() {
        hint_line(theme, "start typing")
    } else {
        let typos = steps.iter().filter(|s| s.typo).count();
        let accuracy = (steps.len() - typos) as f64 / steps.len() as f64 * 100.0;
        Line::styled(
            format!(
                "{:.0} wpm · {accuracy:.0}% accuracy",
                cpm_to_wpm(app.live_speed())
            ),
            Style::new().fg(theme.text),
        )
    });
    lines.push(Line::default());
    lines.push(hint_line(
        theme,
        "esc menu · tab new lesson · ctrl-w delete word",
    ));

    let height = lines.len() as u16;
    let body = centered(area, width, height);
    frame.render_widget(Paragraph::new(lines), body);
}
