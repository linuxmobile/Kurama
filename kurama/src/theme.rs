//! Kurama typography and colour helpers.
//!
//! Colours come from Rune's semantic tokens so dark and light appearances stay
//! consistent with the rest of the platform. The type scale is defined here in
//! one place rather than repeated across views.

use rune::{tok, Color, TextStyle};

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
