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
            density: 0.08,
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
            length: 8,
            chars: Vec::new(),
            delay: (Self::next_rand(seed) % 40) as u16,
            active: false,
        }
    }

    /// Advance drop positions with keyframe easing, smooth momentum transitions, and uncrowded spacing.
    pub fn update(&mut self, width: u16, height: u16, audio_energy: f32, dt: f32) {
        if !self.enabled || width == 0 || height == 0 {
            return;
        }

        let total_cols = width as usize;
        let mut seed = self.rng_seed;
        let dt = dt.clamp(0.001, 0.1);

        // 1. Keyframe-Eased Speed Trajectory:
        // When accelerating up: smooth ease-in over ~0.55s.
        // When slowing down: smooth ease-out gliding down over ~1.8s (zero abrupt braking or jitter).
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
        // Quiet/Paused: ~7% of screen width (spacious, wide breathing gaps)
        // High Intensity: ~22% of screen width (flowing code streams with strict 2-column spacing)
        let target_density = 0.07 + audio_energy.clamp(0.0, 1.0) * 0.15;
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
        let max_active_drops = ((total_cols as f32 * self.density).round() as usize).max(3);

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

            // On initial startup or width change, seed a few widely-spaced drops across the screen
            if old_len == 0 {
                let init_count = ((total_cols as f32 * 0.08).round() as usize).max(3);
                let step = (total_cols / init_count).max(4);
                for i in 0..init_count {
                    let col_idx = (i * step).min(total_cols - 1);
                    let start_y = (Self::next_rand(&mut seed) % (height as u64)) as f32;
                    let len = 7 + (Self::next_rand(&mut seed) % 6) as usize;
                    let mut chars = Vec::with_capacity(len);
                    for _ in 0..len {
                        chars.push(Self::random_char(&mut seed));
                    }
                    self.columns[col_idx] = MatrixDrop {
                        y: start_y,
                        speed: 0.22 + ((Self::next_rand(&mut seed) % 10) as f32) * 0.02,
                        length: len,
                        chars,
                        delay: 0,
                        active: true,
                    };
                }
            }
        }

        // 3. Advance all currently active drops using keyframe-eased speed
        for col in self.columns.iter_mut() {
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
                // Random pause before this column can consider re-spawning
                col.delay = 30 + (Self::next_rand(&mut seed) % 50) as u16;
            } else {
                // Occasional subtle shimmer
                if Self::next_rand(&mut seed) % 12 == 0 && !col.chars.is_empty() {
                    let idx = (Self::next_rand(&mut seed) as usize) % col.chars.len();
                    col.chars[idx] = Self::random_char(&mut seed);
                }
            }
        }

        // 4. Spawn new drops if below target active density
        // Enforce strict min_gap = 2 (at least 2 empty columns between any active drops!)
        let active_count = self.columns.iter().filter(|c| c.active).count();
        if active_count < max_active_drops {
            let min_gap = 2usize;

            let candidates: Vec<usize> = (0..total_cols)
                .filter(|&idx| {
                    if self.columns[idx].active || self.columns[idx].delay > 0 {
                        return false;
                    }
                    // Check horizontal spacing: no active drop within 2 columns
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

                // Trail length naturally follows the smooth speed
                let base_len = 6 + (Self::next_rand(&mut seed) % 5) as usize;
                let len_bonus = ((self.speed_multiplier - 0.35) * 5.0) as usize;
                let length = base_len + len_bonus;

                let mut chars = Vec::with_capacity(length);
                for _ in 0..length {
                    chars.push(Self::random_char(&mut seed));
                }

                self.columns[col_idx] = MatrixDrop {
                    y: 0.0,
                    speed: 0.22 + ((Self::next_rand(&mut seed) % 10) as f32) * 0.02,
                    length,
                    chars,
                    delay: 0,
                    active: true,
                };
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

        // Smooth glow intensity derived from eased speed
        let speed_norm = ((self.speed_multiplier - 0.35) / 1.85).clamp(0.0, 1.0);
        let flash = 160.0 + speed_norm * 65.0;
        let (r_head, g_head, b_head) = (
            ((r_base * 0.75 + flash).min(255.0)) as u8,
            ((g_base * 0.75 + flash).min(255.0)) as u8,
            ((b_base * 0.75 + flash).min(255.0)) as u8,
        );

        let max_trail = 0.16 + speed_norm * 0.08;
        let min_trail = 0.05 + speed_norm * 0.02;

        for (x_idx, col) in self.columns.iter().enumerate() {
            if !col.active || col.delay > 0 {
                continue;
            }

            let x = area.x + x_idx as u16;
            if x >= area.x + area.width {
                break;
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
                    (r_head, g_head, b_head)
                } else {
                    let intensity = (fade * max_trail).max(min_trail);
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
