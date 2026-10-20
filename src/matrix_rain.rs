use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;

const MATRIX_CHARS: &[char] = &[
    'ｦ', 'ｱ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ',
    'ﾀ', 'ﾂ', 'ﾃ', 'ﾅ', 'ﾆ', 'ﾇ', 'ﾈ', 'ﾊ', 'ﾋ', 'ﾎ', 'ﾏ', 'ﾐ', 'ﾑ', 'ﾒ', 'ﾓ',
    'ﾔ', 'ﾕ', 'ﾗ', 'ﾘ', 'ﾜ', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    ':', '*', '<', '>', '+', '-', '=', '|',
];

const CYBERPUNK_PALETTE: &[Color] = &[
    Color::Rgb(129, 213, 205), // Turquoise Teal (Native Wallpaper)
    Color::Rgb(0, 245, 212),   // Electric Neon Cyan
    Color::Rgb(200, 166, 122), // Golden Tan / Amber (Matches screenshot controls)
    Color::Rgb(224, 175, 104), // Warm Gold
    Color::Rgb(80, 200, 120),  // Emerald / Matrix Green
    Color::Rgb(138, 154, 104), // Sage / Olive Green
    Color::Rgb(175, 201, 231), // Sky / Ice Blue (Native Wallpaper)
    Color::Rgb(255, 180, 171), // Coral Rose (Native Wallpaper)
    Color::Rgb(221, 228, 226), // Frosted Mint / White
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
            density: 0.65,
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

    fn pick_color(seed: &mut u64, wallpaper_colors: &[Color]) -> Color {
        let r = Self::next_rand(seed) as usize;
        // Blend native wallpaper colors with vibrant cyberpunk matrix colors
        if !wallpaper_colors.is_empty() && (r % 3 != 0) {
            let idx = r % wallpaper_colors.len();
            wallpaper_colors[idx]
        } else {
            let idx = r % CYBERPUNK_PALETTE.len();
            CYBERPUNK_PALETTE[idx]
        }
    }

    fn init_drop(seed: &mut u64, height: u16, wallpaper_colors: &[Color], initial: bool) -> MatrixDrop {
        let r = Self::next_rand(seed);
        // Extended trail lengths so streams fill plenty of vertical rows from top to bottom!
        let min_len = 16.max(height as usize / 2);
        let max_len = (height as usize + 12).max(28);
        let span = (max_len.saturating_sub(min_len) + 1).max(1);
        let length = min_len + ((r as usize) % span);

        let mut chars = Vec::with_capacity(length);
        for _ in 0..length {
            chars.push(Self::random_char(seed));
        }

        let color = Self::pick_color(seed, wallpaper_colors);

        // 3 Translucency / depth tiers:
        // 40% faint translucent background, 40% medium translucent, 20% vivid foreground
        let r_depth = (r >> 8) % 10;
        let depth = if r_depth < 4 {
            0.35 // Faint translucent background stream
        } else if r_depth < 8 {
            0.65 // Soft medium translucent stream
        } else {
            1.00 // Crisp vivid foreground stream
        };

        let speed = 0.20 + (((r >> 4) % 10) as f32) * 0.02;

        let (y, delay, active) = if initial {
            // Stagger drops across all vertical rows on initial launch
            let start_y = ((r >> 12) % (height as u64 + 10)) as f32;
            (start_y, 0, (r % 10) < 7)
        } else {
            (0.0, ((r >> 12) % 8) as u16, true)
        };

        MatrixDrop {
            y,
            speed,
            length,
            chars,
            delay,
            active,
            color,
            depth,
        }
    }

    /// Advance drop positions with keyframe easing, multi-color wallpaper palettes,
    /// translucent depth layers, and dense continuous full-height rows.
    pub fn update(
        &mut self,
        width: u16,
        height: u16,
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
        // Rich, dense screen coverage (60% to 80% active columns) matching reference design
        let target_density = 0.60 + audio_energy.clamp(0.0, 1.0) * 0.20;
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
                    self.columns.push(Self::init_drop(&mut seed, height, wallpaper_colors, old_len == 0));
                }
            } else {
                self.columns.truncate(total_cols);
            }
        }

        // 3. Advance all active drops
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
                col.y = 0.0;
                // Fast respawn (0 to 6 ticks) so rows are continuously populated with code
                col.delay = (Self::next_rand(&mut seed) % 7) as u16;
                // Re-randomize depth, color, and characters for continuous visual variety
                col.color = Self::pick_color(&mut seed, wallpaper_colors);
                let r_depth = Self::next_rand(&mut seed) % 10;
                col.depth = if r_depth < 4 { 0.35 } else if r_depth < 8 { 0.65 } else { 1.00 };

                let min_len = 16.max(height as usize / 2);
                let max_len = (height as usize + 12).max(28);
                let span = (max_len.saturating_sub(min_len) + 1).max(1);
                col.length = min_len + (Self::next_rand(&mut seed) as usize % span);
                col.chars.clear();
                for _ in 0..col.length {
                    col.chars.push(Self::random_char(&mut seed));
                }
            } else {
                // Occasional subtle shimmer in the trail
                if Self::next_rand(&mut seed) % 12 == 0 && !col.chars.is_empty() {
                    let idx = (Self::next_rand(&mut seed) as usize) % col.chars.len();
                    col.chars[idx] = Self::random_char(&mut seed);
                }
            }
        }

        // 4. Activate dormant columns to meet target density
        let max_active_drops = ((total_cols as f32 * self.density).round() as usize).max(4);
        let active_count = self.columns.iter().filter(|c| c.active).count();

        if active_count < max_active_drops {
            let mut candidates: Vec<usize> = (0..total_cols)
                .filter(|&idx| !self.columns[idx].active && self.columns[idx].delay == 0)
                .collect();

            let deficit = max_active_drops - active_count;
            let to_spawn = deficit.min(3);

            for _ in 0..to_spawn {
                if candidates.is_empty() {
                    break;
                }
                let pick_idx = (Self::next_rand(&mut seed) as usize) % candidates.len();
                let col_idx = candidates.swap_remove(pick_idx);
                self.columns[col_idx] = Self::init_drop(&mut seed, height, wallpaper_colors, false);
            }
        }

        self.rng_seed = seed;
    }

    /// Render falling matrix glyphs on blank background cells.
    /// Strictly protects:
    /// 1. `cover_box_rect` (album art)
    /// 2. `controls_rect` (playback buttons row: play, next, prev, shuffle, repeat)
    /// 3. Non-blank cells (text, borders, timestamps, scrubber)
    /// 4. Non-reset background cells (solid button fills, badges)
    pub fn render(
        &self,
        buf: &mut Buffer,
        area: Rect,
        cover_box_rect: Rect,
        controls_rect: Rect,
        fallback_accent: Color,
    ) {
        if !self.enabled {
            return;
        }

        let speed_norm = ((self.speed_multiplier - 0.35) / 1.85).clamp(0.0, 1.0);

        for (x_idx, col) in self.columns.iter().enumerate() {
            if !col.active || col.delay > 0 {
                continue;
            }

            let x = area.x + x_idx as u16;
            if x >= area.x + area.width {
                break;
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
                let pos = Position { x, y };

                // 1. Cover Box Protection: never draw over album art
                if cover_box_rect.contains(pos) {
                    continue;
                }

                // 2. Controls Buttons Protection: never draw over or inside the button boxes
                if controls_rect.contains(pos) {
                    continue;
                }

                let cell = &mut buf[(x, y)];

                // 3. Any non-blank cell (text, borders, timestamps, waveforms): never overwrite
                if cell.symbol() != " " {
                    continue;
                }

                // 4. Any styled background cell (solid play button, badges): never overwrite
                if cell.bg != Color::Reset {
                    continue;
                }

                // Smooth fade from head to tail
                let fade = 1.0 - (i as f32 / col.length as f32);

                let (r, g, b) = if i == 0 {
                    if col.depth >= 0.85 {
                        let flash = 180.0 + speed_norm * 65.0;
                        (
                            ((r_base * 0.7 + flash).min(255.0)) as u8,
                            ((g_base * 0.7 + flash).min(255.0)) as u8,
                            ((b_base * 0.7 + flash).min(255.0)) as u8,
                        )
                    } else {
                        // Translucent stream head
                        let factor = (0.55 + col.depth * 0.40).min(1.0);
                        (
                            ((r_base * factor).min(255.0)) as u8,
                            ((g_base * factor).min(255.0)) as u8,
                            ((b_base * factor).min(255.0)) as u8,
                        )
                    }
                } else {
                    // Multi-layer translucency: depth 0.35 = faint translucent, 0.65 = mid, 1.0 = vivid
                    let max_trail = (0.28 * col.depth).max(0.08);
                    let min_trail = (0.06 * col.depth).max(0.02);
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
