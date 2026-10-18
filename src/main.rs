mod app;
mod cover_art;
mod lyrics;
mod mpd_client;
mod ui;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::panic;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    // Setup panic hook to restore terminal
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture);
        default_hook(panic_info);
    }));

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let mut app = App::new();

    let tick_rate = Duration::from_millis(33); // ~30 FPS for smooth vinyl rotation
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_millis(0));

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    // Ignore release events to avoid double-triggers
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        app.handle_key_event(key);
                    }
                }
                Event::Mouse(mouse) => {
                    app.handle_mouse_event(mouse);
                }
                Event::Resize(_, _) => {
                    // Handled automatically on next draw
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.tick();
            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    // Clean terminal shutdown
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    Ok(())
}
