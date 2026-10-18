use image::{DynamicImage, GenericImageView, ImageReader, RgbaImage};
use lofty::prelude::*;
use ratatui::style::Color;
use std::path::{Path, PathBuf};

pub struct CoverArtManager {
    cached_file: String,
    cached_image: Option<RgbaImage>,
    music_dir: Option<PathBuf>,
}

impl CoverArtManager {
    pub fn new(music_dir: Option<PathBuf>) -> Self {
        Self {
            cached_file: String::new(),
            cached_image: None,
            music_dir,
        }
    }

    pub fn update_current_song(
        &mut self,
        file_uri: &str,
        mpd_client: &mut crate::mpd_client::MpdClient,
    ) {
        if file_uri == self.cached_file && self.cached_image.is_some() {
            return;
        }

        self.cached_file = file_uri.to_string();
        self.cached_image = None;

        if file_uri.is_empty() {
            return;
        }

        // 1. Try local extraction via lofty
        if let Some(music_dir) = &self.music_dir {
            let full_path = music_dir.join(file_uri);
            if full_path.exists() {
                if let Some(img) = Self::extract_from_lofty(&full_path) {
                    self.cached_image = Some(img);
                    return;
                }

                // 2. Try adjacent folder images
                if let Some(parent) = full_path.parent() {
                    let candidates = [
                        "cover.jpg", "cover.png", "folder.jpg", "folder.png",
                        "front.jpg", "front.png", "album.jpg", "album.png",
                    ];
                    for cand in candidates {
                        let cand_path = parent.join(cand);
                        if cand_path.exists() {
                            if let Ok(reader) = ImageReader::open(&cand_path) {
                                if let Ok(dyn_img) = reader.decode() {
                                    self.cached_image = Some(Self::prepare_square_image(dyn_img));
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Try MPD readpicture / albumart protocol
        if let Some(bytes) = mpd_client.read_album_art_from_mpd(file_uri) {
            if let Ok(dyn_img) = image::load_from_memory(&bytes) {
                self.cached_image = Some(Self::prepare_square_image(dyn_img));
            }
        }
    }

    fn extract_from_lofty(path: &Path) -> Option<RgbaImage> {
        let tagged_file = lofty::probe::Probe::open(path).ok()?.read().ok()?;
        let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag())?;
        for pic in tag.pictures() {
            let data = pic.data();
            if let Ok(dyn_img) = image::load_from_memory(data) {
                return Some(Self::prepare_square_image(dyn_img));
            }
        }
        None
    }

    fn prepare_square_image(img: DynamicImage) -> RgbaImage {
        let (w, h) = img.dimensions();
        let size = w.min(h);
        let x_offset = (w - size) / 2;
        let y_offset = (h - size) / 2;
        let cropped = img.crop_imm(x_offset, y_offset, size, size);
        cropped.resize_exact(256, 256, image::imageops::FilterType::Triangle).to_rgba8()
    }

    /// Renders a pixel grid of size width x (height * 2) containing the spinning vinyl record
    pub fn render_vinyl_frame(
        &self,
        pixel_width: usize,
        pixel_height: usize,
        angle_rad: f32,
        card_bg: Color,
        _title: &str,
        _artist: &str,
    ) -> Vec<Vec<Color>> {
        let bg_rgb = match card_bg {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (28, 17, 12),
        };

        let mut grid = vec![vec![Color::Rgb(bg_rgb.0, bg_rgb.1, bg_rgb.2); pixel_width]; pixel_height];

        if pixel_width < 4 || pixel_height < 4 {
            return grid;
        }

        let cx = (pixel_width as f32 - 1.0) / 2.0;
        let cy = (pixel_height as f32 - 1.0) / 2.0;
        let r_outer = (cx.min(cy) - 0.5).max(2.0);
        let r_art = r_outer * 0.62;
        let r_spindle = r_outer * 0.085;
        let r_pin = r_outer * 0.035;

        let cos_a = (-angle_rad).cos();
        let sin_a = (-angle_rad).sin();

        for py in 0..pixel_height {
            let dy = py as f32 - cy;
            for px in 0..pixel_width {
                let dx = px as f32 - cx;
                let r = (dx * dx + dy * dy).sqrt();

                if r > r_outer {
                    // Background outside vinyl
                    continue;
                }

                if r <= r_pin {
                    // Turntable center spindle pin (metallic silver/gold)
                    grid[py][px] = Color::Rgb(215, 195, 175);
                } else if r <= r_spindle {
                    // Spindle hole (dark hole)
                    grid[py][px] = Color::Rgb(10, 6, 4);
                } else if r <= r_art {
                    // Circular label with album cover art (or retro label)
                    if (r - r_art).abs() < 0.9 {
                        // Bronze label border ring
                        grid[py][px] = Color::Rgb(140, 95, 65);
                    } else if let Some(ref cover) = self.cached_image {
                        // Sample rotated album art
                        let rx = dx * cos_a - dy * sin_a;
                        let ry = dx * sin_a + dy * cos_a;

                        let u = (rx / (2.0 * r_art)) + 0.5;
                        let v = (ry / (2.0 * r_art)) + 0.5;

                        if u >= 0.0 && u <= 1.0 && v >= 0.0 && v <= 1.0 {
                            let img_x = ((u * 255.0).clamp(0.0, 255.0)) as u32;
                            let img_y = ((v * 255.0).clamp(0.0, 255.0)) as u32;
                            let pixel = cover.get_pixel(img_x, img_y);
                            grid[py][px] = Color::Rgb(pixel[0], pixel[1], pixel[2]);
                        } else {
                            grid[py][px] = Color::Rgb(40, 25, 18);
                        }
                    } else {
                        // Procedural retro vinyl label when no album art is available
                        let ring = ((r * 1.5).sin() * 0.5 + 0.5) * 15.0;
                        let label_base = (60.0 + ring) as u8;
                        grid[py][px] = Color::Rgb(label_base + 35, label_base + 10, label_base - 10);
                    }
                } else {
                    // Vinyl record body with grooves and anisotropic light reflections!
                    // 1. Concentric micro-grooves
                    let groove_factor = ((r * 2.8).sin() * 0.5 + 0.5).powf(1.4);
                    let base_v = 15.0 + groove_factor * 12.0;

                    // 2. Anisotropic reflection sheen (real vinyl reflects light at ~45 deg)
                    let phi = dy.atan2(dx);
                    let sheen_angle = 0.7853; // ~45 degrees
                    let delta = phi - sheen_angle;
                    let sheen = delta.cos().abs().powi(10) * 45.0;

                    // 3. Outer rim highlight
                    let rim_boost = if r > r_outer - 1.2 { 30.0 } else { 0.0 };

                    let total_r = (base_v + sheen * 1.1 + rim_boost + 5.0).clamp(0.0, 255.0) as u8;
                    let total_g = (base_v * 0.7 + sheen * 0.75 + rim_boost * 0.7 + 2.0).clamp(0.0, 255.0) as u8;
                    let total_b = (base_v * 0.5 + sheen * 0.5 + rim_boost * 0.5).clamp(0.0, 255.0) as u8;

                    grid[py][px] = Color::Rgb(total_r, total_g, total_b);
                }
            }
        }

        grid
    }
}
