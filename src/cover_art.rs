use base64::Engine;
use image::{codecs::png::PngEncoder, ExtendedColorType, GenericImageView, ImageEncoder, ImageReader, RgbaImage};
use lofty::prelude::*;
use ratatui::style::Color;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct CoverArtManager {
    cached_file: String,
    cached_image: Option<RgbaImage>,
    music_dir: Option<PathBuf>,
    pub supports_kitty: bool,
}

impl CoverArtManager {
    pub fn new(music_dir: Option<PathBuf>) -> Self {
        let supports_kitty = std::env::var("KITTY_WINDOW_ID").is_ok()
            || std::env::var("KITTY_PID").is_ok()
            || std::env::var("TERM").map(|t| t.contains("kitty")).unwrap_or(false);

        Self {
            cached_file: String::new(),
            cached_image: None,
            music_dir,
            supports_kitty,
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

    fn prepare_square_image(img: image::DynamicImage) -> RgbaImage {
        let (w, h) = img.dimensions();
        let size = w.min(h);
        let x_offset = (w - size) / 2;
        let y_offset = (h - size) / 2;
        let cropped = img.crop_imm(x_offset, y_offset, size, size);
        // Resize to 400x400 for high-resolution sampling
        cropped.resize_exact(400, 400, image::imageops::FilterType::Lanczos3).to_rgba8()
    }

    /// Generates a crystal-clear, high-resolution 360x360 PNG of the spinning vinyl record
    pub fn render_highres_vinyl_png(
        &self,
        size_px: u32,
        angle_rad: f32,
        theme_accent: Color,
        _title: &str,
        _artist: &str,
    ) -> Option<Vec<u8>> {
        let mut img = RgbaImage::new(size_px, size_px);
        let cx = (size_px as f32 - 1.0) / 2.0;
        let cy = (size_px as f32 - 1.0) / 2.0;
        let r_outer = cx - 1.5;
        let r_art = r_outer * 0.62;
        let r_spindle = r_outer * 0.085;
        let r_pin = r_outer * 0.035;

        let cos_a = (-angle_rad).cos();
        let sin_a = (-angle_rad).sin();

        let (acc_r, acc_g, acc_b) = match theme_accent {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (193, 193, 255), // fallback light lavender/blue
        };

        for py in 0..size_px {
            let dy = py as f32 - cy;
            for px in 0..size_px {
                let dx = px as f32 - cx;
                let r = (dx * dx + dy * dy).sqrt();

                if r > r_outer {
                    // Fully transparent background outside the disc!
                    continue;
                }

                if r <= r_pin {
                    // Center metallic spindle pin
                    let pin_fade = (1.0 - (r / r_pin)).clamp(0.0, 1.0);
                    let val = (190.0 + pin_fade * 65.0) as u8;
                    img.put_pixel(px, py, image::Rgba([val, val, val, 255]));
                } else if r <= r_spindle {
                    // Spindle hole
                    img.put_pixel(px, py, image::Rgba([12, 12, 16, 255]));
                } else if r <= r_art {
                    // High-res circular label with album cover art (or retro label)
                    if (r - r_art).abs() < 1.2 {
                        // Anti-aliased accent border ring
                        img.put_pixel(px, py, image::Rgba([acc_r, acc_g, acc_b, 255]));
                    } else if let Some(ref cover) = self.cached_image {
                        // Bilinear anti-aliased rotation
                        let rx = dx * cos_a - dy * sin_a;
                        let ry = dx * sin_a + dy * cos_a;

                        let u = (rx / (2.0 * r_art)) + 0.5;
                        let v = (ry / (2.0 * r_art)) + 0.5;

                        if u >= 0.0 && u <= 1.0 && v >= 0.0 && v <= 1.0 {
                            let pixel = sample_bilinear(cover, u, v);
                            img.put_pixel(px, py, image::Rgba(pixel));
                        } else {
                            img.put_pixel(px, py, image::Rgba([20, 20, 26, 255]));
                        }
                    } else {
                        // High-res stylized retro vinyl label matching inspiration image
                        let ring = ((r * 0.45).sin() * 0.5 + 0.5) * 20.0;
                        let base = (25.0 + ring) as u8;
                        let label_r = (base as f32 * 0.9 + acc_r as f32 * 0.15) as u8;
                        let label_g = (base as f32 * 0.9 + acc_g as f32 * 0.15) as u8;
                        let label_b = (base as f32 * 0.9 + acc_b as f32 * 0.25) as u8;
                        img.put_pixel(px, py, image::Rgba([label_r, label_g, label_b, 255]));
                    }
                } else {
                    // High-res vinyl record body with smooth anti-aliased grooves & light sheen
                    // 1. Concentric micro-grooves
                    let groove_factor = ((r * 0.95).sin() * 0.5 + 0.5).powf(1.5);
                    let base_v = 18.0 + groove_factor * 12.0;

                    // 2. Anisotropic light reflection sheen (smooth specular fan at 45 & 225 deg)
                    let phi = dy.atan2(dx);
                    let sheen_angle = 0.7853; // ~45 degrees
                    let delta = phi - sheen_angle;
                    let sheen = delta.cos().abs().powi(12) * 55.0;

                    // 3. Smooth outer edge anti-aliasing
                    let edge_alpha = ((r_outer - r) * 1.5).clamp(0.0, 1.0);

                    let vr = ((base_v + sheen * 0.8) as f32 + acc_r as f32 * 0.08).clamp(0.0, 255.0) as u8;
                    let vg = ((base_v + sheen * 0.8) as f32 + acc_g as f32 * 0.08).clamp(0.0, 255.0) as u8;
                    let vb = ((base_v + sheen * 1.1) as f32 + acc_b as f32 * 0.12).clamp(0.0, 255.0) as u8;
                    let alpha = (255.0 * edge_alpha) as u8;

                    img.put_pixel(px, py, image::Rgba([vr, vg, vb, alpha]));
                }
            }
        }

        // Encode to PNG bytes in memory with fast compression
        let mut png_bytes = Vec::new();
        let encoder = PngEncoder::new_with_quality(
            &mut png_bytes,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::NoFilter,
        );
        encoder.write_image(&img, size_px, size_px, ExtendedColorType::Rgba8).ok()?;

        Some(png_bytes)
    }

    /// Generates a static square high-resolution PNG of the album art (no spinning, crisp and still)
    pub fn render_highres_cover_png(
        &self,
        size_px: u32,
        theme_accent: Color,
        _title: &str,
        _artist: &str,
    ) -> Option<Vec<u8>> {
        let mut img = RgbaImage::new(size_px, size_px);
        let corner_radius = (size_px as f32 * 0.05).max(6.0);

        let (acc_r, acc_g, acc_b) = match theme_accent {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (193, 193, 255),
        };

        let w_f = size_px as f32;
        let h_f = size_px as f32;

        for py in 0..size_px {
            let y = py as f32;
            for px in 0..size_px {
                let x = px as f32;

                // Rounded corner test
                let dx = if x < corner_radius {
                    corner_radius - x
                } else if x > w_f - 1.0 - corner_radius {
                    x - (w_f - 1.0 - corner_radius)
                } else {
                    0.0
                };

                let dy = if y < corner_radius {
                    corner_radius - y
                } else if y > h_f - 1.0 - corner_radius {
                    y - (h_f - 1.0 - corner_radius)
                } else {
                    0.0
                };

                let corner_dist = (dx * dx + dy * dy).sqrt();
                if corner_dist > corner_radius {
                    // Transparent outside rounded corner
                    continue;
                }

                let alpha = (1.0 - (corner_dist - (corner_radius - 1.0)).clamp(0.0, 1.0)) * 255.0;

                // Border edge highlight
                let is_border = x <= 1.5 || x >= w_f - 2.5 || y <= 1.5 || y >= h_f - 2.5
                    || (corner_dist > corner_radius - 2.0);

                if let Some(ref cover) = self.cached_image {
                    let u = x / (w_f - 1.0);
                    let v = y / (h_f - 1.0);
                    let mut pixel = sample_bilinear(cover, u, v);
                    pixel[3] = alpha as u8;

                    if is_border {
                        // Subtle blend with accent border
                        pixel[0] = ((pixel[0] as f32 * 0.7) + (acc_r as f32 * 0.3)) as u8;
                        pixel[1] = ((pixel[1] as f32 * 0.7) + (acc_g as f32 * 0.3)) as u8;
                        pixel[2] = ((pixel[2] as f32 * 0.7) + (acc_b as f32 * 0.3)) as u8;
                    }

                    img.put_pixel(px, py, image::Rgba(pixel));
                } else {
                    // Stylized retro synth placeholder card
                    let grid_line = (px % 24 == 0) || (py % 24 == 0);
                    let base_v = if grid_line { 38u8 } else { 22u8 };
                    let pixel = if is_border {
                        image::Rgba([acc_r, acc_g, acc_b, alpha as u8])
                    } else {
                        image::Rgba([base_v, base_v, base_v + 12, alpha as u8])
                    };
                    img.put_pixel(px, py, pixel);
                }
            }
        }

        let mut png_bytes = Vec::new();
        let encoder = PngEncoder::new_with_quality(
            &mut png_bytes,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::NoFilter,
        );
        encoder.write_image(&img, size_px, size_px, ExtendedColorType::Rgba8).ok()?;

        Some(png_bytes)
    }

    /// Static half-block fallback renderer for non-Kitty terminals
    pub fn render_static_frame(
        &self,
        pixel_width: usize,
        pixel_height: usize,
        theme_accent: Color,
    ) -> Vec<Vec<Color>> {
        let mut grid = vec![vec![Color::Reset; pixel_width]; pixel_height];
        if pixel_width < 4 || pixel_height < 4 {
            return grid;
        }

        let w_f = (pixel_width - 1) as f32;
        let h_f = (pixel_height - 1) as f32;

        for py in 0..pixel_height {
            let v = py as f32 / h_f;
            for px in 0..pixel_width {
                let u = px as f32 / w_f;

                if let Some(ref cover) = self.cached_image {
                    let pixel = sample_bilinear(cover, u, v);
                    grid[py][px] = Color::Rgb(pixel[0], pixel[1], pixel[2]);
                } else {
                    let border = px == 0 || px == pixel_width - 1 || py == 0 || py == pixel_height - 1;
                    if border {
                        grid[py][px] = theme_accent;
                    } else {
                        grid[py][px] = Color::Rgb(25, 25, 35);
                    }
                }
            }
        }

        grid
    }

    /// Half-block fallback renderer (for non-Kitty terminals)
    pub fn render_vinyl_frame(
        &self,
        pixel_width: usize,
        pixel_height: usize,
        angle_rad: f32,
        _theme_accent: Color,
    ) -> Vec<Vec<Color>> {
        let mut grid = vec![vec![Color::Reset; pixel_width]; pixel_height];

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
                    // Transparent
                    continue;
                }

                if r <= r_pin {
                    grid[py][px] = Color::Rgb(220, 220, 240);
                } else if r <= r_spindle {
                    grid[py][px] = Color::Rgb(15, 15, 20);
                } else if r <= r_art {
                    if let Some(ref cover) = self.cached_image {
                        let rx = dx * cos_a - dy * sin_a;
                        let ry = dx * sin_a + dy * cos_a;

                        let u = (rx / (2.0 * r_art)) + 0.5;
                        let v = (ry / (2.0 * r_art)) + 0.5;

                        if u >= 0.0 && u <= 1.0 && v >= 0.0 && v <= 1.0 {
                            let pixel = sample_bilinear(cover, u, v);
                            grid[py][px] = Color::Rgb(pixel[0], pixel[1], pixel[2]);
                        } else {
                            grid[py][px] = Color::Rgb(25, 25, 35);
                        }
                    } else {
                        let ring = ((r * 1.5).sin() * 0.5 + 0.5) * 15.0;
                        let b = (40.0 + ring) as u8;
                        grid[py][px] = Color::Rgb(b, b + 5, b + 20);
                    }
                } else {
                    let groove_factor = ((r * 2.8).sin() * 0.5 + 0.5).powf(1.4);
                    let base_v = 18.0 + groove_factor * 12.0;

                    let phi = dy.atan2(dx);
                    let sheen = (phi - 0.7853).cos().abs().powi(10) * 45.0;

                    let total = (base_v + sheen).clamp(0.0, 255.0) as u8;
                    grid[py][px] = Color::Rgb(total, total, total + 12);
                }
            }
        }

        grid
    }
}

/// Bilinear interpolation sampler for crystal clear rotating images
fn sample_bilinear(img: &RgbaImage, u: f32, v: f32) -> [u8; 4] {
    let w = img.width() as f32;
    let h = img.height() as f32;
    let x = (u * w - 0.5).clamp(0.0, w - 1.0);
    let y = (v * h - 0.5).clamp(0.0, h - 1.0);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(img.width() - 1);
    let y1 = (y0 + 1).min(img.height() - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;

    let p00 = img.get_pixel(x0, y0).0;
    let p10 = img.get_pixel(x1, y0).0;
    let p01 = img.get_pixel(x0, y1).0;
    let p11 = img.get_pixel(x1, y1).0;

    let mut out = [0u8; 4];
    for c in 0..4 {
        let top = p00[c] as f32 * (1.0 - fx) + p10[c] as f32 * fx;
        let bot = p01[c] as f32 * (1.0 - fx) + p11[c] as f32 * fx;
        out[c] = (top * (1.0 - fy) + bot * fy).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// Transmit and display high-resolution image using Kitty Graphics Protocol
pub fn send_kitty_image<W: Write>(
    writer: &mut W,
    png_bytes: &[u8],
    col: u16,
    row: u16,
    width_cells: u16,
    height_cells: u16,
    image_id: u32,
) -> std::io::Result<()> {
    let b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes);

    // Position cursor at 1-indexed (row, col)
    write!(writer, "\x1b[{};{}H", row + 1, col + 1)?;

    let chunk_size = 4096;
    let total = b64.len();
    let mut offset = 0;

    while offset < total {
        let end = (offset + chunk_size).min(total);
        let chunk = &b64[offset..end];
        let is_first = offset == 0;
        let is_last = end == total;
        let m = if is_last { 0 } else { 1 };

        if is_first {
            write!(
                writer,
                "\x1b_Ga=T,f=100,t=d,i={},c={},r={},m={},q=2,C=1;{}\x1b\\",
                image_id, width_cells, height_cells, m, chunk
            )?;
        } else {
            write!(writer, "\x1b_Gm={};{}\x1b\\", m, chunk)?;
        }
        offset = end;
    }
    writer.flush()?;
    Ok(())
}

/// Delete all images displayed via Kitty Graphics Protocol
pub fn clear_kitty_images<W: Write>(writer: &mut W) -> std::io::Result<()> {
    write!(writer, "\x1b_Ga=d,d=A,q=2\x1b\\")?;
    writer.flush()?;
    Ok(())
}
