use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

pub struct VisualizerWidget<'a> {
    pub is_playing: bool,
    pub tick_counter: u64,
    pub theme: &'a Theme,
    pub sample_rate: &'a str,
}

impl<'a> Widget for VisualizerWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || area.width < 15 {
            return;
        }

        // 1. Top label: ● SIGNAL // LIVE [SYMMETRIC_BARS]    MPD 44.1 kHz
        let dot_style = if self.is_playing {
            self.theme.accent()
        } else {
            self.theme.dim()
        };

        let title_line = Line::from(vec![
            Span::styled(" ● ", dot_style),
            Span::styled("SIGNAL // LIVE [SYMMETRIC_BARS]", self.theme.primary_bold()),
            Span::styled(format!("    {}", self.sample_rate), self.theme.dim()),
        ]);

        let title_area = Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(2),
            height: 1,
        };
        Paragraph::new(title_line).render(title_area, buf);

        // 2. Symmetrical bars waveform
        let wave_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(1),
        };

        let width = wave_area.width as usize;
        let t = self.tick_counter as f32 * 0.12;
        let center = (width as f32) / 2.0;

        let bar_chars = [' ', ' ', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        let mut row_chars: Vec<char> = Vec::with_capacity(width);

        for x in 0..width {
            let dist_from_center = (x as f32 - center).abs() / center;
            // Envelope that peaks in center and tapers toward edges
            let envelope = (1.0 - dist_from_center).clamp(0.0, 1.0).powf(0.8);

            let amp = if self.is_playing {
                let wave1 = (x as f32 * 0.35 + t).sin() * 0.5 + 0.5;
                let wave2 = (x as f32 * 0.18 - t * 1.5).cos() * 0.5 + 0.5;
                let wave3 = (x as f32 * 0.6 + t * 0.7).sin() * 0.5 + 0.5;
                let combined = (wave1 * 0.5 + wave2 * 0.3 + wave3 * 0.2) * envelope;
                combined.clamp(0.0, 1.0)
            } else {
                0.05 * envelope
            };

            let idx = (amp * (bar_chars.len() - 1) as f32).round() as usize;
            row_chars.push(bar_chars[idx.min(bar_chars.len() - 1)]);
        }

        let wave_str: String = row_chars.into_iter().collect();
        let wave_line = Line::from(vec![Span::styled(wave_str, Style::default().fg(self.theme.wave_fg))]);

        Paragraph::new(wave_line).render(wave_area, buf);
    }
}
