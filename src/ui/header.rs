use crate::app::ActiveTab;
use crate::mpd_client::SongInfo;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

pub struct HeaderWidget<'a> {
    pub song: Option<&'a SongInfo>,
    pub active_tab: ActiveTab,
    pub theme: &'a Theme,
    pub volume: i32,
    pub status_msg: Option<&'a str>,
}

impl<'a> Widget for HeaderWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 || area.width < 10 {
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Top bar: < Title/Artist ...
                Constraint::Length(1), // Subtitle / Pill tabs
            ])
            .split(area);

        // --- Top Bar ---
        let top_area = chunks[0];
        let (title, artist) = if let Some(song) = self.song {
            (song.display_title(), song.display_artist())
        } else {
            ("Amper Music".to_string(), "MPD Client".to_string())
        };

        // Left button: < Back / Menu
        let left_span = Span::styled(
            " < ",
            Style::default().fg(self.theme.text_secondary).add_modifier(Modifier::BOLD),
        );
        buf.set_string(top_area.x + 1, top_area.y, left_span.content, left_span.style);

        // Center: Track Title & Artist
        let center_text = format!("{}  ·  {}", title, artist);
        let center_max_len = (top_area.width.saturating_sub(16)) as usize;
        let truncated_center = if center_text.chars().count() > center_max_len && center_max_len > 3 {
            let mut s: String = center_text.chars().take(center_max_len - 3).collect();
            s.push_str("...");
            s
        } else {
            center_text
        };

        let center_x = top_area.x + (top_area.width.saturating_sub(truncated_center.len() as u16)) / 2;
        buf.set_string(
            center_x,
            top_area.y,
            &truncated_center,
            Style::default().fg(self.theme.text_primary).add_modifier(Modifier::BOLD),
        );

        // Right button: ··· Options / Help / Vol
        let right_text = if self.volume >= 0 {
            format!("🔊 {}%  [?]", self.volume)
        } else {
            " ··· [?] ".to_string()
        };
        let right_x = top_area.x + top_area.width.saturating_sub(right_text.len() as u16 + 1);
        buf.set_string(
            right_x,
            top_area.y,
            &right_text,
            Style::default().fg(self.theme.text_secondary),
        );

        // --- Tabs / Pills Bar (Songs | Lyrics | Queue | Library) ---
        let tabs_area = chunks[1];
        let tabs = [
            (ActiveTab::Player, " Songs "),
            (ActiveTab::Lyrics, " Lyrics "),
            (ActiveTab::Queue, " Queue "),
            (ActiveTab::Library, " Library "),
        ];

        let mut tab_spans = Vec::new();
        for (i, (tab, label)) in tabs.iter().enumerate() {
            if i > 0 {
                tab_spans.push(Span::styled("  ", Style::default()));
            }
            if *tab == self.active_tab {
                tab_spans.push(Span::styled(
                    *label,
                    self.theme.pill_active(),
                ));
            } else {
                tab_spans.push(Span::styled(
                    *label,
                    self.theme.pill_inactive(),
                ));
            }
        }

        let tabs_line = Line::from(tab_spans);
        let p = Paragraph::new(tabs_line).alignment(Alignment::Center);
        p.render(tabs_area, buf);
    }
}
