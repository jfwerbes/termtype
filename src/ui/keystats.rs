use super::{hint_line, key_style, wpm};
use crate::app::App;
use crate::engine::result::slowest_bigrams;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table};

const ROWS: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];
const BAR: usize = 10;
const BIGRAM_RESULTS: usize = 50;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme();
    let title = Line::styled(
        format!("Key stats · target {} wpm", app.config.lesson.target_wpm),
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
    );
    let [title_area, main, hint_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    frame.render_widget(Paragraph::new(title), title_area);
    frame.render_widget(
        Paragraph::new(hint_line(theme, "esc back · enter practice")),
        hint_area,
    );

    // Wide: table on the left, keyboard above transitions on the right.
    // Narrow: keyboard beside transitions, table below.
    let (table_area, keyboard_area, bigram_area) = if main.width >= 100 {
        let [l, r] = Layout::horizontal([Constraint::Length(62), Constraint::Min(0)]).areas(main);
        let [k, b] = Layout::vertical([Constraint::Length(4), Constraint::Min(0)]).areas(r);
        (l, k, b)
    } else {
        let [top, bottom] =
            Layout::vertical([Constraint::Length(10), Constraint::Min(0)]).areas(main);
        let [k, b] = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)]).areas(top);
        (bottom, k, b)
    };

    frame.render_widget(Paragraph::new(keyboard(app)), keyboard_area);
    frame.render_widget(key_table(app), table_area);
    frame.render_widget(Paragraph::new(bigram_lines(app)), bigram_area);
}

/// QWERTY rows, each key coloured by learning state.
fn keyboard(app: &App) -> Vec<Line<'static>> {
    let theme = app.theme();
    ROWS.iter()
        .enumerate()
        .map(|(i, row)| {
            let mut spans = vec![Span::raw(" ".repeat(i))];
            for (j, c) in row.chars().enumerate() {
                if j > 0 {
                    spans.push(Span::raw(" "));
                }
                let style = app
                    .keys
                    .get(c)
                    .map_or(Style::new().fg(theme.muted), |k| key_style(theme, k));
                spans.push(Span::styled(c.to_string(), style));
            }
            Line::from(spans)
        })
        .collect()
}

fn key_table(app: &App) -> Table<'static> {
    let theme = app.theme();
    let header = Row::new([
        "key", "status", "wpm", "best", "progress", "lessons", "errors",
    ])
    .style(Style::new().fg(theme.muted));
    let rows = app.keys.0.iter().map(|k| {
        let stats = app.key_stats.get(k.letter);
        let status = if k.focused {
            "focus"
        } else if !k.included {
            "locked"
        } else if k.confidence.is_some_and(|c| c >= 1.0) {
            "ok"
        } else if k.forced {
            "forced"
        } else {
            "learning"
        };
        let fmt = |t: Option<f64>| t.map_or("-".to_string(), |t| format!("{:.0}", wpm(t)));
        let filled = (k.confidence.unwrap_or(0.0).min(1.0) * BAR as f64).round() as usize;
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(BAR - filled));
        let hits: u32 = stats.samples.iter().map(|s| s.hit_count).sum();
        let misses: u32 = stats.samples.iter().map(|s| s.miss_count).sum();
        let errors = if hits > 0 {
            format!("{:.1}%", misses as f64 / hits as f64 * 100.0)
        } else {
            "-".into()
        };
        Row::new(vec![
            Cell::from(Span::styled(format!(" {} ", k.letter), key_style(theme, k))),
            Cell::from(status),
            Cell::from(fmt(k.time_to_type)),
            Cell::from(fmt(k.best_time_to_type)),
            Cell::from(Span::styled(
                bar,
                key_style(theme, k)
                    .remove_modifier(Modifier::BOLD)
                    .bg(theme.background),
            )),
            Cell::from(stats.samples.len().to_string()),
            Cell::from(errors),
        ])
        .style(Style::new().fg(theme.text))
    });
    Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Length(9),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(BAR as u16 + 1),
            Constraint::Length(8),
            Constraint::Length(7),
        ],
    )
    .header(header)
}

fn bigram_lines(app: &App) -> Vec<Line<'static>> {
    let theme = app.theme();
    let recent = &app.results[app.results.len().saturating_sub(BIGRAM_RESULTS)..];
    let slow = slowest_bigrams(recent, &app.keys.included_letters(), 10, 8);
    let mut lines = vec![Line::styled(
        format!("slowest transitions (last {} lessons)", recent.len()),
        Style::new().fg(theme.muted),
    )];
    if slow.is_empty() {
        lines.push(Line::styled(
            "not enough data yet",
            Style::new().fg(theme.muted),
        ));
    }
    for b in slow {
        let shown: String = b
            .bigram
            .chars()
            .map(|c| if c == ' ' { '␣' } else { c })
            .collect();
        let errors = b.miss_count as f64 / b.hit_count as f64 * 100.0;
        lines.push(Line::from(vec![
            Span::styled(format!("{shown}  "), Style::new().fg(theme.accent)),
            Span::styled(
                format!(
                    "{:>4.0} ms · {:>3.0} wpm · {errors:.0}% errors",
                    b.time_to_type,
                    wpm(b.time_to_type)
                ),
                Style::new().fg(theme.text),
            ),
        ]));
    }
    lines
}
