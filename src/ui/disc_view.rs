use crate::cover_art::CoverArtManager;
use crate::mpd_client::{MpdStatus, PlaybackState, SongInfo};
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

pub struct DiscView<'a> {
    pub song: Option<&'a SongInfo>,
    pub status: &'a MpdStatus,
    pub cover_art: &'a CoverArtManager,
    pub rotation_angle: f32,
    pub theme: &'a Theme,
    pub is_favorite: bool,
}

impl<'a> Widget for DiscView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 || area.height < 10 {
            let msg = Paragraph::new("Terminal too small for Disc View")
                .style(self.theme.muted_text())
                .alignment(Alignment::Center);
            msg.render(area, buf);
            return;
        }

        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),    // Vinyl Record
                Constraint::Length(2), // Title & Artist
                Constraint::Length(1), // Spacer / Actions row
                Constraint::Length(1), // Timeline Scrubber
                Constraint::Length(2), // Controls row
            ])
            .split(area);

        let disc_area = main_chunks[0];
        let title_area = main_chunks[1];
        let actions_area = main_chunks[2];
        let scrubber_area = main_chunks[3];
        let controls_area = main_chunks[4];

        // --- 1. Render Vinyl Record Disc ---
        // A character cell in standard monospace fonts is ~1:2 (width:height).
        // Dividing 1 cell into 2 vertical half-block pixels ('▀') yields 1:1 square pixels!
        let max_cols_by_width = disc_area.width.saturating_sub(2);
        let max_cols_by_height = (disc_area.height.saturating_sub(1)) * 2;
        let disc_cols = max_cols_by_width.min(max_cols_by_height).max(6);
        let disc_rows = (disc_cols / 2).max(3);

        let disc_x = disc_area.x + (disc_area.width.saturating_sub(disc_cols)) / 2;
        let disc_y = disc_area.y + (disc_area.height.saturating_sub(disc_rows)) / 2;

        let pixel_width = disc_cols as usize;
        let pixel_height = (disc_rows * 2) as usize;

        let (title_str, artist_str) = if let Some(s) = self.song {
            (s.display_title(), s.display_artist())
        } else {
            ("Amper".to_string(), "MPD Music Player".to_string())
        };

        let vinyl_frame = self.cover_art.render_vinyl_frame(
            pixel_width,
            pixel_height,
            self.rotation_angle,
            self.theme.card_bg,
            &title_str,
            &artist_str,
        );

        for r in 0..disc_rows {
            let py_top = (r * 2) as usize;
            let py_bot = (r * 2 + 1) as usize;
            let cur_y = disc_y + r;

            if cur_y >= disc_area.y + disc_area.height {
                break;
            }

            for c in 0..disc_cols {
                let cur_x = disc_x + c;
                if cur_x >= disc_area.x + disc_area.width {
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

        // --- 2. Track Title & Artist ---
        let title_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(title_area);

        let title_line = Line::from(vec![Span::styled(
            &title_str,
            Style::default()
                .fg(self.theme.text_primary)
                .add_modifier(Modifier::BOLD),
        )]);
        Paragraph::new(title_line)
            .alignment(Alignment::Center)
            .render(title_chunks[0], buf);

        let artist_line = Line::from(vec![Span::styled(
            &artist_str,
            Style::default().fg(self.theme.text_secondary),
        )]);
        Paragraph::new(artist_line)
            .alignment(Alignment::Center)
            .render(title_chunks[1], buf);

        // --- 3. Actions Row (Heart / Like, Repeat, Shuffle, Volume) ---
        let heart_span = if self.is_favorite {
            Span::styled(" ♥ Liked ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" ♡ Like ", Style::default().fg(self.theme.text_muted))
        };

        let repeat_span = if self.status.repeat {
            Span::styled(" 🔁 Repeat ", Style::default().fg(self.theme.accent))
        } else {
            Span::styled(" ⇄ Loop ", Style::default().fg(self.theme.text_muted))
        };

        let shuffle_span = if self.status.random {
            Span::styled(" 🔀 Random ", Style::default().fg(self.theme.accent))
        } else {
            Span::styled(" ⤮ Order ", Style::default().fg(self.theme.text_muted))
        };

        let vol_str = if self.status.volume >= 0 {
            format!(" 🎚️ {}% ", self.status.volume)
        } else {
            " 🎚️ -- ".to_string()
        };
        let vol_span = Span::styled(vol_str, Style::default().fg(self.theme.text_secondary));

        let actions_line = Line::from(vec![
            heart_span,
            Span::raw("    "),
            repeat_span,
            Span::raw("    "),
            shuffle_span,
            Span::raw("    "),
            vol_span,
        ]);
        Paragraph::new(actions_line)
            .alignment(Alignment::Center)
            .render(actions_area, buf);

        // --- 4. Scrubber / Timeline Bar ---
        let elapsed = self.status.elapsed;
        let duration = self.status.duration.max(0.1);
        let progress = (elapsed / duration).clamp(0.0, 1.0);

        let elapsed_str = format_time(elapsed);
        let duration_str = format_time(duration);

        let elapsed_len = elapsed_str.len() as u16;
        let duration_len = duration_str.len() as u16;

        let bar_margin = 4u16;
        let scrubber_width = scrubber_area.width.saturating_sub(bar_margin * 2);

        if scrubber_width > elapsed_len + duration_len + 6 {
            let track_width = scrubber_width.saturating_sub(elapsed_len + duration_len + 4) as usize;
            let knob_pos = ((progress * track_width as f64).round() as usize).min(track_width.saturating_sub(1));

            let filled_part: String = "━".repeat(knob_pos);
            let knob_str = "●";
            let empty_part: String = "─".repeat(track_width.saturating_sub(knob_pos + 1));

            let scrubber_line = Line::from(vec![
                Span::styled(format!("{}  ", elapsed_str), Style::default().fg(self.theme.text_muted)),
                Span::styled(filled_part, Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled(knob_str, Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled(empty_part, Style::default().fg(self.theme.pill_bg)),
                Span::styled(format!("  {}", duration_str), Style::default().fg(self.theme.text_muted)),
            ]);

            Paragraph::new(scrubber_line)
                .alignment(Alignment::Center)
                .render(scrubber_area, buf);
        }

        // --- 5. Playback Controls Row ---
        let is_playing = self.status.state == PlaybackState::Play;
        let play_btn_text = if is_playing { "  ⏸  " } else { "  ▶  " };

        let repeat_icon = if self.status.repeat { " 🔁 " } else { " ⇄ " };
        let prev_icon = " ⏮ ";
        let next_icon = " ⏭ ";
        let queue_icon = " ☰ ";

        let btn_style = Style::default().fg(self.theme.text_primary).bg(self.theme.control_bg);
        let play_btn_style = Style::default()
            .fg(self.theme.text_primary)
            .bg(self.theme.accent)
            .add_modifier(Modifier::BOLD);

        let controls_line = Line::from(vec![
            Span::styled(repeat_icon, btn_style),
            Span::raw("    "),
            Span::styled(prev_icon, btn_style),
            Span::raw("   "),
            Span::styled(play_btn_text, play_btn_style),
            Span::raw("   "),
            Span::styled(next_icon, btn_style),
            Span::raw("    "),
            Span::styled(queue_icon, btn_style),
        ]);

        Paragraph::new(controls_line)
            .alignment(Alignment::Center)
            .render(controls_area, buf);
    }
}

fn format_time(secs: f64) -> String {
    let total_secs = secs as u64;
    let mins = total_secs / 60;
    let s = total_secs % 60;
    format!("{:02}:{:02}", mins, s)
}
