//! Layered page numbers: designs that draw a page number's digits as overlapping ink plates.
//!
//! Each digit position is one plate (hundreds, tens, ones; longer numbers repeat the three from
//! the right). The digits are set apart by a random or fixed step, and where plates overlap a
//! mixing mode decides the colour. The designs and which character style uses which design are
//! saved as JSON in the document label [`LABEL`] (see [`Settings`]): a Current Page Number marker
//! set in such a character style is drawn as layered digits instead of ordinary text.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The document label holding the [`Settings`] as JSON.
pub const LABEL: &str = "pageNumbers";

/// The document's page number designs and the character styles that use them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub designs: Vec<Design>,
    /// Character style name → design name.
    pub styles: BTreeMap<String, String>,
}

impl Settings {
    /// The settings stored in `labels` (none, or unreadable JSON: no designs).
    pub fn from_labels(labels: &BTreeMap<String, String>) -> Settings {
        labels.get(LABEL).and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn design(&self, name: &str) -> Option<&Design> {
        self.designs.iter().find(|d| d.name == name)
    }

    /// The design character style `style` uses.
    pub fn for_style(&self, style: &str) -> Option<&Design> {
        self.styles.get(style).and_then(|d| self.design(d))
    }
}

/// CMYK, each 0..1.
pub type Cmyk = [f32; 4];

/// How far each digit moves from the one before it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Offset {
    /// Random on every page: around `tendency`, by up to ± `spread`.
    Variable { tendency: f64, spread: f64 },
    /// The same on every page.
    Fixed { value: f64 },
}

/// Which plate is printed last (lies on top where the mode cares: knockout, depth).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Top {
    #[default]
    Ones,
    Hundreds,
}

/// Raster element shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Dot {
    #[default]
    Round,
    Line,
    Cross,
}

/// How overlapping plates mix. Single plates always show their own ink.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Mix {
    /// Ink amounts add up (subtractive), capped at `ink_limit` total coverage (3.0 = 300 %).
    Overprint { ink_limit: f32, opacity: f32 },
    /// Transparent-filter model: display colours multiply.
    Multiply { strength: f32 },
    /// Paint mixing (single-constant Kubelka-Munk), weighted per plate.
    Pigment { weights: [f32; 3], saturation: f32 },
    /// Light (screen): overlaps get lighter, `glow` further towards `glow_color`.
    Light { strength: f32, glow: f32, glow_color: Cmyk },
    /// The perceptual (OKLab) mean, then lighter or darker and more or less colourful.
    Perceptual { lightness: f32, chroma: f32 },
    /// A step round the colour wheel from the inks' mean hue.
    HueJump { angle: f32, chroma: f32 },
    /// The darker (or, with `lighten`, the lighter) of each channel.
    Darken { strength: f32, lighten: bool },
    /// Difference (or the softer exclusion), never darker than `min_lightness`.
    Difference { exclusion: bool, strength: f32, min_lightness: f32 },
    /// Overlaps show paper (or `fill`); with `cut` the upper plate cuts into the lower ones with
    /// an `edge` of paper around it (points) instead.
    Knockout { cut: bool, edge: f64, fill: Option<Cmyk> },
    /// Each extra layer darkens the top plate's ink by `step`.
    Depth { step: f32 },
    /// Overlaps are a raster of every plate's ink: `pitch` points apart, `size` 0..1 of it.
    Halftone { shape: Dot, pitch: f64, size: f32, angles: [f32; 3] },
    /// Every overlap from `base`, except those in [`Design::custom`].
    Custom { base: Box<Mix> },
}

impl Mix {
    /// Every mode with its default settings, in menu order.
    pub fn all() -> Vec<Mix> {
        vec![
            Mix::Overprint { ink_limit: 3.0, opacity: 1.0 },
            Mix::Multiply { strength: 1.0 },
            Mix::Pigment { weights: [1.0; 3], saturation: 0.0 },
            Mix::Light { strength: 1.0, glow: 0.3, glow_color: [0.0, 0.03, 0.12, 0.0] },
            Mix::Perceptual { lightness: 0.0, chroma: 0.0 },
            Mix::HueJump { angle: 35.0, chroma: 1.0 },
            Mix::Darken { strength: 1.0, lighten: false },
            Mix::Difference { exclusion: false, strength: 1.0, min_lightness: 0.3 },
            Mix::Knockout { cut: false, edge: 0.0, fill: None },
            Mix::Depth { step: 0.35 },
            Mix::Halftone { shape: Dot::Round, pitch: 1.2, size: 0.85, angles: [15.0, 75.0, 45.0] },
            Mix::Custom { base: Box::new(Mix::Overprint { ink_limit: 3.0, opacity: 1.0 }) },
        ]
    }

    /// The mode's English name (a translation key).
    pub fn name(&self) -> &'static str {
        match self {
            Mix::Overprint { .. } => "Overprint",
            Mix::Multiply { .. } => "Multiply",
            Mix::Pigment { .. } => "Pigment",
            Mix::Light { .. } => "Light",
            Mix::Perceptual { .. } => "Perceptual",
            Mix::HueJump { .. } => "Hue Jump",
            Mix::Darken { .. } => "Darken",
            Mix::Difference { .. } => "Difference",
            Mix::Knockout { .. } => "Knockout",
            Mix::Depth { .. } => "Depth",
            Mix::Halftone { .. } => "Halftone",
            Mix::Custom { .. } => "Custom",
        }
    }
}

/// What one overlap shows in [`Mix::Custom`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RegionFill {
    Color { cmyk: Cmyk },
    Paper,
}

/// A layered page number design.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Design {
    pub name: String,
    pub font_family: String,
    /// The font style, with axis values for a variable font: `Black {opsz:72,SOFT:50}`.
    pub font_style: String,
    /// Hundreds, tens, ones.
    pub inks: [Cmyk; 3],
    /// Step to the next digit, a fraction of the digits' mean width.
    pub spacing: Offset,
    /// Step up (negative) or down (positive) to the next digit, a fraction of the digit height.
    pub vertical: Offset,
    /// Misregistration: each plate moves up to this far (points), the same on every page.
    pub registration: f64,
    pub top: Top,
    pub mix: Mix,
    /// [`Mix::Custom`] overlaps by plate set: "12" hundreds+tens, "13", "23", "123".
    pub custom: BTreeMap<String, RegionFill>,
    /// Page name → extra seed ("Reroll" for one page).
    pub rerolls: BTreeMap<String, u32>,
}

impl Default for Design {
    fn default() -> Self {
        Design {
            name: "Riso".into(),
            font_family: "Source Serif 4".into(),
            font_style: "Black".into(),
            inks: [[0.0, 0.8, 0.85, 0.0], [0.2, 0.65, 0.85, 0.55], [0.75, 0.25, 0.0, 0.0]],
            spacing: Offset::Variable { tendency: 0.55, spread: 0.09 },
            vertical: Offset::Variable { tendency: 0.0, spread: 0.22 },
            registration: 0.0,
            top: Top::Ones,
            mix: Mix::Overprint { ink_limit: 3.0, opacity: 1.0 },
            custom: BTreeMap::new(),
            rerolls: BTreeMap::new(),
        }
    }
}

/// The plate (0 hundreds, 1 tens, 2 ones) of character `i` of an `n`-character number: the last
/// three are hundreds, tens, ones, and earlier ones repeat that from the right.
pub fn plate_of(i: usize, n: usize) -> usize {
    2 - (n - 1 - i) % 3
}

/// The [`Design::custom`] key of a plate set (bit 0 hundreds, 1 tens, 2 ones).
pub fn region_key(set: u8) -> String {
    (0..3).filter(|p| set >> p & 1 == 1).map(|p| char::from(b'1' + p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_through_the_label() {
        let mut s = Settings::default();
        let mut d = Design { name: "A".into(), ..Design::default() };
        d.custom.insert(region_key(0b101), RegionFill::Paper);
        d.mix = Mix::Custom { base: Box::new(Mix::Light { strength: 1.0, glow: 0.5, glow_color: [0.0; 4] }) };
        s.designs.push(d);
        s.styles.insert("Pagina".into(), "A".into());
        let mut labels = BTreeMap::new();
        labels.insert(LABEL.to_string(), s.to_json());
        let back = Settings::from_labels(&labels);
        assert_eq!(back, s);
        assert!(back.for_style("Pagina").is_some());
        assert!(back.for_style("Other").is_none());
    }

    #[test]
    fn the_last_three_digits_are_hundreds_tens_ones() {
        assert_eq!((0..3).map(|i| plate_of(i, 3)).collect::<Vec<_>>(), [0, 1, 2]);
        assert_eq!((0..2).map(|i| plate_of(i, 2)).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(plate_of(0, 1), 2);
        assert_eq!((0..4).map(|i| plate_of(i, 4)).collect::<Vec<_>>(), [2, 0, 1, 2]);
    }
}
