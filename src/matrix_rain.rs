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
    pub active: bool,
}

pub struct MatrixRain {
    pub enabled: bool,
    columns: Vec<MatrixDrop>,
    rng_seed: u64,
    pub smoothed_energy: f32,
}

impl MatrixRain {
    pub fn new() -> Self {
        Self {
            enabled: true,
            columns: Vec::new(),
            rng_seed: 0x9E3779B97F4A7C15,
            smoothed_energy: 0.0,
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
            speed: 0.22 + ((Self::next_rand(seed) % 10) as f32) * 0.025,
            length: 8,
            chars: Vec::new(),
            delay: (Self::next_rand(seed) % 40) as u16,
            active: false,
        }
    }

    /// Advance drop positions, spawn new drops according to audio energy, and maintain uncrowded spacing.
    pub fn update(&mut self, width: u16, height: u16, audio_energy: f32) {
        if !self.enabled || width == 0 || height == 0 {
            return;
        }

        let total_cols = width as usize;
        let mut seed = self.rng_seed;

        // Smoothly interpolate energy for organic, fluid transitions (quick attack, natural gravity decay)
        let target_energy = audio_energy.clamp(0.0, 1.0);
        if target_energy > self.smoothed_energy {
            self.smoothed_energy = self.smoothed_energy * 0.35 + target_energy * 0.65;
        } else {
            self.smoothed_energy = (self.smoothed_energy * 0.88).max(0.0);
        }
        let energy = self.smoothed_energy;

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

            // On initial startup or width change, seed a few spaced drops across the screen
            if old_len == 0 {
                let init_count = ((total_cols as f32 * 0.12).round() as usize).max(3);
                let step = (total_cols / init_count).max(2);
                for i in 0..init_count {
                    let col_idx = (i * step + (Self::next_rand(&mut seed) as usize % 2)).min(total_cols - 1);
                    let start_y = (Self::next_rand(&mut seed) % (height as u64)) as f32;
                    let len = 7 + (Self::next_rand(&mut seed) % 8) as usize;
                    let mut chars = Vec::with_capacity(len);
                    for _ in 0..len {
                        chars.push(Self::random_char(&mut seed));
                    }
                    self.columns[col_idx] = MatrixDrop {
                        y: start_y,
                        speed: 0.22 + ((Self::next_rand(&mut seed) % 10) as f32) * 0.025,
                        length: len,
                        chars,
                        delay: 0,
                        active: true,
                    };
                }
            }
        }

        // 1. Dynamic Speed Multiplier:
        // Huge dynamic range (10x+):
        // Silence/Quiet: 0.35x (~0.08 - 0.14 rows/tick, ~3 rows/sec — gentle, dreamy float)
        // Mid volume:    1.25x (~0.30 - 0.50 rows/tick, ~12 rows/sec — rhythmic flow)
        // Peak beat:     3.20x (~0.75 - 1.35 rows/tick, ~35 rows/sec — rapid digital rush)
        let speed_multiplier = 0.35 + energy.powf(1.25) * 2.85;

        // 2. Advance all currently active drops
        let shimmer_chance = if energy > 0.6 { 4 } else if energy > 0.3 { 8 } else { 16 };

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

            col.y += col.speed * speed_multiplier;

            // When drop head falls past the bottom + trail length, drop has finished
            if col.y - (col.length as f32) > height as f32 {
                col.active = false;
                col.y = 0.0;
                col.chars.clear();
                // Random pause before this column can be picked again (longer pauses when calm)
                let base_pause = if energy < 0.3 { 45 } else if energy < 0.6 { 25 } else { 12 };
                col.delay = base_pause + (Self::next_rand(&mut seed) % 30) as u16;
            } else {
                // Occasional shimmer: mutate random character in trail
                if Self::next_rand(&mut seed) % shimmer_chance == 0 && !col.chars.is_empty() {
                    let idx = (Self::next_rand(&mut seed) as usize) % col.chars.len();
                    col.chars[idx] = Self::random_char(&mut seed);
                }
            }
        }

        // 3. Sound-Reactive Density:
        // Dynamic target for total simultaneously active drops on screen:
        // Quiet / Paused: ~8% to 12% of screen width (spacious, breathable, never crowded!)
        // Heavy Beat:     ~26% to 36% of screen width (energetic cascade with clean gaps!)
        let min_density = 0.08f32;
        let max_density = 0.34f32;
        let target_density = min_density + (max_density - min_density) * energy;
        let max_active_drops = ((total_cols as f32 * target_density).round() as usize).max(3);

        let active_count = self.columns.iter().filter(|c| c.active).count();

        // 4. Spawn new drops if below target active density
        if active_count < max_active_drops {
            let deficit = max_active_drops - active_count;
            // Spawn 1 to 3 drops per tick up to deficit
            let to_spawn = deficit.min(if energy > 0.7 { 3 } else if energy > 0.4 { 2 } else { 1 });

            // Minimum horizontal gap:
            // When calm (energy < 0.45): at least 2 empty columns between any active drops
            // When loud (energy >= 0.45): at least 1 empty column between any active drops
            let min_gap = if energy < 0.45 { 2 } else { 1 };

            let mut candidates: Vec<usize> = (0..total_cols)
                .filter(|&idx| {
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

            // Spawn up to to_spawn drops from eligible candidates
            for _ in 0..to_spawn {
                if candidates.is_empty() {
                    break;
                }
                let pick_idx = (Self::next_rand(&mut seed) as usize) % candidates.len();
                let col_idx = candidates.swap_remove(pick_idx);

                // Sound-reactive trail length:
                // Calm: 6 to 11 chars
                // Beat: 14 to 26 chars
                let base_len = 6 + (Self::next_rand(&mut seed) % 6) as usize;
                let sound_bonus = (energy * 14.0) as usize;
                let length = base_len + sound_bonus;

                let mut chars = Vec::with_capacity(length);
                for _ in 0..length {
                    chars.push(Self::random_char(&mut seed));
                }

                self.columns[col_idx] = MatrixDrop {
                    y: 0.0,
                    speed: 0.22 + ((Self::next_rand(&mut seed) % 10) as f32) * 0.025,
                    length,
                    chars,
                    delay: 0,
                    active: true,
                };

                // Prune any candidates that are now too close to this newly spawned column
                candidates.retain(|&idx| {
                    (idx as isize - col_idx as isize).abs() > min_gap as isize
                });
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

        let energy = self.smoothed_energy.clamp(0.0, 1.0);

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
                    // Head glyph: bright glowing accent, extra flash on beats
                    let flash = 180.0 + energy * 75.0;
                    (
                        ((r_base * 0.7 + flash).min(255.0)) as u8,
                        ((g_base * 0.7 + flash).min(255.0)) as u8,
                        ((b_base * 0.7 + flash).min(255.0)) as u8,
                    )
                } else {
                    // Trail: subtle transparent background glow
                    // Pulses slightly with sound energy (16% to 28% peak)
                    let max_trail = 0.16 + energy * 0.12;
                    let min_trail = 0.05 + energy * 0.03;
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
