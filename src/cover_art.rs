use base64::Engine;
use image::{
    codecs::png::PngEncoder, ExtendedColorType, GenericImageView, ImageEncoder, ImageReader,
    RgbaImage,
};
use lofty::prelude::*;
use rayon::prelude::*;
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
            || std::env::var("TERM")
                .map(|t| t.contains("kitty"))
                .unwrap_or(false);

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

        // 1. Try local extraction via lofty (embedded tag art)
        if let Some(music_dir) = &self.music_dir {
            let full_path = music_dir.join(file_uri);
            if full_path.exists() {
                if let Some(img) = Self::extract_from_lofty(&full_path) {
                    self.cached_image = Some(img);
                    return;
                }

                // 2. Try adjacent folder images (cover.jpg, folder.jpg, etc.)
                if let Some(parent) = full_path.parent() {
                    let candidates = [
                        "cover.jpg",
                        "cover.png",
                        "folder.jpg",
                        "folder.png",
                        "front.jpg",
                        "front.png",
                        "album.jpg",
                        "album.png",
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
        let tag = tagged_file
            .primary_tag()
            .or_else(|| tagged_file.first_tag())?;
        for pic in tag.pictures() {
            let data = pic.data();
            if let Ok(dyn_img) = image::load_from_memory(data) {
                return Some(Self::prepare_square_image(dyn_img));
            }
        }
        None
    }

    /// Crop to square and cache at native resolution (up to 800 px cap for memory sanity).
    /// Uses Lanczos3 for maximum quality.
    fn prepare_square_image(img: image::DynamicImage) -> RgbaImage {
        let (w, h) = img.dimensions();
        let size = w.min(h);
        let x_offset = (w - size) / 2;
        let y_offset = (h - size) / 2;
        let cropped = img.crop_imm(x_offset, y_offset, size, size);

        // Keep native resolution up to 1200 px – no downgrade for Kitty HiDPI rendering
        let target = size.min(1200);
        if target == size {
            cropped.to_rgba8()
        } else {
            cropped
                .resize_exact(target, target, image::imageops::FilterType::Lanczos3)
                .to_rgba8()
        }
    }

    // ── Spinning Vinyl PNG ────────────────────────────────────────────────────

    /// High-resolution spinning vinyl PNG.  Uses Rayon to parallelise the per-pixel
    /// inner loop so encoding stays fast even at 480 px.
    pub fn render_highres_vinyl_png(
        &self,
        size_px: u32,
        angle_rad: f32,
        theme_accent: Color,
        _title: &str,
        _artist: &str,
    ) -> Option<Vec<u8>> {
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
            _ => (193, 193, 255),
        };

        let cover_ref = self.cached_image.as_ref();

        // Parallel: each row is computed independently
        let raw_pixels: Vec<u8> = (0..size_px)
            .into_par_iter()
            .flat_map(|py| {
                let dy = py as f32 - cy;
                let mut row = vec![0u8; (size_px * 4) as usize];
                for px in 0..size_px {
                    let dx = px as f32 - cx;
                    let r = (dx * dx + dy * dy).sqrt();

                    if r > r_outer {
                        // Fully transparent – leave as zero (RGBA = 0,0,0,0)
                        continue;
                    }

                    let pixel = if r <= r_pin {
                        let pin_fade = (1.0 - (r / r_pin)).clamp(0.0, 1.0);
                        let val = (190.0 + pin_fade * 65.0) as u8;
                        [val, val, val, 255]
                    } else if r <= r_spindle {
                        [12, 12, 16, 255]
                    } else if r <= r_art {
                        if (r - r_art).abs() < 1.2 {
                            [acc_r, acc_g, acc_b, 255]
                        } else if let Some(cover) = cover_ref {
                            let rx = dx * cos_a - dy * sin_a;
                            let ry = dx * sin_a + dy * cos_a;
                            let u = (rx / (2.0 * r_art)) + 0.5;
                            let v = (ry / (2.0 * r_art)) + 0.5;
                            if u >= 0.0 && u <= 1.0 && v >= 0.0 && v <= 1.0 {
                                sample_bilinear(cover, u, v)
                            } else {
                                [20, 20, 26, 255]
                            }
                        } else {
                            let ring = ((r * 0.45).sin() * 0.5 + 0.5) * 20.0;
                            let base = (25.0 + ring) as u8;
                            [
                                (base as f32 * 0.9 + acc_r as f32 * 0.15) as u8,
                                (base as f32 * 0.9 + acc_g as f32 * 0.15) as u8,
                                (base as f32 * 0.9 + acc_b as f32 * 0.25) as u8,
                                255,
                            ]
                        }
                    } else {
                        let groove_factor = ((r * 0.95).sin() * 0.5 + 0.5).powf(1.5);
                        let base_v = 18.0 + groove_factor * 12.0;
                        let phi = dy.atan2(dx);
                        let sheen = (phi - 0.7853).cos().abs().powi(12) * 55.0;
                        let edge_alpha = ((r_outer - r) * 1.5).clamp(0.0, 1.0);
                        [
                            ((base_v + sheen * 0.8) + acc_r as f32 * 0.08)
                                .clamp(0.0, 255.0) as u8,
                            ((base_v + sheen * 0.8) + acc_g as f32 * 0.08)
                                .clamp(0.0, 255.0) as u8,
                            ((base_v + sheen * 1.1) + acc_b as f32 * 0.12)
                                .clamp(0.0, 255.0) as u8,
                            (255.0 * edge_alpha) as u8,
                        ]
                    };

                    let base = (px * 4) as usize;
                    row[base]     = pixel[0];
                    row[base + 1] = pixel[1];
                    row[base + 2] = pixel[2];
                    row[base + 3] = pixel[3];
                }
                row
            })
            .collect();

        // Build RgbaImage from raw bytes
        let img = RgbaImage::from_raw(size_px, size_px, raw_pixels)?;
        encode_png(&img, size_px, size_px)
    }

    // ── Static Cover PNG ──────────────────────────────────────────────────────

    /// Crystal-clear static album-art PNG at the highest available resolution.
    /// Uses Rayon for parallel row computation.
    pub fn render_highres_cover_png(
        &self,
        size_px: u32,
        theme_accent: Color,
        _title: &str,
        _artist: &str,
    ) -> Option<Vec<u8>> {
        // If we have a real cover, transmit it at NATIVE resolution (no downscale).
        // The image was already cached at min(native, 800) px in prepare_square_image.
        if let Some(ref cover) = self.cached_image {
            let native_size = cover.width(); // square, so w == h
            // Encode native pixels directly – no intermediate resize, perfect quality
            return encode_png(cover, native_size, native_size);
        }

        // No cover art: generate a stylized placeholder
        let cr = (size_px as f32 * 0.06).max(8.0);
        let (acc_r, acc_g, acc_b) = match theme_accent {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (180, 180, 255),
        };
        let wf = size_px as f32;
        let hf = size_px as f32;

        let raw_pixels: Vec<u8> = (0..size_px)
            .into_par_iter()
            .flat_map(|py| {
                let y = py as f32;
                let mut row = vec![0u8; (size_px * 4) as usize];
                for px in 0..size_px {
                    let x = px as f32;
                    let dx = if x < cr {
                        cr - x
                    } else if x > wf - 1.0 - cr {
                        x - (wf - 1.0 - cr)
                    } else {
                        0.0
                    };
                    let dy = if y < cr {
                        cr - y
                    } else if y > hf - 1.0 - cr {
                        y - (hf - 1.0 - cr)
                    } else {
                        0.0
                    };
                    let corner_dist = (dx * dx + dy * dy).sqrt();
                    if corner_dist > cr {
                        continue; // transparent outside rounded corner
                    }
                    let alpha =
                        ((1.0 - (corner_dist - (cr - 1.0)).clamp(0.0, 1.0)) * 255.0) as u8;
                    let is_border = x <= 2.0
                        || x >= wf - 3.0
                        || y <= 2.0
                        || y >= hf - 3.0
                        || corner_dist > cr - 2.0;
                    let pixel = if is_border {
                        [acc_r, acc_g, acc_b, alpha]
                    } else {
                        let grid = (px % 28 == 0) || (py % 28 == 0);
                        let v = if grid { 40u8 } else { 20u8 };
                        [v, v, v + 15, alpha]
                    };
                    let base = (px * 4) as usize;
                    row[base]     = pixel[0];
                    row[base + 1] = pixel[1];
                    row[base + 2] = pixel[2];
                    row[base + 3] = pixel[3];
                }
                row
            })
            .collect();

        let img = RgbaImage::from_raw(size_px, size_px, raw_pixels)?;
        encode_png(&img, size_px, size_px)
    }

    // ── Half-block fallback renderers (non-Kitty terminals) ──────────────────

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
        let wf = (pixel_width - 1) as f32;
        let hf = (pixel_height - 1) as f32;
        for py in 0..pixel_height {
            let v = py as f32 / hf;
            for px in 0..pixel_width {
                let u = px as f32 / wf;
                grid[py][px] = if let Some(ref cover) = self.cached_image {
                    let p = sample_bilinear(cover, u, v);
                    Color::Rgb(p[0], p[1], p[2])
                } else {
                    let border = px == 0 || px == pixel_width - 1 || py == 0 || py == pixel_height - 1;
                    if border { theme_accent } else { Color::Rgb(22, 22, 34) }
                };
            }
        }
        grid
    }

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
                    continue;
                }
                grid[py][px] = if r <= r_pin {
                    Color::Rgb(220, 220, 240)
                } else if r <= r_spindle {
                    Color::Rgb(15, 15, 20)
                } else if r <= r_art {
                    if let Some(ref cover) = self.cached_image {
                        let rx = dx * cos_a - dy * sin_a;
                        let ry = dx * sin_a + dy * cos_a;
                        let u = (rx / (2.0 * r_art)) + 0.5;
                        let v = (ry / (2.0 * r_art)) + 0.5;
                        if u >= 0.0 && u <= 1.0 && v >= 0.0 && v <= 1.0 {
                            let p = sample_bilinear(cover, u, v);
                            Color::Rgb(p[0], p[1], p[2])
                        } else {
                            Color::Rgb(25, 25, 35)
                        }
                    } else {
                        let ring = ((r * 1.5).sin() * 0.5 + 0.5) * 15.0;
                        let b = (40.0 + ring) as u8;
                        Color::Rgb(b, b + 5, b + 20)
                    }
                } else {
                    let groove_factor = ((r * 2.8).sin() * 0.5 + 0.5).powf(1.4);
                    let base_v = 18.0 + groove_factor * 12.0;
                    let phi = dy.atan2(dx);
                    let sheen = (phi - 0.7853).cos().abs().powi(10) * 45.0;
                    let total = (base_v + sheen).clamp(0.0, 255.0) as u8;
                    Color::Rgb(total, total, total + 12)
                };
            }
        }
        grid
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Bilinear interpolation sampler
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

/// Encode an RgbaImage to a PNG byte vector (fast / no-filter compression)
fn encode_png(img: &RgbaImage, w: u32, h: u32) -> Option<Vec<u8>> {
    let mut png_bytes = Vec::new();
    let encoder = PngEncoder::new_with_quality(
        &mut png_bytes,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::NoFilter,
    );
    encoder
        .write_image(img, w, h, ExtendedColorType::Rgba8)
        .ok()?;
    Some(png_bytes)
}

// ── Kitty Graphics Protocol ───────────────────────────────────────────────────

/// Transmit and display a PNG using Kitty Graphics Protocol
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

/// Delete all Kitty Graphics Protocol images
pub fn clear_kitty_images<W: Write>(writer: &mut W) -> std::io::Result<()> {
    write!(writer, "\x1b_Ga=d,d=A,q=2\x1b\\")?;
    writer.flush()?;
    Ok(())
}
