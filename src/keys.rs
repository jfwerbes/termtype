//! Maps terminal key events to app inputs.

use crate::app::Input;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub fn map_key(key: KeyEvent) -> Option<Input> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    Some(match key.code {
        KeyCode::Char('c') if ctrl => Input::Quit,
        // Ctrl-Backspace arrives as Ctrl-H in most terminals.
        KeyCode::Char('w' | 'h') if ctrl => Input::ClearWord,
        KeyCode::Backspace if ctrl || alt => Input::ClearWord,
        KeyCode::Char(_) if ctrl || alt => return None,
        KeyCode::Char(c) => Input::Char(c),
        KeyCode::Backspace => Input::Backspace,
        KeyCode::Tab => Input::Restart,
        KeyCode::Enter => Input::Enter,
        KeyCode::Esc => Input::Esc,
        KeyCode::Up => Input::Up,
        KeyCode::Down => Input::Down,
        KeyCode::Left => Input::Left,
        KeyCode::Right => Input::Right,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn printable_chars() {
        assert_eq!(
            map_key(key(KeyCode::Char('a'), KeyModifiers::NONE)),
            Some(Input::Char('a'))
        );
        assert_eq!(
            map_key(key(KeyCode::Char('A'), KeyModifiers::SHIFT)),
            Some(Input::Char('A'))
        );
        assert_eq!(
            map_key(key(KeyCode::Char(' '), KeyModifiers::NONE)),
            Some(Input::Char(' '))
        );
        assert_eq!(map_key(key(KeyCode::Char('x'), KeyModifiers::ALT)), None);
    }

    #[test]
    fn control_keys() {
        assert_eq!(
            map_key(key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Input::Quit)
        );
        assert_eq!(
            map_key(key(KeyCode::Char('w'), KeyModifiers::CONTROL)),
            Some(Input::ClearWord)
        );
        // Ctrl-Backspace arrives as Ctrl-H in most terminals.
        assert_eq!(
            map_key(key(KeyCode::Char('h'), KeyModifiers::CONTROL)),
            Some(Input::ClearWord)
        );
        assert_eq!(
            map_key(key(KeyCode::Backspace, KeyModifiers::CONTROL)),
            Some(Input::ClearWord)
        );
        assert_eq!(
            map_key(key(KeyCode::Backspace, KeyModifiers::ALT)),
            Some(Input::ClearWord)
        );
        assert_eq!(
            map_key(key(KeyCode::Char('x'), KeyModifiers::CONTROL)),
            None
        );
    }

    #[test]
    fn special_keys() {
        let none = KeyModifiers::NONE;
        assert_eq!(
            map_key(key(KeyCode::Backspace, none)),
            Some(Input::Backspace)
        );
        assert_eq!(map_key(key(KeyCode::Tab, none)), Some(Input::Restart));
        assert_eq!(map_key(key(KeyCode::Enter, none)), Some(Input::Enter));
        assert_eq!(map_key(key(KeyCode::Esc, none)), Some(Input::Esc));
        assert_eq!(map_key(key(KeyCode::Up, none)), Some(Input::Up));
        assert_eq!(map_key(key(KeyCode::Down, none)), Some(Input::Down));
        assert_eq!(map_key(key(KeyCode::Left, none)), Some(Input::Left));
        assert_eq!(map_key(key(KeyCode::Right, none)), Some(Input::Right));
        assert_eq!(map_key(key(KeyCode::F(1), none)), None);
    }

    #[test]
    fn key_releases_are_ignored() {
        let mut k = key(KeyCode::Char('a'), KeyModifiers::NONE);
        k.kind = KeyEventKind::Release;
        assert_eq!(map_key(k), None);
    }
}
