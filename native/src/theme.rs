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

pub(crate) fn parse_web_color(value: &str) -> Option<Color> {
    let value = value.trim();
    if value.starts_with('#') {
        parse_css_hex_color(value)
    } else if let Some(color) = parse_named_color(value) {
        Some(color)
    } else {
        parse_functional_color(value)
    }
}

fn parse_named_color(value: &str) -> Option<Color> {
    match value.trim().to_ascii_lowercase().as_str() {
        "transparent" => Some([0.0, 0.0, 0.0, 0.0]),
        "black" => Some([0.0, 0.0, 0.0, 1.0]),
        "white" => Some([1.0, 1.0, 1.0, 1.0]),
        "red" => Some([1.0, 0.0, 0.0, 1.0]),
        "green" => Some([0.0, 0.5019608, 0.0, 1.0]),
        "blue" => Some([0.0, 0.0, 1.0, 1.0]),
        "gray" | "grey" => Some([0.5019608, 0.5019608, 0.5019608, 1.0]),
        _ => None,
    }
}

fn parse_functional_color(value: &str) -> Option<Color> {
    let value = value.trim();
    if let Some(args) =
        color_function_args(value, "rgb").or_else(|| color_function_args(value, "rgba"))
    {
        return parse_rgb_function(args);
    }
    if let Some(args) =
        color_function_args(value, "hsl").or_else(|| color_function_args(value, "hsla"))
    {
        return parse_hsl_function(args);
    }
    if let Some(args) = color_function_args(value, "hwb") {
        return parse_hwb_function(args);
    }
    if let Some(args) = color_function_args(value, "lab") {
        return parse_lab_function(args);
    }
    if let Some(args) = color_function_args(value, "lch") {
        return parse_lch_function(args);
    }
    if let Some(args) = color_function_args(value, "oklab") {
        return parse_oklab_function(args);
    }
    if let Some(args) = color_function_args(value, "oklch") {
        return parse_oklch_function(args);
    }
    if let Some(args) = color_function_args(value, "color") {
        return parse_color_function(args);
    }
    None
}

fn color_function_args<'a>(value: &'a str, name: &str) -> Option<&'a str> {
    let value = value.trim();
    let prefix = format!("{name}(");
    let lower = value.to_ascii_lowercase();
    if !lower.starts_with(&prefix) || !value.ends_with(')') {
        return None;
    }
    value
        .get(prefix.len()..value.len().saturating_sub(1))
        .map(str::trim)
}

fn color_function_tokens(args: &str) -> Vec<&str> {
    if args.contains(',') {
        args.split(',')
            .flat_map(|part| part.split('/'))
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect()
    } else {
        args.split_whitespace()
            .filter(|part| *part != "/")
            .collect()
    }
}

fn parse_rgb_function(args: &str) -> Option<Color> {
    let tokens = color_function_tokens(args);
    if tokens.len() != 3 && tokens.len() != 4 {
        return None;
    }
    let r = parse_rgb_channel(tokens[0])?;
    let g = parse_rgb_channel(tokens[1])?;
    let b = parse_rgb_channel(tokens[2])?;
    let a = tokens
        .get(3)
        .and_then(|value| parse_alpha_channel(value))
        .unwrap_or(1.0);
    Some([r, g, b, a])
}

fn parse_hsl_function(args: &str) -> Option<Color> {
    let tokens = color_function_tokens(args);
    if tokens.len() != 3 && tokens.len() != 4 {
        return None;
    }
    let hue = parse_hue_degrees(tokens[0])?;
    let saturation = parse_percent_channel(tokens[1])?;
    let lightness = parse_percent_channel(tokens[2])?;
    let alpha = tokens
        .get(3)
        .and_then(|value| parse_alpha_channel(value))
        .unwrap_or(1.0);
    let [r, g, b] = hsl_to_rgb(hue, saturation, lightness);
    Some([r, g, b, alpha])
}

fn parse_hwb_function(args: &str) -> Option<Color> {
    let tokens = color_function_tokens(args);
    if tokens.len() != 3 && tokens.len() != 4 {
        return None;
    }
    let hue = parse_hue_degrees(tokens[0])?;
    let whiteness = parse_percent_channel(tokens[1])?;
    let blackness = parse_percent_channel(tokens[2])?;
    let alpha = tokens
        .get(3)
        .and_then(|value| parse_alpha_channel(value))
        .unwrap_or(1.0);
    let [r, g, b] = hwb_to_rgb(hue, whiteness, blackness);
    Some([r, g, b, alpha])
}

fn parse_lab_function(args: &str) -> Option<Color> {
    let tokens = color_function_tokens(args);
    if tokens.len() != 3 && tokens.len() != 4 {
        return None;
    }
    let lightness = parse_lab_lightness(tokens[0])?;
    let a = parse_lab_axis(tokens[1])?;
    let b = parse_lab_axis(tokens[2])?;
    let alpha = tokens
        .get(3)
        .and_then(|value| parse_alpha_channel(value))
        .unwrap_or(1.0);
    let [r, g, b] = lab_to_srgb(lightness, a, b);
    Some([r, g, b, alpha])
}

fn parse_lch_function(args: &str) -> Option<Color> {
    let tokens = color_function_tokens(args);
    if tokens.len() != 3 && tokens.len() != 4 {
        return None;
    }
    let lightness = parse_lab_lightness(tokens[0])?;
    let chroma = parse_lab_chroma(tokens[1])?;
    let hue = parse_hue_degrees(tokens[2])?.to_radians();
    let alpha = tokens
        .get(3)
        .and_then(|value| parse_alpha_channel(value))
        .unwrap_or(1.0);
    let a = chroma * hue.cos();
    let b = chroma * hue.sin();
    let [r, g, b] = lab_to_srgb(lightness, a, b);
    Some([r, g, b, alpha])
}

fn parse_oklab_function(args: &str) -> Option<Color> {
    let tokens = color_function_tokens(args);
    if tokens.len() != 3 && tokens.len() != 4 {
        return None;
    }
    let lightness = parse_ok_lightness(tokens[0])?;
    let a = parse_ok_lab_axis(tokens[1])?;
    let b = parse_ok_lab_axis(tokens[2])?;
    let alpha = tokens
        .get(3)
        .and_then(|value| parse_alpha_channel(value))
        .unwrap_or(1.0);
    let [r, g, b] = oklab_to_srgb(lightness, a, b);
    Some([r, g, b, alpha])
}

fn parse_oklch_function(args: &str) -> Option<Color> {
    let tokens = color_function_tokens(args);
    if tokens.len() != 3 && tokens.len() != 4 {
        return None;
    }
    let lightness = parse_ok_lightness(tokens[0])?;
    let chroma = parse_ok_chroma(tokens[1])?;
    let hue = parse_hue_degrees(tokens[2])?.to_radians();
    let alpha = tokens
        .get(3)
        .and_then(|value| parse_alpha_channel(value))
        .unwrap_or(1.0);
    let a = chroma * hue.cos();
    let b = chroma * hue.sin();
    let [r, g, b] = oklab_to_srgb(lightness, a, b);
    Some([r, g, b, alpha])
}

fn parse_color_function(args: &str) -> Option<Color> {
    let tokens = color_function_tokens(args);
    if tokens.len() != 4 && tokens.len() != 5 {
        return None;
    }
    let alpha = tokens
        .get(4)
        .and_then(|value| parse_alpha_channel(value))
        .unwrap_or(1.0);
    match tokens[0].to_ascii_lowercase().as_str() {
        "srgb" => Some([
            parse_unit_interval_channel(tokens[1])?,
            parse_unit_interval_channel(tokens[2])?,
            parse_unit_interval_channel(tokens[3])?,
            alpha,
        ]),
        "srgb-linear" => Some([
            srgb_transfer_function(parse_unit_interval_channel(tokens[1])?),
            srgb_transfer_function(parse_unit_interval_channel(tokens[2])?),
            srgb_transfer_function(parse_unit_interval_channel(tokens[3])?),
            alpha,
        ]),
        _ => None,
    }
}

fn parse_rgb_channel(value: &str) -> Option<f32> {
    if let Some(percent) = value.trim().strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v / 100.0).clamp(0.0, 1.0));
    }
    value
        .trim()
        .parse::<f32>()
        .ok()
        .map(|v| (v / 255.0).clamp(0.0, 1.0))
}

fn parse_unit_interval_channel(value: &str) -> Option<f32> {
    if let Some(percent) = value.trim().strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v / 100.0).clamp(0.0, 1.0));
    }
    value.trim().parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
}

fn parse_lab_lightness(value: &str) -> Option<f32> {
    if let Some(percent) = value.trim().strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| v.clamp(0.0, 100.0));
    }
    value
        .trim()
        .parse::<f32>()
        .ok()
        .map(|v| v.clamp(0.0, 100.0))
}

fn parse_lab_axis(value: &str) -> Option<f32> {
    if let Some(percent) = value.trim().strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v / 100.0).clamp(-1.0, 1.0) * 125.0);
    }
    value.trim().parse::<f32>().ok()
}

fn parse_lab_chroma(value: &str) -> Option<f32> {
    if let Some(percent) = value.trim().strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v / 100.0).max(0.0) * 150.0);
    }
    value.trim().parse::<f32>().ok().map(|v| v.max(0.0))
}

fn parse_ok_lightness(value: &str) -> Option<f32> {
    if let Some(percent) = value.trim().strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v / 100.0).clamp(0.0, 1.0));
    }
    value.trim().parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
}

fn parse_ok_lab_axis(value: &str) -> Option<f32> {
    if let Some(percent) = value.trim().strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v / 100.0).clamp(-1.0, 1.0) * 0.4);
    }
    value.trim().parse::<f32>().ok()
}

fn parse_ok_chroma(value: &str) -> Option<f32> {
    if let Some(percent) = value.trim().strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v / 100.0).max(0.0) * 0.4);
    }
    value.trim().parse::<f32>().ok().map(|v| v.max(0.0))
}

fn parse_percent_channel(value: &str) -> Option<f32> {
    value
        .trim()
        .strip_suffix('%')?
        .trim()
        .parse::<f32>()
        .ok()
        .map(|v| (v / 100.0).clamp(0.0, 1.0))
}

fn parse_alpha_channel(value: &str) -> Option<f32> {
    if let Some(percent) = value.trim().strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v / 100.0).clamp(0.0, 1.0));
    }
    value.trim().parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
}

fn parse_hue_degrees(value: &str) -> Option<f32> {
    let value = value.trim();
    if let Some(turn) = value.strip_suffix("turn") {
        return turn
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v * 360.0).rem_euclid(360.0));
    }
    if let Some(grad) = value.strip_suffix("grad") {
        return grad
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v * 0.9).rem_euclid(360.0));
    }
    if let Some(rad) = value.strip_suffix("rad") {
        return rad
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| v.to_degrees().rem_euclid(360.0));
    }
    let number = value
        .strip_suffix("deg")
        .unwrap_or(value)
        .trim()
        .parse::<f32>()
        .ok()?;
    Some(number.rem_euclid(360.0))
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> [f32; 3] {
    let c = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let h = hue / 60.0;
    let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = if h < 1.0 {
        (c, x, 0.0)
    } else if h < 2.0 {
        (x, c, 0.0)
    } else if h < 3.0 {
        (0.0, c, x)
    } else if h < 4.0 {
        (0.0, x, c)
    } else if h < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = lightness - c * 0.5;
    [
        (r1 + m).clamp(0.0, 1.0),
        (g1 + m).clamp(0.0, 1.0),
        (b1 + m).clamp(0.0, 1.0),
    ]
}

fn hwb_to_rgb(hue: f32, whiteness: f32, blackness: f32) -> [f32; 3] {
    let total = whiteness + blackness;
    if total >= 1.0 {
        let gray = whiteness / total;
        return [gray, gray, gray];
    }
    let [r, g, b] = hsl_to_rgb(hue, 1.0, 0.5);
    let scale = 1.0 - total;
    [
        (r * scale + whiteness).clamp(0.0, 1.0),
        (g * scale + whiteness).clamp(0.0, 1.0),
        (b * scale + whiteness).clamp(0.0, 1.0),
    ]
}

fn lab_to_srgb(lightness: f32, a: f32, b: f32) -> [f32; 3] {
    let fy = (lightness + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    let x_d50 = 0.96422 * lab_inv_f(fx);
    let y_d50 = lab_inv_f(fy);
    let z_d50 = 0.82521 * lab_inv_f(fz);

    let x_d65 = 0.9555766 * x_d50 - 0.0230393 * y_d50 + 0.0631636 * z_d50;
    let y_d65 = -0.0282895 * x_d50 + 1.0099416 * y_d50 + 0.0210077 * z_d50;
    let z_d65 = 0.0122982 * x_d50 - 0.0204830 * y_d50 + 1.3299098 * z_d50;

    xyz_d65_to_srgb(x_d65, y_d65, z_d65)
}

fn lab_inv_f(value: f32) -> f32 {
    const EPSILON: f32 = 216.0 / 24389.0;
    const KAPPA: f32 = 24389.0 / 27.0;
    let cubed = value * value * value;
    if cubed > EPSILON {
        cubed
    } else {
        (116.0 * value - 16.0) / KAPPA
    }
}

fn oklab_to_srgb(lightness: f32, a: f32, b: f32) -> [f32; 3] {
    let l = lightness + 0.39633778 * a + 0.21580376 * b;
    let m = lightness - 0.10556135 * a - 0.06385417 * b;
    let s = lightness - 0.08948418 * a - 1.2914855 * b;

    let l = l * l * l;
    let m = m * m * m;
    let s = s * s * s;

    let r = 4.0767417 * l - 3.3077116 * m + 0.23096994 * s;
    let g = -1.268438 * l + 2.6097574 * m - 0.34131938 * s;
    let b = -0.0041960863 * l - 0.7034186 * m + 1.7076147 * s;

    [
        srgb_transfer_function(r),
        srgb_transfer_function(g),
        srgb_transfer_function(b),
    ]
}

fn xyz_d65_to_srgb(x: f32, y: f32, z: f32) -> [f32; 3] {
    let r = 3.2404542 * x - 1.5371385 * y - 0.4985314 * z;
    let g = -0.9692660 * x + 1.8760108 * y + 0.0415560 * z;
    let b = 0.0556434 * x - 0.2040259 * y + 1.0572252 * z;
    [
        srgb_transfer_function(r),
        srgb_transfer_function(g),
        srgb_transfer_function(b),
    ]
}

fn srgb_transfer_function(value: f32) -> f32 {
    let encoded = if value <= 0.0031308 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    encoded.clamp(0.0, 1.0)
}

fn parse_css_hex_color(value: &str) -> Option<Color> {
    let hex = value.trim_start_matches('#');
    if hex.len() == 3 {
        let mut expanded = String::with_capacity(7);
        expanded.push('#');
        for ch in hex.chars() {
            expanded.push(ch);
            expanded.push(ch);
        }
        return parse_hex_color(&expanded);
    }
    if hex.len() == 4 {
        let mut expanded = String::with_capacity(9);
        expanded.push('#');
        for ch in hex.chars() {
            expanded.push(ch);
            expanded.push(ch);
        }
        return parse_css_hex_color(&expanded);
    }
    if hex.len() == 8 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
        return Some([
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        ]);
    }
    parse_hex_color(value)
}

/// Design-token set for primitive and text drawing.
#[derive(Debug, Clone)]
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
            background: rgb(0x0a, 0x0f, 0x14),
            surface: rgb(0x12, 0x19, 0x22),
            surface_alt: rgb(0x1d, 0x28, 0x33),
            text: rgb(0xf2, 0xf6, 0xf8),
            muted_text: rgb(0x91, 0xa0, 0xad),
            accent: rgb(0x37, 0xc6, 0xd0),
            border: rgb(0x26, 0x35, 0x43),
            danger: rgb(0xff, 0x5f, 0x72),
            warning: rgb(0xf4, 0xb8, 0x4a),
            success: rgb(0x45, 0xc4, 0x8a),
            focus: rgb(0x7b, 0xdc, 0xff),
            disabled: rgb(0x5d, 0x6a, 0x75),
            radius: 3.0,
            spacing: 5.0,
            font_size: 13.0,
        }
    }

    pub fn control_height(&self) -> f32 {
        (self.font_size + self.spacing * 2.0 + 2.0).max(25.0)
    }

    /// Extra-small spacing token.
    pub fn space_xs(&self) -> f32 {
        self.spacing * 0.5
    }

    /// Small spacing token.
    pub fn space_sm(&self) -> f32 {
        self.spacing
    }

    /// Medium spacing token.
    pub fn space_md(&self) -> f32 {
        self.spacing * 2.0
    }

    /// Large spacing token.
    pub fn space_lg(&self) -> f32 {
        self.spacing * 3.0
    }

    /// Extra-large spacing token.
    pub fn space_xl(&self) -> f32 {
        self.spacing * 4.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_theme_defaults_are_compact_and_neutral() {
        let theme = Theme::dark();
        assert_color_close(theme.background, rgb(0x0a, 0x0f, 0x14));
        assert_color_close(theme.surface, rgb(0x12, 0x19, 0x22));
        assert_color_close(theme.surface_alt, rgb(0x1d, 0x28, 0x33));
        assert_color_close(theme.accent, rgb(0x37, 0xc6, 0xd0));
        assert_eq!(theme.radius, 3.0);
        assert_eq!(theme.spacing, 5.0);
        assert_eq!(theme.font_size, 13.0);
        assert_eq!(theme.control_height(), 25.0);
        assert_eq!(theme.space_xs(), 2.5);
        assert_eq!(theme.space_sm(), 5.0);
        assert_eq!(theme.space_md(), 10.0);
        assert_eq!(theme.space_lg(), 15.0);
        assert_eq!(theme.space_xl(), 20.0);
    }

    #[test]
    fn web_color_parser_accepts_lab_lch_oklab_and_oklch() {
        let lab_white = parse_web_color("lab(100% 0 0)").expect("lab white");
        assert_color_close(lab_white, [1.0, 1.0, 1.0, 1.0]);

        let lab_gray = parse_web_color("lab(50% 0 0 / 25%)").expect("lab gray");
        assert!((lab_gray[0] - lab_gray[1]).abs() < 0.015);
        assert!((lab_gray[1] - lab_gray[2]).abs() < 0.015);
        assert!((lab_gray[3] - 0.25).abs() < 0.003);

        let lch_gray = parse_web_color("lch(50% 0 0 / 40%)").expect("lch gray");
        assert!((lch_gray[0] - lch_gray[1]).abs() < 0.015);
        assert!((lch_gray[1] - lch_gray[2]).abs() < 0.015);
        assert!((lch_gray[3] - 0.4).abs() < 0.003);

        let oklab_white = parse_web_color("oklab(100% 0 0)").expect("oklab white");
        assert_color_close(oklab_white, [1.0, 1.0, 1.0, 1.0]);

        let oklab_gray = parse_web_color("oklab(50% 0 0 / 35%)").expect("oklab gray");
        assert!((oklab_gray[0] - oklab_gray[1]).abs() < 0.015);
        assert!((oklab_gray[1] - oklab_gray[2]).abs() < 0.015);
        assert!((oklab_gray[3] - 0.35).abs() < 0.003);

        let oklch_blue = parse_web_color("oklch(62% 0.18 240deg / 0.5)").expect("oklch blue");
        assert!(oklch_blue[2] > oklch_blue[0]);
        assert!(oklch_blue[2] > oklch_blue[1]);
        assert!((oklch_blue[3] - 0.5).abs() < 0.003);

        let oklch_white = parse_web_color("oklch(100% 0 0)").expect("oklch white");
        assert_color_close(oklch_white, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn web_color_parser_accepts_hwb() {
        assert_color_close(
            parse_web_color("hwb(0 0% 0%)").expect("hwb red"),
            [1.0, 0.0, 0.0, 1.0],
        );
        assert_color_close(
            parse_web_color("hwb(120deg 0% 50% / 25%)").expect("hwb dark green"),
            [0.0, 0.5, 0.0, 0.25],
        );
        assert_color_close(
            parse_web_color("hwb(60 50% 50%)").expect("hwb normalized gray"),
            [0.5, 0.5, 0.5, 1.0],
        );
    }

    #[test]
    fn web_color_parser_accepts_srgb_color_function_and_rejects_wide_gamut() {
        assert_color_close(
            parse_web_color("color(srgb 1 0.5 0 / 25%)").expect("srgb color"),
            [1.0, 0.5, 0.0, 0.25],
        );
        assert_color_close(
            parse_web_color("color(srgb 100% 50% 0% / 0.4)").expect("srgb percent color"),
            [1.0, 0.5, 0.0, 0.4],
        );
        assert_color_close(
            parse_web_color("color(srgb-linear 1 0 0 / 0.5)").expect("linear srgb color"),
            [1.0, 0.0, 0.0, 0.5],
        );
        assert!(parse_web_color("color(display-p3 1 0 0)").is_none());
    }

    fn assert_color_close(actual: Color, expected: Color) {
        for (actual, expected) in actual.iter().zip(expected) {
            assert!(
                (actual - expected).abs() < 0.01,
                "expected {expected}, got {actual}"
            );
        }
    }
}
