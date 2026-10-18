mod app;
mod cover_art;
mod lyrics;
mod mpd_client;
mod ui;

use anyhow::Result;
use app::{ActiveTab, App};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{stdout, Write};
use std::panic;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    // Setup panic hook to restore terminal
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let mut out = stdout();
        let _ = cover_art::clear_kitty_images(&mut out);
        let _ = execute!(out, LeaveAlternateScreen, DisableMouseCapture);
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
    let mut was_player = true;

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        // In Kitty terminal: render the crystal-clear high resolution spinning vinyl PNG!
        if app.cover_art.supports_kitty {
            if app.active_tab == ActiveTab::Player && !app.show_help {
                let size = terminal.size()?;
                let card_width = 72.min(size.width.saturating_sub(2)).max(28);
                let card_x = (size.width.saturating_sub(card_width)) / 2;

                // Container box inside card:
                // header: 1 line
                // visualizer: 2 lines
                // box starts at y = 3
                let disc_box_y = 3u16;
                let available_h = size.height.saturating_sub(15).max(6);
                let disc_box_h = available_h;
                let disc_box_w = card_width;

                let disc_inner_x = card_x + 1;
                let disc_inner_y = disc_box_y + 1;
                let disc_inner_w = disc_box_w.saturating_sub(2);
                let disc_inner_h = disc_box_h.saturating_sub(2);

                let max_cols = disc_inner_w.saturating_sub(2);
                let max_rows = disc_inner_h;
                let disc_cols = max_cols.min(max_rows * 2).max(6);
                let disc_rows = (disc_cols / 2).max(3);

                let disc_x = disc_inner_x + (disc_inner_w.saturating_sub(disc_cols)) / 2;
                let disc_y = disc_inner_y + (disc_inner_h.saturating_sub(disc_rows)) / 2;

                let (t_str, a_str) = if let Some(ref s) = app.current_song {
                    (s.display_title(), s.display_artist())
                } else {
                    ("In The Soul".to_string(), "MPD".to_string())
                };

                if let Some(png) = app.cover_art.render_highres_vinyl_png(
                    360,
                    app.rotation_angle,
                    app.theme.accent,
                    &t_str,
                    &a_str,
                ) {
                    let mut stdout_handle = std::io::stdout().lock();
                    let _ = cover_art::send_kitty_image(
                        &mut stdout_handle,
                        &png,
                        disc_x,
                        disc_y,
                        disc_cols,
                        disc_rows,
                        1,
                    );
                }
                was_player = true;
            } else if was_player {
                let mut stdout_handle = std::io::stdout().lock();
                let _ = cover_art::clear_kitty_images(&mut stdout_handle);
                was_player = false;
            }
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_millis(0));

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        app.handle_key_event(key);
                    }
                }
                Event::Mouse(mouse) => {
                    app.handle_mouse_event(mouse);
                }
                Event::Resize(_, _) => {
                    if app.cover_art.supports_kitty {
                        let mut out = std::io::stdout().lock();
                        let _ = cover_art::clear_kitty_images(&mut out);
                    }
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
    if app.cover_art.supports_kitty {
        let mut out = std::io::stdout().lock();
        let _ = cover_art::clear_kitty_images(&mut out);
    }
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    Ok(())
}
