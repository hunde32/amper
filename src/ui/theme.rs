use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub bg: Color,
    pub card_bg: Color,
    pub pill_bg: Color,
    pub pill_active_bg: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub vinyl_dark: Color,
    pub vinyl_groove: Color,
    pub vinyl_sheen: Color,
    pub control_bg: Color,
    pub control_active_bg: Color,
}

impl Theme {
    pub fn new() -> Self {
        Self {
            // Warm chocolate / espresso palette from screenshot
            bg: Color::Rgb(28, 17, 12),
            card_bg: Color::Rgb(39, 24, 16),
            pill_bg: Color::Rgb(55, 34, 23),
            pill_active_bg: Color::Rgb(95, 58, 38),
            accent: Color::Rgb(235, 87, 61), // Vibrant coral red
            accent_hover: Color::Rgb(255, 110, 85),
            text_primary: Color::Rgb(250, 245, 240),
            text_secondary: Color::Rgb(190, 165, 150),
            text_muted: Color::Rgb(125, 100, 85),
            vinyl_dark: Color::Rgb(16, 10, 7),
            vinyl_groove: Color::Rgb(28, 18, 13),
            vinyl_sheen: Color::Rgb(85, 55, 38),
            control_bg: Color::Rgb(65, 40, 28),
            control_active_bg: Color::Rgb(110, 68, 45),
        }
    }

    pub fn primary_text(&self) -> Style {
        Style::default().fg(self.text_primary).add_modifier(Modifier::BOLD)
    }

    pub fn secondary_text(&self) -> Style {
        Style::default().fg(self.text_secondary)
    }

    pub fn muted_text(&self) -> Style {
        Style::default().fg(self.text_muted)
    }

    pub fn accent_text(&self) -> Style {
        Style::default().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    pub fn pill_inactive(&self) -> Style {
        Style::default().fg(self.text_secondary).bg(self.pill_bg)
    }

    pub fn pill_active(&self) -> Style {
        Style::default().fg(self.text_primary).bg(self.pill_active_bg).add_modifier(Modifier::BOLD)
    }
}
