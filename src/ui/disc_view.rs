use crate::app::{BottomPanelMode, CoverGeometry, CoverMode};
use crate::cover_art::CoverArtManager;
use crate::lyrics::LyricsManager;
use crate::mpd_client::{MpdStatus, PlaybackState, SongInfo};
use crate::ui::telemetry::TelemetryWidget;
use crate::ui::theme::Theme;
use crate::ui::visualizer::VisualizerWidget;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use std::time::Instant;

pub struct DiscView<'a> {
    pub song: Option<&'a SongInfo>,
    pub status: &'a MpdStatus,
    pub cover_art: &'a CoverArtManager,
    pub rotation_angle: f32,
    pub theme: &'a Theme,
    pub tick_counter: u64,
    pub cover_mode: CoverMode,
    pub bottom_mode: BottomPanelMode,
    pub lyrics_mgr: &'a LyricsManager,
    pub clicked_button: Option<(&'static str, Instant)>,
    pub geom: CoverGeometry,
}

impl<'a> Widget for DiscView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 25 || area.height < 14 {
            let msg = Paragraph::new("Terminal too small for Player")
                .style(self.theme.dim())
                .alignment(Alignment::Center);
            msg.render(area, buf);
            return;
        }

        let is_playing = self.status.state == PlaybackState::Play;

        // ── Layout ──────────────────────────────────────────────────────────────
        //  Row 0  │ 2 lines  │  Visualizer banner + waveform
        //  Row 1  │ Min(6)   │  Cover art box (fills most of available space)
        //  Row 2  │ 1 line   │  Track title  (bold, large)
        //  Row 3  │ 1 line   │  Artist name  (dimmer)
        //  Row 4  │ 1 line   │  blank spacer
        //  Row 5  │ 1 line   │  Scrubber bar  elapsed ██░░░ total  (STANDALONE)
        //  Row 6  │ 1 line   │  blank spacer
        //  Row 7  │ 3 lines  │  Controls  [ ⇄ ] |<< [  ||  ] >>| [ ↻ ]
        //  Row 8  │ 6 lines  │  Telemetry / Visualizer / Lyrics / Metadata panel
        // ────────────────────────────────────────────────────────────────────────
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // 0  visualizer
                Constraint::Min(6),    // 1  cover box
                Constraint::Length(1), // 2  title
                Constraint::Length(1), // 3  artist
                Constraint::Length(1), // 4  spacer
                Constraint::Length(1), // 5  scrubber
                Constraint::Length(1), // 6  spacer
                Constraint::Length(3), // 7  controls (3 explicit rows)
                Constraint::Length(6), // 8  bottom panel
            ])
            .split(area);

        let viz_area       = chunks[0];
        let _cover_box_area = chunks[1];
        let title_area    = chunks[2];
        let artist_area   = chunks[3];
        // chunks[4] is the spacer row – we do nothing with it
        let scrubber_area = chunks[5];
        // chunks[6] is the spacer row – we do nothing with it
        let controls_area = chunks[7];
        let bottom_area   = chunks[8];

        // ── 0. Visualizer ───────────────────────────────────────────────────────
        VisualizerWidget {
            is_playing,
            tick_counter: self.tick_counter,
            theme: self.theme,
            sample_rate: "MPD 44.1 kHz",
        }
        .render(viz_area, buf);

        // ── 1. Cover / Disc Container ───────────────────────────────────────────
        {
            let mode_tag = match self.cover_mode {
                CoverMode::StaticCard    => " [STATIC (v)] ",
                CoverMode::SpinningVinyl => " [VINYL  (v)] ",
            };

            let cover_block = Block::default()
                .borders(Borders::ALL)
                .border_style(self.theme.border_style())
                .title_bottom(Span::styled(mode_tag, self.theme.dim()))
                .style(Style::default().bg(Color::Reset));
            cover_block.render(self.geom.cover_box_rect, buf);

            if self.cover_art.supports_kitty {
                // Blank the cells so the Kitty GPU image shows through cleanly
                for r in 0..self.geom.img_rows {
                    let cy = self.geom.img_y + r;
                    if cy >= buf.area.height { break; }
                    for c in 0..self.geom.img_cols {
                        let cx = self.geom.img_x + c;
                        if cx >= buf.area.width { break; }
                        let cell = &mut buf[(cx, cy)];
                        cell.set_char(' ');
                        cell.set_bg(Color::Reset);
                        cell.set_fg(Color::Reset);
                    }
                }
            } else {
                // Half-block fallback for non-Kitty terminals
                let pixel_w = self.geom.img_cols as usize;
                let pixel_h = (self.geom.img_rows * 2) as usize;

                let frame_colors = match self.cover_mode {
                    CoverMode::StaticCard => self
                        .cover_art
                        .render_static_frame(pixel_w, pixel_h, self.theme.accent),
                    CoverMode::SpinningVinyl => self
                        .cover_art
                        .render_vinyl_frame(pixel_w, pixel_h, self.rotation_angle, self.theme.accent),
                };

                let inner = self.geom.inner_rect;
                for r in 0..self.geom.img_rows {
                    let py_top = (r * 2) as usize;
                    let py_bot = (r * 2 + 1) as usize;
                    let cy = self.geom.img_y + r;
                    if cy >= inner.y + inner.height { break; }

                    for c in 0..self.geom.img_cols {
                        let cx = self.geom.img_x + c;
                        if cx >= inner.x + inner.width { break; }
                        if py_top < frame_colors.len()
                            && py_bot < frame_colors.len()
                            && (c as usize) < frame_colors[0].len()
                        {
                            let cell = &mut buf[(cx, cy)];
                            cell.set_char('▀');
                            cell.set_fg(frame_colors[py_top][c as usize]);
                            cell.set_bg(frame_colors[py_bot][c as usize]);
                        }
                    }
                }
            }
        }

        // ── 2 & 3. Track title and artist ───────────────────────────────────────
        {
            let (title_str, artist_str) = if let Some(s) = self.song {
                (s.display_title(), s.display_artist())
            } else {
                ("No Track".to_string(), "—".to_string())
            };

            let max_w = area.width.saturating_sub(4) as usize;

            let mut display_title = title_str.to_uppercase();
            if display_title.len() > max_w && max_w > 4 {
                display_title.truncate(max_w - 3);
                display_title.push_str("...");
            }

            let mut display_artist = artist_str.clone();
            if display_artist.len() > max_w && max_w > 4 {
                display_artist.truncate(max_w - 3);
                display_artist.push_str("...");
            }

            Paragraph::new(Line::from(Span::styled(display_title, self.theme.primary_bold())))
                .alignment(Alignment::Center)
                .render(title_area, buf);

            Paragraph::new(Line::from(Span::styled(display_artist, self.theme.secondary())))
                .alignment(Alignment::Center)
                .render(artist_area, buf);
        }

        // ── 5. Scrubber – completely isolated on its own row ────────────────────
        {
            let elapsed  = self.status.elapsed;
            let duration = self.status.duration.max(0.1);
            let progress = (elapsed / duration).clamp(0.0, 1.0);

            let elapsed_str  = format_time(elapsed);
            let duration_str = format_time(duration);

            // Reserve exactly 5 chars for each timestamp + 1 space gap each side
            let time_reserve = (elapsed_str.len() + duration_str.len() + 4) as u16;
            let bar_total = scrubber_area.width.saturating_sub(time_reserve) as usize;
            let bar_total = bar_total.max(4);

            let filled  = ((progress * bar_total as f64).round() as usize).min(bar_total);
            let unfilled = bar_total - filled;

            // Use a thin progress indicator (▬ fill, ─ unfill) so it sits cleanly on one line
            let filled_str   = "▬".repeat(filled);
            let unfilled_str = "─".repeat(unfilled);

            let scrubber_line = Line::from(vec![
                Span::styled(format!("{} ", elapsed_str),  self.theme.dim()),
                Span::styled(filled_str,                   self.theme.accent()),
                Span::styled(unfilled_str,                 self.theme.dim()),
                Span::styled(format!(" {}", duration_str), self.theme.dim()),
            ]);

            Paragraph::new(scrubber_line)
                .alignment(Alignment::Center)
                .render(scrubber_area, buf);
        }

        // ── 7. Controls – 3 rows, perfectly contained in controls_area ──────────
        {
            let now = Instant::now();
            let clicked = self.clicked_button.and_then(|(name, time)| {
                if now.duration_since(time).as_millis() < 350 { Some(name) } else { None }
            });

            let shuffle_on = self.status.random || clicked == Some("shuffle");
            let prev_lit   = clicked == Some("prev");
            let play_lit   = clicked == Some("play");
            let next_lit   = clicked == Some("next");
            let repeat_on  = self.status.repeat || clicked == Some("repeat");

            // ── Style tokens ──────────────────────────────────────────────────
            let bdim   = self.theme.border_style();  // inactive box border
            let tnorm  = Style::default().fg(self.theme.text_primary).bg(Color::Reset);

            // Active toggle buttons (shuffle/repeat): accent-filled box
            let pill_on = Style::default()
                .bg(self.theme.accent)
                .fg(Color::Rgb(15, 15, 25))
                .add_modifier(Modifier::BOLD);

            // Center play/pause button: large solid filled block
            let play_bg = if play_lit { Color::White } else { self.theme.accent };
            let play_fg = if play_lit { self.theme.accent } else { Color::Rgb(10, 10, 20) };
            let play_sty = Style::default()
                .bg(play_bg)
                .fg(play_fg)
                .add_modifier(Modifier::BOLD);

            // Clicked prev/next: momentary accent fill
            let prev_sty = if prev_lit { pill_on } else { tnorm.add_modifier(Modifier::BOLD) };
            let next_sty = if next_lit { pill_on } else { tnorm.add_modifier(Modifier::BOLD) };

            // ── Shuffle [ ⇄ ] ────────────────────────────────────────────────
            let (sht, shm, shb) = if shuffle_on {
                (Span::styled("┌───┐", pill_on),
                 Span::styled("│ ⇄ │", pill_on),
                 Span::styled("└───┘", pill_on))
            } else {
                (Span::styled("┌───┐", bdim),
                 Span::styled("│ ⇄ │", tnorm),
                 Span::styled("└───┘", bdim))
            };

            // ── Repeat [ ↻ ] ─────────────────────────────────────────────────
            let (ret, rem, reb) = if repeat_on {
                (Span::styled("┌───┐", pill_on),
                 Span::styled("│ ↻ │", pill_on),
                 Span::styled("└───┘", pill_on))
            } else {
                (Span::styled("┌───┐", bdim),
                 Span::styled("│ ↻ │", tnorm),
                 Span::styled("└───┘", bdim))
            };

            // ── Previous  |<< ────────────────────────────────────────────────
            let (pvt, pvm, pvb) = (
                Span::raw("     "),
                Span::styled(" |<< ", prev_sty),
                Span::raw("     "),
            );

            // ── Next  >>| ────────────────────────────────────────────────────
            let (nxt, nxm, nxb) = (
                Span::raw("     "),
                Span::styled(" >>| ", next_sty),
                Span::raw("     "),
            );

            // ── Play / Pause – center tall filled block ───────────────────────
            let play_icon = if is_playing { "   ||   " } else { "   ▶    " };
            let (pt, pm, pb) = (
                Span::styled("        ", play_sty),
                Span::styled(play_icon,  play_sty),
                Span::styled("        ", play_sty),
            );

            // ── Spacers between buttons ───────────────────────────────────────
            // We use a single space between groups so the play button stands out
            let sp = Span::raw("  ");

            let row1 = Line::from(vec![sht, sp.clone(), pvt, sp.clone(), pt, sp.clone(), nxt, sp.clone(), ret]);
            let row2 = Line::from(vec![shm, sp.clone(), pvm, sp.clone(), pm, sp.clone(), nxm, sp.clone(), rem]);
            let row3 = Line::from(vec![shb, sp.clone(), pvb, sp.clone(), pb, sp.clone(), nxb, sp,           reb]);

            Paragraph::new(vec![row1, row2, row3])
                .alignment(Alignment::Center)
                .render(controls_area, buf);
        }

        // ── 8. Customisable bottom panel ─────────────────────────────────────────
        TelemetryWidget {
            song: self.song,
            status: self.status,
            theme: self.theme,
            tick_counter: self.tick_counter,
            mode: self.bottom_mode,
            lyrics_mgr: self.lyrics_mgr,
        }
        .render(bottom_area, buf);
    }
}

fn format_time(secs: f64) -> String {
    let total = secs as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}
