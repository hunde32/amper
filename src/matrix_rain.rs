use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;

const MATRIX_CHARS: &[char] = &[
    'ｦ', 'ｱ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ',
    'ﾀ', 'ﾂ', 'ﾃ', 'ﾅ', 'ﾆ', 'ﾇ', 'ﾈ', 'ﾊ', 'ﾋ', 'ﾎ', 'ﾏ', 'ﾐ', 'ﾑ', 'ﾒ', 'ﾓ',
    'ﾔ', 'ﾕ', 'ﾗ', 'ﾘ', 'ﾜ', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    ':', '*', '<', '>', '+', '-', '=', '|',
];

/// Critically damped spring smoothing (standard keyframe ease-in-out trajectory).
/// Provides smooth acceleration and deceleration with zero overshoot or abrupt jitter.
fn smooth_damp(
    current: f32,
    target: f32,
    current_velocity: &mut f32,
    smooth_time: f32,
    dt: f32,
) -> f32 {
    let smooth_time = smooth_time.max(0.0001);
    let omega = 2.0 / smooth_time;
    let x = omega * dt;
    let exp = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = current - target;
    let temp = (*current_velocity + omega * change) * dt;
    *current_velocity = (*current_velocity - omega * temp) * exp;
    let mut output = target + (change + temp) * exp;

    // Prevent overshoot past target
    if (target - current > 0.0) == (output > target) {
        output = target;
        *current_velocity = 0.0;
    }
    output
}

#[derive(Clone)]
pub struct MatrixDrop {
    pub y: f32,
    pub speed: f32,
    pub length: usize,
    pub chars: Vec<char>,
    pub delay: u16,
    pub active: bool,
    pub color: Color,
    pub depth: f32, // 0.35 (faint translucent), 0.65 (mid translucent), 1.0 (vivid)
}

pub struct MatrixRain {
    pub enabled: bool,
    columns: Vec<MatrixDrop>,
    rng_seed: u64,
    // Physics-based keyframe easing state:
    pub speed_multiplier: f32,
    speed_velocity: f32,
    pub density: f32,
    density_velocity: f32,
}

impl MatrixRain {
    pub fn new() -> Self {
        Self {
            enabled: true,
            columns: Vec::new(),
            rng_seed: 0x9E3779B97F4A7C15,
            speed_multiplier: 0.35,
            speed_velocity: 0.0,
            density: 0.12,
            density_velocity: 0.0,
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

    fn init_dormant_drop(seed: &mut u64) -> MatrixDrop {
        MatrixDrop {
            y: 0.0,
            speed: 0.22 + ((Self::next_rand(seed) % 10) as f32) * 0.02,
            length: 16,
            chars: Vec::new(),
            delay: (Self::next_rand(seed) % 30) as u16,
            active: false,
            color: Color::Rgb(129, 213, 205),
            depth: 0.65,
        }
    }

    /// Advance drop positions with keyframe easing, multi-color wallpaper palettes,
    /// translucent depth layers, extended row lengths, and complete isolation from UI cards.
    pub fn update(
        &mut self,
        width: u16,
        height: u16,
        card_rect: Rect,
        wallpaper_colors: &[Color],
        audio_energy: f32,
        dt: f32,
    ) {
        if !self.enabled || width == 0 || height == 0 {
            return;
        }

        let total_cols = width as usize;
        let mut seed = self.rng_seed;
        let dt = dt.clamp(0.001, 0.1);

        // 1. Keyframe-Eased Speed Trajectory:
        // Smooth ease-in when accelerating (~0.55s), smooth ease-out when slowing down (~1.80s).
        let target_speed = 0.35 + audio_energy.clamp(0.0, 1.0) * 1.85;
        let speed_smooth_time = if target_speed > self.speed_multiplier {
            0.55
        } else {
            1.80
        };
        self.speed_multiplier = smooth_damp(
            self.speed_multiplier,
            target_speed,
            &mut self.speed_velocity,
            speed_smooth_time,
            dt,
        );

        // 2. Keyframe-Eased Drop Density:
        let target_density = 0.10 + audio_energy.clamp(0.0, 1.0) * 0.20;
        let density_smooth_time = if target_density > self.density {
            0.70
        } else {
            2.00
        };
        self.density = smooth_damp(
            self.density,
            target_density,
            &mut self.density_velocity,
            density_smooth_time,
            dt,
        );

        // Ensure columns vector matches terminal width
        if self.columns.len() != total_cols {
            let old_len = self.columns.len();
            if old_len < total_cols {
                self.columns.reserve(total_cols - old_len);
                for _ in old_len..total_cols {
                    self.columns.push(Self::init_dormant_drop(&mut seed));
                }
            } else {
                self.columns.truncate(total_cols);
            }
        }

        let card_start = card_rect.x as usize;
        let card_end = (card_rect.x + card_rect.width) as usize;

        // 3. Advance all currently active background drops
        for (col_idx, col) in self.columns.iter_mut().enumerate() {
            // Strictly forbid any drop from existing or updating inside the UI card/buttons
            if col_idx >= card_start && col_idx < card_end {
                col.active = false;
                col.delay = 999;
                continue;
            }

            if !col.active {
                if col.delay > 0 {
                    col.delay -= 1;
                }
                continue;
            }

            if col.delay > 0 {
                col.delay -= 1;
                continue;
            }

            col.y += col.speed * self.speed_multiplier;

            // When drop head falls past the bottom + trail length, drop has finished
            if col.y - (col.length as f32) > height as f32 {
                col.active = false;
                col.y = 0.0;
                col.chars.clear();
                // Brief pause before this column can consider re-spawning
                col.delay = (Self::next_rand(&mut seed) % 15) as u16;
            } else {
                // Occasional subtle shimmer in the trail
                if Self::next_rand(&mut seed) % 12 == 0 && !col.chars.is_empty() {
                    let idx = (Self::next_rand(&mut seed) as usize) % col.chars.len();
                    col.chars[idx] = Self::random_char(&mut seed);
                }
            }
        }

        // 4. Calculate background columns available outside card_rect
        let bg_cols_count = (0..total_cols)
            .filter(|&idx| idx < card_start || idx >= card_end)
            .count();
        let max_active_drops = ((bg_cols_count as f32 * self.density).round() as usize).max(2);
        let active_count = self
            .columns
            .iter()
            .enumerate()
            .filter(|&(idx, c)| c.active && (idx < card_start || idx >= card_end))
            .count();

        // 5. Spawn new drops strictly in background columns if below target density
        if active_count < max_active_drops {
            let min_gap = 1usize;

            let candidates: Vec<usize> = (0..total_cols)
                .filter(|&idx| {
                    // Must be strictly outside the UI card
                    if idx >= card_start && idx < card_end {
                        return false;
                    }
                    if self.columns[idx].active || self.columns[idx].delay > 0 {
                        return false;
                    }
                    // Check horizontal spacing: no active drop within min_gap
                    for g in 1..=min_gap {
                        if idx >= g && self.columns[idx - g].active {
                            return false;
                        }
                        if idx + g < total_cols && self.columns[idx + g].active {
                            return false;
                        }
                    }
                    true
                })
                .collect();

            if !candidates.is_empty() {
                let pick_idx = (Self::next_rand(&mut seed) as usize) % candidates.len();
                let col_idx = candidates[pick_idx];

                // Generous trail length so streams have LOTS of rows cascading down
                let min_len = (height as usize / 2).max(12);
                let max_len = (height as usize + 6).max(22);
                let span = (max_len.saturating_sub(min_len) + 1).max(1);
                let length = min_len + (Self::next_rand(&mut seed) as usize % span);

                let mut chars = Vec::with_capacity(length);
                for _ in 0..length {
                    chars.push(Self::random_char(&mut seed));
                }

                // Pick a native color from wallpaper palette
                let color = if !wallpaper_colors.is_empty() {
                    let c_idx = (Self::next_rand(&mut seed) as usize) % wallpaper_colors.len();
                    wallpaper_colors[c_idx]
                } else {
                    Color::Rgb(129, 213, 205)
                };

                // 3 Depth / Translucency tiers:
                // 40% faint translucent background, 40% medium translucent, 20% vivid foreground
                let r_depth = Self::next_rand(&mut seed) % 10;
                let depth = if r_depth < 4 {
                    0.35 // Faint, highly translucent stream
                } else if r_depth < 8 {
                    0.65 // Medium translucent stream
                } else {
                    1.00 // Vivid foreground stream
                };

                self.columns[col_idx] = MatrixDrop {
                    y: 0.0,
                    speed: 0.22 + ((Self::next_rand(&mut seed) % 10) as f32) * 0.02,
                    length,
                    chars,
                    delay: 0,
                    active: true,
                    color,
                    depth,
                };
            }
        }

        self.rng_seed = seed;
    }

    /// Render falling matrix glyphs on blank background cells strictly outside card_rect.
    pub fn render(
        &self,
        buf: &mut Buffer,
        area: Rect,
        card_rect: Rect,
        fallback_accent: Color,
    ) {
        if !self.enabled {
            return;
        }

        let speed_norm = ((self.speed_multiplier - 0.35) / 1.85).clamp(0.0, 1.0);
        let card_start = card_rect.x;
        let card_end = card_rect.x + card_rect.width;

        for (x_idx, col) in self.columns.iter().enumerate() {
            if !col.active || col.delay > 0 {
                continue;
            }

            let x = area.x + x_idx as u16;
            if x >= area.x + area.width {
                break;
            }

            // Absolute guarantee: NEVER draw inside the UI deck width (buttons, controls, cover box)
            if x >= card_start && x < card_end {
                continue;
            }

            let (r_base, g_base, b_base) = match col.color {
                Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
                _ => match fallback_accent {
                    Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
                    _ => (129.0, 213.0, 205.0),
                },
            };

            let head_y = col.y.floor() as isize;

            for (i, &ch) in col.chars.iter().enumerate().take(col.length) {
                let y_pos = head_y - i as isize;
                if y_pos < area.y as isize || y_pos >= (area.y + area.height) as isize {
                    continue;
                }
                let y = y_pos as u16;

                // Extra safety: double-check against card_rect
                if card_rect.contains(Position { x, y }) {
                    continue;
                }

                let cell = &mut buf[(x, y)];
                // Only render where the cell is currently a blank space ' '
                if cell.symbol() != " " {
                    continue;
                }

                // Smooth fade from head to tail
                let fade = 1.0 - (i as f32 / col.length as f32);

                let (r, g, b) = if i == 0 {
                    if col.depth >= 0.85 {
                        let flash = 170.0 + speed_norm * 65.0;
                        (
                            ((r_base * 0.7 + flash).min(255.0)) as u8,
                            ((g_base * 0.7 + flash).min(255.0)) as u8,
                            ((b_base * 0.7 + flash).min(255.0)) as u8,
                        )
                    } else {
                        // Translucent stream head
                        let factor = (0.5 + col.depth * 0.45).min(1.0);
                        (
                            ((r_base * factor).min(255.0)) as u8,
                            ((g_base * factor).min(255.0)) as u8,
                            ((b_base * factor).min(255.0)) as u8,
                        )
                    }
                } else {
                    // Multi-layer translucency: depth 0.35 = faint translucent, 0.65 = mid, 1.0 = vivid
                    let max_trail = (0.24 * col.depth).max(0.06);
                    let min_trail = (0.05 * col.depth).max(0.015);
                    let intensity = (fade * max_trail).max(min_trail);
                    (
                        (r_base * intensity) as u8,
                        (g_base * intensity) as u8,
                        (b_base * intensity) as u8,
                    )
                };

                cell.set_char(ch);
                cell.set_fg(Color::Rgb(r, g, b));
                cell.set_bg(Color::Reset); // 100% background transparency to terminal wallpaper
            }
        }
    }
}
