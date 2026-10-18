use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

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

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border_style())
            .title(Span::styled(" [LIBRARY // MUSIC DIRECTORY] ", self.theme.accent()))
            .style(Style::default().bg(Color::Reset));

        let inner_area = block.inner(area);
        block.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Search / Info header
                Constraint::Min(2),    // File list
                Constraint::Length(1), // Footer hotkeys
            ])
            .split(inner_area);

        // --- 1. Info / Search ---
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
                Span::styled(format!("  ({} results. / to edit, Esc to clear)", self.files.len()), self.theme.dim()),
            ])
        } else {
            Line::from(vec![
                Span::styled("  Library: ", self.theme.primary_bold()),
                Span::styled(format!("{} songs  ·  [/] Search  [a/Enter] Add to queue", self.files.len()), self.theme.dim()),
            ])
        };
        Paragraph::new(info_line).render(chunks[0], buf);

        // --- 2. Files List ---
        let list_area = chunks[1];
        if self.files.is_empty() {
            let empty_msg = Paragraph::new("No files found in ~/Music. Add audio files and run 'mpc update'!")
                .style(self.theme.secondary())
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

            let pointer = if is_selected { "► " } else { "  " };

            let name_style = if is_selected {
                self.theme.primary_bold()
            } else {
                self.theme.secondary()
            };

            let line = Line::from(vec![
                Span::styled(pointer, if is_selected { self.theme.accent() } else { self.theme.dim() }),
                Span::styled("🎵 ", self.theme.accent()),
                Span::styled(file, name_style),
            ]);
            lines.push(line);
        }

        Paragraph::new(lines).render(list_area, buf);

        // --- 3. Footer Hotkeys ---
        let footer_line = Line::from(vec![
            Span::styled(" [Enter/a] ", self.theme.accent()),
            Span::styled("Add to Queue  ", self.theme.dim()),
            Span::styled(" [j/k/↑/↓] ", self.theme.accent()),
            Span::styled("Navigate  ", self.theme.dim()),
            Span::styled(" [/] ", self.theme.accent()),
            Span::styled("Search  ", self.theme.dim()),
            Span::styled(" [Tab] ", self.theme.accent()),
            Span::styled("Player", self.theme.dim()),
        ]);
        Paragraph::new(footer_line).render(chunks[2], buf);
    }
}
