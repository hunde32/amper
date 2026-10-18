use crate::lyrics::LyricsManager;
use crate::mpd_client::SongInfo;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

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

        let inner_area = Rect {
            x: area.x + 2,
            y: area.y + 1,
            width: area.width.saturating_sub(4),
            height: area.height.saturating_sub(2),
        };

        if self.lyrics_mgr.is_loading() {
            let loading_line = Line::from(vec![
                Span::styled("✨ Searching synchronized lyrics from LRCLIB...", self.theme.secondary_text())
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
                    Line::from(vec![Span::styled("♪ No synchronized lyrics available ♪", self.theme.primary_text())]),
                    Line::from(""),
                    Line::from(vec![Span::styled(format!("Track: {} · {}", song_name, artist_name), self.theme.muted_text())]),
                    Line::from(""),
                    Line::from(vec![Span::styled("Press [Tab] to return to Disc Player or [L] to retry fetch", self.theme.secondary_text())]),
                ];
                Paragraph::new(no_lyrics)
                    .alignment(Alignment::Center)
                    .render(inner_area, buf);
                return;
            }
        };

        // Determine active line
        let active_idx = self.lyrics_mgr.get_active_line_index(self.elapsed);

        let total_lines = lyrics_data.lines.len();
        let view_height = inner_area.height as usize;

        // Auto-center around the active line unless manual scroll is active
        let center_idx = self.scroll_offset.or(active_idx).unwrap_or(0);
        let start_idx = center_idx.saturating_sub(view_height / 2);
        let end_idx = (start_idx + view_height).min(total_lines);

        let mut lines_to_render = Vec::new();

        // Padding lines before if needed
        let leading_blanks = (view_height / 2).saturating_sub(center_idx);
        for _ in 0..leading_blanks {
            lines_to_render.push(Line::from(""));
        }

        for i in start_idx..end_idx {
            let line = &lyrics_data.lines[i];
            let is_active = Some(i) == active_idx;

            if is_active {
                lines_to_render.push(Line::from(vec![
                    Span::styled("▶  ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        &line.text,
                        Style::default()
                            .fg(self.theme.text_primary)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else if Some(i) < active_idx {
                // Past lines: dimmed chocolate/beige
                lines_to_render.push(Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(&line.text, Style::default().fg(self.theme.text_muted)),
                ]));
            } else {
                // Future lines: medium beige
                lines_to_render.push(Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(&line.text, Style::default().fg(self.theme.text_secondary)),
                ]));
            }
        }

        Paragraph::new(lines_to_render)
            .alignment(Alignment::Center)
            .render(inner_area, buf);
    }
}
