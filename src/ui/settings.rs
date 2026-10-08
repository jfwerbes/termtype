use super::{centered, hint_line};
use crate::app::{App, Field};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const WIDTH: u16 = 64;

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
    let mut lines = vec![
        Line::styled(
            "Settings",
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
        Line::default(),
    ];
    let label_width = Field::ALL
        .iter()
        .map(|f| f.label().len())
        .max()
        .unwrap_or(0)
        + 2;
    for (i, &field) in Field::ALL.iter().enumerate() {
        let selected = i == app.settings_cursor;
        let marker = if selected { "› " } else { "  " };
        let style = if selected {
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(theme.text)
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{marker}{:<label_width$}", field.label()), style),
            Span::styled(app.field_value(field), style),
        ]));
    }
    lines.push(Line::default());
    lines.push(hint_line(theme, help(Field::ALL[app.settings_cursor])));
    lines.push(Line::default());
    lines.push(hint_line(
        theme,
        "↑↓ select · ←→ or enter change · esc back",
    ));
    let body = centered(area, WIDTH, lines.len() as u16);
    frame.render_widget(Paragraph::new(lines), body);
}
