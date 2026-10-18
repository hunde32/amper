use crate::app::{BottomPanelMode, CoverGeometry, CoverMode};
use crate::cover_art::CoverArtManager;
use crate::lyrics::LyricsManager;
use crate::mpd_client::{MpdStatus, PlaybackState, SongInfo};
use crate::ui::telemetry::TelemetryWidget;
use crate::ui::theme::Theme;
use crate::ui::visualizer::VisualizerWidget;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use std::time::Instant;

pub struct DiscView<'a> {
    pub song: Option<&'a SongInfo>,
    pub status: &'a MpdStatus,
    pub cover_art: &'a CoverArtManager,
    pub rotation_angle: f32,
    pub theme: &'a Theme,
    pub tick_counter: u64,
    pub cover_mode: CoverMode,
    pub bottom_mode: BottomPanelMode,
    pub lyrics_mgr: &'a LyricsManager,
    pub clicked_button: Option<(&'static str, Instant)>,
    pub geom: CoverGeometry,
}

impl<'a> Widget for DiscView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 25 || area.height < 14 {
            let msg = Paragraph::new("Terminal too small for Player")
                .style(self.theme.dim())
                .alignment(Alignment::Center);
            msg.render(area, buf);
            return;
        }

        let is_playing = self.status.state == PlaybackState::Play;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // 1. Visualizer banner
                Constraint::Min(6),    // 2. Cover / Disc container
                Constraint::Length(2), // 3. Title & Artist
                Constraint::Length(1), // 4. Scrubber
                Constraint::Length(3), // 5. Boxed Cyberpunk controls
                Constraint::Length(6), // 6. Customizable bottom panel
            ])
            .split(area);

        let viz_area = chunks[0];
        let cover_box_area = chunks[1];
        let title_area = chunks[2];
        let scrubber_area = chunks[3];
        let controls_area = chunks[4];
        let bottom_area = chunks[5];

        // --- 1. Symmetrical Waveform Visualizer ---
        let viz = VisualizerWidget {
            is_playing,
            tick_counter: self.tick_counter,
            theme: self.theme,
            sample_rate: "MPD 44.1 kHz",
        };
        viz.render(viz_area, buf);

        // --- 2. Cover / Disc Container ---
        let mode_tag = match self.cover_mode {
            CoverMode::StaticCard => " [STATIC COVER (v)] ",
            CoverMode::SpinningVinyl => " [SPINNING VINYL (v)] ",
        };

        let cover_block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border_style())
            .title_bottom(Span::styled(mode_tag, self.theme.dim()))
            .style(Style::default().bg(Color::Reset));
        cover_block.render(cover_box_area, buf);

        // Inner artwork bounds
        let (title_str, artist_str) = if let Some(s) = self.song {
            (s.display_title(), s.display_artist())
        } else {
            ("Corolla Club".to_string(), "NOXYGEN".to_string())
        };

        if self.cover_art.supports_kitty {
            // Keep cells clear so Kitty GPU graphics display crisply without text interference
            for r in 0..self.geom.img_rows {
                let cy = self.geom.img_y + r;
                for c in 0..self.geom.img_cols {
                    let cx = self.geom.img_x + c;
                    if cx < buf.area.width && cy < buf.area.height {
                        let cell = &mut buf[(cx, cy)];
                        cell.set_char(' ');
                        cell.set_bg(Color::Reset);
                    }
                }
            }
        } else {
            // High quality half-block fallback for terminals without Kitty protocol
            let pixel_width = self.geom.img_cols as usize;
            let pixel_height = (self.geom.img_rows * 2) as usize;

            let frame_colors = match self.cover_mode {
                CoverMode::StaticCard => {
                    self.cover_art.render_static_frame(pixel_width, pixel_height, self.theme.accent)
                }
                CoverMode::SpinningVinyl => {
                    self.cover_art.render_vinyl_frame(pixel_width, pixel_height, self.rotation_angle, self.theme.accent)
                }
            };

            for r in 0..self.geom.img_rows {
                let py_top = (r * 2) as usize;
                let py_bot = (r * 2 + 1) as usize;
                let cur_y = self.geom.img_y + r;

                if cur_y >= self.geom.inner_rect.y + self.geom.inner_rect.height {
                    break;
                }

                for c in 0..self.geom.img_cols {
                    let cur_x = self.geom.img_x + c;
                    if cur_x >= self.geom.inner_rect.x + self.geom.inner_rect.width {
                        break;
                    }

                    if py_top < frame_colors.len() && py_bot < frame_colors.len() && (c as usize) < frame_colors[0].len() {
                        let top_col = frame_colors[py_top][c as usize];
                        let bot_col = frame_colors[py_bot][c as usize];

                        let cell = &mut buf[(cur_x, cur_y)];
                        cell.set_char('▀');
                        cell.set_fg(top_col);
                        cell.set_bg(bot_col);
                    }
                }
            }
        }

        // Resolution / format badge in bottom-right corner of inner frame
        let badge_text = match self.cover_mode {
            CoverMode::StaticCard => "128x128_PXL",
            CoverMode::SpinningVinyl => "33_RPM",
        };
        let badge_x = self.geom.inner_rect.x + self.geom.inner_rect.width.saturating_sub(badge_text.len() as u16 + 1);
        let badge_y = self.geom.inner_rect.y + self.geom.inner_rect.height.saturating_sub(1);
        if badge_x < buf.area.width && badge_y < buf.area.height {
            buf.set_string(badge_x, badge_y, badge_text, self.theme.dim());
        }

        // --- 3. Track Title & Artist ---
        let max_w = cover_box_area.width.saturating_sub(4) as usize;
        let mut display_title = title_str.to_uppercase();
        if display_title.len() > max_w && max_w > 3 {
            display_title.truncate(max_w - 3);
            display_title.push_str("...");
        }
        let mut display_artist = artist_str.to_uppercase();
        if display_artist.len() > max_w && max_w > 3 {
            display_artist.truncate(max_w - 3);
            display_artist.push_str("...");
        }

        let title_line = Line::from(vec![Span::styled(
            display_title,
            self.theme.primary_bold(),
        )]);
        let artist_line = Line::from(vec![Span::styled(
            display_artist,
            self.theme.secondary(),
        )]);

        let title_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(title_area);

        Paragraph::new(title_line).alignment(Alignment::Center).render(title_chunks[0], buf);
        Paragraph::new(artist_line).alignment(Alignment::Center).render(title_chunks[1], buf);

        // --- 4. Scrubber Progress Bar ---
        let elapsed = self.status.elapsed;
        let duration = self.status.duration.max(0.1);
        let progress = (elapsed / duration).clamp(0.0, 1.0);

        let elapsed_str = format_time(elapsed);
        let duration_str = format_time(duration);

        let time_width = (elapsed_str.len() + duration_str.len() + 6) as u16;
        let bar_width = (scrubber_area.width.saturating_sub(time_width) as usize).max(4);

        let filled_count = ((progress * bar_width as f64).round() as usize).min(bar_width);
        let unfilled_count = bar_width.saturating_sub(filled_count);

        let filled_bar = "█".repeat(filled_count);
        let unfilled_bar = "░".repeat(unfilled_count);

        let scrubber_line = Line::from(vec![
            Span::styled(format!("{} ", elapsed_str), self.theme.dim()),
            Span::styled(filled_bar, self.theme.accent()),
            Span::styled(unfilled_bar, self.theme.dim()),
            Span::styled(format!(" {}", duration_str), self.theme.dim()),
        ]);

        Paragraph::new(scrubber_line).alignment(Alignment::Center).render(scrubber_area, buf);

        // --- 5. Boxed Controls with Filled Active Background (Matching Reference Screenshot) ---
        let now = Instant::now();
        let clicked = self.clicked_button.and_then(|(name, time)| {
            if now.duration_since(time).as_millis() < 350 {
                Some(name)
            } else {
                None
            }
        });

        let shuffle_active = self.status.random || clicked == Some("shuffle");
        let prev_active = clicked == Some("prev");
        let play_clicked = clicked == Some("play");
        let next_active = clicked == Some("next");
        let repeat_active = self.status.repeat || clicked == Some("repeat");

        let border_dim = self.theme.border_style();
        let text_norm = Style::default().fg(self.theme.text_primary).bg(Color::Reset);

        // 1. Center Play/Pause button: A LARGE, TALL 3-ROW SOLID FILLED ACCENT BLOCK (matching screenshot)
        let play_bg_col = if play_clicked {
            Color::Rgb(255, 255, 255)
        } else {
            self.theme.accent
        };
        let play_fg_col = if play_clicked {
            self.theme.accent
        } else {
            Color::Rgb(255, 255, 255)
        };
        let play_block_style = Style::default()
            .bg(play_bg_col)
            .fg(play_fg_col)
            .add_modifier(Modifier::BOLD);

        let play_icon = if is_playing { "   ||    " } else { "    ▶    " };

        let p_top = Span::styled("         ", play_block_style);
        let p_mid = Span::styled(play_icon, play_block_style);
        let p_bot = Span::styled("         ", play_block_style);

        // 2. Shuffle [ ⇄ ] (Left boxed button, 5 chars wide)
        let active_pill = Style::default()
            .bg(self.theme.accent)
            .fg(Color::Rgb(20, 20, 26))
            .add_modifier(Modifier::BOLD);

        let (shuf_t, shuf_m, shuf_b) = if shuffle_active {
            (
                Span::styled("┌───┐", active_pill),
                Span::styled("│ ⇄ │", active_pill),
                Span::styled("└───┘", active_pill),
            )
        } else {
            (
                Span::styled("┌───┐", border_dim),
                Span::styled("│ ⇄ │", text_norm),
                Span::styled("└───┘", border_dim),
            )
        };

        // 3. Repeat [ ↻ ] (Right boxed button, 5 chars wide)
        let (rep_t, rep_m, rep_b) = if repeat_active {
            (
                Span::styled("┌───┐", active_pill),
                Span::styled("│ ↻ │", active_pill),
                Span::styled("└───┘", active_pill),
            )
        } else {
            (
                Span::styled("┌───┐", border_dim),
                Span::styled("│ ↻ │", text_norm),
                Span::styled("└───┘", border_dim),
            )
        };

        // 4. Previous |<< (Mid-left sleek text, 5 chars wide)
        let (prev_t, prev_m, prev_b) = if prev_active {
            (
                Span::styled("┌───┐", active_pill),
                Span::styled("|<< ", active_pill),
                Span::styled("└───┘", active_pill),
            )
        } else {
            (
                Span::raw("     "),
                Span::styled(" |<< ", text_norm.add_modifier(Modifier::BOLD)),
                Span::raw("     "),
            )
        };

        // 5. Next >>| (Mid-right sleek text, 5 chars wide)
        let (next_t, next_m, next_b) = if next_active {
            (
                Span::styled("┌───┐", active_pill),
                Span::styled(" >>|", active_pill),
                Span::styled("└───┘", active_pill),
            )
        } else {
            (
                Span::raw("     "),
                Span::styled(" >>| ", text_norm.add_modifier(Modifier::BOLD)),
                Span::raw("     "),
            )
        };

        let row1 = Line::from(vec![
            shuf_t,
            Span::raw("   "),
            prev_t,
            Span::raw("   "),
            p_top,
            Span::raw("   "),
            next_t,
            Span::raw("   "),
            rep_t,
        ]);

        let row2 = Line::from(vec![
            shuf_m,
            Span::raw("   "),
            prev_m,
            Span::raw("   "),
            p_mid,
            Span::raw("   "),
            next_m,
            Span::raw("   "),
            rep_m,
        ]);

        let row3 = Line::from(vec![
            shuf_b,
            Span::raw("   "),
            prev_b,
            Span::raw("   "),
            p_bot,
            Span::raw("   "),
            next_b,
            Span::raw("   "),
            rep_b,
        ]);

        let controls_lines = vec![row1, row2, row3];
        Paragraph::new(controls_lines).alignment(Alignment::Center).render(controls_area, buf);

        // --- 6. Customizable Bottom Panel ---
        let bottom_widget = TelemetryWidget {
            song: self.song,
            status: self.status,
            theme: self.theme,
            tick_counter: self.tick_counter,
            mode: self.bottom_mode,
            lyrics_mgr: self.lyrics_mgr,
        };
        bottom_widget.render(bottom_area, buf);
    }
}

fn format_time(secs: f64) -> String {
    let total_secs = secs as u64;
    let mins = total_secs / 60;
    let s = total_secs % 60;
    format!("{:02}:{:02}", mins, s)
}
