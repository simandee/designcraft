//! Objects anchored in text (a picture in a chat line, an icon in a sentence) with the Selection
//! tool, as other objects: a click on one selects it, its bounds show eight handles, and dragging
//! a handle resizes it with its content, keeping its proportions (`anchored.resize`); the text
//! moves around the new size. Escape or a click elsewhere ends it.

use designcraft_doc::{Item, SpreadRef, StoryId};
use designcraft_geom::{Affine, Point, Rect};

use crate::ToolContext;

/// An object in a story's text: the story and the object's index among its anchored objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inline {
    pub story: StoryId,
    pub index: usize,
}

/// Screen pixels around an object or handle that still hit it.
const HIT_PX: f64 = 2.0;
const HANDLE_PX: f64 = 5.0;

/// Every anchored object shown in the visible text frames: (object, its bounds on the canvas).
/// Frontmost last.
fn shown(cx: &ToolContext) -> Vec<(Inline, Rect)> {
    placed(cx).into_iter().map(|(o, m, size)| (o, m.transform_rect_bbox(Rect::from_origin_size(Point::ZERO, size)))).collect()
}

/// Every anchored object shown: (object, the transform from its space to the canvas, its size).
fn placed(cx: &ToolContext) -> Vec<(Inline, Affine, designcraft_geom::Size)> {
    let mut out = Vec::new();
    for slot in &cx.layout.slots {
        let SpreadRef::Doc(_) = slot.spread else { continue };
        let Some(sp) = cx.doc.spread(slot.spread) else { continue };
        let mut frames: Vec<&Item> = Vec::new();
        let mut todo: Vec<&Item> = sp.items.iter().map(AsRef::as_ref).collect();
        todo.reverse();
        while let Some(it) = todo.pop() {
            let locked = it.locked || cx.doc.layer(it.layer).is_some_and(|l| l.locked || !l.visible);
            if it.hidden || locked {
                continue;
            }
            if it.is_text_frame() {
                frames.push(it);
            }
            let mut kids: Vec<&Item> = it.children().iter().map(AsRef::as_ref).collect();
            kids.reverse();
            todo.extend(kids);
        }
        for it in frames {
            let Some(story) = it.text_frame().map(|t| t.story) else { continue };
            let Some(loc) = cx.doc.find(it.id) else { continue };
            let cs = cx.cache.get(cx.doc, story, None);
            let Some(ft) = cs.frame(it.id) else { continue };
            let m = slot.xf * cx.doc.parent_xf(&loc) * cx.doc.text_xf(it);
            for o in &ft.objects {
                out.push((Inline { story, index: o.index }, m * Affine::translate(o.origin.to_vec2()), o.size.into()));
            }
        }
    }
    out
}

/// The frontmost anchored object under canvas point `p`.
pub fn at(cx: &ToolContext, p: Point) -> Option<(Inline, Rect)> {
    let tol = cx.tol(HIT_PX);
    shown(cx).into_iter().rev().find(|(_, r)| r.inflate(tol, tol).contains(p))
}

/// Where object `o` shows on the canvas (its first frame).
pub fn bounds(cx: &ToolContext, o: Inline) -> Option<Rect> {
    shown(cx).into_iter().find(|(i, _)| *i == o).map(|(_, r)| r)
}

/// The handle of `r` (as [`crate::select::handles`] numbers them) under `p`.
pub fn handle_at(cx: &ToolContext, r: Rect, p: Point) -> Option<usize> {
    let tol = cx.tol(HANDLE_PX);
    crate::select::handles(r).iter().position(|h| (h.x - p.x).abs() <= tol && (h.y - p.y).abs() <= tol)
}

/// The width (pt) object `o` gets when handle `h` of its canvas bounds `start` is dragged to `p`:
/// the side opposite the handle stays, corners follow the pointer along the larger change, and
/// the proportions stay. `width` is its width in points at `start`.
pub fn width_for(start: Rect, h: usize, p: Point, width: f64) -> Option<f64> {
    let (w, ht) = (start.width(), start.height());
    if !(w > 1e-9 && ht > 1e-9 && width > 1e-9) {
        return None;
    }
    // The opposite corner or side stays put.
    let fx = match h {
        0 | 6 | 7 => Some((start.x1 - p.x) / w),
        2..=4 => Some((p.x - start.x0) / w),
        _ => None,
    };
    let fy = match h {
        0..=2 => Some((start.y1 - p.y) / ht),
        4..=6 => Some((p.y - start.y0) / ht),
        _ => None,
    };
    let f = match (fx, fy) {
        (Some(a), Some(b)) => if (a - 1.0).abs() >= (b - 1.0).abs() { a } else { b },
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => return None,
    };
    Some((f * width).max(4.0))
}

/// The item of object `o` (its width in points is that of its bounds).
pub fn item<'a>(cx: &'a ToolContext, o: Inline) -> Option<&'a Item> {
    cx.doc.story(o.story)?.objects.get(o.index).map(|a| &a.item)
}

/// The transform from object `o`'s space to the canvas (where its first frame shows it).
pub fn placement(cx: &ToolContext, o: Inline) -> Option<Affine> {
    placed(cx).into_iter().find(|(i, _, _)| *i == o).map(|(_, m, _)| m)
}

fn object<'a>(cx: &'a ToolContext, o: Inline) -> Option<&'a designcraft_doc::AnchoredObject> {
    cx.doc.story(o.story)?.objects.get(o.index).map(AsRef::as_ref)
}

/// Where object `o`'s picture frame shows on the canvas (the object's bounds when it has none).
pub fn frame_rect(cx: &ToolContext, o: Inline) -> Option<Rect> {
    let m = placement(cx, o)?;
    match object(cx, o)?.picture_rect() {
        Some(r) => Some(m.transform_rect_bbox(r)),
        None => bounds(cx, o),
    }
}

/// Where object `o`'s whole picture shows on the canvas (also the part its frame crops).
pub fn content_rect(cx: &ToolContext, o: Inline) -> Option<Rect> {
    Some(placement(cx, o)?.transform_rect_bbox(object(cx, o)?.picture_content()?.0))
}

/// The content grabber of object `o` (the middle of its picture frame), when it has a picture and
/// is large enough on screen to drag around the grabber.
pub fn grabber(cx: &ToolContext, o: Inline) -> Option<Point> {
    object(cx, o)?.picture_content()?;
    let r = frame_rect(cx, o)?;
    let min = cx.tol(GRABBER_PX * 3.0);
    (r.width() >= min && r.height() >= min).then(|| r.center())
}

/// Is `p` on object `o`'s content grabber?
pub fn on_grabber(cx: &ToolContext, o: Inline, p: Point) -> bool {
    grabber(cx, o).is_some_and(|c| (c - p).hypot() <= cx.tol(GRABBER_PX))
}

/// Grab radius of the content grabber, in screen pixels (as for frames on the page).
const GRABBER_PX: f64 = 9.0;

/// Handle `h` of `start` (canvas) dragged to `p`: the box with that side or corner moved there,
/// the opposite one kept; at least `min` wide and tall.
pub fn dragged_rect(start: Rect, h: usize, p: Point, min: f64) -> Rect {
    let mut r = start;
    if matches!(h, 0 | 6 | 7) {
        r.x0 = p.x.min(r.x1 - min);
    }
    if matches!(h, 2..=4) {
        r.x1 = p.x.max(r.x0 + min);
    }
    if matches!(h, 0..=2) {
        r.y0 = p.y.min(r.y1 - min);
    }
    if matches!(h, 4..=6) {
        r.y1 = p.y.max(r.y0 + min);
    }
    r
}

/// The corner or side that stays while handle `h` of `r` is dragged.
pub fn opposite(r: Rect, h: usize) -> Point {
    crate::select::handles(r)[(h + 4) % 8]
}

/// Content grabbers on show for objects in text: the selected one's and the hovered one's.
pub fn grabbers(cx: &ToolContext, hover: Option<Point>) -> Vec<Point> {
    let mut out: Vec<Inline> = cx.selection.inline_object().map(|s| Inline { story: s.story, index: s.index }).into_iter().collect();
    if let Some((o, _)) = hover.and_then(|p| at(cx, p))
        && !out.contains(&o)
    {
        out.push(o);
    }
    out.into_iter().filter_map(|o| grabber(cx, o)).collect()
}
