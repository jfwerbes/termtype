//! An on-screen keyboard: each key in a box, lit up as it is typed.

use crate::app::App;
use crate::config::KeyboardLayout;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use std::collections::HashSet;

/// Four rows of keys whose boxes share borders.
pub const HEIGHT: u16 = 9;

/// A key: the character it types, its left edge in cells, its row and
/// its width from edge to edge.
struct Key {
    ch: char,
    x: u16,
    row: u16,
    width: u16,
}

/// Edge to edge, so neighbouring keys share a border.
const KEY: u16 = 4;
/// Space between the halves of a split keyboard.
const SPLIT_GAP: u16 = 4;

const LETTER_ROWS: [&str; 3] = ["qwertyuiop", "asdfghjkl;", "zxcvbnm,./"];

/// The keys of a layout; `None` for `Off`.
fn keys(layout: KeyboardLayout) -> Option<Vec<Key>> {
    let letters = |x: &dyn Fn(u16, u16) -> u16| -> Vec<Key> {
        LETTER_ROWS
            .iter()
            .zip(0..)
            .flat_map(|(row, r)| {
                row.chars().zip(0..).map(move |(ch, i)| Key {
                    ch,
                    x: x(r, i),
                    row: r,
                    width: KEY,
                })
            })
            .collect()
    };
    let space = |x, width| Key {
        ch: ' ',
        x,
        row: 3,
        width,
    };
    match layout {
        KeyboardLayout::Off => None,
        // Columnar 3x5 halves with a space key on each inner thumb.
        KeyboardLayout::Split => {
            let right = 5 * KEY + SPLIT_GAP;
            let mut keys = letters(&|_, i| {
                if i < 5 {
                    i * KEY
                } else {
                    right + (i - 5) * KEY
                }
            });
            keys.push(space(3 * KEY, 2 * KEY));
            keys.push(space(right, 2 * KEY));
            Some(keys)
        }
        // Row-staggered, like a laptop keyboard.
        KeyboardLayout::Standard => {
            let mut keys = letters(&|r, i| r * 2 + i * KEY);
            // From under c to under m.
            keys.push(space(4 + 2 * KEY, 5 * KEY));
            Some(keys)
        }
    }
}

fn layout_width(keys: &[Key]) -> u16 {
    keys.iter().map(|k| k.x + k.width + 1).max().unwrap_or(0)
}

/// Whether the keyboard is on and fits in `width` columns.
pub fn fits(layout: KeyboardLayout, width: u16) -> bool {
    keys(layout).is_some_and(|k| layout_width(&k) <= width)
}

/// The box-drawing character joining lines that leave a cell upward,
/// downward, leftward and rightward.
fn junction(up: bool, down: bool, left: bool, right: bool) -> char {
    match (up, down, left, right) {
        (false, false, false, false) => ' ',
        (false, false, _, _) => '─',
        (_, _, false, false) => '│',
        (false, true, false, true) => '╭',
        (false, true, true, false) => '╮',
        (true, false, false, true) => '╰',
        (true, false, true, false) => '╯',
        (false, true, true, true) => '┬',
        (true, false, true, true) => '┴',
        (true, true, false, true) => '├',
        (true, true, true, false) => '┤',
        (true, true, true, true) => '┼',
    }
}

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let Some(keys) = keys(app.config.keyboard) else {
        return;
    };
    let theme = app.theme();
    let width = layout_width(&keys);

    // Unit border segments: horizontal from (x, y) to (x + 1, y), vertical
    // from (x, y) to (x, y + 1).
    let mut across = HashSet::new();
    let mut down = HashSet::new();
    for k in &keys {
        let (top, bottom) = (k.row * 2, k.row * 2 + 2);
        for x in k.x..k.x + k.width {
            across.insert((x, top));
            across.insert((x, bottom));
        }
        for y in top..bottom {
            down.insert((k.x, y));
            down.insert((k.x + k.width, y));
        }
    }
    let border = Style::new().fg(theme.muted);
    let mut canvas: Vec<Vec<(char, Style)>> = (0..HEIGHT)
        .map(|y| {
            (0..width)
                .map(|x| {
                    let c = junction(
                        y > 0 && down.contains(&(x, y - 1)),
                        down.contains(&(x, y)),
                        x > 0 && across.contains(&(x - 1, y)),
                        across.contains(&(x, y)),
                    );
                    (c, border)
                })
                .collect()
        })
        .collect();

    let lit = |ch: char| app.lit_keys().filter(|p| p.key == ch).last();
    for k in &keys {
        let style = match lit(k.ch) {
            Some(p) => Style::new()
                .fg(theme.cursor_fg)
                .bg(if p.ok { theme.accent } else { theme.error })
                .add_modifier(Modifier::BOLD),
            None if k.ch == ' ' => Style::new().fg(theme.text),
            None => match app.keys.get(k.ch) {
                Some(l) if l.focused => Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
                Some(l) if l.included => Style::new().fg(theme.text),
                _ => Style::new().fg(theme.muted),
            },
        };
        let row = &mut canvas[(k.row * 2 + 1) as usize];
        for x in k.x + 1..k.x + k.width {
            row[x as usize] = (' ', style);
        }
        let label = if k.ch == ' ' { '␣' } else { k.ch };
        row[(k.x + k.width / 2) as usize].0 = label;
    }

    let lines: Vec<Line> = canvas
        .into_iter()
        .map(|row| {
            Line::from(
                row.into_iter()
                    .map(|(c, s)| Span::styled(c.to_string(), s))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let indent = area.width.saturating_sub(width) / 2;
    let area = Rect {
        x: area.x + indent,
        width: area.width - indent,
        ..area
    };
    frame.render_widget(Paragraph::new(lines), area);
}
