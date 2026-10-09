//! Renders the README screenshots to `docs/screenshots/*.png`.
//!
//!     cargo run --example screenshots
//!
//! Needs `rsvg-convert` (librsvg) for the PNGs; without it the SVGs are
//! kept instead.
//!
//! Plays a simulated typist through a few dozen lessons so every screen
//! has real data, then draws each screen with ratatui's test backend and
//! writes the cells out as SVG. Box-drawing and block characters are drawn
//! as shapes, so they join up whatever font the viewer has.

use anyhow::Result;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::{Buffer, Cell};
use ratatui::style::{Color, Modifier};
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use termtype::app::{App, Field, Input};
use termtype::config::{Config, KeyboardLayout};
use termtype::engine::rng::LessonRng;
use termtype::theme::Theme;

const OUT: &str = "docs/screenshots";
const THEME: &str = "gruvbox";
const LESSONS: usize = 30;
/// Cell size in SVG units.
const CW: f64 = 10.0;
const CH: f64 = 21.0;
const FONT: &str = "'JetBrains Mono', 'Cascadia Mono', 'CaskaydiaCove Nerd Font Mono', \
                    'DejaVu Sans Mono', Menlo, Consolas, monospace";

fn main() -> Result<()> {
    std::fs::create_dir_all(OUT)?;
    let config = Config {
        theme: THEME.into(),
        ..Config::default()
    };
    let themes = vec![Theme::built_in(THEME).expect("built-in theme")];
    let mut app = App::new(config, themes, vec![], LessonRng::seeded(7));
    let mut typist = Typist::new(11);
    let mut clock = Clock::default();

    for lesson in 0..LESSONS {
        typist.finish(&mut app, &mut clock, lesson);
        app.handle(Input::Enter, clock.now, clock.unix());
    }
    typist.finish(&mut app, &mut clock, LESSONS);

    // Menu right after a lesson, with the mode panel.
    save("menu", &app, 80, 27)?;

    // Key stats.
    app.handle(Input::Char('k'), clock.now, clock.unix());
    save("key-stats", &app, 100, 30)?;
    app.handle(Input::Esc, clock.now, clock.unix());

    // A guided lesson part-way through, a key still lit.
    app.handle(Input::Char('g'), clock.now, clock.unix());
    typist.type_some(&mut app, &mut clock, LESSONS, 37);
    save("guided", &app, 80, 26)?;

    // A bigram drill, with the split keyboard.
    app.handle(Input::Esc, clock.now, clock.unix());
    app.config.keyboard = KeyboardLayout::Split;
    app.handle(Input::Char('b'), clock.now, clock.unix());
    typist.type_some(&mut app, &mut clock, LESSONS, 13);
    save("bigrams", &app, 80, 26)?;
    app.config.keyboard = KeyboardLayout::default();

    // Settings, asking before clearing progress.
    app.handle(Input::Esc, clock.now, clock.unix());
    app.handle(Input::Char('s'), clock.now, clock.unix());
    while Field::ALL[app.settings_cursor] != Field::ClearProgress {
        app.handle(Input::Down, clock.now, clock.unix());
    }
    app.handle(Input::Enter, clock.now, clock.unix());
    save("clear-progress", &app, 80, 26)?;
    Ok(())
}

#[derive(Default)]
struct Clock {
    /// Monotonic ms.
    now: f64,
}

impl Clock {
    fn unix(&self) -> i64 {
        1_790_000_000_000 + self.now as i64
    }
}

/// A typist who gets faster lesson by lesson, with a few awkward letters
/// and pairs that stay slow, and the odd wrong key.
struct Typist {
    seed: u64,
}

impl Typist {
    fn new(seed: u64) -> Self {
        Self { seed }
    }

    fn random(&mut self) -> f64 {
        // xorshift64*
        self.seed ^= self.seed >> 12;
        self.seed ^= self.seed << 25;
        self.seed ^= self.seed >> 27;
        (self.seed.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Ms to type `c` after `prev` in the given lesson.
    fn delay(&mut self, lesson: usize, prev: Option<char>, c: char) -> f64 {
        let base = 400.0 - 170.0 * (lesson as f64 / LESSONS as f64).min(1.0);
        let key = match c {
            'l' | 'o' | 'h' => 1.25,
            ' ' => 0.8,
            _ => 1.0,
        };
        let pair = match (prev, c) {
            (Some('r'), 'l') | (Some('n'), 'i') | (Some('t'), 'r') => 2.2,
            (Some('e'), 'a') | (Some('l'), 't') | (Some('o'), 'n') => 1.8,
            (Some('i'), 'n') | (Some('a'), 'r') => 1.5,
            _ => 1.0,
        };
        base * key * pair * (0.85 + 0.3 * self.random())
    }

    /// Types the next character, sometimes hitting a wrong key first.
    fn step(&mut self, app: &mut App, clock: &mut Clock, lesson: usize) {
        let input = app.session.input();
        let text = input.text();
        let pos = input.pos();
        let c = text[pos];
        let prev = pos.checked_sub(1).map(|i| text[i]);
        if c != ' ' && self.random() < 0.02 {
            clock.now += self.delay(lesson, prev, c);
            app.handle(Input::Char('x'), clock.now, clock.unix());
        }
        clock.now += self.delay(lesson, prev, c);
        app.handle(Input::Char(c), clock.now, clock.unix());
    }

    fn finish(&mut self, app: &mut App, clock: &mut Clock, lesson: usize) {
        while !app.session.completed() {
            self.step(app, clock, lesson);
        }
    }

    fn type_some(&mut self, app: &mut App, clock: &mut Clock, lesson: usize, n: usize) {
        for _ in 0..n {
            self.step(app, clock, lesson);
        }
        // Freeze the picture just after the last key, while it is still lit.
        app.now = clock.now + 40.0;
    }
}

fn save(name: &str, app: &App, width: u16, height: u16) -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|f| termtype::ui::draw(f, app))?;
    let svg = to_svg(terminal.backend().buffer(), app.theme());
    let svg_path = Path::new(OUT).join(format!("{name}.svg"));
    let png_path = svg_path.with_extension("png");
    std::fs::write(&svg_path, svg)?;
    let converted = Command::new("rsvg-convert")
        .args(["--zoom", "2", "-o"])
        .arg(&png_path)
        .arg(&svg_path)
        .status()
        .is_ok_and(|s| s.success());
    if converted {
        std::fs::remove_file(&svg_path)?;
        println!("wrote {}", png_path.display());
    } else {
        println!("wrote {} (rsvg-convert unavailable)", svg_path.display());
    }
    Ok(())
}

fn hex(color: Color, default: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Reset => hex(default, Color::Black),
        _ => "#888888".into(),
    }
}

fn escape(c: char) -> String {
    match c {
        '&' => "&amp;".into(),
        '<' => "&lt;".into(),
        '>' => "&gt;".into(),
        c => c.to_string(),
    }
}

/// Which way a box-drawing character's lines leave its cell: up, right,
/// down, left; and whether its corner is rounded.
fn box_arms(c: char) -> Option<([bool; 4], bool)> {
    Some(match c {
        '─' => ([false, true, false, true], false),
        '│' => ([true, false, true, false], false),
        '├' => ([true, true, true, false], false),
        '┤' => ([true, false, true, true], false),
        '┬' => ([false, true, true, true], false),
        '┴' => ([true, true, false, true], false),
        '┼' => ([true, true, true, true], false),
        '╭' => ([false, true, true, false], true),
        '╮' => ([false, false, true, true], true),
        '╰' => ([true, true, false, false], true),
        '╯' => ([true, false, false, true], true),
        _ => return None,
    })
}

/// Height of a lower block element (`▁` to `█`) as a share of the cell.
fn block_height(c: char) -> Option<f64> {
    let i = "▁▂▃▄▅▆▇█".chars().position(|b| b == c)?;
    Some((i + 1) as f64 / 8.0)
}

fn to_svg(buf: &Buffer, theme: &Theme) -> String {
    let area = buf.area;
    let (w, h) = (area.width as f64 * CW, area.height as f64 * CH);
    let background = hex(theme.background, Color::Black);
    let mut svg = String::new();
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="{FONT}" font-size="16">"#
    );
    let _ = writeln!(
        svg,
        r#"<rect width="100%" height="100%" rx="6" fill="{background}"/>"#
    );

    let fg_of = |cell: &Cell| hex(cell.fg, theme.text);
    let bg_of = |cell: &Cell| hex(cell.bg, theme.background);

    for y in 0..area.height {
        let top = y as f64 * CH;
        // Backgrounds, merged into runs.
        let mut x = 0;
        while x < area.width {
            let bg = bg_of(&buf[(x, y)]);
            let start = x;
            while x < area.width && bg_of(&buf[(x, y)]) == bg {
                x += 1;
            }
            if bg != background {
                let _ = writeln!(
                    svg,
                    r#"<rect x="{}" y="{top}" width="{}" height="{CH}" fill="{bg}"/>"#,
                    start as f64 * CW,
                    (x - start) as f64 * CW
                );
            }
        }
        // Lines, blocks and characters, one cell at a time.
        for x in 0..area.width {
            let cell = &buf[(x, y)];
            let c = cell.symbol().chars().next().unwrap_or(' ');
            let left = x as f64 * CW;
            let fg = fg_of(cell);
            if cell.modifier.contains(Modifier::CROSSED_OUT) && c != ' ' {
                let mid = top + CH * 0.5;
                let _ = writeln!(
                    svg,
                    r#"<line x1="{left}" y1="{mid}" x2="{}" y2="{mid}" stroke="{fg}" stroke-width="1.2"/>"#,
                    left + CW
                );
            }
            if let Some((arms, round)) = box_arms(c) {
                let (cx, cy) = (left + CW / 2.0, top + CH / 2.0);
                let ends = [(cx, top), (left + CW, cy), (cx, top + CH), (left, cy)];
                if round {
                    // Two arms joined by a curve that bends through the centre.
                    let r = CW / 2.0;
                    let arm: Vec<usize> = (0..4).filter(|&i| arms[i]).collect();
                    let (a, b) = (ends[arm[0]], ends[arm[1]]);
                    let toward = |(px, py): (f64, f64)| {
                        let step = |d: f64| if d == 0.0 { 0.0 } else { d.signum() * r };
                        (cx + step(px - cx), cy + step(py - cy))
                    };
                    let (a1, b1) = (toward(a), toward(b));
                    let _ = writeln!(
                        svg,
                        r#"<path d="M{:.1} {:.1} L{:.1} {:.1} Q{cx:.1} {cy:.1} {:.1} {:.1} L{:.1} {:.1}" fill="none" stroke="{fg}" stroke-width="1.2"/>"#,
                        a.0, a.1, a1.0, a1.1, b1.0, b1.1, b.0, b.1
                    );
                } else {
                    for (i, &(ex, ey)) in ends.iter().enumerate() {
                        if arms[i] {
                            let _ = writeln!(
                                svg,
                                r#"<line x1="{cx}" y1="{cy}" x2="{ex}" y2="{ey}" stroke="{fg}" stroke-width="1.2" stroke-linecap="square"/>"#
                            );
                        }
                    }
                }
                continue;
            }
            if let Some(share) = block_height(c) {
                let height = CH * share;
                let _ = writeln!(
                    svg,
                    r#"<rect x="{left}" y="{:.1}" width="{CW}" height="{height:.1}" fill="{fg}"/>"#,
                    top + CH - height
                );
                continue;
            }
            if c == ' ' {
                continue;
            }
            let weight = if cell.modifier.contains(Modifier::BOLD) {
                r#" font-weight="bold""#
            } else {
                ""
            };
            let _ = writeln!(
                svg,
                r#"<text x="{:.1}" y="{:.1}" fill="{fg}"{weight} text-anchor="middle">{}</text>"#,
                left + CW / 2.0,
                top + CH * 0.73,
                escape(c)
            );
        }
    }
    svg.push_str("</svg>\n");
    svg
}
