use anyhow::{Context, Result};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use termtype::app::{App, Effect};
use termtype::config::Config;
use termtype::engine::rng::LessonRng;
use termtype::keys::map_key;
use termtype::store::{self, Paths};
use termtype::theme::Theme;

fn main() -> Result<()> {
    let paths = Paths::from_env()?;
    if let Some(arg) = std::env::args().nth(1) {
        match arg.as_str() {
            "-V" | "--version" => println!("termtype {}", env!("CARGO_PKG_VERSION")),
            _ => print_help(&paths),
        }
        return Ok(());
    }

    let config = Config::load(&paths.config_file())?;
    // Warnings are printed after the TUI exits, which clears the screen.
    let mut warnings = vec![];
    let themes = load_themes(&paths, &mut warnings);
    if !themes.iter().any(|t| t.name == config.theme) {
        warnings.push(format!(
            "unknown theme {:?}, using {:?}",
            config.theme, themes[0].name
        ));
    }
    let history = store::load_results(&paths.results_file())?;
    if history.skipped > 0 {
        warnings.push(format!(
            "skipped {} unreadable lines in {}",
            history.skipped,
            paths.results_file().display()
        ));
    }

    let app = App::new(config, themes, history.results, LessonRng::from_entropy());
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, app, &paths);
    ratatui::restore();
    for w in warnings {
        eprintln!("warning: {w}");
    }
    result
}

fn run(terminal: &mut DefaultTerminal, mut app: App, paths: &Paths) -> Result<()> {
    let start = Instant::now();
    let now = || start.elapsed().as_secs_f64() * 1000.0;
    while !app.should_quit {
        app.now = now();
        terminal.draw(|f| termtype::ui::draw(f, &app))?;
        // Wake regularly so the live speed keeps updating, and when a lit
        // key on the keyboard should go dark.
        let wait = app.flash_ends_in().map_or(250.0, |ms| ms.min(250.0));
        if !event::poll(Duration::from_millis(wait.ceil() as u64 + 1))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        let Some(input) = map_key(key) else { continue };
        let unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        for effect in app.handle(input, now(), unix_ms) {
            match effect {
                Effect::SaveResult(r) => store::append_result(&paths.results_file(), &r)
                    .with_context(|| format!("saving to {}", paths.results_file().display()))?,
                Effect::SaveConfig(c) => c.save(&paths.config_file())?,
                Effect::ClearResults => store::clear_results(&paths.results_file())?,
            }
        }
    }
    Ok(())
}

/// Built-in themes plus any valid user themes; broken ones are reported.
fn load_themes(paths: &Paths, warnings: &mut Vec<String>) -> Vec<Theme> {
    Theme::available(&paths.themes_dir())
        .iter()
        .filter_map(|name| match Theme::load(name, &paths.themes_dir()) {
            Ok(t) => Some(t),
            Err(e) => {
                warnings.push(format!("{e:#}"));
                None
            }
        })
        .collect()
}

fn print_help(paths: &Paths) {
    println!(
        "termtype {} — adaptive touch typing in the terminal

Usage: termtype [--help | --version]

Lessons start with six letters and unlock the next one once every
unlocked key reaches the target speed (keybr.com's method).

Keys while typing:  esc menu · tab new lesson · ctrl-w / ctrl-backspace delete word
Keys in the menu:   enter practice · g guided · t transition drills
                    k key stats · s settings · q quit

Files:
  config   {}
  themes   {}/<name>.toml
  history  {}",
        env!("CARGO_PKG_VERSION"),
        paths.config_file().display(),
        paths.themes_dir().display(),
        paths.results_file().display(),
    );
}
