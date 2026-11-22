//! Layered page numbers (see [`designcraft_doc::pagenum`]): the digits as overlapping ink plates,
//! and the colours where they overlap. [`apply`] draws them for the Current Page Number markers
//! of a composed story; [`layered`] gives the shapes alone (the panel's previews use it).

use designcraft_color::{Color, cms::Intent};
use designcraft_doc::pagenum::{Cmyk, Design, Dot, Mix, Offset, RegionFill, Settings, Top, plate_of, region_key};
use designcraft_doc::{Document, Story};
use designcraft_fonts::ScopedFonts;
use designcraft_geom::path::PathData;
use designcraft_geom::pathfinder::{Op, combine};
use designcraft_geom::{Affine, BezPath, Point, Rect, Shape as _};

use crate::{ComposedStory, Deco};

/// One filled shape of a layered number.
#[derive(Clone, Debug)]
pub struct Shape {
    pub path: BezPath,
    pub color: Color,
}

/// A layered number: shapes in paint order, in points, y down, the baseline at y = 0 and the
/// digits starting near x = 0.
#[derive(Clone, Debug, Default)]
pub struct Layered {
    pub shapes: Vec<Shape>,
    pub bounds: Rect,
}

// --- randomness --------------------------------------------------------------------------------

/// SplitMix64: small, fast and the same everywhere, so a page looks the same on every machine.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in -1..1.
    fn signed(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

fn hash(s: &str, salt: u64) -> u64 {
    s.bytes().fold(0xCBF2_9CE4_8422_2325 ^ salt, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3))
}

// --- geometry ----------------------------------------------------------------------------------

/// The shapes of `text` (a page name) set in `design` at `size` points.
pub fn layered(design: &Design, text: &str, size: f64, db: &ScopedFonts) -> Option<Layered> {
    let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
    if chars.is_empty() || !(size.is_finite() && size > 0.0) {
        return None;
    }
    let face = db.face(&design.font_family, &design.font_style);
    let k = size / face.units_per_em();
    let glyphs: Vec<BezPath> = chars
        .iter()
        .map(|&c| {
            let mut p = (*db.outline(&face, face.glyph_for(c))).clone();
            p.apply_affine(Affine::scale(k));
            p
        })
        .collect();
    let boxes: Vec<Rect> = glyphs.iter().map(|p| p.bounding_box()).collect();
    let inked: Vec<&Rect> = boxes.iter().filter(|b| b.width() > 0.0).collect();
    if inked.is_empty() {
        return None;
    }
    let dw = inked.iter().map(|b| b.width()).sum::<f64>() / inked.len() as f64;
    let dh = inked.iter().map(|b| b.height()).fold(0.0, f64::max);
    let reroll = u64::from(design.rerolls.get(text).copied().unwrap_or(0));
    let mut rng = Rng::new(hash(text, reroll.wrapping_mul(0x5851_F42D_4C95_7F2D)));
    let n = chars.len();
    // Centres across, and offsets down, digit by digit.
    let (mut xs, mut ys) = (vec![0.0; n], vec![0.0; n]);
    for i in 1..n {
        let step = match design.spacing {
            Offset::Variable { tendency, spread } => tendency + rng.signed() * spread,
            Offset::Fixed { value } => value,
        };
        xs[i] = xs[i - 1] + step.max(0.05) * dw;
    }
    for (i, y) in ys.iter_mut().enumerate() {
        *y = match design.vertical {
            Offset::Variable { tendency, spread } => (i as f64 * tendency + rng.signed() * spread) * dh,
            Offset::Fixed { value } => i as f64 * value * dh,
        };
    }
    let mean = ys.iter().sum::<f64>() / n as f64;
    // Misregistration: the same for every page of the design.
    let mut reg = Rng::new(hash(&design.name, 7));
    let reg: Vec<(f64, f64)> = (0..3).map(|_| (reg.signed() * design.registration, reg.signed() * design.registration)).collect();
    let mut plates: [Vec<BezPath>; 3] = Default::default();
    for (i, mut p) in glyphs.into_iter().enumerate() {
        let b = boxes[i];
        if b.width() <= 0.0 {
            continue;
        }
        let plate = plate_of(i, n);
        let (rx, ry) = reg[plate];
        p.apply_affine(Affine::translate((xs[i] - b.center().x + dw / 2.0 + rx, ys[i] - mean + ry)));
        plates[plate].push(p);
    }
    let plates: Vec<Option<BezPath>> = plates.iter().map(|ps| union(ps)).collect();
    let shapes = regions(design, &plates);
    let bounds = shapes.iter().map(|s| s.path.bounding_box()).reduce(|a, b| a.union(b)).unwrap_or_default();
    Some(Layered { shapes, bounds })
}

fn to_pd(p: &BezPath) -> PathData {
    PathData::from_bezpath(p)
}

fn union(paths: &[BezPath]) -> Option<BezPath> {
    match paths {
        [] => None,
        [one] => Some(one.clone()),
        _ => combine(Op::Add, &paths.iter().map(to_pd).collect::<Vec<_>>()).map(|p| p.to_bezpath()),
    }
}

/// `keep` minus everything in `cut`.
fn subtract(keep: &BezPath, cut: &[&BezPath]) -> Option<BezPath> {
    if cut.is_empty() {
        return Some(keep.clone());
    }
    let mut v = vec![to_pd(keep)];
    v.extend(cut.iter().map(|p| to_pd(p)));
    combine(Op::Subtract, &v).map(|p| p.to_bezpath())
}

fn intersect(paths: &[&BezPath]) -> Option<BezPath> {
    combine(Op::Intersect, &paths.iter().map(|p| to_pd(p)).collect::<Vec<_>>()).map(|p| p.to_bezpath())
}

/// Plates in paint order, bottom first.
fn paint_order(top: Top) -> [usize; 3] {
    match top {
        Top::Ones => [0, 1, 2],
        Top::Hundreds => [2, 1, 0],
    }
}

/// The plate `path` grown by `edge` points all round.
fn grown(path: &BezPath, edge: f64) -> BezPath {
    if edge <= 0.0 {
        return path.clone();
    }
    let ring = designcraft_geom::kurbo::stroke(
        path.iter(),
        &designcraft_geom::kurbo::Stroke::new(edge * 2.0).with_join(designcraft_geom::kurbo::Join::Round),
        &designcraft_geom::kurbo::StrokeOpts::default(),
        0.05,
    );
    union(&[path.clone(), ring]).unwrap_or_else(|| path.clone())
}

fn regions(design: &Design, plates: &[Option<BezPath>]) -> Vec<Shape> {
    let ink = |p: usize| cmyk(design.inks[p]);
    let order = paint_order(design.top);
    let mut out = Vec::new();
    // Knockout with a cut: every plate whole, minus the grown plates above it.
    if let Mix::Knockout { cut: true, edge, .. } = design.mix {
        for (k, &p) in order.iter().enumerate() {
            let Some(path) = &plates[p] else { continue };
            let above: Vec<BezPath> = order[k + 1..].iter().filter_map(|&q| plates[q].as_ref()).map(|q| grown(q, edge)).collect();
            if let Some(rest) = subtract(path, &above.iter().collect::<Vec<_>>()) {
                out.push(Shape { path: rest, color: ink(p) });
            }
        }
        return out;
    }
    let present: Vec<usize> = (0..3).filter(|&p| plates[p].is_some()).collect();
    for set in 1u8..8 {
        let members: Vec<usize> = (0..3).filter(|p| set >> p & 1 == 1).collect();
        if !members.iter().all(|p| present.contains(p)) {
            continue;
        }
        let inside: Vec<&BezPath> = members.iter().filter_map(|&p| plates[p].as_ref()).collect();
        let others: Vec<&BezPath> = present.iter().filter(|p| !members.contains(p)).filter_map(|&p| plates[p].as_ref()).collect();
        let Some(both) = (if inside.len() == 1 { Some(inside[0].clone()) } else { intersect(&inside) }) else { continue };
        let Some(region) = subtract(&both, &others) else { continue };
        if members.len() == 1 {
            out.push(Shape { path: region, color: ink(members[0]) });
            continue;
        }
        let top = *order.iter().rev().find(|p| members.contains(p)).unwrap_or(&members[0]);
        match fill(design, &design.mix, &members, top, set) {
            Fill::Paper => {}
            Fill::Color(c) => out.push(Shape { path: region, color: c }),
            Fill::Halftone { shape, pitch, size, angles } => {
                for &p in order.iter().filter(|p| members.contains(p)) {
                    let screen = raster(region.bounding_box(), shape, pitch, size, angles[p]);
                    if let Some(dots) = intersect(&[&screen, &region]) {
                        out.push(Shape { path: dots, color: ink(p) });
                    }
                }
            }
        }
    }
    out
}

/// A screen of `shape` elements covering `area`, `pitch` apart, turned by `angle` degrees.
fn raster(area: Rect, shape: Dot, pitch: f64, size: f32, angle: f32) -> BezPath {
    let pitch = pitch.max(0.2);
    let s = f64::from(size.clamp(0.05, 1.0)) * pitch;
    let c = area.center();
    let r = (area.width().hypot(area.height()) / 2.0 / pitch).ceil() as i32 + 1;
    let turn = Affine::rotate_about(f64::from(angle).to_radians(), c);
    let mut out = BezPath::new();
    let mut add = |pts: &[(f64, f64)]| {
        let mut first = true;
        for &(x, y) in pts {
            let p = turn * Point::new(x, y);
            if first {
                out.move_to(p);
                first = false;
            } else {
                out.line_to(p);
            }
        }
        out.close_path();
    };
    match shape {
        Dot::Line => {
            for j in -r..=r {
                let y = c.y + f64::from(j) * pitch;
                let x0 = c.x - f64::from(r) * pitch;
                let x1 = c.x + f64::from(r) * pitch;
                add(&[(x0, y - s / 2.0), (x1, y - s / 2.0), (x1, y + s / 2.0), (x0, y + s / 2.0)]);
            }
        }
        Dot::Round | Dot::Cross => {
            for j in -r..=r {
                for i in -r..=r {
                    let (x, y) = (c.x + f64::from(i) * pitch, c.y + f64::from(j) * pitch);
                    if shape == Dot::Round {
                        let pts: Vec<(f64, f64)> =
                            (0..12).map(|a| f64::from(a) * std::f64::consts::TAU / 12.0).map(|a| (x + s / 2.0 * a.cos(), y + s / 2.0 * a.sin())).collect();
                        add(&pts);
                    } else {
                        let (a, b) = (s / 2.0, s / 6.0);
                        add(&[
                            (x - b, y - a),
                            (x + b, y - a),
                            (x + b, y - b),
                            (x + a, y - b),
                            (x + a, y + b),
                            (x + b, y + b),
                            (x + b, y + a),
                            (x - b, y + a),
                            (x - b, y + b),
                            (x - a, y + b),
                            (x - a, y - b),
                            (x - b, y - b),
                        ]);
                    }
                }
            }
        }
    }
    out
}

// --- colour ------------------------------------------------------------------------------------

enum Fill {
    Paper,
    Color(Color),
    Halftone { shape: Dot, pitch: f64, size: f32, angles: [f32; 3] },
}

fn cmyk(c: Cmyk) -> Color {
    Color::cmyk(c[0], c[1], c[2], c[3])
}

/// A display colour as print CMYK (the working CMYK space).
fn print(rgb: [f32; 3]) -> Color {
    let [c, m, y, k] = Color::rgb(rgb[0].clamp(0.0, 1.0), rgb[1].clamp(0.0, 1.0), rgb[2].clamp(0.0, 1.0)).to_cmyk_managed(Intent::RelativeColorimetric);
    Color::cmyk(c, m, y, k)
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn fill(design: &Design, mix: &Mix, members: &[usize], top: usize, set: u8) -> Fill {
    let inks: Vec<Color> = members.iter().map(|&p| cmyk(design.inks[p])).collect();
    let rgbs: Vec<[f32; 3]> = inks.iter().map(Color::to_rgb).collect();
    let top_rgb = cmyk(design.inks[top]).to_rgb();
    let n = members.len() as f32;
    Fill::Color(match mix {
        Mix::Overprint { ink_limit, opacity } => {
            let mut v = [0.0f32; 4];
            for (k, &p) in members.iter().enumerate() {
                let a = if k == 0 { 1.0 } else { opacity.clamp(0.0, 1.0) };
                for (x, c) in v.iter_mut().zip(design.inks[p]) {
                    *x += c * a;
                }
            }
            let mut v = v.map(|x| x.min(1.0));
            let total: f32 = v.iter().sum();
            let limit = ink_limit.max(1.0);
            if total > limit {
                v = v.map(|x| x * limit / total);
            }
            cmyk(v)
        }
        Mix::Multiply { strength } => {
            let m = rgbs.iter().fold([1.0f32; 3], |a, c| [a[0] * c[0], a[1] * c[1], a[2] * c[2]]);
            print(lerp3(top_rgb, m, strength.clamp(0.0, 1.0)))
        }
        Mix::Pigment { weights, saturation } => {
            let wsum: f32 = members.iter().map(|&p| weights[p].max(0.0)).sum::<f32>().max(1e-3);
            let mut out = [0.0f32; 3];
            for (ch, o) in out.iter_mut().enumerate() {
                let ks: f32 = members
                    .iter()
                    .zip(&rgbs)
                    .map(|(&p, c)| {
                        let r = c[ch].clamp(0.002, 1.0);
                        weights[p].max(0.0) * (1.0 - r).powi(2) / (2.0 * r)
                    })
                    .sum::<f32>()
                    / wsum;
                *o = 1.0 + ks - (ks * ks + 2.0 * ks).sqrt();
            }
            let mut lab = oklab(out);
            lab[1] *= 1.0 + saturation;
            lab[2] *= 1.0 + saturation;
            print(from_oklab(lab))
        }
        Mix::Light { strength, glow, glow_color } => {
            let s = rgbs.iter().fold([0.0f32; 3], |a, c| [1.0 - (1.0 - a[0]) * (1.0 - c[0]), 1.0 - (1.0 - a[1]) * (1.0 - c[1]), 1.0 - (1.0 - a[2]) * (1.0 - c[2])]);
            let c = lerp3(top_rgb, s, strength.clamp(0.0, 1.0));
            print(lerp3(c, cmyk(*glow_color).to_rgb(), glow.clamp(0.0, 1.0)))
        }
        Mix::Perceptual { lightness, chroma } => {
            let mut lab = rgbs.iter().map(|c| oklab(*c)).fold([0.0f32; 3], |a, l| [a[0] + l[0] / n, a[1] + l[1] / n, a[2] + l[2] / n]);
            lab[0] = (lab[0] + lightness * 0.3).clamp(0.0, 1.0);
            lab[1] *= 1.0 + chroma;
            lab[2] *= 1.0 + chroma;
            print(from_oklab(lab))
        }
        Mix::HueJump { angle, chroma } => {
            let labs: Vec<[f32; 3]> = rgbs.iter().map(|c| oklab(*c)).collect();
            let l = labs.iter().map(|l| l[0]).sum::<f32>() / n - 0.05 * (n - 1.0);
            let (s, c) = labs.iter().fold((0.0f32, 0.0f32), |(s, c), l| (s + l[2].atan2(l[1]).sin(), c + l[2].atan2(l[1]).cos()));
            let h = s.atan2(c) + angle.to_radians();
            let cm = labs.iter().map(|l| l[1].hypot(l[2])).fold(0.0, f32::max) * chroma;
            print(from_oklab([l.clamp(0.0, 1.0), cm * h.cos(), cm * h.sin()]))
        }
        Mix::Darken { strength, lighten } => {
            let pick = |a: f32, b: f32| if *lighten { a.max(b) } else { a.min(b) };
            let start = if *lighten { [0.0f32; 3] } else { [1.0f32; 3] };
            let m = rgbs.iter().fold(start, |a, c| [pick(a[0], c[0]), pick(a[1], c[1]), pick(a[2], c[2])]);
            print(lerp3(top_rgb, m, strength.clamp(0.0, 1.0)))
        }
        Mix::Difference { exclusion, strength, min_lightness } => {
            let mut c = rgbs[0];
            for o in &rgbs[1..] {
                for ch in 0..3 {
                    c[ch] = if *exclusion { c[ch] + o[ch] - 2.0 * c[ch] * o[ch] } else { (c[ch] - o[ch]).abs() };
                }
            }
            let c = lerp3(top_rgb, c, strength.clamp(0.0, 1.0));
            let mut lab = oklab(c);
            lab[0] = lab[0].max(*min_lightness);
            print(from_oklab(lab))
        }
        Mix::Knockout { fill, .. } => return fill.map_or(Fill::Paper, |c| Fill::Color(cmyk(c))),
        Mix::Depth { step } => {
            let f = (1.0 - step.clamp(0.0, 1.0)).powi(members.len() as i32 - 1);
            let [c, m, y, k] = design.inks[top];
            cmyk([c, m, y, (1.0 - (1.0 - k) * f).clamp(0.0, 1.0)])
        }
        Mix::Halftone { shape, pitch, size, angles } => return Fill::Halftone { shape: *shape, pitch: *pitch, size: *size, angles: *angles },
        Mix::Custom { base } => {
            return match design.custom.get(&region_key(set)) {
                Some(RegionFill::Paper) => Fill::Paper,
                Some(RegionFill::Color { cmyk: c }) => Fill::Color(cmyk(*c)),
                None => match base.as_ref() {
                    Mix::Custom { .. } => Fill::Paper,
                    b => fill(design, b, members, top, set),
                },
            };
        }
    })
}

fn to_lin(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn from_lin(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

fn oklab(c: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = c.map(to_lin);
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn from_oklab(lab: [f32; 3]) -> [f32; 3] {
    let [l, a, b] = lab;
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_,
        -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_,
        -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_,
    ]
    .map(from_lin)
}

// --- in composed text ---------------------------------------------------------------------------

/// The character style name in force at byte `at` of `story`.
fn char_style_at(story: &Story, at: usize) -> &str {
    let mut pos = 0;
    for r in &story.chars {
        if at < pos + r.len {
            return &r.format.style;
        }
        pos += r.len;
    }
    story.chars.last().map_or("", |r| r.format.style.as_str())
}

/// Draw the Current Page Number markers whose character style has a design as layered digits:
/// their glyphs are hidden and the plates are added as decorations, centred where the glyphs
/// were.
pub(crate) fn apply(doc: &Document, story: &Story, out: &mut ComposedStory, db: &ScopedFonts) {
    use designcraft_doc::story::PAGE_NUMBER;
    for ft in &mut out.frames {
        ft.decos.retain(|d| !d.page_number);
    }
    if !story.text.contains(PAGE_NUMBER) || !doc.labels.contains_key(designcraft_doc::pagenum::LABEL) {
        return;
    }
    let settings = Settings::from_labels(&doc.labels);
    if settings.styles.is_empty() {
        return;
    }
    let is_marker = |b: usize| story.text.get(b..).is_some_and(|t| t.starts_with(PAGE_NUMBER));
    for ft in &mut out.frames {
        for line in &mut ft.lines {
            let mut i = 0;
            while i < line.glyphs.len() {
                let byte = line.glyphs[i].byte;
                if !is_marker(byte) || line.glyphs[i].list_label {
                    i += 1;
                    continue;
                }
                let end = (i..line.glyphs.len()).find(|&j| line.glyphs[j].byte != byte).unwrap_or(line.glyphs.len());
                let run = i..end;
                i = end;
                let Some(design) = settings.for_style(char_style_at(story, byte)) else { continue };
                let text: String = line.glyphs[run.clone()].iter().map(|g| g.rendered_char).collect();
                let g0 = &line.glyphs[run.start];
                let size = out.styles.get(g0.style as usize).map_or(12.0, |s| s.size);
                let Some(l) = layered(design, &text, size, db) else { continue };
                let left = line.glyphs[run.clone()].iter().map(|g| g.x - g.dx).fold(f64::INFINITY, f64::min);
                let width: f64 = line.glyphs[run.clone()].iter().map(|g| g.adv).sum();
                let dx = left + (width - l.bounds.width()) / 2.0 - l.bounds.x0;
                let at = Affine::translate((dx, line.baseline + g0.y));
                for g in &mut line.glyphs[run] {
                    g.visible = false;
                }
                for s in l.shapes {
                    let path = at * s.path;
                    ft.decos.push(Deco { rect: path.bounding_box(), path: Some(path), tint: 1.0, value: Some(s.color), page_number: true, ..Deco::default() });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designcraft_doc::pagenum::Design;

    fn db() -> &'static designcraft_fonts::FontDb {
        designcraft_fonts::FontDb::global()
    }

    fn area(p: &BezPath) -> f64 {
        p.area().abs()
    }

    #[test]
    fn digits_overlap_into_mixed_regions() {
        let db = db().scoped(0);
        let d = Design { spacing: Offset::Fixed { value: 0.5 }, vertical: Offset::Fixed { value: 0.0 }, ..Design::default() };
        let l = layered(&d, "568", 40.0, &db).unwrap();
        let inks: Vec<Color> = d.inks.iter().map(|c| cmyk(*c)).collect();
        assert!(l.shapes.iter().any(|s| !inks.contains(&s.color)), "an overlap gets a mixed colour");
        assert!(l.bounds.width() > 0.0 && l.bounds.height() > 0.0);
    }

    #[test]
    fn a_page_looks_the_same_every_time_and_rerolls_change_it() {
        let db = db().scoped(0);
        let mut d = Design::default();
        let a = layered(&d, "712", 30.0, &db).unwrap().bounds;
        assert_eq!(a, layered(&d, "712", 30.0, &db).unwrap().bounds);
        d.rerolls.insert("712".into(), 1);
        assert_ne!(a, layered(&d, "712", 30.0, &db).unwrap().bounds, "rerolled");
    }

    #[test]
    fn wider_fixed_spacing_spreads_the_digits() {
        let db = db().scoped(0);
        let near = Design { spacing: Offset::Fixed { value: 0.4 }, ..Design::default() };
        let far = Design { spacing: Offset::Fixed { value: 0.9 }, ..Design::default() };
        assert!(layered(&far, "888", 30.0, &db).unwrap().bounds.width() > layered(&near, "888", 30.0, &db).unwrap().bounds.width());
    }

    #[test]
    fn knockout_leaves_overlaps_unprinted() {
        let db = db().scoped(0);
        let fixed = |mix| Design { spacing: Offset::Fixed { value: 0.5 }, vertical: Offset::Fixed { value: 0.0 }, mix, ..Design::default() };
        let ink = |d: &Design| -> f64 { layered(d, "88", 40.0, &db).unwrap().shapes.iter().map(|s| area(&s.path)).sum() };
        let over = ink(&fixed(Mix::Overprint { ink_limit: 3.0, opacity: 1.0 }));
        let knocked = ink(&fixed(Mix::Knockout { cut: false, edge: 0.0, fill: None }));
        assert!(knocked < over, "{knocked} < {over}");
    }
}
