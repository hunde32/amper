use crate::app::{BottomPanelMode, CoverMode};
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
    pub is_favorite: bool,
    pub tick_counter: u64,
    pub cover_mode: CoverMode,
    pub bottom_mode: BottomPanelMode,
    pub lyrics_mgr: &'a LyricsManager,
    pub clicked_button: Option<(&'static str, Instant)>,
}

impl<'a> Widget for DiscView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 25 || area.height < 15 {
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
                Constraint::Min(8),    // 2. Cover / Disc container
                Constraint::Length(2), // 3. Title & Artist
                Constraint::Length(1), // 4. Scrubber
                Constraint::Length(4), // 5. Boxed Cyberpunk controls with filled active button
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
        let cover_inner = cover_block.inner(cover_box_area);
        cover_block.render(cover_box_area, buf);

        // Size of the inner graphic
        let max_cols = cover_inner.width.saturating_sub(2);
        let max_rows = cover_inner.height;
        let disc_cols = max_cols.min(max_rows * 2).max(6);
        let disc_rows = (disc_cols / 2).max(3);

        let disc_x = cover_inner.x + (cover_inner.width.saturating_sub(disc_cols)) / 2;
        let disc_y = cover_inner.y + (cover_inner.height.saturating_sub(disc_rows)) / 2;

        let (title_str, artist_str) = if let Some(s) = self.song {
            (s.display_title(), s.display_artist())
        } else {
            ("Corolla Club".to_string(), "NOXYGEN".to_string())
        };

        let pixel_width = disc_cols as usize;
        let pixel_height = (disc_rows * 2) as usize;

        // Render half-block fallback
        let frame_colors = match self.cover_mode {
            CoverMode::StaticCard => {
                self.cover_art.render_static_frame(pixel_width, pixel_height, self.theme.accent)
            }
            CoverMode::SpinningVinyl => {
                self.cover_art.render_vinyl_frame(pixel_width, pixel_height, self.rotation_angle, self.theme.accent)
            }
        };

        for r in 0..disc_rows {
            let py_top = (r * 2) as usize;
            let py_bot = (r * 2 + 1) as usize;
            let cur_y = disc_y + r;

            if cur_y >= cover_inner.y + cover_inner.height {
                break;
            }

            for c in 0..disc_cols {
                let cur_x = disc_x + c;
                if cur_x >= cover_inner.x + cover_inner.width {
                    break;
                }

                let top_col = frame_colors[py_top][c as usize];
                let bot_col = frame_colors[py_bot][c as usize];

                let cell = &mut buf[(cur_x, cur_y)];
                cell.set_char('▀');
                cell.set_fg(top_col);
                cell.set_bg(bot_col);
            }
        }

        // --- 3. Track Title & Artist ---
        let title_line = Line::from(vec![Span::styled(
            &title_str,
            self.theme.primary_bold(),
        )]);
        let artist_line = Line::from(vec![Span::styled(
            &artist_str,
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

        // --- 5. Boxed Controls with Filled Active Background ---
        let now = Instant::now();
        let clicked = self.clicked_button.and_then(|(name, time)| {
            if now.duration_since(time).as_millis() < 400 {
                Some(name)
            } else {
                None
            }
        });

        let repeat_active = self.status.repeat || clicked == Some("repeat");
        let prev_active = clicked == Some("prev");
        let play_active = is_playing || clicked == Some("play");
        let next_active = clicked == Some("next");
        let shuffle_active = self.status.random || clicked == Some("shuffle");

        // Filled active style (matching screenshot: prominent warm filled button block!)
        let active_style = Style::default()
            .bg(self.theme.accent)
            .fg(Color::Rgb(20, 20, 26))
            .add_modifier(Modifier::BOLD);

        let inactive_style = Style::default().fg(self.theme.text_primary).bg(Color::Reset);
        let border_style = self.theme.border_style();

        // Row 1: Top borders
        let r1_rep = if repeat_active { Span::styled("┌───┐", active_style) } else { Span::styled("┌───┐", border_style) };
        let r1_prev = if prev_active { Span::styled("┌─────┐", active_style) } else { Span::styled("┌─────┐", border_style) };
        let r1_play = if play_active { Span::styled("┌─────────┐", active_style) } else { Span::styled("┌─────────┐", border_style) };
        let r1_next = if next_active { Span::styled("┌─────┐", active_style) } else { Span::styled("┌─────┐", border_style) };
        let r1_shuf = if shuffle_active { Span::styled("┌───┐", active_style) } else { Span::styled("┌───┐", border_style) };

        let row1 = Line::from(vec![
            r1_rep,
            Span::raw("   "),
            r1_prev,
            Span::raw("   "),
            r1_play,
            Span::raw("   "),
            r1_next,
            Span::raw("   "),
            r1_shuf,
        ]);

        // Row 2: Content
        let play_icon = if is_playing { "   ||    " } else { "    ▶    " };

        let r2_rep = if repeat_active { Span::styled("│ ⇄ │", active_style) } else { Span::styled("│ ⇄ │", inactive_style) };
        let r2_prev = if prev_active { Span::styled("│ |<< │", active_style) } else { Span::styled("│ |<< │", inactive_style) };
        let r2_play = if play_active {
            Span::styled(format!("│{}│", play_icon), active_style)
        } else {
            Span::styled(format!("│{}│", play_icon), inactive_style)
        };
        let r2_next = if next_active { Span::styled("│ >>| │", active_style) } else { Span::styled("│ >>| │", inactive_style) };
        let r2_shuf = if shuffle_active { Span::styled("│ ↻ │", active_style) } else { Span::styled("│ ↻ │", inactive_style) };

        let row2 = Line::from(vec![
            r2_rep,
            Span::raw("   "),
            r2_prev,
            Span::raw("   "),
            r2_play,
            Span::raw("   "),
            r2_next,
            Span::raw("   "),
            r2_shuf,
        ]);

        // Row 3: Bottom borders
        let r3_rep = if repeat_active { Span::styled("└───┘", active_style) } else { Span::styled("└───┘", border_style) };
        let r3_prev = if prev_active { Span::styled("└─────┘", active_style) } else { Span::styled("└─────┘", border_style) };
        let r3_play = if play_active { Span::styled("└─────────┘", active_style) } else { Span::styled("└─────────┘", border_style) };
        let r3_next = if next_active { Span::styled("└─────┘", active_style) } else { Span::styled("└─────┘", border_style) };
        let r3_shuf = if shuffle_active { Span::styled("└───┘", active_style) } else { Span::styled("└───┘", border_style) };

        let row3 = Line::from(vec![
            r3_rep,
            Span::raw("   "),
            r3_prev,
            Span::raw("   "),
            r3_play,
            Span::raw("   "),
            r3_next,
            Span::raw("   "),
            r3_shuf,
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
