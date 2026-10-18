use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

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

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border_style())
            .style(Style::default().bg(Color::Reset))
            .title(Span::styled(" [ SHORTCUTS // CONTROLS ] ", self.theme.accent()))
            .title_alignment(Alignment::Center);

        let inner = block.inner(modal_rect);
        block.render(modal_rect, buf);

        let rows = vec![
            Line::from(vec![
                Span::styled(" [Space]        ", self.theme.accent()),
                Span::styled("Play / Pause toggle", self.theme.primary()),
            ]),
            Line::from(vec![
                Span::styled(" [←/h] [→/l]    ", self.theme.accent()),
                Span::styled("Seek -5s / +5s", self.theme.primary()),
            ]),
            Line::from(vec![
                Span::styled(" [n / >] [p / <]", self.theme.accent()),
                Span::styled("Next / Previous track", self.theme.primary()),
            ]),
            Line::from(vec![
                Span::styled(" [↑/k] [↓/j]    ", self.theme.accent()),
                Span::styled("Volume up / down (in Player)", self.theme.primary()),
            ]),
            Line::from(vec![
                Span::styled(" [r]            ", self.theme.accent()),
                Span::styled("Toggle Repeat", self.theme.primary()),
            ]),
            Line::from(vec![
                Span::styled(" [s]            ", self.theme.accent()),
                Span::styled("Toggle Shuffle / Random", self.theme.primary()),
            ]),
            Line::from(vec![
                Span::styled(" [f]            ", self.theme.accent()),
                Span::styled("Toggle Favorite / Like ♥", self.theme.primary()),
            ]),
            Line::from(vec![
                Span::styled(" [v]            ", self.theme.accent()),
                Span::styled("Toggle Cover Mode (Static / Vinyl)", self.theme.primary()),
            ]),
            Line::from(vec![
                Span::styled(" [m / b]        ", self.theme.accent()),
                Span::styled("Cycle Bottom Panel (Telem/Visual/Lyrics)", self.theme.primary()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" [Tab]          ", self.theme.accent()),
                Span::styled("Cycle views (Player/Lyrics/Queue/Lib)", self.theme.primary()),
            ]),
            Line::from(vec![
                Span::styled(" [1] [2] [3] [4]", self.theme.accent()),
                Span::styled("Jump to specific view", self.theme.primary()),
            ]),
            Line::from(vec![
                Span::styled(" [/]            ", self.theme.accent()),
                Span::styled("Search in Queue or Library", self.theme.primary()),
            ]),
            Line::from(vec![
                Span::styled(" [?]            ", self.theme.accent()),
                Span::styled("Toggle Help popup", self.theme.primary()),
            ]),
            Line::from(vec![
                Span::styled(" [q / Esc]      ", self.theme.accent()),
                Span::styled("Quit Amper", self.theme.primary()),
            ]),
        ];

        Paragraph::new(rows).render(inner, buf);
    }
}
