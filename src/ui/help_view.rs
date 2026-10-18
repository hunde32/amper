use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

pub struct HelpModal<'a> {
    pub theme: &'a Theme,
}

impl<'a> Widget for HelpModal<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal_width = 54.min(area.width.saturating_sub(4));
        let modal_height = 20.min(area.height.saturating_sub(2));

        let modal_x = area.x + (area.width.saturating_sub(modal_width)) / 2;
        let modal_y = area.y + (area.height.saturating_sub(modal_height)) / 2;

        let modal_rect = Rect {
            x: modal_x,
            y: modal_y,
            width: modal_width,
            height: modal_height,
        };

        // Clear background
        Clear.render(modal_rect, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.accent))
            .style(Style::default().bg(self.theme.card_bg))
            .title(Span::styled(
                " 🎵 Amper Shortcuts 🎵 ",
                Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center);

        let inner = block.inner(modal_rect);
        block.render(modal_rect, buf);

        let rows = vec![
            Line::from(vec![
                Span::styled(" [Space]        ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled("Play / Pause toggle", self.theme.primary_text()),
            ]),
            Line::from(vec![
                Span::styled(" [←/h] [→/l]    ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled("Seek -5s / +5s", self.theme.primary_text()),
            ]),
            Line::from(vec![
                Span::styled(" [n / >] [p / <]", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled("Next / Previous track", self.theme.primary_text()),
            ]),
            Line::from(vec![
                Span::styled(" [↑/k] [↓/j]    ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled("Volume up / down (in Player)", self.theme.primary_text()),
            ]),
            Line::from(vec![
                Span::styled(" [r]            ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled("Toggle Repeat (Off / All)", self.theme.primary_text()),
            ]),
            Line::from(vec![
                Span::styled(" [s]            ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled("Toggle Shuffle / Random", self.theme.primary_text()),
            ]),
            Line::from(vec![
                Span::styled(" [f]            ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled("Toggle Favorite / Like ♥", self.theme.primary_text()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" [Tab]          ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled("Cycle views (Disc/Lyrics/Queue)", self.theme.primary_text()),
            ]),
            Line::from(vec![
                Span::styled(" [1] [2] [3] [4]", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled("Jump: Player/Lyrics/Queue/Lib", self.theme.primary_text()),
            ]),
            Line::from(vec![
                Span::styled(" [/]            ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled("Search in Queue or Library", self.theme.primary_text()),
            ]),
            Line::from(vec![
                Span::styled(" [?]            ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled("Close this Help window", self.theme.primary_text()),
            ]),
            Line::from(vec![
                Span::styled(" [q / Esc]      ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled("Quit Amper", self.theme.primary_text()),
            ]),
        ];

        Paragraph::new(rows).render(inner, buf);
    }
}
