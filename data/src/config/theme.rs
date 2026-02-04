/// <https://github.com/iced-rs/iced/blob/master/core/src/theme/palette.rs> &
/// <https://github.com/squidowl/halloy/blob/main/data/src/appearance/theme.rs>
/// All credits and thanks to the authors of [`Halloy`] and [`iced_core`]
pub use professional_trading_theme as default_theme;

use iced_core::{
    Color,
    theme::{Custom, Palette},
};
use palette::{
    FromColor, Hsva, RgbHue,
    rgb::{Rgb, Rgba},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Theme(pub iced_core::Theme);

#[derive(Serialize, Deserialize)]
struct SerTheme {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    palette: Option<Palette>,
}

impl Default for Theme {
    fn default() -> Self {
        Self(iced_core::Theme::Custom(
            professional_trading_theme().into(),
        ))
    }
}

impl From<Theme> for iced_core::Theme {
    fn from(val: Theme) -> Self {
        val.0
    }
}

/// NEW: Enhanced professional trading theme optimized for footprint charts
pub fn professional_trading_theme() -> Custom {
    Custom::new(
        "Pro Trader".to_string(),
        Palette {
            background: Color::from_rgb8(18, 18, 24), // Dark blue-gray for better contrast
            text: Color::from_rgb8(220, 220, 220),    // Brighter text for readability
            primary: Color::from_rgb8(100, 149, 237), // Cornflower blue for primary elements
            success: Color::from_rgb8(50, 205, 50),   // Bright green for buy signals
            danger: Color::from_rgb8(220, 80, 60),    // Bright red for sell signals
            warning: Color::from_rgb8(255, 215, 0),   // Gold for warnings/key levels
        },
    )
}

/// NEW: Alternative high-contrast theme for trading
pub fn high_contrast_trading_theme() -> Custom {
    Custom::new(
        "High Contrast".to_string(),
        Palette {
            background: Color::from_rgb8(10, 10, 15), // Very dark background
            text: Color::from_rgb8(240, 240, 240),    // Very bright text
            primary: Color::from_rgb8(70, 130, 180),  // Steel blue
            success: Color::from_rgb8(0, 255, 127),   // Spring green
            danger: Color::from_rgb8(255, 69, 0),     // Red-orange
            warning: Color::from_rgb8(255, 223, 0),   // Golden yellow
        },
    )
}

/// NEW: Light theme for traders who prefer bright interfaces
pub fn light_trading_theme() -> Custom {
    Custom::new(
        "Light Trader".to_string(),
        Palette {
            background: Color::from_rgb8(248, 248, 255), // Ghost white
            text: Color::from_rgb8(40, 40, 40),          // Dark gray
            primary: Color::from_rgb8(30, 144, 255),     // Dodger blue
            success: Color::from_rgb8(34, 139, 34),      // Forest green
            danger: Color::from_rgb8(178, 34, 34),       // Firebrick red
            warning: Color::from_rgb8(205, 133, 63),     // Peru brown
        },
    )
}

/// NEW: Color palette specifically optimized for footprint chart visualization
pub fn footprint_optimized_theme() -> Custom {
    Custom::new(
        "Footprint Pro".to_string(),
        Palette {
            background: Color::from_rgb8(20, 20, 26), // Optimal dark background
            text: Color::from_rgb8(230, 230, 230),    // High visibility text
            primary: Color::from_rgb8(65, 105, 225),  // Royal blue
            success: Color::from_rgb8(60, 220, 120),  // Bright emerald green
            danger: Color::from_rgb8(240, 80, 80),    // Bright coral red
            warning: Color::from_rgb8(255, 200, 50),  // Bright amber
        },
    )
}

impl Serialize for Theme {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let iced_core::Theme::Custom(custom) = &self.0 {
            let is_default_theme = custom.to_string() == "Pro Trader";
            let ser_theme = SerTheme {
                name: if is_default_theme {
                    "pro-trader"
                } else {
                    "custom"
                }
                .to_string(),
                palette: if is_default_theme {
                    None
                } else {
                    Some(self.0.palette())
                },
            };
            ser_theme.serialize(serializer)
        } else {
            let theme_str = match self.0 {
                iced_core::Theme::Ferra => "ferra",
                iced_core::Theme::Dark => "dark",
                iced_core::Theme::Light => "light",
                iced_core::Theme::Dracula => "dracula",
                iced_core::Theme::Nord => "nord",
                iced_core::Theme::SolarizedLight => "solarized_light",
                iced_core::Theme::SolarizedDark => "solarized_dark",
                iced_core::Theme::GruvboxLight => "gruvbox_light",
                iced_core::Theme::GruvboxDark => "gruvbox_dark",
                iced_core::Theme::CatppuccinLatte => "catppuccino_latte",
                iced_core::Theme::CatppuccinFrappe => "catppuccino_frappe",
                iced_core::Theme::CatppuccinMacchiato => "catppuccino_macchiato",
                iced_core::Theme::CatppuccinMocha => "catppuccino_mocha",
                iced_core::Theme::TokyoNight => "tokyo_night",
                iced_core::Theme::TokyoNightStorm => "tokyo_night_storm",
                iced_core::Theme::TokyoNightLight => "tokyo_night_light",
                iced_core::Theme::KanagawaWave => "kanagawa_wave",
                iced_core::Theme::KanagawaDragon => "kanagawa_dragon",
                iced_core::Theme::KanagawaLotus => "kanagawa_lotus",
                iced_core::Theme::Moonfly => "moonfly",
                iced_core::Theme::Nightfly => "nightfly",
                iced_core::Theme::Oxocarbon => "oxocarbon",
                _ => unreachable!(),
            };
            theme_str.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Theme {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value =
            serde_json::Value::deserialize(deserializer).map_err(serde::de::Error::custom)?;

        if let Some(s) = value.as_str() {
            let theme = match s {
                "ferra" => iced_core::Theme::Ferra,
                "dark" => iced_core::Theme::Dark,
                "light" => iced_core::Theme::Light,
                "dracula" => iced_core::Theme::Dracula,
                "nord" => iced_core::Theme::Nord,
                "solarized_light" => iced_core::Theme::SolarizedLight,
                "solarized_dark" => iced_core::Theme::SolarizedDark,
                "gruvbox_light" => iced_core::Theme::GruvboxLight,
                "gruvbox_dark" => iced_core::Theme::GruvboxDark,
                "catppuccino_latte" => iced_core::Theme::CatppuccinLatte,
                "catppuccino_frappe" => iced_core::Theme::CatppuccinFrappe,
                "catppuccino_macchiato" => iced_core::Theme::CatppuccinMacchiato,
                "catppuccino_mocha" => iced_core::Theme::CatppuccinMocha,
                "tokyo_night" => iced_core::Theme::TokyoNight,
                "tokyo_night_storm" => iced_core::Theme::TokyoNightStorm,
                "tokyo_night_light" => iced_core::Theme::TokyoNightLight,
                "kanagawa_wave" => iced_core::Theme::KanagawaWave,
                "kanagawa_dragon" => iced_core::Theme::KanagawaDragon,
                "kanagawa_lotus" => iced_core::Theme::KanagawaLotus,
                "moonfly" => iced_core::Theme::Moonfly,
                "nightfly" => iced_core::Theme::Nightfly,
                "oxocarbon" => iced_core::Theme::Oxocarbon,
                "pro-trader" => Theme::default().0,
                "high-contrast" => iced_core::Theme::Custom(high_contrast_trading_theme().into()),
                "light-trader" => iced_core::Theme::Custom(light_trading_theme().into()),
                "footprint-pro" => iced_core::Theme::Custom(footprint_optimized_theme().into()),
                "lux-chart" | "flowsurface" => Theme::default().0,
                _ => {
                    return Err(serde::de::Error::custom(format!("Invalid theme: {}", s)));
                }
            };
            return Ok(Theme(theme));
        }

        let serialized = SerTheme::deserialize(value).map_err(serde::de::Error::custom)?;

        let theme = match serialized.name.as_str() {
            "pro-trader" | "lux-chart" | "flowsurface" => Theme::default().0,
            "high-contrast" => iced_core::Theme::Custom(high_contrast_trading_theme().into()),
            "light-trader" => iced_core::Theme::Custom(light_trading_theme().into()),
            "footprint-pro" => iced_core::Theme::Custom(footprint_optimized_theme().into()),
            "custom" => {
                if let Some(palette) = serialized.palette {
                    iced_core::Theme::Custom(Custom::new("Custom".to_string(), palette).into())
                } else {
                    return Err(serde::de::Error::custom(
                        "Custom theme missing palette data",
                    ));
                }
            }
            _ => return Err(serde::de::Error::custom("Invalid theme")),
        };

        Ok(Theme(theme))
    }
}

/// NEW: Enhanced color utility functions for trading visualization
pub fn get_rejection_color(
    zone_type: crate::chart::kline::RejectionType,
    palette: &Palette,
) -> Color {
    match zone_type {
        crate::chart::kline::RejectionType::BuyerRejection => brighten(palette.success, 0.2),
        crate::chart::kline::RejectionType::SellerRejection => brighten(palette.danger, 0.2),
        crate::chart::kline::RejectionType::KeyLevel => brighten(palette.warning, 0.2),
    }
}

/// NEW: Get color for large orders with intensity based on volume
pub fn get_large_order_color(
    is_buy: bool,
    volume: f32,
    max_volume: f32,
    palette: &Palette,
) -> Color {
    let base_color = if is_buy {
        palette.success
    } else {
        palette.danger
    };
    let intensity = (volume / max_volume.max(1.0)).min(1.0);
    let alpha = 0.5 + (intensity * 0.5);
    base_color.scale_alpha(alpha)
}

/// NEW: Get enhanced colors for volume bars with better contrast
pub fn get_volume_bar_color(
    volume: f32,
    max_volume: f32,
    is_buy: bool,
    palette: &Palette,
) -> Color {
    let base_color = if is_buy {
        palette.success
    } else {
        palette.danger
    };
    let intensity = (volume / max_volume.max(1.0)).min(1.0);

    // Enhanced contrast for better visibility
    if intensity > 0.7 {
        brighten(base_color, 0.3)
    } else if intensity < 0.3 {
        darken(base_color, 0.2)
    } else {
        base_color
    }
}

/// NEW: Brighten a color (more sophisticated than lighten)
pub fn brighten(color: Color, amount: f32) -> Color {
    let mut hsva = to_hsva(color);
    hsva.value = (hsva.value + amount).min(1.0);
    hsva.saturation = (hsva.saturation + amount * 0.2).min(1.0);
    from_hsva(hsva)
}

/// NEW: Create gradient colors for smooth transitions
pub fn create_gradient(start: Color, end: Color, steps: usize) -> Vec<Color> {
    let start_hsva = to_hsva(start);
    let end_hsva = to_hsva(end);

    (0..steps)
        .map(|i| {
            let ratio = i as f32 / (steps - 1) as f32;
            let h = start_hsva.hue.into_degrees()
                + (end_hsva.hue.into_degrees() - start_hsva.hue.into_degrees()) * ratio;
            let s = start_hsva.saturation + (end_hsva.saturation - start_hsva.saturation) * ratio;
            let v = start_hsva.value + (end_hsva.value - start_hsva.value) * ratio;
            let a = start_hsva.alpha + (end_hsva.alpha - start_hsva.alpha) * ratio;

            from_hsva(Hsva::new(RgbHue::from_degrees(h), s, v, a))
        })
        .collect()
}

/// NEW: Check if color combination has sufficient contrast for accessibility
pub fn has_sufficient_contrast(foreground: Color, background: Color) -> bool {
    let l1 = luminance(foreground);
    let l2 = luminance(background);
    let contrast = if l1 > l2 {
        (l1 + 0.05) / (l2 + 0.05)
    } else {
        (l2 + 0.05) / (l1 + 0.05)
    };
    contrast >= 4.5 // WCAG AA standard
}

/// NEW: Calculate luminance for contrast checking
fn luminance(color: Color) -> f32 {
    let r = if color.r <= 0.03928 {
        color.r / 12.92
    } else {
        ((color.r + 0.055) / 1.055).powf(2.4)
    };
    let g = if color.g <= 0.03928 {
        color.g / 12.92
    } else {
        ((color.g + 0.055) / 1.055).powf(2.4)
    };
    let b = if color.b <= 0.03928 {
        color.b / 12.92
    } else {
        ((color.b + 0.055) / 1.055).powf(2.4)
    };
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// NEW: Generate complementary colors for better visual hierarchy
pub fn get_complementary_color(color: Color) -> Color {
    let mut hsva = to_hsva(color);
    hsva.hue = hsva.hue + RgbHue::from_degrees(180.0);
    from_hsva(hsva)
}

/// NEW: Generate analogous colors (colors next to each other on color wheel)
pub fn get_analogous_colors(color: Color, count: usize) -> Vec<Color> {
    let base_hsva = to_hsva(color);
    let step = 30.0; // 30 degree steps

    (0..count)
        .map(|i| {
            let offset = (i as f32 - (count as f32 - 1.0) / 2.0) * step;
            let mut hsva = base_hsva;
            hsva.hue = hsva.hue + RgbHue::from_degrees(offset);
            from_hsva(hsva)
        })
        .collect()
}

pub fn hex_to_color(hex: &str) -> Option<Color> {
    if hex.len() == 7 || hex.len() == 9 {
        let hash = &hex[0..1];
        let r = u8::from_str_radix(&hex[1..3], 16);
        let g = u8::from_str_radix(&hex[3..5], 16);
        let b = u8::from_str_radix(&hex[5..7], 16);
        let a = (hex.len() == 9)
            .then(|| u8::from_str_radix(&hex[7..9], 16).ok())
            .flatten();

        return match (hash, r, g, b, a) {
            ("#", Ok(r), Ok(g), Ok(b), None) => Some(Color {
                r: f32::from(r) / 255.0,
                g: f32::from(g) / 255.0,
                b: f32::from(b) / 255.0,
                a: 1.0,
            }),
            ("#", Ok(r), Ok(g), Ok(b), Some(a)) => Some(Color {
                r: f32::from(r) / 255.0,
                g: f32::from(g) / 255.0,
                b: f32::from(b) / 255.0,
                a: f32::from(a) / 255.0,
            }),
            _ => None,
        };
    }

    None
}

pub fn color_to_hex(color: Color) -> String {
    use std::fmt::Write;

    let mut hex = String::with_capacity(9);

    let [r, g, b, a] = color.into_rgba8();

    let _ = write!(&mut hex, "#");
    let _ = write!(&mut hex, "{r:02X}");
    let _ = write!(&mut hex, "{g:02X}");
    let _ = write!(&mut hex, "{b:02X}");

    if a < u8::MAX {
        let _ = write!(&mut hex, "{a:02X}");
    }

    hex
}

pub fn from_hsva(color: Hsva) -> Color {
    to_color(palette::Srgba::from_color(color))
}

fn to_color(rgba: Rgba) -> Color {
    Color {
        r: rgba.color.red,
        g: rgba.color.green,
        b: rgba.color.blue,
        a: rgba.alpha,
    }
}

pub fn to_hsva(color: Color) -> Hsva {
    Hsva::from_color(to_rgba(color))
}

fn to_rgb(color: Color) -> Rgb {
    Rgb {
        red: color.r,
        green: color.g,
        blue: color.b,
        ..Rgb::default()
    }
}

fn to_rgba(color: Color) -> Rgba {
    Rgba {
        alpha: color.a,
        color: to_rgb(color),
    }
}

pub fn darken(color: Color, amount: f32) -> Color {
    let mut hsl = to_hsl(color);

    hsl.l = if hsl.l - amount < 0.0 {
        0.0
    } else {
        hsl.l - amount
    };

    from_hsl(hsl)
}

pub fn lighten(color: Color, amount: f32) -> Color {
    let mut hsl = to_hsl(color);

    hsl.l = if hsl.l + amount > 1.0 {
        1.0
    } else {
        hsl.l + amount
    };

    from_hsl(hsl)
}

fn to_hsl(color: Color) -> Hsl {
    let x_max = color.r.max(color.g).max(color.b);
    let x_min = color.r.min(color.g).min(color.b);
    let c = x_max - x_min;
    let l = x_max.midpoint(x_min);

    let h = if c == 0.0 {
        0.0
    } else if x_max == color.r {
        60.0 * ((color.g - color.b) / c).rem_euclid(6.0)
    } else if x_max == color.g {
        60.0 * (((color.b - color.r) / c) + 2.0)
    } else {
        // x_max == color.b
        60.0 * (((color.r - color.g) / c) + 4.0)
    };

    let s = if l == 0.0 || l == 1.0 {
        0.0
    } else {
        (x_max - l) / l.min(1.0 - l)
    };

    Hsl {
        h,
        s,
        l,
        a: color.a,
    }
}

pub fn is_dark(color: Color) -> bool {
    let brightness = (color.r * 299.0 + color.g * 587.0 + color.b * 114.0) / 1000.0;
    brightness < 0.5
}

struct Hsl {
    h: f32,
    s: f32,
    l: f32,
    a: f32,
}

// https://en.wikipedia.org/wiki/HSL_and_HSV#HSL_to_RGB
fn from_hsl(hsl: Hsl) -> Color {
    let c = (1.0 - (2.0 * hsl.l - 1.0).abs()) * hsl.s;
    let h = hsl.h / 60.0;
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
        // h < 6.0
        (c, 0.0, x)
    };

    let m = hsl.l - (c / 2.0);

    Color {
        r: r1 + m,
        g: g1 + m,
        b: b1 + m,
        a: hsl.a,
    }
}

pub fn from_hsv_degrees(h_deg: f32, s: f32, v: f32) -> Color {
    // Hue in degrees [0,360), s,v in [0,1]
    let hue = RgbHue::from_degrees(h_deg);
    from_hsva(Hsva::new(hue, s, v, 1.0))
}

/// NEW: Theme manager for dynamic theme switching
pub struct ThemeManager {
    current_theme: Theme,
    available_themes: Vec<(String, Theme)>,
}

impl ThemeManager {
    pub fn new() -> Self {
        let themes = vec![
            ("Pro Trader".to_string(), Theme::default()),
            (
                "High Contrast".to_string(),
                Theme(iced_core::Theme::Custom(
                    high_contrast_trading_theme().into(),
                )),
            ),
            (
                "Light Trader".to_string(),
                Theme(iced_core::Theme::Custom(light_trading_theme().into())),
            ),
            (
                "Footprint Pro".to_string(),
                Theme(iced_core::Theme::Custom(footprint_optimized_theme().into())),
            ),
        ];

        Self {
            current_theme: Theme::default(),
            available_themes: themes,
        }
    }

    pub fn get_current_theme(&self) -> &Theme {
        &self.current_theme
    }

    pub fn get_available_themes(&self) -> &[(String, Theme)] {
        &self.available_themes
    }

    pub fn set_theme(&mut self, theme_name: &str) -> bool {
        if let Some((_, theme)) = self
            .available_themes
            .iter()
            .find(|(name, _)| name == theme_name)
        {
            self.current_theme = theme.clone();
            true
        } else {
            false
        }
    }
}
