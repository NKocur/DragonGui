/// RGBA color in normalized float space.
pub type Color = [f32; 4];

fn rgb(r: u8, g: u8, b: u8) -> Color {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

pub(crate) fn parse_hex_color(value: &str) -> Option<Color> {
    let hex = value.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(rgb(r, g, b))
}

/// Design-token set for primitive and text drawing.
#[derive(Clone)]
pub struct Theme {
    /// Window / outermost background.
    pub background: Color,
    /// Panel and card surface.
    pub surface: Color,
    /// Alternative surface for interactive shells.
    pub surface_alt: Color,
    /// Primary text colour.
    pub text: Color,
    /// Secondary / placeholder text colour.
    pub muted_text: Color,
    /// Accent / primary highlight colour.
    pub accent: Color,
    /// Subtle border between regions.
    pub border: Color,
    /// Semantic danger colour.
    pub danger: Color,
    /// Semantic warning colour.
    pub warning: Color,
    /// Semantic success colour.
    pub success: Color,
    /// Keyboard focus ring colour.
    pub focus: Color,
    /// Disabled foreground/outline colour.
    pub disabled: Color,
    /// Corner radius for interactive shells, in logical pixels.
    pub radius: f32,
    /// Base spacing token in logical pixels.
    pub spacing: f32,
    /// Base font size in logical pixels.
    pub font_size: f32,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            background: rgb(0x12, 0x12, 0x1a),
            surface: rgb(0x1e, 0x1e, 0x2e),
            surface_alt: rgb(0x32, 0x32, 0x4a),
            text: rgb(0xf0, 0xf0, 0xf7),
            muted_text: rgb(0xa8, 0xa8, 0xba),
            accent: rgb(0x7b, 0x73, 0xff),
            border: rgb(0x38, 0x38, 0x50),
            danger: rgb(0xff, 0x5c, 0x7a),
            warning: rgb(0xff, 0xbf, 0x47),
            success: rgb(0x43, 0xd4, 0x8f),
            focus: rgb(0x6b, 0xdc, 0xff),
            disabled: rgb(0x66, 0x66, 0x7a),
            radius: 6.0,
            spacing: 8.0,
            font_size: 14.0,
        }
    }

    pub fn control_height(&self) -> f32 {
        (self.font_size + self.spacing * 2.0 + 4.0).max(28.0)
    }
}
