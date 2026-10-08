use super::{border_note, centered, footer, keys_panel, panel};
use crate::app::App;
use crate::engine::result::cpm_to_wpm;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Sparkline};

const MAX_WIDTH: u16 = 76;
const HISTORY: usize = 70;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme();
    let width = area.width.min(MAX_WIDTH);
    let bold = |c| Style::new().fg(c).add_modifier(Modifier::BOLD);
    let muted = Style::new().fg(theme.muted);

    // Lesson panel.
    let (title, lesson_lines) = match &app.last_lesson {
        Some(last) => {
            let r = &last.result;
            let speed = cpm_to_wpm(r.speed());
            let mut first = vec![Span::styled(format!("{speed:.1} wpm"), bold(theme.text))];
            if let Some(prev) = last.previous_speed.filter(|_| last.counted) {
                let delta = speed - cpm_to_wpm(prev);
                let color = if delta >= 0.0 { theme.good } else { theme.bad };
                first.push(Span::styled(
                    format!(" ({delta:+.1})"),
                    Style::new().fg(color),
                ));
            }
            first.push(Span::styled(
                format!("    {:.1}% accuracy", r.accuracy() * 100.0),
                Style::new().fg(theme.text),
            ));
            first.push(Span::styled(format!("    {:.1}s", r.time / 1000.0), muted));
            let mut lines = vec![Line::from(first)];
            if !last.counted {
                lines.push(Line::styled(
                    "not counted: needs 10+ characters, 1s+ and 3+ distinct keys",
                    muted,
                ));
            }
            for key in &last.new_keys {
                lines.push(Line::styled(
                    format!("new key unlocked: {key}"),
                    bold(theme.good),
                ));
            }
            ("lesson complete", lines)
        }
        None => (
            "termtype",
            vec![Line::styled(
                "Press enter to start a lesson.",
                Style::new().fg(theme.text),
            )],
        ),
    };
    let lesson_height = lesson_lines.len() as u16 + 2;

    // Progress panel: a sparkline once there is a trend to show.
    let speeds: Vec<u64> = app.results[app.results.len().saturating_sub(HISTORY)..]
        .iter()
        .map(|r| cpm_to_wpm(r.speed()).round() as u64)
        .collect();
    let spark_height = if speeds.len() >= 2 { 4 } else { 0 };
    let progress_height = spark_height + 1 + 2;

    let body = centered(area, width, lesson_height + progress_height + 4 + 2);
    let [lesson_area, progress_area, keys_area, _, footer_area] = Layout::vertical([
        Constraint::Length(lesson_height),
        Constraint::Length(progress_height),
        Constraint::Length(4),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(body);

    frame.render_widget(
        Paragraph::new(lesson_lines).block(panel(theme, title)),
        lesson_area,
    );

    let progress = panel(theme, "progress").title_top(border_note(
        theme,
        vec![Span::styled(
            format!("{} lessons", app.results.len()),
            muted,
        )],
    ));
    let inner = progress.inner(progress_area);
    frame.render_widget(progress, progress_area);
    let [spark_area, summary_area] =
        Layout::vertical([Constraint::Length(spark_height), Constraint::Length(1)]).areas(inner);
    frame.render_widget(
        Sparkline::default()
            .data(&speeds)
            .style(Style::new().fg(theme.accent)),
        spark_area,
    );
    frame.render_widget(Paragraph::new(history_line(app)), summary_area);

    keys_panel(frame, app, keys_area);
    frame.render_widget(
        footer(
            theme,
            "enter next · t transitions · g guided · k stats · s settings · q quit",
        ),
        footer_area,
    );
}

fn history_line(app: &App) -> Line<'static> {
    let muted = Style::new().fg(app.theme().muted);
    if app.results.is_empty() {
        return Line::styled("no lessons yet", muted);
    }
    let speeds: Vec<f64> = app.results.iter().map(|r| cpm_to_wpm(r.speed())).collect();
    let recent = &speeds[speeds.len().saturating_sub(10)..];
    let avg = recent.iter().sum::<f64>() / recent.len() as f64;
    let best = speeds.iter().copied().fold(0.0, f64::max);
    Line::styled(
        format!("last 10 avg {avg:.1} wpm · best {best:.1} wpm"),
        muted,
    )
}
