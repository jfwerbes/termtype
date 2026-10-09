//! User settings, stored as TOML. Defaults mirror keybr's.

use crate::engine::lesson::LessonSettings;
use crate::engine::session::DrillSettings;
use crate::engine::textgen::TextSettings;
use crate::engine::textinput::Settings as InputSettings;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub lesson: LessonConfig,
    pub input: InputConfig,
    pub drill: DrillConfig,
    pub theme: String,
    /// The on-screen keyboard below the lesson.
    pub keyboard: KeyboardLayout,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyboardLayout {
    /// Row-staggered, like a laptop keyboard.
    #[default]
    Standard,
    /// Columnar split ergo halves (3x5 plus thumbs).
    Split,
    Off,
}

impl KeyboardLayout {
    pub const ALL: [KeyboardLayout; 3] = [Self::Standard, Self::Split, Self::Off];

    pub fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Split => "split",
            Self::Off => "off",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LessonConfig {
    /// Target speed per key, in words per minute (keybr default 35 = 175 CPM).
    pub target_wpm: f64,
    /// 0..=1: fraction of locked letters to unlock up front.
    pub alphabet_size: f64,
    /// Judge keys by current rather than best-ever speed.
    pub recover_keys: bool,
    /// Use real words when enough of them fit the unlocked letters.
    pub natural_words: bool,
    /// 0..=1: lesson length from 100 to 200 characters.
    pub length: f64,
    /// Times each word is repeated in a row.
    pub repeat_words: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InputConfig {
    pub stop_on_error: bool,
    pub forgive_errors: bool,
    pub space_skips_words: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DrillConfig {
    /// Stop on every mistake and retype the failed word.
    pub enabled: bool,
    pub repeat_count: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lesson: LessonConfig::default(),
            input: InputConfig::default(),
            drill: DrillConfig::default(),
            theme: "terminal".into(),
            keyboard: KeyboardLayout::default(),
        }
    }
}

impl Default for LessonConfig {
    fn default() -> Self {
        Self {
            target_wpm: 35.0,
            alphabet_size: 0.0,
            recover_keys: false,
            natural_words: true,
            length: 0.0,
            repeat_words: 1,
        }
    }
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            stop_on_error: true,
            forgive_errors: true,
            space_skips_words: false,
        }
    }
}

impl Default for DrillConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            repeat_count: 10,
        }
    }
}

impl Config {
    /// Reads the config, or the defaults if the file does not exist.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(source) => toml::from_str(&source)
                .with_context(|| format!("invalid config {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
        }
    }

    /// Writes the config atomically.
    pub fn save(&self, path: &Path) -> Result<()> {
        write_atomic(path, toml::to_string_pretty(self)?.as_bytes())
            .with_context(|| format!("cannot write {}", path.display()))
    }

    pub fn lesson_settings(&self) -> LessonSettings {
        LessonSettings {
            target_speed: self.lesson.target_wpm * 5.0,
            alphabet_size: self.lesson.alphabet_size.clamp(0.0, 1.0),
            recover_keys: self.lesson.recover_keys,
        }
    }

    pub fn text_settings(&self) -> TextSettings {
        TextSettings {
            length: self.lesson.length.clamp(0.0, 1.0),
            natural_words: self.lesson.natural_words,
            repeat_words: self.lesson.repeat_words.max(1),
        }
    }

    pub fn input_settings(&self) -> InputSettings {
        InputSettings {
            stop_on_error: self.input.stop_on_error,
            forgive_errors: self.input.forgive_errors,
            space_skips_words: self.input.space_skips_words,
        }
    }

    pub fn drill_settings(&self) -> DrillSettings {
        DrillSettings {
            enabled: self.drill.enabled,
            repeat_count: self.drill.repeat_count.max(1),
        }
    }
}

/// Writes via a temporary file and rename, so a crash never leaves a
/// half-written file.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let dir = path.parent().context("path has no parent")?;
    std::fs::create_dir_all(dir)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_mirror_keybr() {
        let c = Config::default();
        assert_eq!(c.lesson_settings(), LessonSettings::default());
        assert_eq!(c.lesson_settings().target_speed, 175.0);
        assert_eq!(c.text_settings(), TextSettings::default());
        assert_eq!(c.input_settings(), InputSettings::default());
        assert_eq!(c.drill_settings(), DrillSettings::default());
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            Config::load(&dir.path().join("config.toml")).unwrap(),
            Config::default()
        );
    }

    #[test]
    fn partial_file_fills_in_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "theme = \"nord\"\nkeyboard = \"split\"\n[drill]\nenabled = true\n",
        )
        .unwrap();
        let c = Config::load(&path).unwrap();
        assert_eq!(c.theme, "nord");
        assert_eq!(c.keyboard, KeyboardLayout::Split);
        assert!(c.drill.enabled);
        assert_eq!(c.drill.repeat_count, 10);
        assert_eq!(c.lesson, LessonConfig::default());
    }

    #[test]
    fn unknown_keys_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[lesson]\ntarget_wmp = 40\n").unwrap();
        let err = format!("{:#}", Config::load(&path).unwrap_err());
        assert!(err.contains("target_wmp"), "{err}");
        assert!(err.contains("config.toml"), "{err}");
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/config.toml");
        let mut c = Config::default();
        c.lesson.target_wpm = 50.0;
        c.drill.enabled = true;
        c.keyboard = KeyboardLayout::Off;
        c.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), c);
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn converts_to_engine_settings() {
        let c = Config {
            lesson: LessonConfig {
                target_wpm: 50.0,
                alphabet_size: 0.5,
                recover_keys: true,
                natural_words: false,
                length: 1.0,
                repeat_words: 3,
            },
            drill: DrillConfig {
                enabled: true,
                repeat_count: 5,
            },
            ..Config::default()
        };
        assert_eq!(
            c.lesson_settings(),
            LessonSettings {
                target_speed: 250.0,
                alphabet_size: 0.5,
                recover_keys: true
            }
        );
        assert_eq!(
            c.text_settings(),
            TextSettings {
                length: 1.0,
                natural_words: false,
                repeat_words: 3
            }
        );
        assert_eq!(
            c.drill_settings(),
            DrillSettings {
                enabled: true,
                repeat_count: 5
            }
        );
    }
}
