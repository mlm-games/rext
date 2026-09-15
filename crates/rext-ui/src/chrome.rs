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
    pub diff_added: Color,
    pub diff_modified: Color,
    pub diff_deleted: Color,
    pub syn_keyword: Color,
    pub syn_string: Color,
    pub syn_comment: Color,
    pub syn_number: Color,
    pub syn_type: Color,
    pub syn_function: Color,
    pub syn_variable: Color,
    pub syn_operator: Color,
    pub syn_property: Color,
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
        diff_added: Color::from_rgb(0x98, 0xC3, 0x79),
        diff_modified: Color::from_rgb(0xE5, 0xC0, 0x7B),
        diff_deleted: Color::from_rgb(0xE0, 0x6C, 0x75),
        syn_keyword: Color::from_rgb(0xC6, 0x78, 0xDD),
        syn_string: Color::from_rgb(0x98, 0xC3, 0x79),
        syn_comment: Color::from_rgb(0x5C, 0x63, 0x70),
        syn_number: Color::from_rgb(0xD1, 0x9A, 0x66),
        syn_type: Color::from_rgb(0xE5, 0xC0, 0x7B),
        syn_function: Color::from_rgb(0x61, 0xAF, 0xEF),
        syn_variable: Color::from_rgb(0xAB, 0xB2, 0xBF),
        syn_operator: Color::from_rgb(0x56, 0xB6, 0xC2),
        syn_property: Color::from_rgb(0xE0, 0x6C, 0x75),
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
