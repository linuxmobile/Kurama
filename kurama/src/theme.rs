//! Kurama typography and colour helpers.
//!
//! Colours come from Rune's semantic tokens so dark and light appearances stay
//! consistent with the rest of the platform. The type scale is defined here in
//! one place rather than repeated across views.

use rune::{Color, TextStyle, tok};

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
