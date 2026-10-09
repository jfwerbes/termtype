//! Lesson history, one JSON result per line.

use crate::engine::result::LessonResult;
use anyhow::{Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Where termtype keeps its files.
#[derive(Debug, Clone)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl Paths {
    /// `~/.config/termtype` and `~/.local/share/termtype` on Linux.
    pub fn from_env() -> Result<Self> {
        let config = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("no config directory"))?;
        let data = dirs::data_dir().ok_or_else(|| anyhow::anyhow!("no data directory"))?;
        Ok(Self {
            config_dir: config.join("termtype"),
            data_dir: data.join("termtype"),
        })
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    pub fn themes_dir(&self) -> PathBuf {
        self.config_dir.join("themes")
    }

    pub fn results_file(&self) -> PathBuf {
        self.data_dir.join("results.jsonl")
    }
}

/// Results that loaded, plus how many lines were unreadable.
#[derive(Debug, Default)]
pub struct History {
    pub results: Vec<LessonResult>,
    pub skipped: usize,
}

/// Loads all results. Unreadable lines are skipped, never deleted.
pub fn load_results(path: &Path) -> Result<History> {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(History::default()),
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
    };
    let mut history = History::default();
    for line in source.lines().filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str(line) {
            Ok(r) => history.results.push(r),
            Err(_) => history.skipped += 1,
        }
    }
    Ok(history)
}

pub fn append_result(path: &Path, result: &LessonResult) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut line = serde_json::to_string(result)?;
    line.push('\n');
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("cannot open {}", path.display()))?;
    f.write_all(line.as_bytes())?;
    Ok(())
}

/// Moves the history aside to `<file>.bak`, replacing any older backup.
pub fn clear_results(path: &Path) -> Result<()> {
    match std::fs::rename(path, backup_path(path)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(e).with_context(|| format!("cannot clear {}", path.display()))
        }
        _ => Ok(()),
    }
}

pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".bak");
    name.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::result::tests::fake_result;

    #[test]
    fn missing_file_is_empty_history() {
        let dir = tempfile::tempdir().unwrap();
        let h = load_results(&dir.path().join("results.jsonl")).unwrap();
        assert!(h.results.is_empty());
        assert_eq!(h.skipped, 0);
    }

    #[test]
    fn append_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data/results.jsonl");
        let r1 = fake_result(1, &[('a', 100), ('b', 200), ('c', 300)]);
        let r2 = fake_result(2, &[('a', 150)]);
        append_result(&path, &r1).unwrap();
        append_result(&path, &r2).unwrap();
        let h = load_results(&path).unwrap();
        assert_eq!(h.results, vec![r1, r2]);
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 2);
    }

    #[test]
    fn corrupt_lines_are_skipped_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("results.jsonl");
        let r1 = fake_result(1, &[('a', 100)]);
        append_result(&path, &r1).unwrap();
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(f, "{{not json").unwrap();
        writeln!(f).unwrap();
        drop(f);
        let r2 = fake_result(2, &[('b', 100)]);
        append_result(&path, &r2).unwrap();
        let h = load_results(&path).unwrap();
        assert_eq!(h.results, vec![r1, r2]);
        assert_eq!(h.skipped, 1);
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("{not json")
        );
    }

    #[test]
    fn clear_moves_history_to_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("results.jsonl");
        let r1 = fake_result(1, &[('a', 100)]);
        append_result(&path, &r1).unwrap();
        clear_results(&path).unwrap();
        assert!(load_results(&path).unwrap().results.is_empty());
        assert_eq!(load_results(&backup_path(&path)).unwrap().results, vec![r1]);
        // Nothing to clear is fine.
        clear_results(&path).unwrap();
    }
}
