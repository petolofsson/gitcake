mod app;
mod cli;
mod config;
mod ui;

use std::{io, time::Duration};

use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;
use config::Config;

fn main() -> io::Result<()> {
    if std::env::args().len() > 1 {
        let parsed = cli::Cli::parse();
        if let Err(e) = cli::run(parsed) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    let config = Config::load();
    let mut app = App::new(config);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_tui(&mut terminal, &mut app);

    app.cleanup();

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
    )?;
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
            while event::poll(Duration::from_millis(0))? {
                let _ = event::read();
            }
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
