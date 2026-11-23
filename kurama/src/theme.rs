//! Kurama typography and colour helpers.
//!
//! Colours come from Rune's semantic tokens so dark and light appearances stay
//! consistent with the rest of the platform. The type scale is defined here in
//! one place rather than repeated across views.

use rune::{tok, Color, TextStyle};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub enum ThemePreference {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemePreference {
    pub const ALL: [Self; 3] = [Self::System, Self::Dark, Self::Light];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }

    pub fn from_config(value: &str) -> Self {
        match value {
            "dark" => Self::Dark,
            "light" => Self::Light,
            _ => Self::System,
        }
    }

    pub fn as_config(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::System => "settings-theme-system",
            Self::Dark => "settings-theme-dark",
            Self::Light => "settings-theme-light",
        }
    }

    pub fn dark(self, system_dark: bool) -> bool {
        match self {
            Self::System => system_dark,
            Self::Dark => true,
            Self::Light => false,
        }
    }
}

pub const WINDOW_TITLE: TextStyle = TextStyle::new(21.0, 650.0);
pub const SECTION_TITLE: TextStyle = TextStyle::new(12.0, 650.0);
pub const ROW_TITLE: TextStyle = TextStyle::new(13.0, 560.0);
pub const ROW_SUBTITLE: TextStyle = TextStyle::new(12.0, 400.0);
pub const META: TextStyle = TextStyle::new(11.5, 450.0);

pub fn accent(dark: bool) -> Color {
    tok::accent(0, dark)
}

pub fn selected_row(dark: bool) -> Color {
    accent(dark).alpha(if dark { 0.16 } else { 0.12 })
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub canvas: Color,
    pub sidebar: Color,
    pub list: Color,
    pub panel: Color,
    pub raised: Color,
    pub separator: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub selected: Color,
    pub selected_text: Color,
    pub danger: Color,
    pub site_tile: Color,
    pub site_glyph: Color,
}

fn hex(value: &str) -> Color {
    rune::color::hex(value).unwrap_or(Color::WHITE)
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        static DARK: OnceLock<Palette> = OnceLock::new();
        static LIGHT: OnceLock<Palette> = OnceLock::new();
        let cache = if dark { &DARK } else { &LIGHT };
        *cache.get_or_init(|| Palette::build(dark))
    }

    fn build(dark: bool) -> Self {
        if dark {
            Self {
                canvas: hex("#171718"),
                sidebar: hex("#1e1e20"),
                list: hex("#1a1a1c"),
                panel: hex("#202022"),
                raised: hex("#29292c"),
                separator: hex("#303034"),
                text: hex("#f4f4f5"),
                muted: hex("#9b9ba5"),
                accent: hex("#3b82f6"),
                selected: hex("#3b82f6"),
                selected_text: Color::WHITE,
                danger: hex("#ef5a5a"),
                site_tile: hex("#f1f2f4"),
                site_glyph: hex("#4b5563"),
            }
        } else {
            Self {
                canvas: hex("#f7f7f8"),
                sidebar: hex("#f2f3f5"),
                list: hex("#fbfbfc"),
                panel: hex("#ffffff"),
                raised: hex("#f4f5f7"),
                separator: hex("#e4e6ea"),
                text: hex("#18181b"),
                muted: hex("#6b7280"),
                accent: hex("#2563eb"),
                selected: hex("#2563eb"),
                selected_text: Color::WHITE,
                danger: hex("#dc2626"),
                site_tile: hex("#ffffff"),
                site_glyph: hex("#4b5563"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ThemePreference;

    #[test]
    fn theme_preference_round_trips_through_config() {
        for theme in ThemePreference::ALL {
            assert_eq!(ThemePreference::from_config(theme.as_config()), theme);
        }
        assert_eq!(
            ThemePreference::from_config("unknown"),
            ThemePreference::System
        );
    }

    #[test]
    fn only_system_follows_the_system_appearance() {
        assert!(!ThemePreference::System.dark(false));
        assert!(ThemePreference::System.dark(true));
        assert!(ThemePreference::Dark.dark(false));
        assert!(!ThemePreference::Light.dark(true));
    }
}
