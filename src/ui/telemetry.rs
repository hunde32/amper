use crate::mpd_client::{MpdStatus, PlaybackState, SongInfo};
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

pub struct TelemetryWidget<'a> {
    pub song: Option<&'a SongInfo>,
    pub status: &'a MpdStatus,
    pub theme: &'a Theme,
    pub tick_counter: u64,
}

impl<'a> Widget for TelemetryWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 5 || area.width < 25 {
            return;
        }

        let is_playing = self.status.state == PlaybackState::Play;

        // Detect format from filename
        let ext = if let Some(s) = self.song {
            let path = std::path::Path::new(&s.file);
            path.extension()
                .map(|e| e.to_string_lossy().to_uppercase())
                .unwrap_or_else(|| "MP3".to_string())
        } else {
            "MP3".to_string()
        };

        let is_lossless = ext == "FLAC" || ext == "WAV" || ext == "ALAC";
        let loss_tag = if is_lossless { "[LOSSLESS]" } else { "[LOSSY]" };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border_style())
            .title(Span::styled(" ■ AUDIO MONITOR // TELEMETRY ", self.theme.accent()))
            .title_bottom(Span::styled(format!(" {} ", loss_tag), self.theme.accent_secondary()));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 4 {
            return;
        }

        let bit_depth = if is_lossless { "24" } else { "16" };
        let kbps = if is_lossless { "920" } else { "320" };

        let est_size_mb = if let Some(s) = self.song {
            let mins = s.duration / 60.0;
            let mb = mins * if is_lossless { 20.0 } else { 2.4 };
            format!("{:.1}MB", mb.max(1.0))
        } else {
            "7.0MB".to_string()
        };

        // Line 1: FMT: M4A                  BIT: 16
        let line1 = Line::from(vec![
            Span::styled(" FMT: ", self.theme.dim()),
            Span::styled(format!("{:<14}", ext), self.theme.primary_bold()),
            Span::styled("BIT: ", self.theme.dim()),
            Span::styled(bit_depth, self.theme.primary_bold()),
        ]);

        // Line 2: KBPS: 320                 SIZE: 7.0MB
        let line2 = Line::from(vec![
            Span::styled(" KBPS: ", self.theme.dim()),
            Span::styled(format!("{:<13}", kbps), self.theme.primary_bold()),
            Span::styled("SIZE: ", self.theme.dim()),
            Span::styled(est_size_mb, self.theme.primary_bold()),
        ]);

        // Line 3: OUT: PIPEWIRE / PULSE
        let line3 = Line::from(vec![
            Span::styled(" OUT: ", self.theme.dim()),
            Span::styled("PIPEWIRE / PULSEAUDIO", self.theme.secondary()),
        ]);

        // Line 4: Dotted separator
        let sep_width = (inner.width as usize).saturating_sub(2);
        let line4 = Line::from(vec![Span::styled(
            format!(" {}", "·".repeat(sep_width)),
            self.theme.dim(),
        )]);

        // Line 5: Activity dots + Volume bar
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
}
