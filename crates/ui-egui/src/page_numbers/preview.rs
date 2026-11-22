//! Pictures for the Page Numbers panel: a design on a page number (drawn by the same code as the
//! page), and the Custom mode's diagram of the seven overlaps.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use designcraft_compose::pagenum::{layered, region_color};
use designcraft_doc::pagenum::Design;
use designcraft_fonts::ScopedFonts;
use designcraft_render::vello_cpu::{self, kurbo, peniko};
use egui::{Color32, Rect, Sense, Ui, pos2, vec2};

use crate::theme::Tokens;

/// The paper behind previews.
const PAPER: [u8; 3] = [246, 240, 227];

/// Pictures already drawn, by design, text and size.
#[derive(Default)]
pub struct Cache {
    map: HashMap<u64, egui::TextureHandle>,
}

fn key(design: &Design, text: &str, w: u32, h: u32) -> u64 {
    let mut s = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(design).unwrap_or_default().hash(&mut s);
    (text, w, h).hash(&mut s);
    s.finish()
}

fn rgba(c: &designcraft_color::Color) -> peniko::Color {
    let [r, g, b, a] = c.to_rgba8(1.0);
    peniko::Color::from_rgba8(r, g, b, a)
}

/// `text` in `design` on paper, `w` × `h` points; the button for it (clickable).
#[allow(clippy::too_many_arguments)]
pub fn show(ui: &mut Ui, cache: &mut Cache, design: &Design, text: &str, fonts: &ScopedFonts, w: f32, h: f32, _pad: f32) -> egui::Response {
    let ppp = ui.ctx().pixels_per_point();
    let (pw, ph) = (((w * ppp).round() as u32).clamp(8, 2048), ((h * ppp).round() as u32).clamp(8, 2048));
    let k = key(design, text, pw, ph);
    if cache.map.len() > 300 {
        cache.map.clear();
    }
    let tex = cache.map.entry(k).or_insert_with(|| {
        let mut rc = vello_cpu::RenderContext::new_with(pw as u16, ph as u16, vello_cpu::RenderSettings { num_threads: 0, ..Default::default() });
        rc.set_paint(peniko::Color::from_rgba8(PAPER[0], PAPER[1], PAPER[2], 255));
        rc.fill_rect(&kurbo::Rect::new(0.0, 0.0, f64::from(pw), f64::from(ph)));
        if let Some(l) = layered(design, text, 100.0, fonts) {
            let b = l.bounds;
            let s = (f64::from(pw) * 0.86 / b.width()).min(f64::from(ph) * 0.8 / b.height());
            let xf = kurbo::Affine::translate((f64::from(pw) / 2.0, f64::from(ph) / 2.0))
                * kurbo::Affine::scale(s)
                * kurbo::Affine::translate(-b.center().to_vec2());
            rc.set_transform(xf);
            for sh in &l.shapes {
                rc.set_paint(rgba(&sh.color));
                rc.fill_path(&sh.path);
            }
        }
        rc.flush();
        let mut pm = vello_cpu::Pixmap::new(pw as u16, ph as u16);
        rc.render(&mut pm, &mut vello_cpu::Resources::new());
        let img = egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], pm.data_as_u8_slice());
        crate::widgets::load_texture(ui.ctx(), "page-number-preview", img, egui::TextureOptions::LINEAR)
    });
    let (r, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    resp
}

/// Circle centres and radius of the diagram in a `w` wide box (unit: points from its top left).
fn circles(w: f32) -> ([egui::Pos2; 3], f32) {
    let h = w * 0.62;
    let r = h * 0.3;
    let c = pos2(w / 2.0, h / 2.0);
    ([pos2(c.x - r * 0.45, c.y - r * 0.3), pos2(c.x + r * 0.45, c.y - r * 0.3), pos2(c.x, c.y + r * 0.42)], r)
}

fn code_at(p: egui::Pos2, centres: &[egui::Pos2; 3], r: f32) -> u8 {
    (0..3).filter(|&i| centres[i].distance(p) < r).fold(0, |a, i| a | 1 << i)
}

/// The Custom diagram: three plates (hundreds top left, tens top right, ones below), each overlap
/// in its colour, `selected` outlined. Returns the overlap clicked.
pub fn venn(ui: &mut Ui, design: &Design, selected: u8, w: f32) -> Option<u8> {
    let t = Tokens::get(ui.ctx());
    let h = w * 0.62;
    let ppp = ui.ctx().pixels_per_point();
    let (pw, ph) = ((w * ppp) as usize, (h * ppp) as usize);
    let (centres, r) = circles(w);
    let colors: Vec<[u8; 3]> = (0u8..8)
        .map(|set| match set {
            0 => PAPER,
            s => region_color(design, s).map_or(PAPER, |c| {
                let [r, g, b, _] = c.to_rgba8(1.0);
                [r, g, b]
            }),
        })
        .collect();
    // Drawn again only when the design, the pick or the size changed; the handle is kept, or
    // the texture would be freed before it is painted.
    let k = {
        let mut s = std::collections::hash_map::DefaultHasher::new();
        (serde_json::to_string(design).unwrap_or_default(), selected, pw, ph, t.accent.to_array()).hash(&mut s);
        s.finish()
    };
    let store = egui::Id::unique("page-number-venn");
    let kept: Option<(u64, egui::TextureHandle)> = ui.data(|m| m.get_temp(store));
    let tex = match kept {
        Some((kk, tex)) if kk == k => tex,
        _ => {
            let mut px = vec![0u8; pw * ph * 4];
            let code = |x: usize, y: usize| code_at(pos2(x as f32 / ppp, y as f32 / ppp), &centres, r);
            for y in 0..ph {
                for x in 0..pw {
                    let c = code(x, y);
                    let edge = (x > 0 && code(x - 1, y) != c) || (y > 0 && code(x, y - 1) != c);
                    let sel_edge = c == selected
                        && (1..=(3.0 * ppp) as usize)
                            .any(|d| x < d || y < d || code(x - d, y) != c || code(x, y - d) != c || code(x + d, y) != c || code(x, y + d) != c);
                    let rgb = if sel_edge && c != 0 {
                        let a = t.accent;
                        [a.r(), a.g(), a.b()]
                    } else if edge {
                        [255, 255, 255]
                    } else {
                        colors[c as usize]
                    };
                    let i = (y * pw + x) * 4;
                    px[i..i + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                }
            }
            let img = egui::ColorImage::from_rgba_unmultiplied([pw, ph], &px);
            let tex = crate::widgets::load_texture(ui.ctx(), "page-number-venn", img, egui::TextureOptions::LINEAR);
            ui.data_mut(|m| m.insert_temp(store, (k, tex.clone())));
            tex
        }
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    ui.painter().image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, "Overlaps"));
    let clicked = resp.clicked().then(|| resp.interact_pointer_pos()).flatten().map(|p| code_at(p - rect.min.to_vec2(), &centres, r));
    // Keyboard: arrow keys step through the four overlaps.
    let overlaps = [0b011u8, 0b101, 0b110, 0b111];
    let key = resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowDown));
    let stepped = key.then(|| overlaps[(overlaps.iter().position(|o| *o == selected).unwrap_or(0) + 1) % overlaps.len()]);
    clicked.filter(|c| c.count_ones() >= 2).or(stepped)
}
