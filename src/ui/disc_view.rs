use crate::cover_art::CoverArtManager;
use crate::mpd_client::{MpdStatus, PlaybackState, SongInfo};
use crate::ui::telemetry::TelemetryWidget;
use crate::ui::theme::Theme;
use crate::ui::visualizer::VisualizerWidget;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

pub struct DiscView<'a> {
    pub song: Option<&'a SongInfo>,
    pub status: &'a MpdStatus,
    pub cover_art: &'a CoverArtManager,
    pub rotation_angle: f32,
    pub theme: &'a Theme,
    pub is_favorite: bool,
    pub tick_counter: u64,
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

        // Vertical layout matching the inspiration screenshot:
        // 1. Audio Visualizer banner (waveform)
        // 2. Center Vinyl Disc container
        // 3. Track Title & Artist
        // 4. Progress bar [||||||||········]
        // 5. Boxed playback controls [ ⇄ ] [ |<< ] [ || ] [ >>| ] [ ↻ ]
        // 6. Audio Monitor // Telemetry bottom card
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // 1. Visualizer
                Constraint::Min(8),    // 2. Vinyl disc container
                Constraint::Length(2), // 3. Title & Artist
                Constraint::Length(1), // 4. Timeline scrubber
                Constraint::Length(3), // 5. Boxed controls
                Constraint::Length(6), // 6. Telemetry card
            ])
            .split(area);

        let viz_area = chunks[0];
        let disc_box_area = chunks[1];
        let title_area = chunks[2];
        let scrubber_area = chunks[3];
        let controls_area = chunks[4];
        let telemetry_area = chunks[5];

        // --- 1. Symmetrical Waveform Visualizer ---
        let viz = VisualizerWidget {
            is_playing,
            tick_counter: self.tick_counter,
            theme: self.theme,
            sample_rate: "MPD 44.1 kHz",
        };
        viz.render(viz_area, buf);

        // --- 2. Vinyl Record Container Card ---
        let disc_block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border_style())
            .style(Style::default().bg(ratatui::style::Color::Reset));
        let disc_inner = disc_block.inner(disc_box_area);
        disc_block.render(disc_box_area, buf);

        // Calculate disc dimensions inside container
        let max_cols = disc_inner.width.saturating_sub(2);
        let max_rows = disc_inner.height;
        let disc_cols = max_cols.min(max_rows * 2).max(6);
        let disc_rows = (disc_cols / 2).max(3);

        let disc_x = disc_inner.x + (disc_inner.width.saturating_sub(disc_cols)) / 2;
        let disc_y = disc_inner.y + (disc_inner.height.saturating_sub(disc_rows)) / 2;

        let (title_str, artist_str) = if let Some(s) = self.song {
            (s.display_title(), s.display_artist())
        } else {
            ("In The Soul".to_string(), "MPD".to_string())
        };

        // If not using Kitty graphics or as fallback background:
        let pixel_width = disc_cols as usize;
        let pixel_height = (disc_rows * 2) as usize;
        let vinyl_frame = self.cover_art.render_vinyl_frame(
            pixel_width,
            pixel_height,
            self.rotation_angle,
            self.theme.accent,
        );

        for r in 0..disc_rows {
            let py_top = (r * 2) as usize;
            let py_bot = (r * 2 + 1) as usize;
            let cur_y = disc_y + r;

            if cur_y >= disc_inner.y + disc_inner.height {
                break;
            }

            for c in 0..disc_cols {
                let cur_x = disc_x + c;
                if cur_x >= disc_inner.x + disc_inner.width {
                    break;
                }

                let top_col = vinyl_frame[py_top][c as usize];
                let bot_col = vinyl_frame[py_bot][c as usize];

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

        // --- 4. Scrubber / Progress Bar matching inspiration: 0:05 [||||||······] 3:00 ---
        let elapsed = self.status.elapsed;
        let duration = self.status.duration.max(0.1);
        let progress = (elapsed / duration).clamp(0.0, 1.0);

        let elapsed_str = format_time(elapsed);
        let duration_str = format_time(duration);

        let time_width = (elapsed_str.len() + duration_str.len() + 6) as u16;
        let bar_width = (scrubber_area.width.saturating_sub(time_width) as usize).max(4);

        let filled_count = ((progress * bar_width as f64).round() as usize).min(bar_width);
        let unfilled_count = bar_width.saturating_sub(filled_count);

        let filled_bar = "|".repeat(filled_count);
        let unfilled_bar = "·".repeat(unfilled_count);

        let scrubber_line = Line::from(vec![
            Span::styled(format!("{} [", elapsed_str), self.theme.dim()),
            Span::styled(filled_bar, self.theme.accent()),
            Span::styled(unfilled_bar, self.theme.dim()),
            Span::styled(format!("] {}", duration_str), self.theme.dim()),
        ]);

        Paragraph::new(scrubber_line).alignment(Alignment::Center).render(scrubber_area, buf);

        // --- 5. Boxed Cyberpunk Controls matching inspiration image ---
        //  ┌───┐   ┌─────┐   ┌──────┐   ┌─────┐   ┌───┐
        //  │ ⇄ │   │ |<< │   │  ||  │   │ >>| │   │ ↻ │
        //  └───┘   └─────┘   └──────┘   └─────┘   └───┘
        let repeat_icon = if self.status.repeat { " 🔁 " } else { " ⇄ " };
        let play_icon = if is_playing { "  ||  " } else { "  ▶   " };
        let prev_icon = " |<< ";
        let next_icon = " >>| ";
        let queue_icon = " ↻ ";

        let row1 = Line::from(vec![
            Span::styled("┌───┐   ┌─────┐   ┌──────┐   ┌─────┐   ┌───┐", self.theme.border_style()),
        ]);
        let row2 = Line::from(vec![
            Span::styled("│", self.theme.border_style()),
            Span::styled(repeat_icon, if self.status.repeat { self.theme.accent() } else { self.theme.secondary() }),
            Span::styled("│   │", self.theme.border_style()),
            Span::styled(prev_icon, self.theme.primary_bold()),
            Span::styled("│   │", self.theme.border_style()),
            Span::styled(play_icon, self.theme.accent()),
            Span::styled("│   │", self.theme.border_style()),
            Span::styled(next_icon, self.theme.primary_bold()),
            Span::styled("│   │", self.theme.border_style()),
            Span::styled(queue_icon, self.theme.secondary()),
            Span::styled("│", self.theme.border_style()),
        ]);
        let row3 = Line::from(vec![
            Span::styled("└───┘   └─────┘   └──────┘   └─────┘   └───┘", self.theme.border_style()),
        ]);

        let controls_lines = vec![row1, row2, row3];
        Paragraph::new(controls_lines).alignment(Alignment::Center).render(controls_area, buf);

        // --- 6. Telemetry Status Card ---
        let telemetry = TelemetryWidget {
            song: self.song,
            status: self.status,
            theme: self.theme,
            tick_counter: self.tick_counter,
        };
        telemetry.render(telemetry_area, buf);
    }
}

fn format_time(secs: f64) -> String {
    let total_secs = secs as u64;
    let mins = total_secs / 60;
    let s = total_secs % 60;
    format!("{:02}:{:02}", mins, s)
}
