use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;

const MATRIX_CHARS: &[char] = &[
    'ｦ', 'ｱ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ',
    'ﾀ', 'ﾂ', 'ﾃ', 'ﾅ', 'ﾆ', 'ﾇ', 'ﾈ', 'ﾊ', 'ﾋ', 'ﾎ', 'ﾏ', 'ﾐ', 'ﾑ', 'ﾒ', 'ﾓ',
    'ﾔ', 'ﾕ', 'ﾗ', 'ﾘ', 'ﾜ', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    ':', '*', '<', '>', '+', '-', '=', '|',
];

#[derive(Clone)]
pub struct MatrixDrop {
    pub y: f32,
    pub speed: f32,
    pub length: usize,
    pub chars: Vec<char>,
    pub delay: u16,
}

pub struct MatrixRain {
    pub enabled: bool,
    columns: Vec<MatrixDrop>,
    rng_seed: u64,
}

impl MatrixRain {
    pub fn new() -> Self {
        Self {
            enabled: true,
            columns: Vec::new(),
            rng_seed: 0x9E3779B97F4A7C15,
        }
    }

    fn next_rand(seed: &mut u64) -> u64 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *seed
    }

    fn random_char(seed: &mut u64) -> char {
        let idx = (Self::next_rand(seed) as usize) % MATRIX_CHARS.len();
        MATRIX_CHARS[idx]
    }

    fn new_drop(seed: &mut u64, height: u16, initial: bool) -> MatrixDrop {
        let r = Self::next_rand(seed);
        let speed = 0.25 + ((r % 10) as f32) * 0.04; // 0.25 to 0.65 rows/tick
        let length = 7 + ((r >> 4) % 15) as usize;   // 7 to 21 chars trail
        let mut chars = Vec::with_capacity(length);
        for _ in 0..length {
            chars.push(Self::random_char(seed));
        }

        // On initial startup, stagger drops across the screen
        let (y, delay) = if initial {
            let start_y = ((r >> 8) % (height as u64 + 10)) as f32;
            (start_y, 0)
        } else {
            let delay = ((r >> 12) % 60) as u16;
            (0.0, delay)
        };

        MatrixDrop {
            y,
            speed,
            length,
            chars,
            delay,
        }
    }

    /// Advance drop positions and mutate characters.
    pub fn update(&mut self, width: u16, height: u16, audio_energy: f32) {
        if !self.enabled || width == 0 || height == 0 {
            return;
        }

        let needed_cols = width as usize;
        let mut seed = self.rng_seed;

        if self.columns.len() < needed_cols {
            let start = self.columns.len();
            for _ in start..needed_cols {
                let drop = Self::new_drop(&mut seed, height, true);
                self.columns.push(drop);
            }
        }

        // Speed boost with audio energy for reactive rhythm
        let speed_multiplier = 1.0 + audio_energy.clamp(0.0, 1.0) * 0.35;

        for col in self.columns.iter_mut() {
            if col.delay > 0 {
                col.delay -= 1;
                continue;
            }

            col.y += col.speed * speed_multiplier;

            // When a drop falls past the bottom of the screen, reset it to the top
            if col.y - (col.length as f32) > height as f32 {
                col.y = 0.0;
                col.delay = (Self::next_rand(&mut seed) % 50) as u16;
                col.speed = 0.25 + ((Self::next_rand(&mut seed) % 10) as f32) * 0.04;
                col.length = 7 + ((Self::next_rand(&mut seed) >> 4) % 15) as usize;
                col.chars.clear();
                for _ in 0..col.length {
                    col.chars.push(Self::random_char(&mut seed));
                }
            } else {
                // Occasional shimmer: mutate random character in trail
                if Self::next_rand(&mut seed) % 8 == 0 && !col.chars.is_empty() {
                    let idx = (Self::next_rand(&mut seed) as usize) % col.chars.len();
                    col.chars[idx] = Self::random_char(&mut seed);
                }
            }
        }

        self.rng_seed = seed;
    }

    /// Render falling matrix glyphs on blank background cells.
    pub fn render(
        &self,
        buf: &mut Buffer,
        area: Rect,
        cover_box_rect: Rect,
        accent: Color,
    ) {
        if !self.enabled {
            return;
        }

        let (r_base, g_base, b_base) = match accent {
            Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
            _ => (40.0, 220.0, 150.0), // fallback cyber green
        };

        for (x_idx, col) in self.columns.iter().enumerate() {
            let x = area.x + x_idx as u16;
            if x >= area.x + area.width {
                break;
            }

            if col.delay > 0 {
                continue;
            }

            let head_y = col.y.floor() as isize;

            for (i, &ch) in col.chars.iter().enumerate().take(col.length) {
                let y_pos = head_y - i as isize;
                if y_pos < area.y as isize || y_pos >= (area.y + area.height) as isize {
                    continue;
                }
                let y = y_pos as u16;

                // Don't draw over the album art container
                if cover_box_rect.contains(Position { x, y }) {
                    continue;
                }

                let cell = &mut buf[(x, y)];
                // Only render where the cell is currently a blank space ' '
                // This ensures UI elements, text, and borders stay clean on top!
                if cell.symbol() != " " {
                    continue;
                }

                // Smooth fade from head to tail
                let fade = 1.0 - (i as f32 / col.length as f32);

                let (r, g, b) = if i == 0 {
                    // Head glyph: bright glowing accent
                    (
                        ((r_base * 0.8 + 255.0 * 0.4).min(255.0)) as u8,
                        ((g_base * 0.8 + 255.0 * 0.4).min(255.0)) as u8,
                        ((b_base * 0.8 + 255.0 * 0.4).min(255.0)) as u8,
                    )
                } else {
                    // Trail: subtle transparent background glow (10% - 28% brightness)
                    let intensity = (fade * 0.28).max(0.08);
                    (
                        (r_base * intensity) as u8,
                        (g_base * intensity) as u8,
                        (b_base * intensity) as u8,
                    )
                };

                cell.set_char(ch);
                cell.set_fg(Color::Rgb(r, g, b));
                cell.set_bg(Color::Reset);
            }
        }
    }
}
