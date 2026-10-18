use crate::app::BottomPanelMode;
use crate::lyrics::LyricsManager;
use crate::mpd_client::{MpdStatus, PlaybackState, SongInfo};
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

pub struct TelemetryWidget<'a> {
    pub song: Option<&'a SongInfo>,
    pub status: &'a MpdStatus,
    pub theme: &'a Theme,
    pub tick_counter: u64,
    pub mode: BottomPanelMode,
    pub lyrics_mgr: &'a LyricsManager,
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

                let line1 = Line::from(vec![
                    Span::styled(" FMT: ", self.theme.dim()),
                    Span::styled(format!("{:<14}", ext), self.theme.primary_bold()),
                    Span::styled("BIT: ", self.theme.dim()),
                    Span::styled(bit_depth, self.theme.primary_bold()),
                ]);

                let line2 = Line::from(vec![
                    Span::styled(" KBPS: ", self.theme.dim()),
                    Span::styled(format!("{:<13}", kbps), self.theme.primary_bold()),
                    Span::styled("SIZE: ", self.theme.dim()),
                    Span::styled(est_size_mb, self.theme.primary_bold()),
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
                // Multi-band graphic equalizer
                let num_bands = (inner.width as usize / 4).max(6).min(18);
                let t = self.tick_counter as f32 * 0.15;
                let bar_chars = [' ', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

                let mut rows: Vec<Line> = Vec::new();
                for r in (0..inner.height as usize).rev() {
                    let mut spans = Vec::new();
                    spans.push(Span::raw("  "));

                    for b in 0..num_bands {
                        let phase = b as f32 * 0.45;
                        let amp = if is_playing {
                            let w1 = (t * 1.5 + phase).sin() * 0.5 + 0.5;
                            let w2 = (t * 0.8 - phase * 0.7).cos() * 0.5 + 0.5;
                            (w1 * 0.6 + w2 * 0.4).powf(1.2)
                        } else {
                            0.05
                        };

                        let max_h = inner.height as f32;
                        let height_val = amp * max_h;
                        let diff = height_val - r as f32;

                        let ch = if diff >= 1.0 {
                            '█'
                        } else if diff > 0.0 {
                            let idx = (diff * (bar_chars.len() - 1) as f32).round() as usize;
                            bar_chars[idx.min(bar_chars.len() - 1)]
                        } else {
                            ' '
                        };

                        let bar_style = if r >= (max_h * 0.7) as usize {
                            self.theme.accent_secondary()
                        } else {
                            self.theme.accent()
                        };

                        spans.push(Span::styled(format!("{}   ", ch), bar_style));
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

                    let lines = vec![
                        Line::from(vec![
                            Span::styled("   ", self.theme.dim()),
                            Span::styled(prev_text, self.theme.dim()),
                        ]),
                        Line::from(vec![
                            Span::styled("▶  ", self.theme.accent()),
                            Span::styled(cur_text, self.theme.primary_bold()),
                        ]),
                        Line::from(vec![
                            Span::styled("   ", self.theme.dim()),
                            Span::styled(next_text, self.theme.secondary()),
                        ]),
                    ];
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
