use crate::lyrics::LyricsManager;
use crate::mpd_client::SongInfo;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

/// Wrap text cleanly into multiple lines at word boundaries so words and letters are never cut off.
pub fn wrap_line_words(text: &str, max_width: usize) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return vec![String::new()];
    }
    let max_w = max_width.max(10);
    let mut result = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            if word.len() > max_w {
                // Break overly long single words safely at character boundaries
                let mut start = 0;
                while start < word.len() {
                    let end = (start + max_w).min(word.len());
                    let safe_end = match word.is_char_boundary(end) {
                        true => end,
                        false => {
                            let mut b = end;
                            while b > start && !word.is_char_boundary(b) {
                                b -= 1;
                            }
                            if b == start { end } else { b }
                        }
                    };
                    result.push(word[start..safe_end].to_string());
                    start = safe_end;
                }
            } else {
                current_line.push_str(word);
            }
        } else if current_line.len() + 1 + word.len() <= max_w {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            result.push(current_line);
            current_line = word.to_string();
        }
    }
    if !current_line.is_empty() {
        result.push(current_line);
    }
    if result.is_empty() {
        vec![String::new()]
    } else {
        result
    }
}

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
        let view_h = inner_area.height as usize;
        let center_idx = self.scroll_offset.or(active_idx).unwrap_or(0);

        // Maximum line width with safety margin to prevent edge clipping
        let max_line_w = (inner_area.width as usize).saturating_sub(6).max(16);

        let center_wrapped = wrap_line_words(&lyrics_data.lines[center_idx].text, max_line_w);
        let center_h = center_wrapped.len();
        let target_y = (view_h / 2).saturating_sub(center_h / 2);

        // Collect lines above center_idx
        let mut top_lines: Vec<Line> = Vec::new();
        let mut rows_above_needed = target_y;
        let mut cur_above = center_idx;
        while cur_above > 0 && rows_above_needed > 0 {
            cur_above -= 1;
            let wrapped = wrap_line_words(&lyrics_data.lines[cur_above].text, max_line_w);
            let is_act = Some(cur_above) == active_idx;
            let is_past = active_idx.map_or(false, |act| cur_above < act);
            let style = if is_act {
                Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)
            } else if is_past {
                self.theme.dim()
            } else {
                self.theme.secondary()
            };
            let bullet = if is_act { "▶  " } else { "   " };

            let mut line_rows = Vec::new();
            for (r_i, text_chunk) in wrapped.into_iter().enumerate() {
                let pfx = if r_i == 0 { bullet } else { "   " };
                line_rows.push(Line::from(vec![
                    Span::styled(pfx, if is_act { self.theme.accent() } else { Style::default() }),
                    Span::styled(text_chunk, style),
                ]));
            }
            let line_rows_len = line_rows.len();
            for r in line_rows.into_iter().rev() {
                top_lines.insert(0, r);
            }
            rows_above_needed = rows_above_needed.saturating_sub(line_rows_len);
        }

        // Add padding blanks if at the start of song to keep center lyric vertically centered
        let mut final_lines = Vec::new();
        for _ in 0..rows_above_needed {
            final_lines.push(Line::from(""));
        }
        final_lines.extend(top_lines);

        // Add center lyric rows
        let center_is_act = Some(center_idx) == active_idx;
        let center_is_past = active_idx.map_or(false, |act| center_idx < act);
        let center_style = if center_is_act {
            Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)
        } else if center_is_past {
            self.theme.dim()
        } else {
            self.theme.secondary()
        };
        let center_bullet = if center_is_act { "▶  " } else { "   " };
        for (r_i, text_chunk) in center_wrapped.into_iter().enumerate() {
            let pfx = if r_i == 0 { center_bullet } else { "   " };
            final_lines.push(Line::from(vec![
                Span::styled(pfx, if center_is_act { self.theme.accent() } else { Style::default() }),
                Span::styled(text_chunk, center_style),
            ]));
        }

        // Add following lines below center_idx until viewport height is reached
        let mut cur_below = center_idx + 1;
        while cur_below < total_lines && final_lines.len() < view_h {
            let wrapped = wrap_line_words(&lyrics_data.lines[cur_below].text, max_line_w);
            let is_act = Some(cur_below) == active_idx;
            let is_past = active_idx.map_or(false, |act| cur_below < act);
            let style = if is_act {
                Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)
            } else if is_past {
                self.theme.dim()
            } else {
                self.theme.secondary()
            };
            let bullet = if is_act { "▶  " } else { "   " };

            for (r_i, text_chunk) in wrapped.into_iter().enumerate() {
                if final_lines.len() >= view_h {
                    break;
                }
                let pfx = if r_i == 0 { bullet } else { "   " };
                final_lines.push(Line::from(vec![
                    Span::styled(pfx, if is_act { self.theme.accent() } else { Style::default() }),
                    Span::styled(text_chunk, style),
                ]));
            }
            cur_below += 1;
        }

        Paragraph::new(final_lines)
            .alignment(Alignment::Center)
            .render(inner_area, buf);
    }
}
