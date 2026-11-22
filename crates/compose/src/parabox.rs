//! Paragraph border and shading: one box per column a paragraph (or a run of consecutive
//! paragraphs with the same settings) sits in, drawn once the lines are in place.

use designcraft_doc::{BoxBottom, BoxTop, BoxWidth, Cap, Document, Join, ParaProps, StrokeType};
use designcraft_geom::corners::{Corner, box_outline};
use designcraft_geom::{BezPath, Rect, Shape, kurbo};

use crate::{ComposedStory, Deco, FrameSpec, Line};

/// Flattening tolerance of stroked outlines (points).
const TOL: f64 = 0.05;
/// Most dashes drawn along one border (more falls back to a solid stroke).
const MAX_DASHES: f64 = 20_000.0;

/// The border and shading settings of a paragraph (the stroke type resolved against the document's
/// stroke styles). Consecutive paragraphs with equal settings share a box when they merge.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ParaBox {
    shading: Option<Shading>,
    border: Option<Border>,
    merge: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct Shading {
    color: String,
    tint: f32,
    offsets: [f64; 4],
    corners: [Corner; 4],
    width: BoxWidth,
    top: BoxTop,
    bottom: BoxBottom,
    clip: bool,
    nonprinting: bool,
    overprint: bool,
    shadow: Option<Shadow>,
    speckle: Option<Speckle>,
    noise: Option<Noise>,
}

/// Paper grain over the shading box (dark, multiplied) and its shadow (light flecks).
#[derive(Clone, Debug, PartialEq)]
struct Noise {
    /// 0 … 1: how strong the grain is.
    amount: f64,
}

/// Small specks of `color` scattered over the shading box (and its shadow).
#[derive(Clone, Debug, PartialEq)]
struct Speckle {
    color: String,
    tint: f32,
    overprint: bool,
    /// 0 … 1: how many of the possible specks are drawn.
    amount: f64,
    /// The specks lie over the paragraph's text (and its inline objects), not under it.
    over_text: bool,
}

/// A solid copy of the shading box behind it, moved by `offset` (right, down).
#[derive(Clone, Debug, PartialEq)]
struct Shadow {
    color: String,
    tint: f32,
    overprint: bool,
    offset: [f64; 2],
}

#[derive(Clone, Debug, PartialEq)]
struct Border {
    color: String,
    tint: f32,
    /// Top, left, bottom, right.
    weights: [f64; 4],
    offsets: [f64; 4],
    kind: StrokeType,
    cap: Cap,
    join: Join,
    corners: [Corner; 4],
    width: BoxWidth,
    top: BoxTop,
    bottom: BoxBottom,
    display_if_splits: bool,
    gap_color: String,
    gap_tint: f32,
    overprint: bool,
    gap_overprint: bool,
}

fn finite(v: f64) -> f64 {
    if v.is_finite() { v } else { 0.0 }
}

impl ParaBox {
    pub(crate) fn of(doc: &Document, pp: &ParaProps) -> Option<ParaBox> {
        if !pp.shading_on && !pp.border_on {
            return None;
        }
        let shading = pp.shading_on.then(|| Shading {
            color: pp.shading_color.clone(),
            tint: pp.shading_tint,
            offsets: pp.shading_offsets.map(finite),
            corners: pp.shading_corners,
            width: pp.shading_width,
            top: pp.shading_top,
            bottom: pp.shading_bottom,
            clip: pp.shading_clip,
            nonprinting: pp.shading_nonprinting,
            overprint: pp.shading_overprint,
            shadow: pp.shading_shadow_on.then(|| Shadow {
                color: pp.shading_shadow_color.clone(),
                tint: pp.shading_shadow_tint,
                overprint: pp.shading_shadow_overprint,
                offset: pp.shading_shadow_offset.map(|v| finite(v).clamp(-1000.0, 1000.0)),
            }),
            speckle: (finite(pp.shading_speckle) > 0.0).then(|| Speckle {
                color: pp.shading_speckle_color.clone(),
                tint: pp.shading_speckle_tint,
                overprint: pp.shading_speckle_overprint,
                amount: finite(pp.shading_speckle).clamp(0.0, 1.0),
                over_text: pp.shading_speckle_over_text,
            }),
            noise: (finite(pp.shading_noise) > 0.0).then(|| Noise { amount: finite(pp.shading_noise).clamp(0.0, 1.0) }),
        });
        let border = pp.border_on.then(|| Border {
            color: pp.border_color.clone(),
            tint: pp.border_tint,
            weights: pp.border_weights.map(|w| finite(w).clamp(0.0, 1000.0)),
            offsets: pp.border_offsets.map(finite),
            kind: doc.stroke_kind(&pp.border_type),
            cap: pp.border_cap,
            join: pp.border_join,
            corners: pp.border_corners,
            width: pp.border_width,
            top: pp.border_top,
            bottom: pp.border_bottom,
            display_if_splits: pp.border_display_if_splits,
            gap_color: pp.border_gap_color.clone(),
            gap_tint: pp.border_gap_tint,
            overprint: pp.border_overprint,
            gap_overprint: pp.border_gap_overprint,
        });
        Some(ParaBox { shading, border, merge: pp.border_merge })
    }
}

/// Lines of one box in one frame: a part ends where the next line moves to another column or
/// sub-column.
struct Part<'a> {
    fi: usize,
    lines: Vec<&'a Line>,
}

/// Draw the boxes of `boxes` (one entry per paragraph) into `out`.
pub(crate) fn draw(out: &mut ComposedStory, boxes: &[Option<ParaBox>], frames: &[FrameSpec]) {
    // How many "under" decorations each frame holds so far: they stay at the front of its list.
    let mut under_count = vec![0usize; out.frames.len()];
    let mut pi = 0;
    while pi < boxes.len() {
        let Some(b) = boxes.get(pi).and_then(Option::as_ref) else {
            pi += 1;
            continue;
        };
        let mut end = pi;
        while b.merge && boxes.get(end + 1).and_then(Option::as_ref).is_some_and(|n| n == b) {
            end += 1;
        }
        let decos = group_decos(out, pi..=end, b, frames);
        for (fi, under, d) in decos {
            if let Some(ft) = out.frames.get_mut(fi) {
                if under {
                    // Behind the text and the other decorations, in story order: a later
                    // paragraph's shading lies over an earlier one's where they overlap.
                    let at = under_count.get(fi).copied().unwrap_or(0).min(ft.decos.len());
                    ft.decos.insert(at, d);
                    if let Some(n) = under_count.get_mut(fi) {
                        *n += 1;
                    }
                } else {
                    ft.decos.push(d)
                }
            }
        }
        pi = end + 1;
    }
}

/// The shading box for one part of a paragraph run, if it has any area.
fn shading_deco(s: &Shading, part: &Part<'_>, frames: &[FrameSpec], first_part: bool, last_part: bool) -> Option<Deco> {
    let r = inner_rect(&part.lines, s.width, s.top, s.bottom)?;
    let [t, l, bo, rr] = s.offsets;
    let mut rect = Rect::new(r.x0 - l, r.y0 - t, r.x1 + rr, r.y1 + bo);
    if s.clip
        && let Some(f) = frames.get(part.fi)
    {
        rect = rect.intersect(frame_rect(f));
    }
    if rect.width() > 0.0 && rect.height() > 0.0 {
        // Corners stay square where the box is cut by a split.
        let corners = cut_corners(s.corners, !first_part, !last_part);
        let path = (!corners.iter().all(Corner::is_none)).then(|| box_outline(rect, &corners, [true; 4]));
        return Some(Deco {
            rect,
            color: s.color.clone(),
            tint: s.tint,
            path,
            nonprinting: s.nonprinting,
            overprint: s.overprint,
            ..Deco::default()
        });
    }
    None
}

/// The shadow of a shading box: the same rectangle and corners, moved and in the shadow's colour.
fn shadow_deco(sh: &Shadow, shading: &Deco, nonprinting: bool) -> Deco {
    let [dx, dy] = sh.offset;
    let shift = kurbo::Affine::translate((dx, dy));
    Deco {
        rect: shading.rect + kurbo::Vec2::new(dx, dy),
        color: sh.color.clone(),
        tint: sh.tint,
        path: shading.path.as_ref().map(|p| shift * p.clone()),
        nonprinting,
        overprint: sh.overprint,
        ..Deco::default()
    }
}

/// Grid step of the specks (points): one possible speck per cell.
const SPECK_STEP: f64 = 1.5;
/// Most specks over one box: a bigger box gets sparser specks, evenly, instead of more.
const MAX_SPECKS: usize = 80_000;
/// Most grid cells looked at for one box (about 3,000 pt square); a bigger box gets no specks
/// rather than a long loop.
const MAX_SPECK_CELLS: f64 = 4.0e6;

/// A stable pseudo-random number in 0..1 for grid cell (`ix`, `iy`) and channel `k`.
fn speck_noise(ix: i64, iy: i64, k: u64) -> f64 {
    let mut h =
        (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (iy as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ k.wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 33;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// The specks over a box: small round dots of varied size on a jittered grid, kept inside the
/// box's outline. The grid is anchored to the frame, so the pattern doesn't change when the text
/// around it does.
fn speckle_deco(sp: &Speckle, under: &Deco) -> Option<Deco> {
    let rect = under.rect;
    let cells = (rect.width() / SPECK_STEP + 1.0) * (rect.height() / SPECK_STEP + 1.0);
    if !(cells <= MAX_SPECK_CELLS) {
        return None;
    }
    let outline = under.path.clone().unwrap_or_else(|| rect.to_path(TOL));
    let mut dots: Vec<[f32; 3]> = Vec::new();
    let (x0, y0) = ((rect.x0 / SPECK_STEP).floor() as i64, (rect.y0 / SPECK_STEP).floor() as i64);
    let (x1, y1) = ((rect.x1 / SPECK_STEP).ceil() as i64, (rect.y1 / SPECK_STEP).ceil() as i64);
    // Up to a third of the cells at full amount: beyond that the specks read as a tint.
    let chance = (sp.amount * 0.33).min(MAX_SPECKS as f64 / cells);
    'rows: for iy in y0..y1 {
        for ix in x0..x1 {
            if speck_noise(ix, iy, 0) >= chance {
                continue;
            }
            let c = kurbo::Point::new((ix as f64 + speck_noise(ix, iy, 1)) * SPECK_STEP, (iy as f64 + speck_noise(ix, iy, 2)) * SPECK_STEP);
            // Mostly fine dust, now and then a bigger fleck.
            let size = speck_noise(ix, iy, 3);
            let r = if size > 0.93 { 0.45 + size * 0.35 } else { 0.12 + size * 0.28 };
            if !rect.contains(c) || !outline.contains(c) {
                continue;
            }
            dots.push([c.x as f32, c.y as f32, r as f32]);
            if dots.len() >= MAX_SPECKS {
                break 'rows;
            }
        }
    }
    (!dots.is_empty()).then(|| Deco {
        rect,
        color: sp.color.clone(),
        tint: sp.tint,
        path: None,
        nonprinting: under.nonprinting,
        overprint: sp.overprint,
        over_text: sp.over_text,
        speckle: true,
        grain: None,
        dots: Some(dots.into()),
    })
}

/// How strong dark grain gets at full amount: it multiplies a light box, so a little goes far.
const DARK_GRAIN: f64 = 0.3;

/// Paper grain over a box, inside its outline: dark grain multiplied over the box itself, light
/// flecks over its shadow. Drawn under the text, and like the specks only in high-quality display.
fn grain_deco(amount: f64, light: bool, under: &Deco) -> Deco {
    let opacity = if light { amount } else { amount * DARK_GRAIN };
    Deco {
        rect: under.rect,
        color: String::new(),
        tint: 1.0,
        path: Some(under.path.clone().unwrap_or_else(|| under.rect.to_path(TOL))),
        nonprinting: under.nonprinting,
        overprint: false,
        over_text: false,
        speckle: true,
        grain: Some(crate::Grain { light, opacity: opacity as f32 }),
        dots: None,
    }
}

/// The border strokes (and plain edges) for one part of a paragraph run, pushed onto `decos`.
fn border_decos(bd: &Border, part: &Part<'_>, first_part: bool, last_part: bool, decos: &mut Vec<(usize, bool, Deco)>) {
    let Some(r) = inner_rect(&part.lines, bd.width, bd.top, bd.bottom) else { return };
    let [t, l, bo, rr] = bd.offsets;
    let o = Rect::new(r.x0 - l, r.y0 - t, r.x1 + rr, r.y1 + bo);
    let open_top = !first_part && !bd.display_if_splits;
    let open_bottom = !last_part && !bd.display_if_splits;
    for (path, gap) in border_paths(bd, o, open_top, open_bottom) {
        let (color, tint, overprint) =
            if gap { (bd.gap_color.clone(), bd.gap_tint, bd.gap_overprint) } else { (bd.color.clone(), bd.tint, bd.overprint) };
        decos.push((part.fi, false, Deco { rect: path.bounding_box(), color, tint, path: Some(path), overprint, ..Deco::default() }));
    }
    if is_plain(bd, open_top, open_bottom) {
        decos.extend(
            plain_edges(bd, o, open_top, open_bottom)
                .into_iter()
                .map(|rect| (part.fi, false, Deco { rect, color: bd.color.clone(), tint: bd.tint, overprint: bd.overprint, ..Deco::default() })),
        );
    }
}

/// The decorations of paragraphs `paras` sharing box `b`: (frame, drawn under the rest, deco).
fn group_decos(out: &ComposedStory, paras: std::ops::RangeInclusive<usize>, b: &ParaBox, frames: &[FrameSpec]) -> Vec<(usize, bool, Deco)> {
    let mut parts: Vec<Part<'_>> = Vec::new();
    for (fi, ft) in out.frames.iter().enumerate() {
        let lines: Vec<&Line> = ft.lines.iter().filter(|l| paras.contains(&l.para)).collect();
        for part in lines.chunk_by(|a, b| a.column == b.column && b.baseline > a.baseline) {
            parts.push(Part { fi, lines: part.to_vec() });
        }
    }
    let n = parts.len();
    let mut decos = Vec::new();
    for (k, part) in parts.iter().enumerate() {
        let (first_part, last_part) = (k == 0, k + 1 == n);
        if let Some(s) = &b.shading
            && let Some(deco) = shading_deco(s, part, frames, first_part, last_part)
        {
            // The shadow goes first: "under" decorations are drawn in the order they are pushed.
            if let Some(sh) = &s.shadow {
                let shadow = shadow_deco(sh, &deco, s.nonprinting);
                let specks = s.speckle.as_ref().and_then(|sp| speckle_deco(sp, &shadow));
                let grain = s.noise.as_ref().map(|n| grain_deco(n.amount, true, &shadow));
                decos.push((part.fi, true, shadow));
                decos.extend(grain.map(|d| (part.fi, true, d)));
                decos.extend(specks.map(|d| (part.fi, !d.over_text, d)));
            }
            let specks = s.speckle.as_ref().and_then(|sp| speckle_deco(sp, &deco));
            let grain = s.noise.as_ref().map(|n| grain_deco(n.amount, false, &deco));
            decos.push((part.fi, true, deco));
            decos.extend(grain.map(|d| (part.fi, true, d)));
            decos.extend(specks.map(|d| (part.fi, !d.over_text, d)));
        }
        if let Some(bd) = &b.border {
            border_decos(bd, part, first_part, last_part, &mut decos);
        }
    }
    decos
}

/// The frame's edges in its inner space (the text area out to the insets).
fn frame_rect(f: &FrameSpec) -> Rect {
    let a = f.area;
    if f.vertical {
        return a;
    }
    let [t, l, b, r] = f.opts.inset.map(finite);
    Rect::new(a.x0 - l, a.y0 - t, a.x1 + r, a.y1 + b)
}

/// `corners` with the top pair square when the box is cut at the top, the bottom pair when cut at
/// the bottom (top left, top right, bottom right, bottom left).
fn cut_corners(mut corners: [Corner; 4], top: bool, bottom: bool) -> [Corner; 4] {
    for (i, c) in corners.iter_mut().enumerate() {
        if (top && i < 2) || (bottom && i >= 2) {
            *c = Corner::default();
        }
    }
    corners
}

/// The em box of a line's tallest glyph: how far its top is above the baseline and its bottom below.
fn em_box(l: &Line) -> Option<(f64, f64)> {
    l.glyphs.iter().filter(|g| g.visible && g.sy.is_finite() && g.sy > 0.0).fold(None, |acc: Option<(f64, f64)>, g| {
        let (top, bottom) = g.face.em_box();
        let (t, b) = (top * g.sy, -bottom * g.sy);
        Some(acc.map_or((t, b), |(at, ab)| (at.max(t), ab.max(b))))
    })
}

/// The box around `lines` before offsets: the column or the text across, from the top edge of the
/// first line to the bottom edge of the last.
fn inner_rect(lines: &[&Line], width: BoxWidth, top: BoxTop, bottom: BoxBottom) -> Option<Rect> {
    let (a, z) = (lines.first()?, lines.last()?);
    let y0 = match top {
        BoxTop::Ascent => a.baseline - a.ascent,
        BoxTop::Baseline => a.baseline,
        BoxTop::EmBox => a.baseline - em_box(a).map_or(a.ascent, |e| e.0),
        BoxTop::Leading => a.baseline - a.leading.max(a.ascent),
    };
    let y1 = match bottom {
        BoxBottom::Descent => z.baseline + z.descent,
        BoxBottom::Baseline => z.baseline,
        BoxBottom::EmBox => z.baseline + em_box(z).map_or(z.descent, |e| e.1),
    };
    let column = || (lines.iter().map(|l| l.x0).fold(f64::INFINITY, f64::min), lines.iter().map(|l| l.x1).fold(f64::NEG_INFINITY, f64::max));
    let (x0, x1) = match width {
        BoxWidth::Column => column(),
        BoxWidth::Text => {
            let start = lines.iter().flat_map(|l| l.glyphs.iter().filter(|g| g.visible).map(|g| g.x.min(g.x + g.adv))).fold(f64::INFINITY, f64::min);
            let end = lines.iter().filter(|l| l.glyphs.iter().any(|g| g.visible)).map(|l| l.end_x).fold(f64::NEG_INFINITY, f64::max);
            if start.is_finite() && end.is_finite() && end > start { (start, end) } else { column() }
        }
    };
    (x0.is_finite() && x1.is_finite() && y0.is_finite() && y1.is_finite()).then(|| Rect::new(x0, y0, x1, y1))
}

/// The edges drawn: top, right, bottom, left (a cut end, or a side of no weight, isn't).
fn edges(bd: &Border, open_top: bool, open_bottom: bool) -> [bool; 4] {
    let [wt, wl, wb, wr] = bd.weights;
    [wt > 0.0 && !open_top, wr > 0.0, wb > 0.0 && !open_bottom, wl > 0.0]
}

/// The common weight of the drawn edges, when they all have the same one.
fn equal_weight(bd: &Border, drawn: [bool; 4]) -> Option<f64> {
    let [wt, wl, wb, wr] = bd.weights;
    let w: Vec<f64> = [wt, wr, wb, wl].into_iter().zip(drawn).filter(|(_, d)| *d).map(|(w, _)| w).collect();
    let first = *w.first()?;
    w.iter().all(|x| (x - first).abs() < 1e-9).then_some(first)
}

/// A solid border with no corner shapes (or different weights per side) is filled rectangles, one
/// per edge.
fn is_plain(bd: &Border, open_top: bool, open_bottom: bool) -> bool {
    let drawn = edges(bd, open_top, open_bottom);
    let corners = cut_corners(bd.corners, open_top, open_bottom);
    bd.kind == StrokeType::Solid && (corners.iter().all(Corner::is_none) || equal_weight(bd, drawn).is_none())
}

/// The edge rectangles of a plain border, outside the offset box `o`.
fn plain_edges(bd: &Border, o: Rect, open_top: bool, open_bottom: bool) -> Vec<Rect> {
    let [wt, wl, wb, wr] = bd.weights;
    let [top, right, bottom, left] = edges(bd, open_top, open_bottom);
    let mut v = Vec::new();
    if top {
        v.push(Rect::new(o.x0 - wl, o.y0 - wt, o.x1 + wr, o.y0));
    }
    if bottom {
        v.push(Rect::new(o.x0 - wl, o.y1, o.x1 + wr, o.y1 + wb));
    }
    if left {
        v.push(Rect::new(o.x0 - wl, o.y0, o.x0, o.y1));
    }
    if right {
        v.push(Rect::new(o.x1, o.y0, o.x1 + wr, o.y1));
    }
    v
}

/// The stroked outlines of a border that isn't plain, each with whether it is the gap colour
/// (drawn first, under the dashes). The stroke lies outside the offset box `o`.
fn border_paths(bd: &Border, o: Rect, open_top: bool, open_bottom: bool) -> Vec<(BezPath, bool)> {
    if is_plain(bd, open_top, open_bottom) {
        return Vec::new();
    }
    let drawn = edges(bd, open_top, open_bottom);
    let mut out = Vec::new();
    let mut add = |path: &BezPath, w: f64, perimeter: f64| {
        if bd.kind != StrokeType::Solid {
            let gap = stroke_outline(path, w, &StrokeType::Solid, Cap::Butt, bd.join, perimeter);
            out.push((gap, true));
        }
        out.push((stroke_outline(path, w, &bd.kind, bd.cap, bd.join, perimeter), false));
    };
    match equal_weight(bd, drawn) {
        Some(w) => {
            // One outline along the stroke's centre, corners shaped where two edges meet.
            let c = Rect::new(o.x0 - w / 2.0, o.y0 - w / 2.0, o.x1 + w / 2.0, o.y1 + w / 2.0);
            let corners = cut_corners(bd.corners, open_top, open_bottom);
            add(&box_outline(c, &corners, drawn), w, 2.0 * (c.width() + c.height()));
        }
        None => {
            // Different weights: each edge on its own.
            let [wt, wl, wb, wr] = bd.weights;
            let lines = [
                (drawn[0], wt, (o.x0 - wl, o.y0 - wt / 2.0), (o.x1 + wr, o.y0 - wt / 2.0)),
                (drawn[1], wr, (o.x1 + wr / 2.0, o.y0), (o.x1 + wr / 2.0, o.y1)),
                (drawn[2], wb, (o.x0 - wl, o.y1 + wb / 2.0), (o.x1 + wr, o.y1 + wb / 2.0)),
                (drawn[3], wl, (o.x0 - wl / 2.0, o.y0), (o.x0 - wl / 2.0, o.y1)),
            ];
            for (on, w, a, b) in lines {
                if on {
                    let mut p = BezPath::new();
                    p.move_to(a);
                    p.line_to(b);
                    add(&p, w, kurbo::Point::from(a).distance(b.into()));
                }
            }
        }
    }
    out
}

/// The filled outline of stroking `path` `w` wide in stroke type `kind`.
fn stroke_outline(path: &BezPath, w: f64, kind: &StrokeType, cap: Cap, join: Join, length: f64) -> BezPath {
    if !(w.is_finite() && w > 0.0) {
        return BezPath::new();
    }
    if let Some(o) = kind.outline(path, w, TOL) {
        return o;
    }
    let mut st = kurbo::Stroke::new(w)
        .with_caps(match cap {
            Cap::Butt => kurbo::Cap::Butt,
            Cap::Round => kurbo::Cap::Round,
            Cap::Projecting => kurbo::Cap::Square,
        })
        .with_join(match join {
            Join::Miter => kurbo::Join::Miter,
            Join::Round => kurbo::Join::Round,
            Join::Bevel => kurbo::Join::Bevel,
        });
    let dashes: Option<Vec<f64>> = match kind {
        StrokeType::Dashed { pattern } => {
            let p: Vec<f64> = pattern.iter().copied().filter(|v| v.is_finite() && *v >= 0.0).collect();
            Some(if p.iter().sum::<f64>() > 1e-3 { p } else { vec![3.0 * w, 2.0 * w] })
        }
        StrokeType::Dotted => match dots(path, w) {
            Some(d) => return d,
            None => None,
        },
        _ => None,
    };
    if let Some(d) = dashes {
        let period: f64 = d.iter().sum();
        if period > 1e-3 && length / period < MAX_DASHES {
            st = st.with_dashes(0.0, d);
        }
    }
    kurbo::stroke(path.iter(), &st, &kurbo::StrokeOpts::default(), TOL)
}

/// Round dots `w` wide along `path`, about `2w` apart and evenly spaced by arc length: around a
/// closed subpath the spacing divides its length exactly, an open one has a dot at each end.
/// `None` (stroke solid) when there would be more than [`MAX_DASHES`] dots.
fn dots(path: &BezPath, w: f64) -> Option<BezPath> {
    use kurbo::{ParamCurve, ParamCurveArclen, PathEl, PathSeg};
    // Subpaths as (segments, closed).
    let mut subs: Vec<(Vec<PathSeg>, bool)> = Vec::new();
    let (mut start, mut cur) = (kurbo::Point::ZERO, kurbo::Point::ZERO);
    for el in path.elements() {
        let seg = match *el {
            PathEl::MoveTo(p) => {
                subs.push((Vec::new(), false));
                (start, cur) = (p, p);
                continue;
            }
            PathEl::LineTo(p) => PathSeg::Line(kurbo::Line::new(cur, p)),
            PathEl::QuadTo(a, p) => PathSeg::Quad(kurbo::QuadBez::new(cur, a, p)),
            PathEl::CurveTo(a, b, p) => PathSeg::Cubic(kurbo::CubicBez::new(cur, a, b, p)),
            PathEl::ClosePath => {
                if let Some(s) = subs.last_mut() {
                    s.1 = true;
                    if cur.distance(start) > 1e-9 {
                        s.0.push(PathSeg::Line(kurbo::Line::new(cur, start)));
                    }
                }
                cur = start;
                continue;
            }
        };
        cur = seg.end();
        if let Some(s) = subs.last_mut() {
            s.0.push(seg);
        }
    }
    const ACC: f64 = 1e-4;
    let mut out = BezPath::new();
    let mut count = 0.0;
    for (segs, closed) in subs {
        let lens: Vec<f64> = segs.iter().map(|s| s.arclen(ACC)).collect();
        let total: f64 = lens.iter().sum();
        if !(total.is_finite() && total > 1e-9) {
            continue;
        }
        let n = (total / (2.0 * w)).round().max(1.0);
        count += n + 1.0;
        if count > MAX_DASHES {
            return None;
        }
        let step = total / n;
        let dots = if closed { n as usize } else { n as usize + 1 };
        let (mut k, mut before) = (0, 0.0);
        for i in 0..dots {
            let s = i as f64 * step;
            while k + 1 < segs.len() && before + lens.get(k).copied().unwrap_or(0.0) < s {
                before += lens.get(k).copied().unwrap_or(0.0);
                k += 1;
            }
            let (Some(seg), Some(&l)) = (segs.get(k), lens.get(k)) else { continue };
            let t = if l > 1e-12 { seg.inv_arclen((s - before).clamp(0.0, l), ACC) } else { 0.0 };
            out.extend(kurbo::Circle::new(seg.eval(t), w / 2.0).path_elements(TOL));
        }
    }
    Some(out)
}

#[cfg(test)]
mod dotted_tests {
    use super::*;

    /// The centres of the dots (one subpath each) of a dotted outline.
    fn dot_centres(p: &BezPath) -> Vec<kurbo::Point> {
        let mut out = Vec::new();
        let mut cur = BezPath::new();
        for el in p.elements() {
            if matches!(el, kurbo::PathEl::MoveTo(_)) && !cur.elements().is_empty() {
                out.push(cur.bounding_box().center());
                cur = BezPath::new();
            }
            cur.push(*el);
        }
        if !cur.elements().is_empty() {
            out.push(cur.bounding_box().center());
        }
        out
    }

    /// Distance along the outline of `r`, clockwise from its top left corner, of point `q` on it.
    fn along(r: Rect, q: kurbo::Point) -> f64 {
        let (w, h) = (r.width(), r.height());
        if (q.y - r.y0).abs() < 1e-6 {
            q.x - r.x0
        } else if (q.x - r.x1).abs() < 1e-6 {
            w + q.y - r.y0
        } else if (q.y - r.y1).abs() < 1e-6 {
            w + h + r.x1 - q.x
        } else {
            2.0 * w + h + r.y1 - q.y
        }
    }

    fn assert_even(gaps: &[f64], expected: f64) {
        for g in gaps {
            assert!((g - expected).abs() < 0.05, "uneven dot gaps {gaps:?}, expected {expected}");
        }
    }

    /// Grain follows the box's rounded outline, is dark over the box and light over its shadow,
    /// and gets stronger with the amount.
    #[test]
    fn grain_follows_the_box_and_the_amount() {
        let rect = Rect::new(10.0, 20.0, 90.0, 50.0);
        let corners = [Corner { shape: designcraft_geom::corners::CornerShape::Rounded, size: 9.0 }; 4];
        let outline = box_outline(rect, &corners, [true; 4]);
        let under = Deco { rect, color: "[Black]".into(), tint: 1.0, path: Some(outline.clone()), ..Default::default() };
        let (weak, strong) = (grain_deco(0.2, false, &under), grain_deco(0.9, false, &under));
        assert_eq!(strong.path.as_ref(), Some(&outline), "inside the rounded box, not its bounds");
        assert!(!strong.over_text && strong.speckle, "under the text, drawn with the specks");
        let op = |d: &Deco| d.grain.map(|g| g.opacity).unwrap_or(0.0);
        assert!(op(&strong) > op(&weak), "stronger with the amount");
        assert!(!strong.grain.unwrap().light && grain_deco(0.5, true, &under).grain.unwrap().light);
    }

    /// Specks stay inside the box, and a box too big for every cell to get its speck gets sparser
    /// specks all the way down instead of stopping part way.
    #[test]
    fn specks_cover_a_big_box_evenly_and_stay_inside() {
        let rect = Rect::new(0.0, 0.0, 600.0, 900.0);
        let under = Deco { rect, color: "[Black]".into(), tint: 1.0, ..Default::default() };
        let sp = Speckle { amount: 1.0, color: "[Paper]".into(), tint: 1.0, over_text: false, overprint: false };
        let dots = speckle_deco(&sp, &under).and_then(|d| d.dots).unwrap();
        assert!(dots.len() <= MAX_SPECKS);
        assert!(dots.iter().all(|&[x, y, _]| rect.contains(kurbo::Point::new(f64::from(x), f64::from(y)))), "inside the box");
        assert!(dots.iter().any(|d| f64::from(d[1]) > rect.y1 - 50.0), "specks reach the bottom of the box");
        let huge = Deco { rect: Rect::new(0.0, 0.0, 1.0e6, 1.0e6), ..under };
        assert!(speckle_deco(&sp, &huge).is_none(), "a huge box gets no specks rather than a long loop");
    }

    /// The tile is noise: varied, and dark grain centred on half ink while light grain is mostly
    /// clear with some flecks.
    #[test]
    fn grain_tiles_are_noise() {
        let mean = |t: &[u8]| t.iter().map(|v| f64::from(*v)).sum::<f64>() / t.len() as f64;
        let (dark, light) = (crate::grain_tile(false), crate::grain_tile(true));
        assert!((100.0..156.0).contains(&mean(dark)), "dark mean {}", mean(dark));
        assert!(mean(light) < mean(dark), "light grain is mostly clear");
        assert!(dark.iter().min() < Some(&60) && dark.iter().max() > Some(&200), "a wide spread");
    }

    #[test]
    fn dotted_box_border_dots_are_evenly_spaced() {
        let r = Rect::new(0.0, 0.0, 100.3, 37.7);
        let path = box_outline(r, &[Corner::default(); 4], [true; 4]);
        let len = 2.0 * (r.width() + r.height());
        let out = stroke_outline(&path, 1.0, &StrokeType::Dotted, Cap::Butt, Join::Miter, len);
        let mut s: Vec<f64> = dot_centres(&out).into_iter().map(|q| along(r, q)).collect();
        s.sort_by(f64::total_cmp);
        let n = (len / 2.0).round() as usize;
        assert_eq!(s.len(), n, "dot count");
        let mut gaps: Vec<f64> = s.windows(2).map(|p| p[1] - p[0]).collect();
        gaps.push(s[0] + len - s[n - 1]);
        assert_even(&gaps, len / n as f64);
    }

    #[test]
    fn dotted_edge_dots_are_evenly_spaced_and_reach_both_ends() {
        for (w, l) in [(1.0, 50.3), (2.5, 37.7), (0.7, 100.3)] {
            let mut p = BezPath::new();
            p.move_to((0.0, 0.0));
            p.line_to((l, 0.0));
            let out = stroke_outline(&p, w, &StrokeType::Dotted, Cap::Butt, Join::Miter, l);
            let mut xs: Vec<f64> = dot_centres(&out).into_iter().map(|q| q.x).collect();
            xs.sort_by(f64::total_cmp);
            let n = (l / (2.0 * w)).round() as usize;
            assert_eq!(xs.len(), n + 1, "dot count for w {w}, length {l}");
            assert!(xs[0].abs() < 0.05 && (xs[n] - l).abs() < 0.05, "ends {xs:?}");
            let gaps: Vec<f64> = xs.windows(2).map(|p| p[1] - p[0]).collect();
            assert_even(&gaps, l / n as f64);
        }
    }
}
