mod app;
mod cli;
mod config;
mod ui;

use std::{io, time::Duration};

use clap::Parser;
use crossterm::{
    event,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;
use config::Config;

fn main() -> io::Result<()> {
    let parsed = cli::Cli::parse();

    if let Some(command) = parsed.command {
        if let Err(e) = cli::run(command, parsed.repo) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    let (mut config, load_warning) = Config::load();
    if parsed.new {
        config.repo_path = None;
    } else if let Some(path) = parsed.repo {
        config.repo_path = Some(path);
    }
    let mut app = App::new(config, load_warning);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_tui(&mut terminal, &mut app);

    app.cleanup();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Some(msg) = app.exit_message {
        println!("{msg}");
    }

    result
}

fn run_tui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        if app.needs_clear {
            terminal.clear()?;
            app.needs_clear = false;
        }
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            let ev = event::read()?;
            app.handle_event(ev);
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
