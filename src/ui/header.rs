use crate::app::ActiveTab;
use crate::mpd_client::SongInfo;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

pub struct HeaderWidget<'a> {
    pub song: Option<&'a SongInfo>,
    pub active_tab: ActiveTab,
    pub theme: &'a Theme,
    pub is_favorite: bool,
}

impl<'a> Widget for HeaderWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 || area.width < 10 {
            return;
        }

        // Top bar matching inspiration: ^              [SONGS] [LYRICS] [QUEUE] [LIKE]
        let left_icon = Span::styled(" ^ ", self.theme.accent());

        let songs_style = if self.active_tab == ActiveTab::Player {
            self.theme.accent()
        } else {
            self.theme.secondary()
        };

        let lyrics_style = if self.active_tab == ActiveTab::Lyrics {
            self.theme.accent()
        } else {
            self.theme.secondary()
        };

        let queue_style = if self.active_tab == ActiveTab::Queue {
            self.theme.accent()
        } else {
            self.theme.secondary()
        };

        let lib_style = if self.active_tab == ActiveTab::Library {
            self.theme.accent()
        } else {
            self.theme.secondary()
        };

        let like_span = if self.is_favorite {
            Span::styled(" [♥ LIKED] ", self.theme.accent_secondary())
        } else {
            Span::styled(" [LIKE] ", self.theme.dim())
        };

        let header_line = Line::from(vec![
            left_icon,
            Span::raw(" "),
            Span::styled("[SONGS]", songs_style),
            Span::raw(" "),
            Span::styled("[LYRICS]", lyrics_style),
            Span::raw(" "),
            Span::styled("[QUEUE]", queue_style),
            Span::raw(" "),
            Span::styled("[LIB]", lib_style),
            Span::raw("   "),
            like_span,
            Span::styled(" [?] ", self.theme.dim()),
        ]);

        Paragraph::new(header_line)
            .alignment(Alignment::Center)
            .render(area, buf);
    }
}
