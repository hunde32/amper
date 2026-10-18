use ratatui::style::{Color, Modifier, Style};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct Theme {
    pub bg: Color,
    pub border: Color,
    pub accent: Color,
    pub accent_secondary: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_dim: Color,
    pub wave_fg: Color,

    colors_file: Option<PathBuf>,
    last_modified: Option<SystemTime>,
}

impl Theme {
    pub fn new() -> Self {
        let mut theme = Self {
            // Transparent background to show terminal opacity and wallpaper
            bg: Color::Reset,
            border: Color::LightBlue,
            accent: Color::LightCyan,
            accent_secondary: Color::LightMagenta,
            text_primary: Color::Reset,
            text_secondary: Color::Gray,
            text_dim: Color::DarkGray,
            wave_fg: Color::LightBlue,

            colors_file: None,
            last_modified: None,
        };

        theme.init_from_kitty_config();
        theme
    }

    pub fn init_from_kitty_config(&mut self) {
        let candidates = [
            dirs::home_dir().map(|h| h.join(".config/kitty/colors.conf")),
            dirs::home_dir().map(|h| h.join(".cache/wal/colors-kitty.conf")),
            std::env::var("KITTY_CONFIG_DIRECTORY").ok().map(|d| PathBuf::from(d).join("colors.conf")),
        ];

        for cand in candidates.into_iter().flatten() {
            if cand.exists() {
                self.colors_file = Some(cand);
                self.reload_if_changed();
                break;
            }
        }
    }

    pub fn reload_if_changed(&mut self) {
        let path = match &self.colors_file {
            Some(p) => p.clone(),
            None => return,
        };

        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => return,
        };

        let modified = metadata.modified().ok();
        if modified != self.last_modified {
            self.last_modified = modified;
            if let Ok(content) = fs::read_to_string(&path) {
                self.parse_kitty_colors(&content);
            }
        }
    }

    fn parse_kitty_colors(&mut self, content: &str) {
        let mut fg = None;
        let mut color1 = None;
        let mut color2 = None;
        let mut color3 = None;
        let mut color4 = None;
        let mut color6 = None;
        let mut color7 = None;

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let key = parts[0];
                let val = parts[1];
                match key {
                    "foreground" => fg = parse_hex_color(val),
                    "color1" => color1 = parse_hex_color(val),
                    "color2" => color2 = parse_hex_color(val),
                    "color3" => color3 = parse_hex_color(val),
                    "color4" => color4 = parse_hex_color(val),
                    "color6" => color6 = parse_hex_color(val),
                    "color7" => color7 = parse_hex_color(val),
                    _ => {}
                }
            }
        }

        // Always transparent background
        self.bg = Color::Reset;

        if let Some(c) = fg {
            self.text_primary = c;
        }
        if let Some(c) = color4 {
            // Main wallpaper accent (usually primary blue/purple/accent from Matugen)
            self.accent = c;
            self.border = c;
            self.wave_fg = c;
        }
        if let Some(c) = color2.or(color1) {
            self.accent_secondary = c;
        }
        if let Some(c) = color6.or(color3) {
            self.text_secondary = c;
        }
        if let Some(c) = color7 {
            self.text_primary = c;
        }
        self.text_dim = Color::DarkGray;
    }

    pub fn primary(&self) -> Style {
        Style::default().fg(self.text_primary).bg(Color::Reset)
    }

    pub fn primary_bold(&self) -> Style {
        Style::default().fg(self.text_primary).bg(Color::Reset).add_modifier(Modifier::BOLD)
    }

    pub fn secondary(&self) -> Style {
        Style::default().fg(self.text_secondary).bg(Color::Reset)
    }

    pub fn dim(&self) -> Style {
        Style::default().fg(self.text_dim).bg(Color::Reset)
    }

    pub fn accent(&self) -> Style {
        Style::default().fg(self.accent).bg(Color::Reset).add_modifier(Modifier::BOLD)
    }

    pub fn accent_secondary(&self) -> Style {
        Style::default().fg(self.accent_secondary).bg(Color::Reset)
    }

    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border).bg(Color::Reset)
    }
}

fn parse_hex_color(hex: &str) -> Option<Color> {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() == 6 {
        let r = u8::from_str_radix(&clean[0..2], 16).ok()?;
        let g = u8::from_str_radix(&clean[2..4], 16).ok()?;
        let b = u8::from_str_radix(&clean[4..6], 16).ok()?;
        Some(Color::Rgb(r, g, b))
    } else {
        None
    }
}
