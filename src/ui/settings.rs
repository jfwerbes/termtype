use super::{centered, footer, panel};
use crate::app::{App, Field};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Padding, Paragraph};

const WIDTH: u16 = 66;

fn help(field: Field) -> &'static str {
    match field {
        Field::TargetWpm => "A key is learned once it reaches this speed.",
        Field::AlphabetSize => "Unlock this share of the remaining letters straight away.",
        Field::RecoverKeys => "Focus keys that have slowed down again, not just new ones.",
        Field::NaturalWords => "Use real words when enough fit the unlocked letters.",
        Field::Length => "Characters per lesson.",
        Field::RepeatWords => "Type every word this many times in a row.",
        Field::StopOnError => "The cursor waits until the right key is pressed.",
        Field::ForgiveErrors => "Recover automatically from a skipped or extra key.",
        Field::SpaceSkipsWords => "Space jumps to the next word, counting the rest as errors.",
        Field::Drill => "A mistake stops you; the word must then be retyped cleanly.",
        Field::DrillRepeatCount => "How many clean repetitions the drill asks for.",
        Field::Theme => "Colours. Add your own in ~/.config/termtype/themes/.",
    }
}

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme();
    let label_width = Field::ALL
        .iter()
        .map(|f| f.label().len())
        .max()
        .unwrap_or(0)
        + 3;
    let mut lines: Vec<Line> = Field::ALL
        .iter()
        .enumerate()
        .map(|(i, &field)| {
            let selected = i == app.settings_cursor;
            let (marker, label_style, value_style) = if selected {
                let s = Style::new().fg(theme.accent).add_modifier(Modifier::BOLD);
                ("› ", s, s)
            } else {
                (
                    "  ",
                    Style::new().fg(theme.text),
                    Style::new().fg(theme.muted),
                )
            };
            Line::from(vec![
                Span::styled(
                    format!("{marker}{:<label_width$}", field.label()),
                    label_style,
                ),
                Span::styled(app.field_value(field), value_style),
            ])
        })
        .collect();
    lines.push(Line::default());
    lines.push(Line::styled(
        help(Field::ALL[app.settings_cursor]),
        Style::new().fg(theme.muted),
    ));

    let height = lines.len() as u16 + 4;
    let body = centered(area, WIDTH, height + 2);
    let [panel_area, _, footer_area] = Layout::vertical([
        Constraint::Length(height),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(body);
    let block = panel(theme, "settings").padding(Padding::new(2, 2, 1, 1));
    frame.render_widget(Paragraph::new(lines).block(block), panel_area);
    frame.render_widget(
        footer(theme, "↑↓ select · ←→ or enter change · esc back"),
        footer_area,
    );
}
