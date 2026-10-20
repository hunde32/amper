use crate::audio_visualizer::AudioVisualizer;
use crate::cover_art::CoverArtManager;
use crate::lyrics::LyricsManager;
use crate::matrix_rain::MatrixRain;
use crate::mpd_client::{MpdClient, MpdStatus, PlaybackState, SongInfo};
use crate::ui::theme::Theme;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Player,
    Lyrics,
    Queue,
    Library,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverMode {
    StaticCard,
    SpinningVinyl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomPanelMode {
    Telemetry,
    Visualizer,
    MiniLyrics,
    Metadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverGeometry {
    pub card_rect: ratatui::layout::Rect,
    pub cover_box_rect: ratatui::layout::Rect,
    pub inner_rect: ratatui::layout::Rect,
    pub img_x: u16,
    pub img_y: u16,
    pub img_cols: u16,
    pub img_rows: u16,
}

pub struct App {
    pub should_quit: bool,
    pub active_tab: ActiveTab,
    pub cover_mode: CoverMode,
    pub bottom_mode: BottomPanelMode,
    pub show_help: bool,
    pub is_favorite: bool,
    pub theme: Theme,

    pub mpd: MpdClient,
    pub mpd_status: MpdStatus,
    pub current_song: Option<SongInfo>,
    pub playlist: Vec<SongInfo>,
    pub library_files: Vec<String>,

    pub cover_art: CoverArtManager,
    pub lyrics_mgr: LyricsManager,
    pub audio_viz: AudioVisualizer,
    pub matrix_rain: MatrixRain,
    pub rotation_angle: f32,
    pub tick_counter: u64,
    pub clicked_button: Option<(&'static str, Instant)>,

    pub queue_selected_idx: usize,
    pub queue_search_query: String,

    pub library_selected_idx: usize,
    pub library_search_query: String,

    pub is_searching: bool,
    pub lyrics_scroll_offset: Option<usize>,

    pub status_message: Option<String>,
    pub status_message_time: Option<Instant>,

    last_poll_time: Instant,
    last_frame_time: Instant,
}

impl App {
    pub fn new() -> Self {
        let mut mpd = MpdClient::new();
        let _ = mpd.connect();

        let music_dir = mpd.music_dir().cloned();
        let cover_art = CoverArtManager::new(music_dir);
        let lyrics_mgr = LyricsManager::new();
        let audio_viz = AudioVisualizer::new();
        let matrix_rain = MatrixRain::new();

        let mut app = Self {
            should_quit: false,
            active_tab: ActiveTab::Player,
            cover_mode: CoverMode::StaticCard,
            bottom_mode: BottomPanelMode::Telemetry,
            show_help: false,
            is_favorite: false,
            theme: Theme::new(),

            mpd,
            mpd_status: MpdStatus::default(),
            current_song: None,
            playlist: Vec::new(),
            library_files: Vec::new(),

            cover_art,
            lyrics_mgr,
            audio_viz,
            matrix_rain,
            rotation_angle: 0.0,
            tick_counter: 0,
            clicked_button: None,

            queue_selected_idx: 0,
            queue_search_query: String::new(),

            library_selected_idx: 0,
            library_search_query: String::new(),

            is_searching: false,
            lyrics_scroll_offset: None,

            status_message: Some("Welcome to Amper! Connected to MPD.".to_string()),
            status_message_time: Some(Instant::now()),

            last_poll_time: Instant::now() - Duration::from_secs(5),
            last_frame_time: Instant::now(),
        };

        app.refresh_mpd(true);
        app
    }

    pub fn set_status_message(&mut self, msg: &str) {
        self.status_message = Some(msg.to_string());
        self.status_message_time = Some(Instant::now());
    }

    pub fn refresh_mpd(&mut self, force_full: bool) {
        if let Ok(status) = self.mpd.get_status() {
            self.mpd_status = status;
        }

        if let Ok(song_opt) = self.mpd.get_current_song() {
            let song_changed = match (&self.current_song, &song_opt) {
                (Some(a), Some(b)) => a.id != b.id || a.file != b.file,
                (None, Some(_)) => true,
                (Some(_), None) => true,
                (None, None) => false,
            };

            if song_changed || force_full {
                if let Some(ref song) = song_opt {
                    let file_uri = song.file.clone();
                    self.cover_art.update_current_song(&file_uri, &mut self.mpd);

                    let music_path = self.mpd.music_dir().map(|d| d.join(&song.file));
                    self.lyrics_mgr.update_track(
                        &song.display_artist(),
                        &song.display_title(),
                        music_path.as_ref(),
                        song.duration,
                    );
                    self.lyrics_scroll_offset = None;
                }
                self.current_song = song_opt;
            }
        }

        if force_full || self.playlist.is_empty() {
            if let Ok(list) = self.mpd.get_playlist() {
                self.playlist = list;
            }
            if let Ok(files) = self.mpd.list_all_files() {
                self.library_files = files;
            }
        }
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        // Poll MPD status every 250ms
        if now.duration_since(self.last_poll_time) >= Duration::from_millis(250) {
            self.last_poll_time = now;
            self.refresh_mpd(false);
        } else if self.mpd_status.state == PlaybackState::Play {
            // Local smooth interpolation between polls
            self.mpd_status.elapsed += dt as f64;
            if self.mpd_status.duration > 0.0 && self.mpd_status.elapsed > self.mpd_status.duration {
                self.mpd_status.elapsed = self.mpd_status.duration;
            }
        }

        // Animate spinning vinyl when playing
        if self.mpd_status.state == PlaybackState::Play {
            self.rotation_angle = (self.rotation_angle + dt * 1.8) % (std::f32::consts::PI * 2.0);
        }

        // Advance Matrix rain animation & react to audio energy
        let (term_w, term_h) = crossterm::terminal::size().unwrap_or((80, 24));
        let geom = Self::get_cover_geometry(ratatui::layout::Rect {
            x: 0,
            y: 0,
            width: term_w,
            height: term_h,
        });
        let energy = if self.mpd_status.state == PlaybackState::Play {
            self.audio_viz.get_reactive_energy()
        } else {
            0.0
        };
        self.matrix_rain.update(
            term_w,
            term_h,
            geom.card_rect,
            &self.theme.wallpaper_colors,
            energy,
            dt,
        );

        self.tick_counter = self.tick_counter.wrapping_add(1);
        if self.tick_counter % 60 == 0 {
            self.theme.reload_if_changed();
        }

        // Poll background lyrics fetcher
        self.lyrics_mgr.poll_updates();

        // Clear toast after 3 seconds
        if let Some(msg_time) = self.status_message_time {
            if now.duration_since(msg_time) >= Duration::from_secs(3) {
                self.status_message = None;
                self.status_message_time = None;
            }
        }
    }

    pub fn toggle_cover_mode(&mut self) {
        self.cover_mode = match self.cover_mode {
            CoverMode::StaticCard => CoverMode::SpinningVinyl,
            CoverMode::SpinningVinyl => CoverMode::StaticCard,
        };
        let name = match self.cover_mode {
            CoverMode::StaticCard => "Static Album Card",
            CoverMode::SpinningVinyl => "Spinning Vinyl Record",
        };
        self.set_status_message(&format!("Cover View: {}", name));
    }

    pub fn cycle_bottom_panel(&mut self) {
        self.bottom_mode = match self.bottom_mode {
            BottomPanelMode::Telemetry => BottomPanelMode::Visualizer,
            BottomPanelMode::Visualizer => BottomPanelMode::MiniLyrics,
            BottomPanelMode::MiniLyrics => BottomPanelMode::Metadata,
            BottomPanelMode::Metadata => BottomPanelMode::Telemetry,
        };
        let name = match self.bottom_mode {
            BottomPanelMode::Telemetry => "Audio Monitor // Telemetry",
            BottomPanelMode::Visualizer => "Spectrum // Visualizer",
            BottomPanelMode::MiniLyrics => "Live Synchronized Lyrics",
            BottomPanelMode::Metadata => "Track Details // Metadata",
        };
        self.set_status_message(&format!("Bottom Panel: {}", name));
    }

    pub fn filtered_queue(&self) -> Vec<SongInfo> {
        if self.queue_search_query.is_empty() {
            return self.playlist.clone();
        }
        let q = self.queue_search_query.to_lowercase();
        self.playlist
            .iter()
            .filter(|s| {
                s.title.to_lowercase().contains(&q)
                    || s.artist.to_lowercase().contains(&q)
                    || s.album.to_lowercase().contains(&q)
                    || s.file.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }

    pub fn filtered_library(&self) -> Vec<String> {
        if self.library_search_query.is_empty() {
            return self.library_files.clone();
        }
        let q = self.library_search_query.to_lowercase();
        self.library_files
            .iter()
            .filter(|f| f.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        if self.show_help {
            if matches!(key.code, KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q')) {
                self.show_help = false;
            }
            return;
        }

        if self.is_searching {
            match key.code {
                KeyCode::Enter => {
                    self.is_searching = false;
                }
                KeyCode::Esc => {
                    self.is_searching = false;
                    match self.active_tab {
                        ActiveTab::Queue => self.queue_search_query.clear(),
                        ActiveTab::Library => self.library_search_query.clear(),
                        _ => {}
                    }
                }
                KeyCode::Backspace => match self.active_tab {
                    ActiveTab::Queue => {
                        self.queue_search_query.pop();
                    }
                    ActiveTab::Library => {
                        self.library_search_query.pop();
                    }
                    _ => {}
                },
                KeyCode::Char(c) => match self.active_tab {
                    ActiveTab::Queue => {
                        self.queue_search_query.push(c);
                    }
                    ActiveTab::Library => {
                        self.library_search_query.push(c);
                    }
                    _ => {}
                },
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('?') => {
                self.show_help = true;
            }
            KeyCode::Tab => {
                self.active_tab = match self.active_tab {
                    ActiveTab::Player => ActiveTab::Lyrics,
                    ActiveTab::Lyrics => ActiveTab::Queue,
                    ActiveTab::Queue => ActiveTab::Library,
                    ActiveTab::Library => ActiveTab::Player,
                };
            }
            KeyCode::Char('1') => self.active_tab = ActiveTab::Player,
            KeyCode::Char('2') => self.active_tab = ActiveTab::Lyrics,
            KeyCode::Char('3') => self.active_tab = ActiveTab::Queue,
            KeyCode::Char('4') => self.active_tab = ActiveTab::Library,

            // Playback controls
            KeyCode::Char(' ') => {
                self.clicked_button = Some(("play", Instant::now()));
                let _ = self.mpd.toggle_pause();
                self.refresh_mpd(false);
            }
            KeyCode::Char('n') | KeyCode::Char('>') => {
                self.clicked_button = Some(("next", Instant::now()));
                let _ = self.mpd.next();
                self.set_status_message("Next track");
                self.refresh_mpd(false);
            }
            KeyCode::Char('p') | KeyCode::Char('<') => {
                self.clicked_button = Some(("prev", Instant::now()));
                let _ = self.mpd.previous();
                self.set_status_message("Previous track");
                self.refresh_mpd(false);
            }
            KeyCode::Left | KeyCode::Char('h') if self.active_tab == ActiveTab::Player => {
                let _ = self.mpd.seek_relative(-5.0);
                self.set_status_message("Seek -5s");
                self.refresh_mpd(false);
            }
            KeyCode::Right | KeyCode::Char('l') if self.active_tab == ActiveTab::Player => {
                let _ = self.mpd.seek_relative(5.0);
                self.set_status_message("Seek +5s");
                self.refresh_mpd(false);
            }
            KeyCode::Up | KeyCode::Char('+') if self.active_tab == ActiveTab::Player => {
                let _ = self.mpd.change_volume(5);
                self.refresh_mpd(false);
            }
            KeyCode::Down | KeyCode::Char('-') if self.active_tab == ActiveTab::Player => {
                let _ = self.mpd.change_volume(-5);
                self.refresh_mpd(false);
            }
            KeyCode::Char('r') => {
                self.clicked_button = Some(("repeat", Instant::now()));
                let _ = self.mpd.toggle_repeat();
                self.refresh_mpd(false);
                let mode = if self.mpd_status.repeat { "On" } else { "Off" };
                self.set_status_message(&format!("Repeat: {}", mode));
            }
            KeyCode::Char('s') => {
                self.clicked_button = Some(("shuffle", Instant::now()));
                let _ = self.mpd.toggle_random();
                self.refresh_mpd(false);
                let mode = if self.mpd_status.random { "On" } else { "Off" };
                self.set_status_message(&format!("Shuffle: {}", mode));
            }
            KeyCode::Char('v') => {
                self.toggle_cover_mode();
            }
            KeyCode::Char('m') | KeyCode::Char('b') => {
                self.cycle_bottom_panel();
            }
            KeyCode::Char('x') | KeyCode::Char('X') => {
                self.matrix_rain.enabled = !self.matrix_rain.enabled;
                let state = if self.matrix_rain.enabled { "Enabled" } else { "Disabled" };
                self.set_status_message(&format!("Matrix Rain: {}", state));
            }
            KeyCode::Char('f') => {
                self.is_favorite = !self.is_favorite;
                let msg = if self.is_favorite { "Added to Liked Songs ♥" } else { "Removed from Liked Songs" };
                self.set_status_message(msg);
            }
            KeyCode::Char('L') => {
                if let Some(ref song) = self.current_song {
                    let music_path = self.mpd.music_dir().map(|d| d.join(&song.file));
                    self.lyrics_mgr.update_track(
                        &song.display_artist(),
                        &song.display_title(),
                        music_path.as_ref(),
                        song.duration,
                    );
                    self.set_status_message("Fetching lyrics from LRCLIB...");
                }
            }

            // Tab-specific controls
            KeyCode::Char('/') => {
                if matches!(self.active_tab, ActiveTab::Queue | ActiveTab::Library) {
                    self.is_searching = true;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => match self.active_tab {
                ActiveTab::Queue => {
                    let max_len = self.filtered_queue().len();
                    if max_len > 0 && self.queue_selected_idx + 1 < max_len {
                        self.queue_selected_idx += 1;
                    }
                }
                ActiveTab::Library => {
                    let max_len = self.filtered_library().len();
                    if max_len > 0 && self.library_selected_idx + 1 < max_len {
                        self.library_selected_idx += 1;
                    }
                }
                ActiveTab::Lyrics => {
                    let current = self.lyrics_scroll_offset.or_else(|| self.lyrics_mgr.get_active_line_index(self.mpd_status.elapsed)).unwrap_or(0);
                    self.lyrics_scroll_offset = Some(current + 1);
                }
                _ => {}
            },
            KeyCode::Up | KeyCode::Char('k') => match self.active_tab {
                ActiveTab::Queue => {
                    if self.queue_selected_idx > 0 {
                        self.queue_selected_idx -= 1;
                    }
                }
                ActiveTab::Library => {
                    if self.library_selected_idx > 0 {
                        self.library_selected_idx -= 1;
                    }
                }
                ActiveTab::Lyrics => {
                    let current = self.lyrics_scroll_offset.or_else(|| self.lyrics_mgr.get_active_line_index(self.mpd_status.elapsed)).unwrap_or(0);
                    if current > 0 {
                        self.lyrics_scroll_offset = Some(current - 1);
                    }
                }
                _ => {}
            },
            KeyCode::Char('c') if self.active_tab == ActiveTab::Lyrics => {
                self.lyrics_scroll_offset = None;
                self.set_status_message("Synced to playback");
            }
            KeyCode::Enter => match self.active_tab {
                ActiveTab::Queue => {
                    let filtered = self.filtered_queue();
                    if let Some(song) = filtered.get(self.queue_selected_idx) {
                        let _ = self.mpd.play_id(song.id);
                        self.refresh_mpd(false);
                    }
                }
                ActiveTab::Library => {
                    let filtered = self.filtered_library();
                    if let Some(file) = filtered.get(self.library_selected_idx) {
                        let _ = self.mpd.add_to_queue(file);
                        let _ = self.mpd.play();
                        self.refresh_mpd(true);
                        self.set_status_message(&format!("Added and playing: {}", file));
                    }
                }
                _ => {}
            },
            KeyCode::Char('a') if self.active_tab == ActiveTab::Library => {
                let filtered = self.filtered_library();
                if let Some(file) = filtered.get(self.library_selected_idx) {
                    let _ = self.mpd.add_to_queue(file);
                    self.refresh_mpd(true);
                    self.set_status_message(&format!("Added to queue: {}", file));
                }
            }
            KeyCode::Char('d') if self.active_tab == ActiveTab::Queue => {
                let filtered = self.filtered_queue();
                if let Some(song) = filtered.get(self.queue_selected_idx) {
                    let _ = self.mpd.delete_id(song.id);
                    self.refresh_mpd(true);
                    self.set_status_message("Track deleted from queue");
                }
            }
            KeyCode::Char('c') if self.active_tab == ActiveTab::Queue => {
                let _ = self.mpd.clear_queue();
                self.refresh_mpd(true);
                self.set_status_message("Queue cleared");
            }
            _ => {}
        }
    }

    pub fn get_cover_geometry(terminal_size: ratatui::layout::Rect) -> CoverGeometry {
        // ── Card container ────────────────────────────────────────────────────
        // Balanced width for a sleek cyberpunk music deck (matches reference screenshots)
        let card_width = 48.min(terminal_size.width.saturating_sub(2)).max(32);
        let card_height = terminal_size.height;
        let card_x = terminal_size.x + (terminal_size.width.saturating_sub(card_width)) / 2;
        let card_y = terminal_size.y;
        let card_rect = ratatui::layout::Rect {
            x: card_x,
            y: card_y,
            width: card_width,
            height: card_height,
        };

        // ── Cover box position inside the card ───────────────────────────────
        // Rows above cover box: header (1), visualizer (2) = 3 rows
        // Rows below cover box: title (1), artist (1), spacer (1), scrubber (1), spacer (1),
        //                       controls (3), spacer (1), bottom panel (6), toast (1) = 16 rows
        let above_cover = 3u16;
        let below_cover = 16u16;

        let avail_h = card_height.saturating_sub(above_cover + below_cover);

        // In terminal cells (~1:2 aspect ratio), a square image has cols = rows * 2.
        // We leave 2 rows and 2 cols for the border of the cover box.
        let max_img_rows = avail_h.saturating_sub(2).max(2);
        let max_img_cols = card_width.saturating_sub(4).max(4);

        let img_rows = max_img_rows.min(max_img_cols / 2).max(2);
        let img_cols = img_rows * 2;

        // The cover box wraps the image tightly with a 1-cell border on all sides:
        let cover_box_w = img_cols + 2;
        let cover_box_h = img_rows + 2;

        // Center the cover box within the available space
        let cover_box_x = card_x + (card_width.saturating_sub(cover_box_w)) / 2;
        let cover_box_y = card_y + above_cover + (avail_h.saturating_sub(cover_box_h)) / 2;

        let cover_box_rect = ratatui::layout::Rect {
            x: cover_box_x,
            y: cover_box_y,
            width: cover_box_w,
            height: cover_box_h,
        };

        // Inner rect (image fits 100% edge-to-edge inside the 1-cell border)
        let inner_x = cover_box_x + 1;
        let inner_y = cover_box_y + 1;
        let inner_w = img_cols;
        let inner_h = img_rows;
        let inner_rect = ratatui::layout::Rect {
            x: inner_x,
            y: inner_y,
            width: inner_w,
            height: inner_h,
        };

        let img_x = inner_x;
        let img_y = inner_y;

        CoverGeometry {
            card_rect,
            cover_box_rect,
            inner_rect,
            img_x,
            img_y,
            img_cols,
            img_rows,
        }
    }

    pub fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let (term_w, term_h) = crossterm::terminal::size().unwrap_or((80, 24));
            let geom = Self::get_cover_geometry(ratatui::layout::Rect {
                x: 0,
                y: 0,
                width: term_w,
                height: term_h,
            });

            // 1. Top Header clicks
            if mouse.row <= 1 {
                let col = mouse.column;
                let cx = term_w / 2;
                if col < geom.card_rect.x + 8 {
                    self.show_help = !self.show_help;
                } else if col >= cx.saturating_sub(18) && col <= cx.saturating_sub(10) {
                    self.active_tab = ActiveTab::Player;
                } else if col >= cx.saturating_sub(9) && col <= cx.saturating_sub(1) {
                    self.active_tab = ActiveTab::Lyrics;
                } else if col >= cx && col <= cx + 8 {
                    self.active_tab = ActiveTab::Queue;
                } else if col >= cx + 9 && col <= cx + 17 {
                    self.active_tab = ActiveTab::Library;
                } else if col >= cx + 18 && col <= cx + 26 {
                    self.is_favorite = !self.is_favorite;
                    let msg = if self.is_favorite { "Added to Liked Songs ♥" } else { "Removed from Liked Songs" };
                    self.set_status_message(msg);
                }
                return;
            }

            // 2. Player Controls clicks
            if self.active_tab == ActiveTab::Player && !self.show_help {
                let cx = term_w / 2;
                let col = mouse.column;
                let row = mouse.row;

                // Clicking cover box toggles cover mode (Static vs Vinyl)
                if row >= geom.cover_box_rect.y
                    && row < geom.cover_box_rect.y + geom.cover_box_rect.height
                    && col >= geom.cover_box_rect.x
                    && col < geom.cover_box_rect.x + geom.cover_box_rect.width
                {
                    self.toggle_cover_mode();
                    return;
                }

                // Vertical positions matching DiscView chunks layout
                let body_h = geom.card_rect.height.saturating_sub(2);
                let title_y = 1 + body_h.saturating_sub(15);
                let scrubber_y = title_y + 3;
                let controls_y = title_y + 5;
                let bottom_y = title_y + 9; // after 3 rows of controls + 1 row of spacer

                // Scrubber seek click
                if row == scrubber_y {
                    let bar_w = (geom.card_rect.width.saturating_sub(14) as usize).max(4) as f64;
                    let bar_start = (cx as f64) - (bar_w / 2.0);
                    let bar_end = (cx as f64) + (bar_w / 2.0);
                    if (col as f64) >= bar_start && (col as f64) <= bar_end && self.mpd_status.duration > 0.0 {
                        let ratio = ((col as f64 - bar_start) / bar_w).clamp(0.0, 1.0);
                        let target = ratio * self.mpd_status.duration;
                        let _ = self.mpd.seek_absolute(target);
                        self.refresh_mpd(false);
                        return;
                    }
                }

                // Controls row click (3 rows)
                if row >= controls_y && row < controls_y + 3 {
                    // Center button: Play/Pause (cx - 4 ..= cx + 4)
                    if col >= cx.saturating_sub(4) && col <= cx + 4 {
                        self.clicked_button = Some(("play", Instant::now()));
                        let _ = self.mpd.toggle_pause();
                        self.refresh_mpd(false);
                    } else if col >= cx.saturating_sub(13) && col <= cx.saturating_sub(7) {
                        self.clicked_button = Some(("prev", Instant::now()));
                        let _ = self.mpd.previous();
                        self.set_status_message("Previous track");
                        self.refresh_mpd(false);
                    } else if col >= cx + 7 && col <= cx + 13 {
                        self.clicked_button = Some(("next", Instant::now()));
                        let _ = self.mpd.next();
                        self.set_status_message("Next track");
                        self.refresh_mpd(false);
                    } else if col >= cx.saturating_sub(20) && col <= cx.saturating_sub(15) {
                        self.clicked_button = Some(("shuffle", Instant::now()));
                        let _ = self.mpd.toggle_random();
                        self.refresh_mpd(false);
                    } else if col >= cx + 15 && col <= cx + 20 {
                        self.clicked_button = Some(("repeat", Instant::now()));
                        let _ = self.mpd.toggle_repeat();
                        self.refresh_mpd(false);
                    }
                    return;
                }

                // Bottom panel header click: cycle mode
                if row >= bottom_y && row < bottom_y + 6 {
                    self.cycle_bottom_panel();
                    return;
                }
            }
        }
    }
}
