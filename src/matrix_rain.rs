use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};

/// Fullwidth double-width characters (width = 2).
/// In terminal emulators (e.g. Kitty), fullwidth characters render at literally
/// double the width and fill a large, prominent font box with bold strokes.
const MATRIX_CHARS: &[char] = &[
    // Fullwidth Numbers (visually twice as wide, massive font presence)
    '０', '１', '２', '３', '４', '５', '６', '７', '８', '９',
    // Fullwidth Symbols (dense, heavy ink fill)
    '＃', '＄', '％', '＆', '＠', '＊', '？', '！', '＾', '＋', '＝', '＜', '＞', '（', '）', '／', '＼',
    // Fullwidth Letters (strong, wide letterforms)
    'ｈ', 'ｋ', 'ｍ', 'ｗ', 'ｘ', 'ｚ', 'ｂ', 'ｄ', 'ｐ', 'ｑ', 'Ｈ', 'Ｋ', 'Ｍ', 'Ｗ', 'Ｘ', 'Ｚ', 'Ｂ', 'Ｒ',
    // Fullwidth Katakana (authentic Matrix aesthetic, large double-cell font)
    'ア', 'ウ', 'エ', 'オ', 'カ', 'キ', 'ケ', 'コ', 'サ', 'シ', 'ス', 'セ', 'ソ',
    'タ', 'ツ', 'テ', 'ナ', 'ニ', 'ヌ', 'ネ', 'ハ', 'ヒ', 'ホ', 'マ', 'ミ', 'ム', 'メ', 'モ',
    'ヤ', 'ユ', 'ラ', 'リ', 'ワ',
];

/// Helper to extract float RGB values from Ratatui Color.
fn color_to_rgb(c: Color) -> (f32, f32, f32) {
    match c {
        Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
        _ => (129.0, 213.0, 205.0),
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
    pub depth: f32,          // 0.35 (faint translucent), 0.65 (mid translucent), 1.0 (vivid)
    pub pulse_offset: usize, // Index along trail of radiant pulse node
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
            density: 0.70,
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

    fn init_drop(seed: &mut u64, height: u16, initial: bool) -> MatrixDrop {
        let r = Self::next_rand(seed);
        // Generous trail lengths so the vertical gradient spans deeply down the screen
        let min_len = 22.max(height as usize / 2);
        let max_len = (height as usize + 24).max(36);
        let span = (max_len.saturating_sub(min_len) + 1).max(1);
        let length = min_len + ((r as usize) % span);

        let mut chars = Vec::with_capacity(length);
        for _ in 0..length {
            chars.push(Self::random_char(seed));
        }

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
            // Stagger drops across vertical rows on initial launch
            let start_y = ((r >> 12) % (height as u64 + 10)) as f32;
            (start_y, 0, (r % 10) < 7)
        } else {
            (0.0, ((r >> 12) % 8) as u16, true)
        };

        // Radiant pulse node along the stream trail
        let pulse_offset = 4 + ((r >> 16) as usize % 7);

        MatrixDrop {
            y,
            speed,
            length,
            chars,
            delay,
            active,
            depth,
            pulse_offset,
        }
    }

    /// Advance drop positions with keyframe easing, single unified wallpaper color,
    /// large fullwidth streams (2 cells wide), and deep continuous vertical trails.
    pub fn update(
        &mut self,
        width: u16,
        height: u16,
        _wallpaper_accent: Color,
        audio_energy: f32,
        dt: f32,
    ) {
        if !self.enabled || width < 4 || height < 4 {
            return;
        }

        // Each stream is 2 terminal columns wide for large double-cell font size
        let total_slots = (width as usize / 2).max(1);
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
        let target_density = 0.65 + audio_energy.clamp(0.0, 1.0) * 0.20;
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

        // Ensure columns vector matches stream slots count
        if self.columns.len() != total_slots {
            let old_len = self.columns.len();
            if old_len < total_slots {
                self.columns.reserve(total_slots - old_len);
                for _ in old_len..total_slots {
                    self.columns.push(Self::init_drop(&mut seed, height, old_len == 0));
                }
            } else {
                self.columns.truncate(total_slots);
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
                col.delay = (Self::next_rand(&mut seed) % 7) as u16;
                let r_depth = Self::next_rand(&mut seed) % 10;
                col.depth = if r_depth < 4 { 0.35 } else if r_depth < 8 { 0.65 } else { 1.00 };

                col.pulse_offset = 4 + (Self::next_rand(&mut seed) as usize % 7);

                let min_len = 22.max(height as usize / 2);
                let max_len = (height as usize + 24).max(36);
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
        let max_active_drops = ((total_slots as f32 * self.density).round() as usize).max(4);
        let active_count = self.columns.iter().filter(|c| c.active).count();

        if active_count < max_active_drops {
            let mut candidates: Vec<usize> = (0..total_slots)
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
                self.columns[col_idx] = Self::init_drop(&mut seed, height, false);
            }
        }

        self.rng_seed = seed;
    }

    /// Render falling matrix glyphs on blank background cells.
    /// Features:
    /// - Double-width fullwidth characters (visually twice as wide, huge font appearance)
    /// - Strictly ONE unified color from the user's wallpaper (no multi-color confusion)
    /// - Massive, expansive Global Illumination (GI) radiance field with silky-smooth cosine gradient
    /// - Long, continuous multi-stage vertical trail gradient
    /// - Strictly protects `cover_box_rect`, `controls_rect`, text cells, and styled button backgrounds
    pub fn render(
        &self,
        buf: &mut Buffer,
        area: Rect,
        cover_box_rect: Rect,
        controls_rect: Rect,
        wallpaper_accent: Color,
    ) {
        if !self.enabled || area.width < 4 || area.height < 4 {
            return;
        }

        let term_w = area.width as usize;
        let term_h = area.height as usize;
        let num_slots = term_w / 2;
        let speed_norm = ((self.speed_multiplier - 0.35) / 1.85).clamp(0.0, 1.0);

        // Single unified wallpaper accent color (e.g. #81d5cd turquoise teal)
        let (r_base, g_base, b_base) = color_to_rgb(wallpaper_accent);

        // ── Phase 1: Collect VFX Light Emitters (Drop Heads + Radiant Pulse Nodes) ──
        let mut emitters = Vec::with_capacity(self.columns.len() * 2);

        for (slot_idx, col) in self.columns.iter().enumerate() {
            if !col.active || col.delay > 0 || slot_idx >= num_slots {
                continue;
            }

            // Center of the 2-cell wide stream
            let x_center = slot_idx * 2 + 1;

            // 1. Drop Head Emitter:
            let head_y = col.y.floor() as isize;
            if head_y >= 0 && head_y < term_h as isize {
                let intensity = if col.depth >= 0.85 {
                    1.30 + speed_norm * 0.60
                } else {
                    0.85 * col.depth
                };
                emitters.push(LightEmitter {
                    x: x_center,
                    y: head_y as usize,
                    r: r_base,
                    g: g_base,
                    b: b_base,
                    intensity,
                    // Expansive light radius (gradient looks much bigger)
                    radius_x: 9.5,
                    radius_y: 15.5,
                });
            }

            // 2. Radiant Pulse Node Emitter:
            if col.pulse_offset < col.length {
                let pulse_y = (col.y - col.pulse_offset as f32).floor() as isize;
                if pulse_y >= 0 && pulse_y < term_h as isize {
                    emitters.push(LightEmitter {
                        x: x_center,
                        y: pulse_y as usize,
                        r: r_base,
                        g: g_base,
                        b: b_base,
                        intensity: 1.05 * col.depth,
                        radius_x: 8.5,
                        radius_y: 13.5,
                    });
                }
            }
        }

        // ── Phase 2: Compute 2D Global Illumination Radiance Field ───────────────
        let grid_size = term_w * term_h;
        let mut light_r = vec![0.0f32; grid_size];
        let mut light_g = vec![0.0f32; grid_size];
        let mut light_b = vec![0.0f32; grid_size];
        let mut light_n = vec![0.0f32; grid_size];

        for em in emitters {
            let rx = em.radius_x;
            let ry = em.radius_y;
            let min_x = em.x.saturating_sub(rx.ceil() as usize);
            let max_x = (em.x + rx.ceil() as usize).min(term_w.saturating_sub(1));
            let min_y = em.y.saturating_sub(ry.ceil() as usize);
            let max_y = (em.y + ry.ceil() as usize).min(term_h.saturating_sub(1));

            for gx in min_x..=max_x {
                let dx = (gx as f32 - em.x as f32).abs() / rx;
                for gy in min_y..=max_y {
                    let dy = (gy as f32 - em.y as f32).abs() / ry;
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq < 1.0 {
                        let dist = dist_sq.sqrt();
                        // Cosine-squared falloff gives a broad, luminous core and a silky-smooth outer gradient
                        let falloff = ((1.0 - dist) * std::f32::consts::FRAC_PI_2).sin().powi(2);
                        let contrib = em.intensity * falloff;
                        let idx = gy * term_w + gx;
                        light_r[idx] += em.r * contrib;
                        light_g[idx] += em.g * contrib;
                        light_b[idx] += em.b * contrib;
                        light_n[idx] += contrib;
                    }
                }
            }
        }

        // ── Phase 3: Render Matrix Rain with Double-Width Glyphs & Expansive Gradient ──
        for (slot_idx, col) in self.columns.iter().enumerate() {
            if !col.active || col.delay > 0 {
                continue;
            }

            let x0 = area.x + (slot_idx * 2) as u16;
            let x1 = x0 + 1;
            if x1 >= area.x + area.width {
                break;
            }

            let head_y = col.y.floor() as isize;

            for (i, &ch) in col.chars.iter().enumerate().take(col.length) {
                let y_pos = head_y - i as isize;
                if y_pos < area.y as isize || y_pos >= (area.y + area.height) as isize {
                    continue;
                }
                let y = y_pos as u16;
                let pos0 = Position { x: x0, y };
                let pos1 = Position { x: x1, y };

                // 1. Cover Box Protection: never draw over album art
                if cover_box_rect.contains(pos0) || cover_box_rect.contains(pos1) {
                    continue;
                }

                // 2. Controls Buttons Protection: never draw over or inside the button boxes
                if controls_rect.contains(pos0) || controls_rect.contains(pos1) {
                    continue;
                }

                let cell0 = &buf[(x0, y)];
                let cell1 = &buf[(x1, y)];

                // 3. Any non-blank cell (text, borders, timestamps, waveforms): never overwrite
                if cell0.symbol() != " " || cell1.symbol() != " " {
                    continue;
                }

                // 4. Any styled background cell (solid play button, badges): never overwrite
                if cell0.bg != Color::Reset || cell1.bg != Color::Reset {
                    continue;
                }

                let grid_idx = (y - area.y) as usize * term_w + (x0 - area.x) as usize;
                let gi_r = light_r.get(grid_idx).copied().unwrap_or(0.0);
                let gi_g = light_g.get(grid_idx).copied().unwrap_or(0.0);
                let gi_b = light_b.get(grid_idx).copied().unwrap_or(0.0);
                let gi_energy = light_n.get(grid_idx).copied().unwrap_or(0.0);

                let is_head = i == 0;
                let is_pulse = i == col.pulse_offset;

                let (r, g, b, modifier) = if is_head {
                    let modifier = Modifier::BOLD;
                    if col.depth >= 0.85 {
                        let flash = 200.0 + speed_norm * 55.0;
                        (
                            ((r_base * 0.50 + flash).min(255.0)) as u8,
                            ((g_base * 0.50 + flash).min(255.0)) as u8,
                            ((b_base * 0.50 + flash).min(255.0)) as u8,
                            modifier,
                        )
                    } else {
                        let factor = (0.70 + col.depth * 0.30).min(1.0);
                        (
                            ((r_base * factor).min(255.0)) as u8,
                            ((g_base * factor).min(255.0)) as u8,
                            ((b_base * factor).min(255.0)) as u8,
                            modifier,
                        )
                    }
                } else if is_pulse {
                    // Luminous pulse node (unified wallpaper accent with extra glow boost)
                    let modifier = Modifier::BOLD;
                    let pulse_boost = 75.0;
                    (
                        ((r_base * 0.85 + pulse_boost).min(255.0)) as u8,
                        ((g_base * 0.85 + pulse_boost).min(255.0)) as u8,
                        ((b_base * 0.85 + pulse_boost).min(255.0)) as u8,
                        modifier,
                    )
                } else {
                    // Trail characters: smooth, expansive gradient spanning up to 40+ rows
                    let t = i as f32 / col.length as f32;
                    let fade = (1.0 - t).max(0.0);
                    let curve = fade.powf(1.4);
                    let max_trail = (0.55 * col.depth).max(0.18);
                    let min_trail = (0.04 * col.depth).max(0.015);
                    let intensity = (curve * max_trail).max(min_trail);

                    let r_trail = r_base * intensity;
                    let g_trail = g_base * intensity;
                    let b_trail = b_base * intensity;

                    // Diffuse Global Illumination: broad soft color wash
                    let gi_bleed = 0.85;
                    let final_r = ((r_trail + gi_r * gi_bleed).min(255.0)) as u8;
                    let final_g = ((g_trail + gi_g * gi_bleed).min(255.0)) as u8;
                    let final_b = ((b_trail + gi_b * gi_bleed).min(255.0)) as u8;

                    let modifier = if i < 10 || gi_energy > 0.12 {
                        Modifier::BOLD
                    } else if col.depth <= 0.35 && i > 15 {
                        Modifier::DIM
                    } else {
                        Modifier::BOLD
                    };

                    (final_r, final_g, final_b, modifier)
                };

                let style = Style::default()
                    .fg(Color::Rgb(r, g, b))
                    .bg(Color::Reset)
                    .add_modifier(modifier);

                // Render double-width fullwidth glyph: sets (x0, y) to ch and (x1, y) to ""
                buf.set_string(x0, y, ch.to_string(), style);
            }
        }
    }
}

