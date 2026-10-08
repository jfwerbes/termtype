//! Colour themes: semantic slots with hex colours, from TOML.

use anyhow::{Context, Result, bail};
use ratatui::style::Color;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

/// Semantic colour slots used by the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: String,
    /// `Color::Reset` keeps the terminal's own background.
    pub background: Color,
    /// Text not typed yet.
    pub text: Color,
    /// Text typed correctly.
    pub typed: Color,
    /// Text typed after a mistake, and error messages.
    pub error: Color,
    pub cursor_fg: Color,
    pub cursor_bg: Color,
    /// Headings, the focused key and highlights.
    pub accent: Color,
    /// Hints, borders and locked keys.
    pub muted: Color,
    /// Keys at or above the target speed.
    pub good: Color,
    /// Keys well below the target speed.
    pub bad: Color,
}

/// Parses `#rrggbb`, `#rgb`, an ANSI colour name or `default`.
pub fn parse_color(s: &str) -> Result<Color> {
    if let Some(hex) = s.strip_prefix('#') {
        let digits: Vec<u8> = hex
            .chars()
            .map(|c| c.to_digit(16).map(|d| d as u8))
            .collect::<Option<_>>()
            .with_context(|| format!("bad hex colour {s:?}"))?;
        return match digits[..] {
            [r, g, b] => Ok(Color::Rgb(r * 17, g * 17, b * 17)),
            [r1, r2, g1, g2, b1, b2] => Ok(Color::Rgb(r1 * 16 + r2, g1 * 16 + g2, b1 * 16 + b2)),
            _ => bail!("bad hex colour {s:?}: expected #rgb or #rrggbb"),
        };
    }
    Ok(match s.to_ascii_lowercase().as_str() {
        "default" | "reset" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::Gray,
        "bright-black" | "gray" | "grey" => Color::DarkGray,
        "bright-red" => Color::LightRed,
        "bright-green" => Color::LightGreen,
        "bright-yellow" => Color::LightYellow,
        "bright-blue" => Color::LightBlue,
        "bright-magenta" => Color::LightMagenta,
        "bright-cyan" => Color::LightCyan,
        "bright-white" => Color::White,
        _ => bail!("unknown colour {s:?}: use #rrggbb, an ANSI name or \"default\""),
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    base: Option<String>,
    #[serde(default)]
    colors: BTreeMap<String, String>,
}

fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Hex palette in slot order: background, text, typed, error, cursor_fg,
/// cursor_bg, accent, muted, good, bad.
fn palette(name: &str, c: [u32; 10]) -> Theme {
    let [
        background,
        text,
        typed,
        error,
        cursor_fg,
        cursor_bg,
        accent,
        muted,
        good,
        bad,
    ] = c.map(rgb);
    Theme {
        name: name.into(),
        background,
        text,
        typed,
        error,
        cursor_fg,
        cursor_bg,
        accent,
        muted,
        good,
        bad,
    }
}

impl Theme {
    /// The built-in theme names, in display order.
    pub const BUILT_IN: [&str; 4] = ["terminal", "gruvbox", "catppuccin-mocha", "nord"];

    pub fn built_in(name: &str) -> Option<Theme> {
        Some(match name {
            // Uses the terminal's own palette, so it matches any colour scheme.
            "terminal" => Theme {
                name: name.into(),
                background: Color::Reset,
                text: Color::Reset,
                typed: Color::DarkGray,
                error: Color::Red,
                cursor_fg: Color::Black,
                cursor_bg: Color::Gray,
                accent: Color::Cyan,
                muted: Color::DarkGray,
                good: Color::Green,
                bad: Color::Red,
            },
            "gruvbox" => palette(
                name,
                [
                    0x282828, 0xebdbb2, 0x928374, 0xfb4934, 0x282828, 0xfabd2f, 0x83a598, 0x665c54,
                    0xb8bb26, 0xfb4934,
                ],
            ),
            "catppuccin-mocha" => palette(
                name,
                [
                    0x1e1e2e, 0xcdd6f4, 0x6c7086, 0xf38ba8, 0x1e1e2e, 0xf5e0dc, 0x89b4fa, 0x585b70,
                    0xa6e3a1, 0xf38ba8,
                ],
            ),
            "nord" => palette(
                name,
                [
                    0x2e3440, 0xeceff4, 0x616e88, 0xbf616a, 0x2e3440, 0x88c0d0, 0x88c0d0, 0x4c566a,
                    0xa3be8c, 0xbf616a,
                ],
            ),
            _ => return None,
        })
    }

    fn slot_mut(&mut self, slot: &str) -> Option<&mut Color> {
        Some(match slot {
            "background" => &mut self.background,
            "text" => &mut self.text,
            "typed" => &mut self.typed,
            "error" => &mut self.error,
            "cursor_fg" => &mut self.cursor_fg,
            "cursor_bg" => &mut self.cursor_bg,
            "accent" => &mut self.accent,
            "muted" => &mut self.muted,
            "good" => &mut self.good,
            "bad" => &mut self.bad,
            _ => return None,
        })
    }

    /// Parses a theme file. Slots left out keep the colours of `base`
    /// (default `terminal`).
    pub fn from_toml(name: &str, source: &str) -> Result<Theme> {
        let file: ThemeFile = toml::from_str(source)?;
        let base = file.base.as_deref().unwrap_or("terminal");
        let mut theme =
            Theme::built_in(base).with_context(|| format!("unknown base theme {base:?}"))?;
        theme.name = name.into();
        for (slot, value) in &file.colors {
            let color = parse_color(value).with_context(|| format!("colour {slot:?}"))?;
            *theme
                .slot_mut(slot)
                .with_context(|| format!("unknown colour slot {slot:?}"))? = color;
        }
        Ok(theme)
    }

    /// Finds `name` in the user's themes directory, then the built-ins.
    pub fn load(name: &str, themes_dir: &Path) -> Result<Theme> {
        let path = themes_dir.join(format!("{name}.toml"));
        match std::fs::read_to_string(&path) {
            Ok(source) => Theme::from_toml(name, &source)
                .with_context(|| format!("invalid theme {}", path.display())),
            Err(_) => Theme::built_in(name).with_context(|| format!("unknown theme {name:?}")),
        }
    }

    /// All theme names: built-ins first, then user themes.
    pub fn available(themes_dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = Self::BUILT_IN.iter().map(|s| s.to_string()).collect();
        let mut user: Vec<String> = std::fs::read_dir(themes_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                (path.extension()? == "toml")
                    .then(|| path.file_stem()?.to_str().map(String::from))?
            })
            .filter(|n| !names.contains(n))
            .collect();
        user.sort();
        names.extend(user);
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_colors() {
        assert_eq!(parse_color("#ff8000").unwrap(), Color::Rgb(255, 128, 0));
        assert_eq!(parse_color("#F80").unwrap(), Color::Rgb(255, 136, 0));
        assert_eq!(parse_color("red").unwrap(), Color::Red);
        assert_eq!(parse_color("bright-blue").unwrap(), Color::LightBlue);
        assert_eq!(parse_color("default").unwrap(), Color::Reset);
        assert!(parse_color("#12345").is_err());
        assert!(parse_color("#gggggg").is_err());
        assert!(parse_color("chartreuse").is_err());
    }

    #[test]
    fn built_ins_exist() {
        for name in Theme::BUILT_IN {
            assert_eq!(Theme::built_in(name).unwrap().name, name);
        }
        assert_eq!(
            Theme::built_in("terminal").unwrap().background,
            Color::Reset
        );
        assert!(Theme::built_in("nope").is_none());
    }

    #[test]
    fn partial_theme_inherits_terminal() {
        let t = Theme::from_toml("mine", "[colors]\naccent = \"#112233\"\n").unwrap();
        let base = Theme::built_in("terminal").unwrap();
        assert_eq!(t.name, "mine");
        assert_eq!(t.accent, Color::Rgb(0x11, 0x22, 0x33));
        assert_eq!(t.text, base.text);
    }

    #[test]
    fn theme_can_extend_built_in() {
        let t = Theme::from_toml("mine", "base = \"nord\"\n[colors]\nerror = \"red\"\n").unwrap();
        let nord = Theme::built_in("nord").unwrap();
        assert_eq!(t.background, nord.background);
        assert_eq!(t.error, Color::Red);
    }

    #[test]
    fn bad_themes_are_rejected() {
        let err = format!(
            "{:#}",
            Theme::from_toml("x", "[colors]\naccnet = \"red\"\n").unwrap_err()
        );
        assert!(err.contains("accnet"), "{err}");
        let err = format!(
            "{:#}",
            Theme::from_toml("x", "[colors]\naccent = \"#zzz\"\n").unwrap_err()
        );
        assert!(err.contains("accent"), "{err}");
        assert!(Theme::from_toml("x", "base = \"nope\"\n").is_err());
    }

    #[test]
    fn load_prefers_user_theme() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("nord.toml"),
            "[colors]\naccent = \"#010203\"\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("zen.toml"), "").unwrap();
        assert_eq!(
            Theme::load("nord", dir.path()).unwrap().accent,
            Color::Rgb(1, 2, 3)
        );
        assert_eq!(Theme::load("gruvbox", dir.path()).unwrap().name, "gruvbox");
        assert!(Theme::load("missing", dir.path()).is_err());
        assert_eq!(
            Theme::available(dir.path()),
            ["terminal", "gruvbox", "catppuccin-mocha", "nord", "zen"]
        );
    }
}
