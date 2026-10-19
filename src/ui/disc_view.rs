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
                Constraint::Length(2), // 0  visualizer (perfect waveform banner)
                Constraint::Min(6),    // 1  cover box
                Constraint::Length(1), // 2  title
                Constraint::Length(1), // 3  artist
                Constraint::Length(1), // 4  spacer
                Constraint::Length(1), // 5  scrubber
                Constraint::Length(1), // 6  spacer
                Constraint::Length(3), // 7  controls (3 explicit rows)
                Constraint::Length(1), // 8  spacer between controls and lower telemetry
                Constraint::Length(6), // 9  customisable bottom panel (telemetry)
            ])
            .split(area);

        let viz_area        = chunks[0];
        let _cover_box_area = chunks[1];
        let title_area      = chunks[2];
        let artist_area     = chunks[3];
        // chunks[4] is the spacer row
        let scrubber_area   = chunks[5];
        // chunks[6] is the spacer row
        let controls_area   = chunks[7];
        // chunks[8] is the spacer row between controls and lower telemetry
        let bottom_area     = chunks[9];

        // ── 0. Visualizer (preserved exactly as user requested) ───────────────────
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

        // ── 7. Controls – 3 rows, beautifully boxed and perfectly centered ──────
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
            let border_inactive = self.theme.dim();
            let border_active   = Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD);
            let border_clicked  = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);

            let icon_dim   = Style::default().fg(self.theme.text_secondary);
            let icon_lit   = Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD);
            let icon_click = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);
            let icon_norm  = Style::default().fg(self.theme.text_primary).add_modifier(Modifier::BOLD);

            // 1. Shuffle Button [ ⇄ ] (5 cols: ┌───┐ / │ ⇄ │ / └───┘)
            let (shuf_b_sty, shuf_i_sty) = if clicked == Some("shuffle") {
                (border_clicked, icon_click)
            } else if shuffle_on {
                (border_active, icon_lit)
            } else {
                (border_inactive, icon_dim)
            };
            let shuf_t = Span::styled("┌───┐", shuf_b_sty);
            let shuf_m = vec![
                Span::styled("│", shuf_b_sty),
                Span::styled(" ⇄ ", shuf_i_sty),
                Span::styled("│", shuf_b_sty),
            ];
            let shuf_b = Span::styled("└───┘", shuf_b_sty);

            // 2. Previous Button [ |<< ] (7 cols: ┌─────┐ / │ |<< │ / └─────┘)
            let (prev_b_sty, prev_i_sty) = if prev_lit {
                (border_clicked, icon_click)
            } else {
                (border_inactive, icon_norm)
            };
            let prev_t = Span::styled("┌─────┐", prev_b_sty);
            let prev_m = vec![
                Span::styled("│", prev_b_sty),
                Span::styled(" |<< ", prev_i_sty),
                Span::styled("│", prev_b_sty),
            ];
            let prev_b = Span::styled("└─────┘", prev_b_sty);

            // 3. Play / Pause Button (9 cols: Solid accent block with centered glyph)
            let play_bg = if play_lit { Color::White } else { self.theme.accent };
            let play_fg = if play_lit { self.theme.accent } else { Color::Rgb(10, 10, 20) };
            let play_sty = Style::default()
                .bg(play_bg)
                .fg(play_fg)
                .add_modifier(Modifier::BOLD);

            let play_icon = if is_playing { "   | |   " } else { "    ▶    " };
            let play_t = Span::styled("         ", play_sty);
            let play_m = Span::styled(play_icon,   play_sty);
            let play_b = Span::styled("         ", play_sty);

            // 4. Next Button [ >>| ] (7 cols: ┌─────┐ / │ >>| │ / └─────┘)
            let (next_b_sty, next_i_sty) = if next_lit {
                (border_clicked, icon_click)
            } else {
                (border_inactive, icon_norm)
            };
            let next_t = Span::styled("┌─────┐", next_b_sty);
            let next_m = vec![
                Span::styled("│", next_b_sty),
                Span::styled(" >>| ", next_i_sty),
                Span::styled("│", next_b_sty),
            ];
            let next_b = Span::styled("└─────┘", next_b_sty);

            // 5. Repeat Button [ ↻ ] (5 cols: ┌───┐ / │ ↻ │ / └───┘)
            let (rep_b_sty, rep_i_sty) = if clicked == Some("repeat") {
                (border_clicked, icon_click)
            } else if repeat_on {
                (border_active, icon_lit)
            } else {
                (border_inactive, icon_dim)
            };
            let rep_t = Span::styled("┌───┐", rep_b_sty);
            let rep_m = vec![
                Span::styled("│", rep_b_sty),
                Span::styled(" ↻ ", rep_i_sty),
                Span::styled("│", rep_b_sty),
            ];
            let rep_b = Span::styled("└───┘", rep_b_sty);

            let sp = Span::raw("  ");

            let mut row1_spans = Vec::with_capacity(9);
            row1_spans.push(shuf_t);
            row1_spans.push(sp.clone());
            row1_spans.push(prev_t);
            row1_spans.push(sp.clone());
            row1_spans.push(play_t);
            row1_spans.push(sp.clone());
            row1_spans.push(next_t);
            row1_spans.push(sp.clone());
            row1_spans.push(rep_t);
            let row1 = Line::from(row1_spans);

            let mut row2_spans = Vec::with_capacity(17);
            row2_spans.extend(shuf_m);
            row2_spans.push(sp.clone());
            row2_spans.extend(prev_m);
            row2_spans.push(sp.clone());
            row2_spans.push(play_m);
            row2_spans.push(sp.clone());
            row2_spans.extend(next_m);
            row2_spans.push(sp.clone());
            row2_spans.extend(rep_m);
            let row2 = Line::from(row2_spans);

            let mut row3_spans = Vec::with_capacity(9);
            row3_spans.push(shuf_b);
            row3_spans.push(sp.clone());
            row3_spans.push(prev_b);
            row3_spans.push(sp.clone());
            row3_spans.push(play_b);
            row3_spans.push(sp.clone());
            row3_spans.push(next_b);
            row3_spans.push(sp);
            row3_spans.push(rep_b);
            let row3 = Line::from(row3_spans);

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
