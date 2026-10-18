use crate::mpd_client::{MpdStatus, PlaybackState, SongInfo};
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

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

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border_style())
            .title(Span::styled(" [QUEUE // PLAYLIST] ", self.theme.accent()))
            .style(Style::default().bg(Color::Reset));

        let inner_area = block.inner(area);
        block.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Search bar or queue info
                Constraint::Min(2),    // Playlist items
                Constraint::Length(1), // Footer hotkeys
            ])
            .split(inner_area);

        // --- 1. Search / Info Bar ---
        let info_line = if self.is_searching {
            Line::from(vec![
                Span::styled(" 🔍 Search: ", self.theme.accent()),
                Span::styled(format!("{}_", self.search_query), self.theme.primary_bold()),
                Span::styled("  (Enter to finish, Esc to clear)", self.theme.dim()),
            ])
        } else if !self.search_query.is_empty() {
            Line::from(vec![
                Span::styled(" 🔍 Filter: ", self.theme.accent()),
                Span::styled(self.search_query, self.theme.primary_bold()),
                Span::styled(format!("  ({} results. / to edit, Esc to clear)", self.playlist.len()), self.theme.dim()),
            ])
        } else {
            Line::from(vec![
                Span::styled("  Queue: ", self.theme.primary_bold()),
                Span::styled(format!("{} tracks  ·  [/] Search  [d] Delete  [c] Clear", self.playlist.len()), self.theme.dim()),
            ])
        };
        Paragraph::new(info_line).render(chunks[0], buf);

        // --- 2. Playlist Table / Items ---
        let list_area = chunks[1];
        if self.playlist.is_empty() {
            let empty_msg = Paragraph::new("Queue is empty. Press [4] or [Tab] to go to Library and add songs!")
                .style(self.theme.secondary())
                .alignment(Alignment::Center);
            empty_msg.render(list_area, buf);
            return;
        }

        let view_height = list_area.height as usize;
        let total_items = self.playlist.len();

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
                Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().bg(Color::Reset)
            };

            let title_style = if is_currently_playing {
                self.theme.accent()
            } else if is_selected {
                self.theme.primary_bold()
            } else {
                self.theme.primary()
            };

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

            let pointer = if is_selected { "► " } else { "  " };

            let line = Line::from(vec![
                Span::styled(pointer, row_style),
                Span::styled(play_indicator, if is_currently_playing { self.theme.accent() } else { self.theme.dim() }),
                Span::styled(num_str, self.theme.dim()),
                Span::styled(display_text, title_style),
                Span::styled(padding, Style::default().bg(Color::Reset)),
                Span::styled(format!("  {} ", duration_str), self.theme.dim()),
            ]);

            item_lines.push(line);
        }

        Paragraph::new(item_lines).render(list_area, buf);

        // --- 3. Footer Hotkeys ---
        let footer_line = Line::from(vec![
            Span::styled(" [Enter] ", self.theme.accent()),
            Span::styled("Play  ", self.theme.dim()),
            Span::styled(" [j/k/↑/↓] ", self.theme.accent()),
            Span::styled("Navigate  ", self.theme.dim()),
            Span::styled(" [d] ", self.theme.accent()),
            Span::styled("Delete  ", self.theme.dim()),
            Span::styled(" [Space] ", self.theme.accent()),
            Span::styled("Pause/Play  ", self.theme.dim()),
            Span::styled(" [Tab] ", self.theme.accent()),
            Span::styled("Player", self.theme.dim()),
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
