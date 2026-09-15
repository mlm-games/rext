use repose_core::Color;

/// Editor surface colors (not M3 tokens). Zed One Dark-ish.
#[derive(Clone, Copy)]
pub struct EditorChrome {
    pub editor_bg: Color,
    pub gutter_bg: Color,
    pub gutter_fg: Color,
    pub gutter_fg_active: Color,
    pub current_line: Color,
    pub text: Color,
    pub caret: Color,
    pub selection: Color,
    pub border: Color,
}

pub fn zed_chrome() -> EditorChrome {
    EditorChrome {
        editor_bg: Color::from_rgb(0x28, 0x2C, 0x34),
        gutter_bg: Color::from_rgb(0x28, 0x2C, 0x34),
        gutter_fg: Color::from_rgb(0x4B, 0x52, 0x63),
        gutter_fg_active: Color::from_rgb(0xAB, 0xB2, 0xBF),
        current_line: Color::from_rgba(0xFF, 0xFF, 0xFF, 0x0A),
        text: Color::from_rgb(0xAB, 0xB2, 0xBF),
        caret: Color::from_rgb(0x52, 0x8B, 0xFF),
        selection: Color::from_rgba(0x52, 0x8B, 0xFF, 0x40),
        border: Color::from_rgb(0x36, 0x3C, 0x46),
    }
}

pub mod layout {
    pub const FONT_PX: f32 = 14.0;
    pub const LINE_H: f32 = 22.0;
    pub const GUTTER_PAD: f32 = 12.0;
    pub const TEXT_PAD: f32 = 16.0;
    pub const TAB_H: f32 = 34.0;
    pub const STATUS_H: f32 = 26.0;
}
