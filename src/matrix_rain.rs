use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier};

const MATRIX_CHARS: &[char] = &[
    // Bold numbers (from user example: 8, 3, 2, 1, 0...)
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    // Dense ASCII symbols (from user example: $, #, %, (, !, ), ^, &, *, @)
    '$', '#', '%', '&', '@', '?', '!', '^', '*', '+', '=', '<', '>', '~', '/', '\\', '(', ')',
    // Strong dense letters (from user example: h, 3, ...)
    'h', 'k', 'm', 'w', 'x', 'z', 'b', 'd', 'p', 'q', 'H', 'K', 'M', 'W', 'X', 'Z', 'B', 'R',
    // Classic half-width Katakana
    'ｦ', 'ｱ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ',
    'ﾀ', 'ﾂ', 'ﾃ', 'ﾅ', 'ﾆ', 'ﾇ', 'ﾈ', 'ﾊ', 'ﾋ', 'ﾎ', 'ﾏ', 'ﾐ', 'ﾑ', 'ﾒ', 'ﾓ',
    'ﾔ', 'ﾕ', 'ﾗ', 'ﾘ', 'ﾜ',
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

const PULSE_PALETTE: &[Color] = &[
    Color::Rgb(0, 190, 255),   // Electric Cyan-Blue (from user example)
    Color::Rgb(0, 245, 212),   // Neon Cyan
    Color::Rgb(255, 190, 70),  // Warm Radiant Gold
    Color::Rgb(255, 110, 180), // Neon Rose / Hot Pink
    Color::Rgb(60, 255, 160),  // Electric Matrix Mint
    Color::Rgb(185, 130, 255), // Radiant Violet
    Color::Rgb(255, 225, 120), // Vivid Champagne
];

/// Helper to extract float RGB values from Ratatui Color.
fn color_to_rgb(c: Color, fallback: Color) -> (f32, f32, f32) {
    match c {
        Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
        _ => match fallback {
            Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
            _ => (129.0, 213.0, 205.0),
        },
    }
}

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
    pub pulse_offset: usize, // Index along trail of radiant pulse node
    pub pulse_color: Color,  // Vivid color for the pulse node (e.g. Electric Blue)
}

struct LightEmitter {
    x: usize,
    y: usize,
    r: f32,
    g: f32,
    b: f32,
    intensity: f32,
    radius_x: f32,
    radius_y: f32,
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
        // Extended trail lengths so streams fill plenty of vertical rows from top to bottom
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

        // Radiant pulse node along the stream trail (acts as a localized VFX point light emitter)
        let pulse_offset = 3 + ((r >> 16) as usize % 6);
        let pulse_idx = ((r >> 20) as usize) % PULSE_PALETTE.len();
        let pulse_color = PULSE_PALETTE[pulse_idx];

        MatrixDrop {
            y,
            speed,
            length,
            chars,
            delay,
            active,
            color,
            depth,
            pulse_offset,
            pulse_color,
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
                // Re-randomize depth, color, pulse node, and characters for continuous visual variety
                col.color = Self::pick_color(&mut seed, wallpaper_colors);
                let r_depth = Self::next_rand(&mut seed) % 10;
                col.depth = if r_depth < 4 { 0.35 } else if r_depth < 8 { 0.65 } else { 1.00 };

                col.pulse_offset = 3 + (Self::next_rand(&mut seed) as usize % 6);
                let p_idx = (Self::next_rand(&mut seed) as usize) % PULSE_PALETTE.len();
                col.pulse_color = PULSE_PALETTE[p_idx];

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
    /// Features:
    /// - 2D VFX Global Illumination (GI) radiance field with chromatic bloom and smooth distance falloff
    /// - Radiant pulse nodes (e.g. Electric Blue point lights illuminating surrounding characters)
    /// - Bold typography modifier for head and illuminated characters (crisper, larger visual font weight)
    /// - Strictly protects `cover_box_rect`, `controls_rect`, text cells, and styled button backgrounds
    pub fn render(
        &self,
        buf: &mut Buffer,
        area: Rect,
        cover_box_rect: Rect,
        controls_rect: Rect,
        fallback_accent: Color,
    ) {
        if !self.enabled || area.width == 0 || area.height == 0 {
            return;
        }

        let width = area.width as usize;
        let height = area.height as usize;
        let speed_norm = ((self.speed_multiplier - 0.35) / 1.85).clamp(0.0, 1.0);

        // ── Phase 1: Collect all VFX Light Emitters (Drop Heads + Radiant Pulse Nodes) ──
        let mut emitters = Vec::with_capacity(self.columns.len() * 2);

        for (x_idx, col) in self.columns.iter().enumerate() {
            if !col.active || col.delay > 0 || x_idx >= width {
                continue;
            }

            let (r_base, g_base, b_base) = color_to_rgb(col.color, fallback_accent);

            // 1. Drop Head Emitter:
            let head_y = col.y.floor() as isize;
            if head_y >= 0 && head_y < height as isize {
                let intensity = if col.depth >= 0.85 {
                    0.95 + speed_norm * 0.45
                } else {
                    0.60 * col.depth
                };
                emitters.push(LightEmitter {
                    x: x_idx,
                    y: head_y as usize,
                    r: r_base,
                    g: g_base,
                    b: b_base,
                    intensity,
                    radius_x: 2.6,
                    radius_y: 4.0,
                });
            }

            // 2. Radiant Pulse Node Emitter (like the blue '2' in the reference example):
            if col.pulse_offset < col.length {
                let pulse_y = (col.y - col.pulse_offset as f32).floor() as isize;
                if pulse_y >= 0 && pulse_y < height as isize {
                    let (pr, pg, pb) = color_to_rgb(col.pulse_color, fallback_accent);
                    emitters.push(LightEmitter {
                        x: x_idx,
                        y: pulse_y as usize,
                        r: pr,
                        g: pg,
                        b: pb,
                        intensity: 0.90 * col.depth,
                        radius_x: 2.5,
                        radius_y: 3.8,
                    });
                }
            }
        }

        // ── Phase 2: Compute 2D Global Illumination Radiance Field ───────────────
        let grid_size = width * height;
        let mut light_r = vec![0.0f32; grid_size];
        let mut light_g = vec![0.0f32; grid_size];
        let mut light_b = vec![0.0f32; grid_size];
        let mut light_n = vec![0.0f32; grid_size];

        for em in emitters {
            let rx = em.radius_x;
            let ry = em.radius_y;
            let min_x = em.x.saturating_sub(rx.ceil() as usize);
            let max_x = (em.x + rx.ceil() as usize).min(width.saturating_sub(1));
            let min_y = em.y.saturating_sub(ry.ceil() as usize);
            let max_y = (em.y + ry.ceil() as usize).min(height.saturating_sub(1));

            for gx in min_x..=max_x {
                let dx = (gx as f32 - em.x as f32).abs() / rx;
                for gy in min_y..=max_y {
                    let dy = (gy as f32 - em.y as f32).abs() / ry;
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq < 1.0 {
                        let dist = dist_sq.sqrt();
                        // Smooth quadratic falloff (VFX light attenuation)
                        let falloff = (1.0 - dist) * (1.0 - dist);
                        let contrib = em.intensity * falloff;
                        let idx = gy * width + gx;
                        light_r[idx] += em.r * contrib;
                        light_g[idx] += em.g * contrib;
                        light_b[idx] += em.b * contrib;
                        light_n[idx] += contrib;
                    }
                }
            }
        }

        // ── Phase 3: Render Matrix Rain with Diffuse Color Bleed & Bold Weights ───
        for (x_idx, col) in self.columns.iter().enumerate() {
            if !col.active || col.delay > 0 {
                continue;
            }

            let x = area.x + x_idx as u16;
            if x >= area.x + area.width {
                break;
            }

            let (r_base, g_base, b_base) = color_to_rgb(col.color, fallback_accent);
            let (pr, pg, pb) = color_to_rgb(col.pulse_color, fallback_accent);
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

                let grid_idx = (y - area.y) as usize * width + (x - area.x) as usize;
                let gi_r = light_r.get(grid_idx).copied().unwrap_or(0.0);
                let gi_g = light_g.get(grid_idx).copied().unwrap_or(0.0);
                let gi_b = light_b.get(grid_idx).copied().unwrap_or(0.0);
                let gi_energy = light_n.get(grid_idx).copied().unwrap_or(0.0);

                let is_head = i == 0;
                let is_pulse = i == col.pulse_offset;

                let (r, g, b, modifier) = if is_head {
                    let modifier = Modifier::BOLD;
                    if col.depth >= 0.85 {
                        let flash = 185.0 + speed_norm * 65.0;
                        (
                            ((r_base * 0.7 + flash).min(255.0)) as u8,
                            ((g_base * 0.7 + flash).min(255.0)) as u8,
                            ((b_base * 0.7 + flash).min(255.0)) as u8,
                            modifier,
                        )
                    } else {
                        let factor = (0.60 + col.depth * 0.40).min(1.0);
                        (
                            ((r_base * factor).min(255.0)) as u8,
                            ((g_base * factor).min(255.0)) as u8,
                            ((b_base * factor).min(255.0)) as u8,
                            modifier,
                        )
                    }
                } else if is_pulse {
                    // Luminous pulse node: vivid color (e.g. Electric Blue '2')
                    let modifier = Modifier::BOLD;
                    (
                        ((pr * 0.85 + 40.0).min(255.0)) as u8,
                        ((pg * 0.85 + 40.0).min(255.0)) as u8,
                        ((pb * 0.85 + 40.0).min(255.0)) as u8,
                        modifier,
                    )
                } else {
                    // Trail characters: smooth fade from head to tail + Global Illumination bleed
                    let fade = 1.0 - (i as f32 / col.length as f32);
                    let max_trail = (0.28 * col.depth).max(0.08);
                    let min_trail = (0.06 * col.depth).max(0.02);
                    let intensity = (fade * max_trail).max(min_trail);

                    let r_trail = r_base * intensity;
                    let g_trail = g_base * intensity;
                    let b_trail = b_base * intensity;

                    // Diffuse Global Illumination: smooth color wash blending into base color
                    let gi_bleed = 0.75;
                    let final_r = ((r_trail + gi_r * gi_bleed).min(255.0)) as u8;
                    let final_g = ((g_trail + gi_g * gi_bleed).min(255.0)) as u8;
                    let final_b = ((b_trail + gi_b * gi_bleed).min(255.0)) as u8;

                    // Typographic bold weighting when illuminated by GI, giving a crisper/larger font feel
                    let modifier = if gi_energy > 0.22 {
                        Modifier::BOLD
                    } else if col.depth <= 0.35 && gi_energy < 0.08 && i > 3 {
                        Modifier::DIM
                    } else {
                        Modifier::empty()
                    };

                    (final_r, final_g, final_b, modifier)
                };

                cell.set_char(ch);
                cell.set_fg(Color::Rgb(r, g, b));
                cell.set_bg(Color::Reset); // 100% background transparency to terminal wallpaper
                cell.modifier = modifier;
            }
        }
    }
}

