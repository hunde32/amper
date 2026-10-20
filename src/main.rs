mod app;
mod audio_visualizer;
mod cover_art;
mod lyrics;
mod matrix_rain;
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
use ratatui::style::Color;
use ratatui::Terminal;
use std::io::stdout;
use std::panic;
use std::time::{Duration, Instant};

#[derive(PartialEq, Eq)]
struct KittyCacheKey {
    song_file: String,
    cover_mode: app::CoverMode,
    img_x: u16,
    img_y: u16,
    img_cols: u16,
    img_rows: u16,
    accent: (u8, u8, u8),
}

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

    let tick_rate = Duration::from_millis(33); // ~30 FPS
    let mut last_tick = Instant::now();
    let mut last_kitty_key: Option<KittyCacheKey> = None;
    let mut last_vinyl_render = Instant::now();
    let mut last_was_player = false;

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        // In Kitty terminal: render the crystal-clear high resolution PNG
        if app.cover_art.supports_kitty {
            if app.active_tab == ActiveTab::Player && !app.show_help {
                let size = terminal.size()?;
                let geom = App::get_cover_geometry(size.into());

                let disc_x = geom.img_x;
                let disc_y = geom.img_y;
                let disc_cols = geom.img_cols;
                let disc_rows = geom.img_rows;

                let (t_str, a_str) = if let Some(ref s) = app.current_song {
                    (s.display_title(), s.display_artist())
                } else {
                    ("In The Soul".to_string(), "MPD".to_string())
                };

                let accent_rgb = match app.theme.accent {
                    Color::Rgb(r, g, b) => (r, g, b),
                    _ => (193, 193, 255),
                };

                let song_file = app.current_song.as_ref().map(|s| s.file.clone()).unwrap_or_default();

                match app.cover_mode {
                    app::CoverMode::StaticCard => {
                        let current_key = KittyCacheKey {
                            song_file,
                            cover_mode: app::CoverMode::StaticCard,
                            img_x: disc_x,
                            img_y: disc_y,
                            img_cols: disc_cols,
                            img_rows: disc_rows,
                            accent: accent_rgb,
                        };

                        // Zero-overhead cache: ONLY transmit to Kitty when song, layout, or mode changed!
                        if last_kitty_key.as_ref() != Some(&current_key) {
                            // Estimate actual pixel size from cell count.
                            // Kitty typical cell: ~10 px wide × 20 px tall.
                            // We take the larger dimension so the square PNG fills the area.
                            let cell_px_w: u32 = 10;
                            let cell_px_h: u32 = 20;
                            let size_px = (disc_cols as u32 * cell_px_w)
                                .max(disc_rows as u32 * cell_px_h);
                            if let Some(png) = app.cover_art.render_highres_cover_png(
                                size_px,
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
                            last_kitty_key = Some(current_key);
                        }
                    }
                    app::CoverMode::SpinningVinyl => {
                        let is_playing = app.mpd_status.state == mpd_client::PlaybackState::Play;

                        let should_render = if !is_playing {
                            // Paused: render once at current angle!
                            let current_key = KittyCacheKey {
                                song_file,
                                cover_mode: app::CoverMode::SpinningVinyl,
                                img_x: disc_x,
                                img_y: disc_y,
                                img_cols: disc_cols,
                                img_rows: disc_rows,
                                accent: accent_rgb,
                            };
                            if last_kitty_key.as_ref() != Some(&current_key) {
                                last_kitty_key = Some(current_key);
                                true
                            } else {
                                false
                            }
                        } else {
                            // Playing: throttle to ~16 FPS (60ms) to ensure butter smoothness without PTY lag
                            last_kitty_key = None;
                            if last_vinyl_render.elapsed() >= Duration::from_millis(60) {
                                last_vinyl_render = Instant::now();
                                true
                            } else {
                                false
                            }
                        };

                        if should_render {
                            let cell_px_w: u32 = 10;
                            let cell_px_h: u32 = 20;
                            let size_px = (disc_cols as u32 * cell_px_w)
                                .max(disc_rows as u32 * cell_px_h)
                                .max(360);
                            if let Some(png) = app.cover_art.render_highres_vinyl_png(
                                size_px,
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
                        }
                    }
                }
                last_was_player = true;
            } else if last_was_player {
                let mut stdout_handle = std::io::stdout().lock();
                let _ = cover_art::clear_kitty_images(&mut stdout_handle);
                last_was_player = false;
                last_kitty_key = None;
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
                        last_kitty_key = None;
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
