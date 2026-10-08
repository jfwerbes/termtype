use super::{border_note, centered, footer, keys_panel, panel, wrap_chars};
use crate::app::App;
use crate::engine::result::cpm_to_wpm;
use crate::engine::textinput::{Attr, StyledChar};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Padding, Paragraph};

const MAX_WIDTH: u16 = 76;
const TEXT_LINES: usize = 5;
/// Border (2) plus horizontal padding (2 x 2).
const CHROME: u16 = 6;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme();
    let width = area.width.min(MAX_WIDTH);
    let input = app.session.input();
    let wrapped = wrap_chars(&input.chars(), width.saturating_sub(CHROME) as usize);

    // Keep the cursor's line in view, with one line of context above.
    let cursor_line = wrapped
        .iter()
        .position(|l| l.iter().any(|c| c.attr == Attr::Cursor))
        .unwrap_or(wrapped.len().saturating_sub(1));
    let visible = wrapped.len().min(TEXT_LINES);
    let start = cursor_line.saturating_sub(1).min(wrapped.len() - visible);

    let char_span = |c: &StyledChar| -> Span<'static> {
        let shown = |blank: &str| {
            if c.ch == ' ' {
                blank.to_string()
            } else {
                c.ch.to_string()
            }
        };
        match c.attr {
            Attr::Normal => Span::styled(shown(" "), Style::new().fg(theme.text)),
            Attr::Hit => Span::styled(shown(" "), Style::new().fg(theme.typed)),
            Attr::Miss => Span::styled(shown("·"), Style::new().fg(theme.error)),
            Attr::Garbage => Span::styled(
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
                Span::styled(shown(" "), Style::new().fg(theme.cursor_fg).bg(bg))
            }
        }
    };
    let lines: Vec<Line> = wrapped[start..start + visible]
        .iter()
        .map(|line| Line::from(line.iter().map(char_span).collect::<Vec<_>>()))
        .collect();

    let mut block = panel(theme, "termtype").padding(Padding::new(2, 2, 1, 1));
    let steps = input.steps();
    block = block.title_top(if steps.is_empty() {
        border_note(
            theme,
            vec![Span::styled("start typing", Style::new().fg(theme.muted))],
        )
    } else {
        let typos = steps.iter().filter(|s| s.typo).count();
        let accuracy = (steps.len() - typos) as f64 / steps.len() as f64 * 100.0;
        border_note(
            theme,
            vec![Span::styled(
                format!("{:.0} wpm · {accuracy:.0}%", cpm_to_wpm(app.live_speed())),
                Style::new().fg(theme.text).add_modifier(Modifier::BOLD),
            )],
        )
    });
    if let Some(d) = app.session.drill() {
        block = block.title_bottom(border_note(
            theme,
            vec![
                Span::styled("retype ", Style::new().fg(theme.muted)),
                Span::styled(
                    d.word.clone(),
                    Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}/{}", d.done, d.target),
                    Style::new().fg(theme.accent),
                ),
            ],
        ));
    }

    let lesson_height = visible as u16 + 4;
    let body = centered(area, width, lesson_height + 4 + 2);
    let [lesson_area, keys_area, _, footer_area] = Layout::vertical([
        Constraint::Length(lesson_height),
        Constraint::Length(4),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(body);
    frame.render_widget(Paragraph::new(lines).block(block), lesson_area);
    keys_panel(frame, app, keys_area);
    frame.render_widget(
        footer(theme, "esc menu · tab new lesson · ctrl-w delete word"),
        footer_area,
    );
}
