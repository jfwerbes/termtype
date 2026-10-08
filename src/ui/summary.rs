use super::{centered, focus_line, hint_line, key_strip};
use crate::app::App;
use crate::engine::result::{LessonResult, cpm_to_wpm};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Sparkline};

const MAX_WIDTH: u16 = 78;
const HISTORY: usize = 60;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme();
    let width = area.width.min(MAX_WIDTH);
    let bold = |c| Style::new().fg(c).add_modifier(Modifier::BOLD);
    let muted = Style::new().fg(theme.muted);

    let mut top = vec![];
    match &app.last_lesson {
        Some(last) => {
            top.push(Line::styled("Lesson complete", bold(theme.accent)));
            top.push(Line::default());
            let r = &last.result;
            let speed = cpm_to_wpm(r.speed());
            let mut spans = vec![Span::styled(format!("{speed:.1} wpm"), bold(theme.text))];
            if let Some(prev) = last.previous_speed.filter(|_| last.counted) {
                let delta = speed - cpm_to_wpm(prev);
                let color = if delta >= 0.0 { theme.good } else { theme.bad };
                spans.push(Span::styled(
                    format!(" ({delta:+.1})"),
                    Style::new().fg(color),
                ));
            }
            spans.push(Span::styled(
                format!("   {:.1}% accuracy", r.accuracy() * 100.0),
                Style::new().fg(theme.text),
            ));
            spans.push(Span::styled(format!("   {:.1}s", r.time / 1000.0), muted));
            top.push(Line::from(spans));
            if !last.counted {
                top.push(Line::styled(
                    "not counted: needs 10+ characters, 1s+ and 3+ distinct keys",
                    muted,
                ));
            }
            for key in &last.new_keys {
                top.push(Line::styled(
                    format!("new key unlocked: {key}"),
                    bold(theme.good),
                ));
            }
        }
        None => top.push(Line::styled("termtype", bold(theme.accent))),
    }
    top.push(Line::default());
    top.push(key_strip(app, width));
    top.push(focus_line(app));

    let speeds: Vec<u64> = app
        .results
        .iter()
        .rev()
        .take(HISTORY)
        .rev()
        .map(|r| cpm_to_wpm(r.speed()).round() as u64)
        .collect();
    let history = history_line(app, &app.results);
    let menu = hint_line(
        theme,
        "enter next lesson · k key stats · s settings · q quit",
    );

    let spark_height = if speeds.is_empty() { 0 } else { 4 };
    let height = top.len() as u16 + 1 + spark_height + 1 + 2;
    let body = centered(area, width, height);
    let [top_area, _, spark_area, history_area, _, menu_area] = Layout::vertical([
        Constraint::Length(top.len() as u16),
        Constraint::Length(1),
        Constraint::Length(spark_height),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(body);
    frame.render_widget(Paragraph::new(top), top_area);
    frame.render_widget(
        Sparkline::default()
            .data(&speeds)
            .style(Style::new().fg(theme.accent)),
        spark_area,
    );
    frame.render_widget(Paragraph::new(history), history_area);
    frame.render_widget(Paragraph::new(menu), menu_area);
}

fn history_line(app: &App, results: &[LessonResult]) -> Line<'static> {
    let muted = Style::new().fg(app.theme().muted);
    if results.is_empty() {
        return Line::styled("no lessons yet", muted);
    }
    let speeds: Vec<f64> = results.iter().map(|r| cpm_to_wpm(r.speed())).collect();
    let recent = &speeds[speeds.len().saturating_sub(10)..];
    let avg = recent.iter().sum::<f64>() / recent.len() as f64;
    let best = speeds.iter().copied().fold(0.0, f64::max);
    Line::styled(
        format!(
            "{} lessons · last 10 avg {avg:.1} wpm · best {best:.1} wpm",
            results.len()
        ),
        muted,
    )
}
