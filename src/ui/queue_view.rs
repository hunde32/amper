use crate::mpd_client::{MpdStatus, PlaybackState, SongInfo};
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

pub struct QueueView<'a> {
    pub playlist: &'a [SongInfo],
    pub current_song_id: Option<u32>,
    pub status: &'a MpdStatus,
    pub selected_idx: usize,
    pub search_query: &'a str,
    pub is_searching: bool,
    pub theme: &'a Theme,
}

impl<'a> Widget for QueueView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 4 || area.width < 10 {
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Search bar or queue info
                Constraint::Min(2),    // Playlist items
                Constraint::Length(1), // Footer hotkeys
            ])
            .split(area);

        // --- 1. Search / Info Bar ---
        let info_line = if self.is_searching {
            Line::from(vec![
                Span::styled(" 🔍 Search: ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{}_", self.search_query), self.theme.primary_text()),
                Span::styled("  (Press Enter to finish, Esc to clear)", self.theme.muted_text()),
            ])
        } else if !self.search_query.is_empty() {
            Line::from(vec![
                Span::styled(" 🔍 Filter: ", Style::default().fg(self.theme.accent)),
                Span::styled(self.search_query, self.theme.primary_text()),
                Span::styled(format!("  (Found {} tracks. Press / to edit, Esc to clear)", self.playlist.len()), self.theme.muted_text()),
            ])
        } else {
            Line::from(vec![
                Span::styled("  Current Queue  ", self.theme.primary_text()),
                Span::styled(format!("({} tracks)  ·  [/] Search  [d] Delete  [c] Clear", self.playlist.len()), self.theme.muted_text()),
            ])
        };
        Paragraph::new(info_line).render(chunks[0], buf);

        // --- 2. Playlist Table / Items ---
        let list_area = chunks[1];
        if self.playlist.is_empty() {
            let empty_msg = Paragraph::new("Queue is empty. Press [4] or [Tab] to go to Library and add songs!")
                .style(self.theme.secondary_text())
                .alignment(Alignment::Center);
            empty_msg.render(list_area, buf);
            return;
        }

        let view_height = list_area.height as usize;
        let total_items = self.playlist.len();

        // Calculate scrolling window so selected_idx is visible
        let start_idx = if self.selected_idx >= view_height {
            self.selected_idx.saturating_sub(view_height / 2).min(total_items.saturating_sub(view_height))
        } else {
            0
        };
        let end_idx = (start_idx + view_height).min(total_items);

        let mut item_lines = Vec::new();
        for i in start_idx..end_idx {
            let song = &self.playlist[i];
            let is_selected = i == self.selected_idx;
            let is_currently_playing = Some(song.id) == self.current_song_id;

            let play_indicator = if is_currently_playing {
                if self.status.state == PlaybackState::Play {
                    " ▶ "
                } else {
                    " ⏸ "
                }
            } else {
                "   "
            };

            let num_str = format!("{:2}. ", i + 1);
            let duration_str = format_duration(song.duration);

            let row_style = if is_selected {
                Style::default().bg(self.theme.pill_active_bg)
            } else {
                Style::default()
            };

            let title_style = if is_currently_playing {
                Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().fg(self.theme.text_primary).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.theme.text_primary)
            };

            let _artist_style = Style::default().fg(self.theme.text_secondary);
            let time_style = Style::default().fg(self.theme.text_muted);

            // Compute available width for title & artist
            let fixed_len = 3 + num_str.len() + duration_str.len() + 6;
            let content_width = (list_area.width as usize).saturating_sub(fixed_len);
            let title_part = song.display_title();
            let artist_part = format!(" · {}", song.display_artist());

            let full_text = format!("{}{}", title_part, artist_part);
            let display_text = if full_text.chars().count() > content_width && content_width > 3 {
                let mut s: String = full_text.chars().take(content_width - 3).collect();
                s.push_str("...");
                s
            } else {
                full_text
            };

            let pad_len = content_width.saturating_sub(display_text.chars().count());
            let padding = " ".repeat(pad_len);

            let line = Line::from(vec![
                Span::styled(play_indicator, if is_currently_playing { Style::default().fg(self.theme.accent) } else { row_style }),
                Span::styled(num_str, Style::default().fg(self.theme.text_muted)),
                Span::styled(display_text, title_style),
                Span::styled(padding, row_style),
                Span::styled(format!("  {} ", duration_str), time_style),
            ]);

            item_lines.push(line);
        }

        Paragraph::new(item_lines).render(list_area, buf);

        // --- 3. Footer Hotkeys ---
        let footer_line = Line::from(vec![
            Span::styled(" [Enter] ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Play Track  ", self.theme.secondary_text()),
            Span::styled(" [j/k/↑/↓] ", Style::default().fg(self.theme.accent)),
            Span::styled("Navigate  ", self.theme.secondary_text()),
            Span::styled(" [d] ", Style::default().fg(self.theme.accent)),
            Span::styled("Delete  ", self.theme.secondary_text()),
            Span::styled(" [Space] ", Style::default().fg(self.theme.accent)),
            Span::styled("Pause/Play  ", self.theme.secondary_text()),
            Span::styled(" [Tab] ", Style::default().fg(self.theme.accent)),
            Span::styled("Back to Disc", self.theme.secondary_text()),
        ]);
        Paragraph::new(footer_line).render(chunks[2], buf);
    }
}

fn format_duration(secs: f64) -> String {
    let total_secs = secs as u64;
    let mins = total_secs / 60;
    let s = total_secs % 60;
    format!("{:02}:{:02}", mins, s)
}
