use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

pub struct LibraryView<'a> {
    pub files: &'a [String],
    pub selected_idx: usize,
    pub search_query: &'a str,
    pub is_searching: bool,
    pub theme: &'a Theme,
}

impl<'a> Widget for LibraryView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 4 || area.width < 10 {
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Search / Info header
                Constraint::Min(2),    // File list
                Constraint::Length(1), // Footer hotkeys
            ])
            .split(area);

        // --- 1. Info / Search ---
        let info_line = if self.is_searching {
            Line::from(vec![
                Span::styled(" 🔍 Search Library: ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{}_", self.search_query), self.theme.primary_text()),
                Span::styled("  (Enter to finish, Esc to clear)", self.theme.muted_text()),
            ])
        } else if !self.search_query.is_empty() {
            Line::from(vec![
                Span::styled(" 🔍 Filter: ", Style::default().fg(self.theme.accent)),
                Span::styled(self.search_query, self.theme.primary_text()),
                Span::styled(format!("  ({} results. Press / to search, Esc to clear)", self.files.len()), self.theme.muted_text()),
            ])
        } else {
            Line::from(vec![
                Span::styled("  Music Library  ", self.theme.primary_text()),
                Span::styled(format!("({} songs available)  ·  [/] Search  [a/Enter] Add to queue", self.files.len()), self.theme.muted_text()),
            ])
        };
        Paragraph::new(info_line).render(chunks[0], buf);

        // --- 2. Files List ---
        let list_area = chunks[1];
        if self.files.is_empty() {
            let empty_msg = Paragraph::new("No files found. Put music in ~/Music and run 'mpc update'!")
                .style(self.theme.secondary_text())
                .alignment(Alignment::Center);
            empty_msg.render(list_area, buf);
            return;
        }

        let view_height = list_area.height as usize;
        let total_items = self.files.len();

        let start_idx = if self.selected_idx >= view_height {
            self.selected_idx.saturating_sub(view_height / 2).min(total_items.saturating_sub(view_height))
        } else {
            0
        };
        let end_idx = (start_idx + view_height).min(total_items);

        let mut lines = Vec::new();
        for i in start_idx..end_idx {
            let file = &self.files[i];
            let is_selected = i == self.selected_idx;

            let _row_style = if is_selected {
                Style::default().bg(self.theme.pill_active_bg)
            } else {
                Style::default()
            };

            let name_style = if is_selected {
                Style::default().fg(self.theme.text_primary).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.theme.text_secondary)
            };

            let icon = " 🎵 ";
            let display_name = file;

            let line = Line::from(vec![
                Span::styled(icon, Style::default().fg(self.theme.accent)),
                Span::styled(display_name, name_style),
            ]);
            lines.push(line);
        }

        Paragraph::new(lines).render(list_area, buf);

        // --- 3. Footer Hotkeys ---
        let footer_line = Line::from(vec![
            Span::styled(" [Enter/a] ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Add to Queue  ", self.theme.secondary_text()),
            Span::styled(" [j/k/↑/↓] ", Style::default().fg(self.theme.accent)),
            Span::styled("Navigate  ", self.theme.secondary_text()),
            Span::styled(" [/] ", Style::default().fg(self.theme.accent)),
            Span::styled("Filter  ", self.theme.secondary_text()),
            Span::styled(" [Tab] ", Style::default().fg(self.theme.accent)),
            Span::styled("Back to Player", self.theme.secondary_text()),
        ]);
        Paragraph::new(footer_line).render(chunks[2], buf);
    }
}
