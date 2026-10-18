use crate::lyrics::LyricsManager;
use crate::mpd_client::SongInfo;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

pub struct LyricsView<'a> {
    pub lyrics_mgr: &'a LyricsManager,
    pub song: Option<&'a SongInfo>,
    pub elapsed: f64,
    pub scroll_offset: Option<usize>,
    pub theme: &'a Theme,
}

impl<'a> Widget for LyricsView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 4 || area.width < 10 {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border_style())
            .title(Span::styled(" [LYRICS // SYNCHRONIZED] ", self.theme.accent()))
            .style(Style::default().bg(Color::Reset));

        let inner_area = block.inner(area);
        block.render(area, buf);

        if self.lyrics_mgr.is_loading() {
            let loading_line = Line::from(vec![
                Span::styled("⚡ Querying LRCLIB synchronized lyrics...", self.theme.secondary())
            ]);
            Paragraph::new(loading_line)
                .alignment(Alignment::Center)
                .render(inner_area, buf);
            return;
        }

        let lyrics_data = match self.lyrics_mgr.current_lyrics() {
            Some(data) if !data.lines.is_empty() => data,
            _ => {
                let song_name = self.song.map(|s| s.display_title()).unwrap_or_else(|| "current song".to_string());
                let artist_name = self.song.map(|s| s.display_artist()).unwrap_or_else(|| "artist".to_string());
                let no_lyrics = vec![
                    Line::from(vec![Span::styled("♪ No synchronized lyrics found ♪", self.theme.primary_bold())]),
                    Line::from(""),
                    Line::from(vec![Span::styled(format!("Track: {} · {}", song_name, artist_name), self.theme.dim())]),
                    Line::from(""),
                    Line::from(vec![Span::styled("Press [Tab] to return to Player or [L] to retry fetch", self.theme.secondary())]),
                ];
                Paragraph::new(no_lyrics)
                    .alignment(Alignment::Center)
                    .render(inner_area, buf);
                return;
            }
        };

        let active_idx = self.lyrics_mgr.get_active_line_index(self.elapsed);
        let total_lines = lyrics_data.lines.len();
        let view_height = inner_area.height as usize;

        let center_idx = self.scroll_offset.or(active_idx).unwrap_or(0);
        let start_idx = center_idx.saturating_sub(view_height / 2);
        let end_idx = (start_idx + view_height).min(total_lines);

        let mut lines_to_render = Vec::new();
        let leading_blanks = (view_height / 2).saturating_sub(center_idx);
        for _ in 0..leading_blanks {
            lines_to_render.push(Line::from(""));
        }

        for i in start_idx..end_idx {
            let line = &lyrics_data.lines[i];
            let is_active = Some(i) == active_idx;

            if is_active {
                lines_to_render.push(Line::from(vec![
                    Span::styled("▶  ", self.theme.accent()),
                    Span::styled(&line.text, self.theme.accent()),
                ]));
            } else if Some(i) < active_idx {
                lines_to_render.push(Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(&line.text, self.theme.dim()),
                ]));
            } else {
                lines_to_render.push(Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(&line.text, self.theme.secondary()),
                ]));
            }
        }

        Paragraph::new(lines_to_render)
            .alignment(Alignment::Center)
            .render(inner_area, buf);
    }
}
