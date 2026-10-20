use crate::app::BottomPanelMode;
use crate::audio_visualizer::AudioVisualizer;
use crate::lyrics::LyricsManager;
use crate::mpd_client::{MpdStatus, PlaybackState, SongInfo};
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

pub struct TelemetryWidget<'a> {
    pub song: Option<&'a SongInfo>,
    pub status: &'a MpdStatus,
    pub theme: &'a Theme,
    pub tick_counter: u64,
    pub mode: BottomPanelMode,
    pub lyrics_mgr: &'a LyricsManager,
    pub audio_viz: &'a AudioVisualizer,
}

impl<'a> Widget for TelemetryWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 5 || area.width < 25 {
            return;
        }

        let is_playing = self.status.state == PlaybackState::Play;

        // Card title and mode pill
        let (title_str, mode_tag) = match self.mode {
            BottomPanelMode::Telemetry => (" ■ AUDIO MONITOR // TELEMETRY ", "[MODE: TELEMETRY (m)]"),
            BottomPanelMode::Visualizer => (" ■ SPECTRUM // VISUALIZER ", "[MODE: SPECTRUM (m)]"),
            BottomPanelMode::MiniLyrics => (" ■ LIVE LYRICS // MINI ", "[MODE: LYRICS (m)]"),
            BottomPanelMode::Metadata => (" ■ TRACK DETAILS // METADATA ", "[MODE: DETAILS (m)]"),
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border_style())
            .title(Span::styled(title_str, self.theme.accent()))
            .title_bottom(Span::styled(format!(" {} ", mode_tag), self.theme.accent_secondary()));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 3 {
            return;
        }

        match self.mode {
            BottomPanelMode::Telemetry => {
                let ext = if let Some(s) = self.song {
                    let path = std::path::Path::new(&s.file);
                    path.extension()
                        .map(|e| e.to_string_lossy().to_uppercase())
                        .unwrap_or_else(|| "MP3".to_string())
                } else {
                    "MP3".to_string()
                };

                let is_lossless = ext == "FLAC" || ext == "WAV" || ext == "ALAC";
                let bit_depth = if is_lossless { "24" } else { "16" };
                let kbps = if is_lossless { "920" } else { "320" };

                let est_size_mb = if let Some(s) = self.song {
                    let mins = s.duration / 60.0;
                    let mb = mins * if is_lossless { 20.0 } else { 2.4 };
                    format!("{:.1}MB", mb.max(1.0))
                } else {
                    "6.3MB".to_string()
                };

                let left1 = format!(" FMT: {}", ext);
                let right1 = format!("BIT: {} ", bit_depth);
                let gap1 = (inner.width as usize).saturating_sub(left1.len() + right1.len());

                let line1 = Line::from(vec![
                    Span::styled(" FMT: ", self.theme.dim()),
                    Span::styled(ext, self.theme.primary_bold()),
                    Span::raw(" ".repeat(gap1)),
                    Span::styled("BIT: ", self.theme.dim()),
                    Span::styled(format!("{} ", bit_depth), self.theme.primary_bold()),
                ]);

                let left2 = format!(" KBPS: {}", kbps);
                let right2 = format!("SIZE: {} ", est_size_mb);
                let gap2 = (inner.width as usize).saturating_sub(left2.len() + right2.len());

                let line2 = Line::from(vec![
                    Span::styled(" KBPS: ", self.theme.dim()),
                    Span::styled(kbps, self.theme.primary_bold()),
                    Span::raw(" ".repeat(gap2)),
                    Span::styled("SIZE: ", self.theme.dim()),
                    Span::styled(format!("{} ", est_size_mb), self.theme.primary_bold()),
                ]);

                let line3 = Line::from(vec![
                    Span::styled(" OUT: ", self.theme.dim()),
                    Span::styled("PIPEWIRE / PULSEAUDIO", self.theme.secondary()),
                ]);

                let sep_width = (inner.width as usize).saturating_sub(2);
                let line4 = Line::from(vec![Span::styled(
                    format!(" {}", "·".repeat(sep_width)),
                    self.theme.dim(),
                )]);

                let cycle = if is_playing {
                    (self.tick_counter / 6) % 3
                } else {
                    0
                };
                let dots = match cycle {
                    0 => "● ○ ○",
                    1 => "○ ● ○",
                    _ => "○ ○ ●",
                };

                let vol = self.status.volume.clamp(0, 100);
                let vol_blocks = ((vol as f32 / 100.0) * 10.0).round() as usize;
                let vol_filled = "█".repeat(vol_blocks);
                let vol_empty = "░".repeat(10usize.saturating_sub(vol_blocks));

                let line5 = Line::from(vec![
                    Span::styled(" MPD ", self.theme.dim()),
                    Span::styled(format!("{} ", dots), self.theme.accent()),
                    Span::styled(format!("  VOL: {:>3}% [", vol), self.theme.dim()),
                    Span::styled(vol_filled, self.theme.accent()),
                    Span::styled(vol_empty, self.theme.dim()),
                    Span::styled("]", self.theme.dim()),
                ]);

                let lines = vec![line1, line2, line3, line4, line5];
                Paragraph::new(lines).render(inner, buf);
            }

            BottomPanelMode::Visualizer => {
                // Real-time audio spectrum analyzer powered by FFT on live PCM audio
                let usable_width = (inner.width as usize).saturating_sub(2);
                let num_bands = (usable_width / 2).max(8).min(24);
                let (bands, peaks) = self.audio_viz.get_spectrum(num_bands);

                let bar_chars = [' ', ' ', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
                let max_h = inner.height as f32;

                let mut rows: Vec<Line> = Vec::with_capacity(inner.height as usize);
                for r in (0..inner.height as usize).rev() {
                    let mut spans = Vec::new();
                    // Left margin to center the visualizer
                    let total_viz_width = num_bands * 2;
                    let pad = (inner.width as usize).saturating_sub(total_viz_width) / 2;
                    if pad > 0 {
                        spans.push(Span::raw(" ".repeat(pad)));
                    }

                    for b in 0..num_bands {
                        let amp = bands.get(b).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                        let peak = peaks.get(b).copied().unwrap_or(0.0).clamp(0.0, 1.0);

                        let bar_level = amp * max_h;
                        let peak_level = (peak * max_h).floor() as usize;

                        let diff = bar_level - r as f32;

                        let (ch, is_peak) = if diff >= 1.0 {
                            ('█', false)
                        } else if diff > 0.0 {
                            let idx = (diff * (bar_chars.len() - 1) as f32).round() as usize;
                            (bar_chars[idx.min(bar_chars.len() - 1)], false)
                        } else if r == peak_level && peak_level > 0 {
                            // Floating peak cap indicator (like hardware meters)
                            ('▔', true)
                        } else {
                            (' ', false)
                        };

                        let style = if is_peak {
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(ratatui::style::Modifier::BOLD)
                        } else if r >= (max_h * 0.65) as usize {
                            self.theme.accent_secondary()
                        } else {
                            self.theme.accent()
                        };

                        spans.push(Span::styled(format!("{} ", ch), style));
                    }
                    rows.push(Line::from(spans));
                }
                Paragraph::new(rows).render(inner, buf);
            }

            BottomPanelMode::MiniLyrics => {
                let active_idx = self.lyrics_mgr.get_active_line_index(self.status.elapsed);
                if let Some(lyrics) = self.lyrics_mgr.current_lyrics() {
                    if lyrics.lines.is_empty() {
                        let msg = Paragraph::new("No lyrics available for current track")
                            .style(self.theme.dim())
                            .alignment(ratatui::layout::Alignment::Center);
                        msg.render(inner, buf);
                        return;
                    }

                    let cur_i = active_idx.unwrap_or(0);
                    let prev_text = if cur_i > 0 {
                        &lyrics.lines[cur_i - 1].text
                    } else {
                        "..."
                    };
                    let cur_text = &lyrics.lines[cur_i].text;
                    let next_text = if cur_i + 1 < lyrics.lines.len() {
                        &lyrics.lines[cur_i + 1].text
                    } else {
                        "..."
                    };

                    let text_w = (inner.width as usize).saturating_sub(4).max(10);
                    let cur_sublines = crate::ui::lyrics_view::wrap_line_words(cur_text, text_w);
                    let avail_h = inner.height as usize;

                    let mut lines = Vec::new();
                    if cur_sublines.len() == 1 {
                        if avail_h >= 3 {
                            lines.push(Line::from(vec![
                                Span::styled("   ", self.theme.dim()),
                                Span::styled(fit_one_line_words(prev_text, text_w), self.theme.dim()),
                            ]));
                        }
                        lines.push(Line::from(vec![
                            Span::styled("▶  ", self.theme.accent()),
                            Span::styled(&cur_sublines[0], self.theme.primary_bold()),
                        ]));
                        if lines.len() < avail_h {
                            lines.push(Line::from(vec![
                                Span::styled("   ", self.theme.dim()),
                                Span::styled(fit_one_line_words(next_text, text_w), self.theme.secondary()),
                            ]));
                        }
                    } else if cur_sublines.len() == 2 {
                        if avail_h >= 4 {
                            lines.push(Line::from(vec![
                                Span::styled("   ", self.theme.dim()),
                                Span::styled(fit_one_line_words(prev_text, text_w), self.theme.dim()),
                            ]));
                        }
                        lines.push(Line::from(vec![
                            Span::styled("▶  ", self.theme.accent()),
                            Span::styled(&cur_sublines[0], self.theme.primary_bold()),
                        ]));
                        lines.push(Line::from(vec![
                            Span::styled("   ", Style::default()),
                            Span::styled(&cur_sublines[1], self.theme.primary_bold()),
                        ]));
                        if lines.len() < avail_h {
                            lines.push(Line::from(vec![
                                Span::styled("   ", self.theme.dim()),
                                Span::styled(fit_one_line_words(next_text, text_w), self.theme.secondary()),
                            ]));
                        }
                    } else {
                        for (r_i, chunk) in cur_sublines.into_iter().enumerate() {
                            if lines.len() >= avail_h {
                                break;
                            }
                            let bullet = if r_i == 0 { "▶  " } else { "   " };
                            lines.push(Line::from(vec![
                                Span::styled(bullet, if r_i == 0 { self.theme.accent() } else { Style::default() }),
                                Span::styled(chunk, self.theme.primary_bold()),
                            ]));
                        }
                    }

                    Paragraph::new(lines).render(inner, buf);
                } else if self.lyrics_mgr.is_loading() {
                    let msg = Paragraph::new("Fetching synchronized lyrics from LRCLIB...")
                        .style(self.theme.secondary())
                        .alignment(ratatui::layout::Alignment::Center);
                    msg.render(inner, buf);
                } else {
                    let msg = Paragraph::new("Press [L] to fetch lyrics or [m] to switch view")
                        .style(self.theme.dim())
                        .alignment(ratatui::layout::Alignment::Center);
                    msg.render(inner, buf);
                }
            }

            BottomPanelMode::Metadata => {
                let song = self.song;
                let title = song.map(|s| s.display_title()).unwrap_or_else(|| "Unknown".to_string());
                let artist = song.map(|s| s.display_artist()).unwrap_or_else(|| "Unknown".to_string());
                let album = song.map(|s| s.display_album()).unwrap_or_else(|| "Unknown".to_string());
                let file = song.map(|s| s.file.clone()).unwrap_or_default();
                let dur = song.map(|s| format!("{:.0}s", s.duration)).unwrap_or_default();

                let lines = vec![
                    Line::from(vec![
                        Span::styled(" TITLE:  ", self.theme.dim()),
                        Span::styled(title, self.theme.primary_bold()),
                    ]),
                    Line::from(vec![
                        Span::styled(" ARTIST: ", self.theme.dim()),
                        Span::styled(artist, self.theme.secondary()),
                    ]),
                    Line::from(vec![
                        Span::styled(" ALBUM:  ", self.theme.dim()),
                        Span::styled(album, self.theme.primary_bold()),
                        Span::styled("   DUR: ", self.theme.dim()),
                        Span::styled(dur, self.theme.secondary()),
                    ]),
                    Line::from(vec![
                        Span::styled(" FILE:   ", self.theme.dim()),
                        Span::styled(file, self.theme.dim()),
                    ]),
                ];
                Paragraph::new(lines).render(inner, buf);
            }
        }
    }
}

fn fit_one_line_words(text: &str, max_width: usize) -> String {
    let text = text.trim();
    if text.len() <= max_width {
        return text.to_string();
    }
    let target = max_width.saturating_sub(3);
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut result = String::new();
    for word in words {
        if result.is_empty() {
            if word.len() <= target {
                result.push_str(word);
            } else {
                let mut end = target.min(word.len());
                while end > 0 && !word.is_char_boundary(end) {
                    end -= 1;
                }
                result.push_str(&word[..end]);
                break;
            }
        } else if result.len() + 1 + word.len() <= target {
            result.push(' ');
            result.push_str(word);
        } else {
            break;
        }
    }
    result.push_str("...");
    result
}

