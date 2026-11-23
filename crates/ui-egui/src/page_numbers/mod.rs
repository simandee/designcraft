//! Page Numbers panel: layered page number designs (see `designcraft_doc::pagenum`). A preview of
//! the page in view and its neighbours, then Shape · Colors · Mixing tabs, and the button that
//! gives the document's page numbers the design. Every change goes through the `pageNumbers.*`
//! commands, so scripts and the automation interface can do the same.

mod preview;

use std::collections::BTreeMap;

use designcraft_doc::pagenum::{Cmyk, Design, Dot, Mix, Offset, RegionFill, Settings, Top, region_key};
use egui::{Color32, Ui};
use serde_json::{Value, json};

use crate::DesignApp;
use crate::theme::Tokens;
use crate::widgets::{self, NumField};

pub const PANEL: &str = "pageNumbers";

/// The Jahresbuch extensions speak German whatever language DesignCraft is set to (the user's
/// wish): the texts keep English keys and are looked up in the German catalogue.
const LANG: &str = "de";

/// Fonts that suit layered numbers, with the style to start from; shown when installed.
const RECOMMENDED: &[(&str, &str)] = &[
    ("Fraunces", "Black {opsz:72,SOFT:50}"),
    ("Ultra", "Regular"),
    ("Unbounded", "Black"),
    ("Bricolage Grotesque", "ExtraBold"),
    ("Work Sans", "Black"),
    ("Space Grotesk", "Bold"),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Shape,
    Colors,
    Mixing,
}

/// What the panel remembers between frames.
#[derive(Default)]
pub struct State {
    /// The design shown (by name).
    pub design: Option<String>,
    pub tab: Tab,
    /// The overlap picked in the Custom diagram (bit 0 hundreds, 1 tens, 2 ones).
    pub region: u8,
    /// The ink whose CMYK fields are open (0 hundreds, 1 tens, 2 ones).
    pub ink_open: Option<usize>,
    /// Custom was just picked: scroll its diagram into view.
    pub scroll_to_custom: bool,
    pub textures: preview::Cache,
}

/// Command app.pageNumbersPanel: show the panel, at `tab` and `design` when given.
pub fn open(app: &mut DesignApp, p: &Value) -> Result<Value, String> {
    match p.get("tab").and_then(Value::as_str) {
        Some("shape") => app.page_numbers.tab = Tab::Shape,
        Some("colors") => app.page_numbers.tab = Tab::Colors,
        Some("mixing") => {
            app.page_numbers.tab = Tab::Mixing;
            // With Custom mixing, open at its diagram.
            app.page_numbers.scroll_to_custom = true;
        }
        Some(t) => return Err(format!("unknown tab `{t}` (shape, colors, mixing)")),
        None => {}
    }
    if let Some(d) = p.get("design").and_then(Value::as_str) {
        app.page_numbers.design = Some(d.to_string());
    }
    if app.ui.docking_flyout.as_deref() != Some(PANEL) {
        app.run("window.panel", json!({"panel": PANEL}))?;
    }
    Ok(Value::Null)
}

fn tr<'a>(_app: &DesignApp, s: &'a str) -> &'a str {
    crate::i18n::tr(LANG, s)
}

fn hint(ui: &mut Ui, s: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(s).size(10.5).color(t.text_dim));
}

/// The page in view (index, name), else the first page.
fn page_in_view(app: &DesignApp) -> Option<(usize, String)> {
    let doc = &app.session.active()?.doc;
    let abs = crate::canvas::page_in_view(app).and_then(|(id, _)| doc.pages().position(|(_, _, p)| p.id == id)).unwrap_or(0);
    (abs < doc.page_count()).then(|| (abs, doc.page_name(abs)))
}

/// Save `design` (replacing `old`) as a live edit: a drag is one undo step.
fn save(app: &mut DesignApp, design: &Design, old: &str) {
    let label = tr(app, "Page Number Design").to_string();
    app.live_preview(&label, "pageNumbers.setDesign", json!({"design": design, "replace": old}));
}

fn run(app: &mut DesignApp, cmd: &str, params: Value) {
    app.end_live_preview();
    if let Err(e) = app.run(cmd, params) {
        app.status_error("Seitennummerierung", e);
    }
}

/// A new design's starting point: the first recommended font that is installed.
fn new_design(app: &DesignApp, name: String) -> Design {
    let fonts = crate::panels::fonts(app);
    let (family, style) = RECOMMENDED.iter().find(|(f, _)| fonts.has_family(f)).map_or(("Source Serif 4", "Bold"), |&(f, s)| (f, s));
    Design { name, font_family: family.into(), font_style: style.into(), ..Design::default() }
}

fn unique_name(set: &Settings, base: &str) -> String {
    (1..).map(|i| if i == 1 { base.to_string() } else { format!("{base} {i}") }).find(|n| set.design(n).is_none()).unwrap_or_default()
}

pub fn show(app: &mut DesignApp, ui: &mut Ui) {
    widgets::selection_band(ui, "Seitennummerierung");
    let Some(st) = app.session.active() else {
        hint(ui, tr(app, "Open a document first."));
        return;
    };
    let set = Settings::from_labels(&st.doc.labels);
    let fw = widgets::full_width(ui);
    // Design picker.
    if app.page_numbers.design.as_ref().is_none_or(|n| set.design(n).is_none()) {
        app.page_numbers.design = set.designs.first().map(|d| d.name.clone());
    }
    let names: Vec<String> = set.designs.iter().map(|d| d.name.clone()).collect();
    widgets::caption(ui, tr(app, "Design"));
    ui.horizontal(|ui| {
        let cur = app.page_numbers.design.clone().unwrap_or_default();
        let at = names.iter().position(|n| *n == cur);
        if !names.is_empty()
            && let Some(i) = widgets::dropdown_list(ui, &cur, fw - 60.0, &names, at)
        {
            app.end_live_preview();
            app.page_numbers.design = names.get(i).cloned();
        }
        if widgets::outline_button(ui, tr(app, "New"), if names.is_empty() { fw } else { 52.0 }).clicked() {
            let d = new_design(app, unique_name(&set, "Riso"));
            app.page_numbers.design = Some(d.name.clone());
            run(app, "pageNumbers.setDesign", json!({"design": d}));
        }
    });
    let Some(mut design) = app.page_numbers.design.as_ref().and_then(|n| set.design(n)).cloned() else {
        hint(ui, tr(app, "No design yet: New makes one."));
        return;
    };
    let before = design.clone();
    let old = design.name.clone();
    let page = page_in_view(app);
    let page_name = page.as_ref().map_or_else(|| "712".to_string(), |(_, n)| n.clone());
    preview_section(app, ui, &design, page.as_ref());
    widgets::rule(ui);
    let labels = [tr(app, "Shape"), tr(app, "Colors"), tr(app, "Mixing")].map(str::to_string);
    let active = app.page_numbers.tab as usize;
    if let Some(i) = seg(ui, "pn-seg-1", &labels.iter().map(String::as_str).collect::<Vec<_>>(), active, fw) {
        app.page_numbers.tab = [Tab::Shape, Tab::Colors, Tab::Mixing][i];
    }
    ui.add_space(4.0);
    match app.page_numbers.tab {
        Tab::Shape => shape_tab(app, ui, &mut design),
        Tab::Colors => colors_tab(app, ui, &mut design),
        Tab::Mixing => mixing_tab(app, ui, &mut design, &page_name),
    }
    if design != before {
        if design.name.trim().is_empty() || (design.name != old && set.design(&design.name).is_some()) {
            design.name = old.clone();
        }
        app.page_numbers.design = Some(design.name.clone());
        save(app, &design, &old);
    }
    apply_section(app, ui, &set, &design);
    // A field or spinner is still held: the live edit stays open (one undo step).
    if ui.input(|i| i.pointer.primary_down()) {
        app.hold_live_preview();
    }
}

// --- preview ------------------------------------------------------------------------------------

fn preview_section(app: &mut DesignApp, ui: &mut Ui, design: &Design, page: Option<&(usize, String)>) {
    let fw = widgets::full_width(ui);
    let name = page.map_or_else(|| "712".to_string(), |(_, n)| n.clone());
    widgets::rule(ui);
    let title = format!("{} {name}", tr(app, "Preview · Page"));
    let rerolled = design.rerolls.contains_key(&name);
    let (reroll, reset) = (tr(app, "Reroll").to_string(), tr(app, "Reset").to_string());
    let mut act = None;
    widgets::section_row(ui, &title, |ui| {
        if rerolled && widgets::link_label(ui, &reset, 44.0).on_hover_text("Back to this page's first layout").clicked() {
            act = Some(true);
        }
        if widgets::link_label(ui, &reroll, 76.0).clicked() {
            act = Some(false);
        }
    });
    if let Some(reset) = act {
        run(app, "pageNumbers.reroll", json!({"design": design.name, "page": name, "reset": reset}));
    }
    let fonts = crate::panels::fonts(app);
    preview::show(ui, &mut app.page_numbers.textures, design, &name, &fonts, fw, fw * 0.5, 1.0);
    // The neighbours, so a change shows on several pages.
    let neighbours: Vec<String> = match (page, app.session.active()) {
        (Some((abs, _)), Some(st)) => {
            let n = st.doc.page_count();
            [abs.checked_sub(1), Some(abs + 1).filter(|p| *p < n), Some(abs + 2).filter(|p| *p < n)]
                .into_iter()
                .flatten()
                .take(3)
                .map(|p| st.doc.page_name(p))
                .collect()
        }
        _ => ["711", "713", "714"].map(String::from).to_vec(),
    };
    if !neighbours.is_empty() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let w = (fw - 4.0) / 3.0;
            for n in &neighbours {
                preview::show(ui, &mut app.page_numbers.textures, design, n, &fonts, w, w * 0.6, 0.0);
            }
        });
        hint(ui, &format!("{} {}", tr(app, "Neighbouring pages"), neighbours.join(" · ")));
    }
}

/// A segmented control with its own id (several in one panel would share one).
fn seg(ui: &mut Ui, id: &str, labels: &[&str], active: usize, width: f32) -> Option<usize> {
    ui.push_id(id, |ui| widgets::segmented(ui, labels, active, width)).inner
}

// --- Shape tab ----------------------------------------------------------------------------------

const LW: f32 = 104.0;

/// A labelled number field; true when changed.
fn number(ui: &mut Ui, label: &str, id: &str, v: &mut f64, suffix: &str, range: (f64, f64), decimals: usize) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        widgets::caption_w(ui, label, LW);
        if let Some(n) = NumField::number(id, Some(*v), suffix, decimals).spinner().range(range.0, range.1).width(76.0).name(label).show(ui) {
            *v = n;
            changed = true;
        }
    });
    changed
}

/// A percentage field over a stored fraction.
fn percent(ui: &mut Ui, label: &str, id: &str, v: &mut f64, range: (f64, f64)) -> bool {
    let mut p = (*v * 100.0).round();
    if number(ui, label, id, &mut p, " %", (range.0 * 100.0, range.1 * 100.0), 0) {
        *v = p / 100.0;
        true
    } else {
        false
    }
}

fn percent32(ui: &mut Ui, label: &str, id: &str, v: &mut f32, range: (f64, f64)) -> bool {
    let mut f = f64::from(*v);
    let c = percent(ui, label, id, &mut f, range);
    if c {
        *v = f as f32;
    }
    c
}

/// Variable or fixed, with its fields. `tendency` and `fixed` take the value ranges.
fn offset(app: &DesignApp, ui: &mut Ui, title: &str, id: &str, o: &mut Offset, tendency: (f64, f64), fixed: (f64, f64)) {
    let fw = widgets::full_width(ui);
    widgets::caption(ui, title);
    let labels = [tr(app, "Variable"), tr(app, "Fixed")];
    let active = usize::from(matches!(o, Offset::Fixed { .. }));
    if let Some(i) = seg(ui, id, &labels, active, fw)
        && i != active
    {
        *o = match *o {
            Offset::Variable { tendency, .. } => Offset::Fixed { value: tendency },
            Offset::Fixed { value } => Offset::Variable { tendency: value, spread: 0.1 },
        };
    }
    match o {
        Offset::Variable { tendency: t, spread } => {
            percent(ui, tr(app, "Tendency"), &format!("{id}-t"), t, tendency);
            percent(ui, tr(app, "Scatter"), &format!("{id}-s"), spread, (0.0, 0.6));
        }
        Offset::Fixed { value } => {
            percent(ui, tr(app, "Step"), &format!("{id}-f"), value, fixed);
        }
    }
}

/// `Black {opsz:72,SOFT:50}` → ("Black", axes).
fn split_style(style: &str) -> (String, BTreeMap<String, f32>) {
    let Some(open) = style.find('{') else { return (style.trim().to_string(), BTreeMap::new()) };
    let axes = style[open + 1..]
        .trim_end_matches('}')
        .split(',')
        .filter_map(|kv| {
            let (k, v) = kv.split_once(':')?;
            Some((k.trim().to_string(), v.trim().parse().ok()?))
        })
        .collect();
    (style[..open].trim().to_string(), axes)
}

fn join_style(base: &str, axes: &BTreeMap<String, f32>) -> String {
    if axes.is_empty() {
        return base.to_string();
    }
    let a: Vec<String> = axes.iter().map(|(k, v)| format!("{k}:{v}")).collect();
    format!("{base} {{{}}}", a.join(","))
}

fn shape_tab(app: &mut DesignApp, ui: &mut Ui, d: &mut Design) {
    let fw = widgets::full_width(ui);
    widgets::caption(ui, tr(app, "Name"));
    let id = ui.make_persistent_id("page-numbers-name");
    let mut name: String = ui
        .data(|m| m.get_temp(id))
        .filter(|_| ui.memory(|m| m.has_focus(egui::Id::unique("page-numbers-name-field"))))
        .unwrap_or_else(|| d.name.clone());
    let r = ui.add(egui::TextEdit::singleline(&mut name).id(egui::Id::unique("page-numbers-name-field")).desired_width(fw));
    ui.data_mut(|m| m.insert_temp(id, name.clone()));
    if r.lost_focus() && !name.trim().is_empty() {
        d.name = name.trim().to_string();
    }
    widgets::rule(ui);
    widgets::section(ui, tr(app, "Font"), true);
    if let Some(pick) = crate::panels::font_combo(app, ui, "page-numbers-font", &d.font_family.clone(), fw) {
        d.font_style = crate::panels::pick_style(app, &pick);
        d.font_family = pick.family;
    }
    let fonts = crate::panels::fonts(app);
    let (base, mut axes) = split_style(&d.font_style);
    let styles = fonts.styles(&d.font_family);
    if styles.len() > 1
        && let Some(i) = widgets::dropdown_list(ui, &base, fw, &styles, styles.iter().position(|s| *s == base))
    {
        d.font_style = join_style(&styles[i], &axes);
    }
    let all = fonts.axes(&d.font_family, &base);
    for (tag, label, min, def, max) in all {
        let mut v = f64::from(axes.get(&tag).copied().unwrap_or(def));
        if number(ui, &label, &format!("page-numbers-axis-{tag}"), &mut v, "", (f64::from(min), f64::from(max)), 0) {
            axes.insert(tag.clone(), v as f32);
            d.font_style = join_style(&base, &axes);
        }
    }
    // Recommended fonts that are installed: one click.
    let rec: Vec<&(&str, &str)> = RECOMMENDED.iter().filter(|(f, _)| fonts.has_family(f) && *f != d.font_family).collect();
    if !rec.is_empty() {
        hint(ui, tr(app, "Recommended:"));
        ui.horizontal_wrapped(|ui| {
            for (f, s) in rec {
                if ui.small_button(*f).clicked() {
                    d.font_family = (*f).into();
                    d.font_style = (*s).into();
                }
            }
        });
    }
    widgets::rule(ui);
    widgets::section(ui, tr(app, "Arrangement"), true);
    offset(app, ui, tr(app, "Spacing"), "page-numbers-spacing", &mut d.spacing, (0.2, 1.2), (0.1, 1.5));
    ui.add_space(4.0);
    offset(app, ui, tr(app, "Vertical Alignment"), "page-numbers-vertical", &mut d.vertical, (-0.5, 0.5), (-0.6, 0.6));
    ui.add_space(4.0);
    number(ui, tr(app, "Misregistration"), "page-numbers-reg", &mut d.registration, " pt", (0.0, 5.0), 1);
    widgets::caption(ui, tr(app, "On Top"));
    let labels = [tr(app, "Ones"), tr(app, "Hundreds")];
    if let Some(i) = seg(ui, "pn-seg-3", &labels, usize::from(d.top == Top::Hundreds), fw) {
        d.top = if i == 0 { Top::Ones } else { Top::Hundreds };
    }
}

// --- Colors tab ---------------------------------------------------------------------------------

fn color32(c: Cmyk) -> Color32 {
    let [r, g, b, a] = designcraft_color::Color::cmyk(c[0], c[1], c[2], c[3]).to_rgba8(1.0);
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// CMYK fields (percent) and a list of the document's process swatches to copy from.
fn cmyk_fields(app: &DesignApp, ui: &mut Ui, id: &str, c: &mut Cmyk) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        for (k, l) in ["C", "M", "Y", "K"].iter().enumerate() {
            let mut v = (f64::from(c[k]) * 100.0).round();
            ui.label(*l);
            if let Some(n) = NumField::number(&format!("{id}-{l}"), Some(v), "", 0).range(0.0, 100.0).width(38.0).name(l).show(ui) {
                v = n;
                c[k] = (v / 100.0) as f32;
                changed = true;
            }
        }
    });
    let Some(st) = app.session.active() else { return changed };
    let swatches: Vec<(String, Cmyk)> = st
        .doc
        .swatches
        .iter()
        .filter(|s| !s.hidden)
        .filter_map(|s| match &s.value {
            designcraft_color::swatch::SwatchValue::Color { color: designcraft_color::Color::Cmyk { c, m, y, k }, .. } => {
                Some((s.name.clone(), [*c, *m, *y, *k]))
            }
            _ => None,
        })
        .collect();
    if !swatches.is_empty() {
        let names: Vec<String> = swatches.iter().map(|s| s.0.clone()).collect();
        let label = tr(app, "From Swatch…").to_string();
        if let Some(i) = widgets::dropdown_list(ui, &label, widgets::full_width(ui), &names, None) {
            *c = swatches[i].1;
            changed = true;
        }
    }
    changed
}

fn colors_tab(app: &mut DesignApp, ui: &mut Ui, d: &mut Design) {
    let t = Tokens::get(ui.ctx());
    for (p, label) in [tr(app, "Hundreds"), tr(app, "Tens"), tr(app, "Ones")].map(str::to_string).iter().enumerate() {
        let open = app.page_numbers.ink_open == Some(p);
        ui.horizontal(|ui| {
            if widgets::swatch_chip(ui, 18.0, Some(color32(d.inks[p])), None).on_hover_text(label).clicked() {
                app.page_numbers.ink_open = if open { None } else { Some(p) };
            }
            ui.label(egui::RichText::new(label).size(11.0).color(t.text_strong));
            let [c, m, y, k] = d.inks[p].map(|v| (v * 100.0).round());
            ui.label(egui::RichText::new(format!("C{c} M{m} Y{y} K{k}")).size(10.0).color(t.text_dim));
        });
        if open {
            cmyk_fields(app, ui, &format!("page-numbers-ink-{p}"), &mut d.inks[p]);
            ui.add_space(4.0);
        }
    }
    hint(ui, tr(app, "Click a colour to change it. Inks are CMYK, as the print shop prints them."));
}

// --- Mixing tab ---------------------------------------------------------------------------------

/// The mode `m` with the design's own settings when it is the design's mode.
fn same_kind(a: &Mix, b: &Mix) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

fn mixing_tab(app: &mut DesignApp, ui: &mut Ui, d: &mut Design, page: &str) {
    let t = Tokens::get(ui.ctx());
    let fw = widgets::full_width(ui);
    let fonts = crate::panels::fonts(app);
    // Gallery: every mode on this page's number.
    let cols = 4;
    let w = (fw - 3.0 * f32::from(cols as u8 - 1)) / cols as f32;
    let modes = Mix::all();
    let mut picked = None;
    egui::Grid::new("page-numbers-modes").spacing(egui::vec2(3.0, 1.0)).show(ui, |ui| {
        for (k, m) in modes.iter().enumerate() {
            let mine = same_kind(m, &d.mix);
            let shown = Design { mix: if mine { d.mix.clone() } else { m.clone() }, ..d.clone() };
            ui.vertical(|ui| {
                let r = preview::show(ui, &mut app.page_numbers.textures, &shown, page, &fonts, w, w * 0.7, 0.0);
                let name = tr(app, m.name());
                let r = r.on_hover_text(name);
                r.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, mine, name));
                if mine {
                    ui.painter().rect_stroke(r.rect.expand(1.0), 3.0, egui::Stroke::new(2.0, t.accent), egui::StrokeKind::Outside);
                }
                if r.clicked() && !mine {
                    picked = Some(m.clone());
                }
                ui.label(egui::RichText::new(name).size(9.0).color(if mine { t.text_strong } else { t.text_dim }));
            });
            if k % cols == cols - 1 {
                ui.end_row();
            }
        }
    });
    if let Some(m) = picked {
        app.page_numbers.scroll_to_custom = matches!(m, Mix::Custom { .. });
        d.mix = m;
    }
    widgets::rule(ui);
    let title = tr(app, d.mix.name()).to_string();
    widgets::section(ui, &title, true);
    mode_fields(app, ui, d);
}

fn mode_fields(app: &mut DesignApp, ui: &mut Ui, d: &mut Design) {
    let fw = widgets::full_width(ui);
    let tr = |s: &'static str| crate::i18n::tr(LANG, s).to_string();
    let mut custom = false;
    match &mut d.mix {
        Mix::Overprint { ink_limit, opacity } => {
            percent32(ui, &tr("Total Ink Limit"), "pn-tac", ink_limit, (1.0, 4.0));
            percent32(ui, &tr("Opacity"), "pn-op", opacity, (0.0, 1.0));
        }
        Mix::Multiply { strength } => {
            percent32(ui, &tr("Strength"), "pn-st", strength, (0.0, 1.0));
        }
        Mix::Pigment { weights, saturation } => {
            for (p, l) in ["Hundreds", "Tens", "Ones"].iter().enumerate() {
                percent32(ui, &format!("{} {}", tr("Ink Weight"), tr(l)), &format!("pn-w{p}"), &mut weights[p], (0.0, 3.0));
            }
            percent32(ui, &tr("Saturation"), "pn-sat", saturation, (-1.0, 1.0));
        }
        Mix::Light { strength, glow, glow_color } => {
            percent32(ui, &tr("Strength"), "pn-st", strength, (0.0, 1.0));
            percent32(ui, &tr("Glow"), "pn-glow", glow, (0.0, 1.0));
            widgets::caption(ui, &tr("Glow Colour"));
            cmyk_fields(app, ui, "pn-glowc", glow_color);
        }
        Mix::Perceptual { lightness, chroma } => {
            percent32(ui, &tr("Lightness"), "pn-l", lightness, (-1.0, 1.0));
            percent32(ui, &tr("Saturation"), "pn-c", chroma, (-1.0, 1.0));
        }
        Mix::HueJump { angle, chroma } => {
            let mut a = f64::from(*angle);
            if number(ui, &tr("Angle"), "pn-angle", &mut a, "°", (-180.0, 180.0), 0) {
                *angle = a as f32;
            }
            percent32(ui, &tr("Saturation"), "pn-c", chroma, (0.0, 2.0));
        }
        Mix::Darken { strength, lighten } => {
            percent32(ui, &tr("Strength"), "pn-st", strength, (0.0, 1.0));
            let labels = [tr("Darken"), tr("Lighten")];
            if let Some(i) = seg(ui, "pn-seg-4", &[labels[0].as_str(), labels[1].as_str()], usize::from(*lighten), fw) {
                *lighten = i == 1;
            }
        }
        Mix::Difference { exclusion, strength, min_lightness } => {
            let labels = [tr("Difference"), tr("Exclusion")];
            if let Some(i) = seg(ui, "pn-seg-5", &[labels[0].as_str(), labels[1].as_str()], usize::from(*exclusion), fw) {
                *exclusion = i == 1;
            }
            percent32(ui, &tr("Strength"), "pn-st", strength, (0.0, 1.0));
            percent32(ui, &tr("Minimum Lightness"), "pn-minl", min_lightness, (0.0, 1.0));
        }
        Mix::Knockout { cut, edge, fill } => {
            let labels = [tr("Overlaps Are Paper"), tr("Upper Digit Cuts")];
            if let Some(i) = seg(ui, "pn-seg-6", &[labels[0].as_str(), labels[1].as_str()], usize::from(*cut), fw) {
                *cut = i == 1;
            }
            if *cut {
                number(ui, &tr("Edge"), "pn-edge", edge, " pt", (0.0, 6.0), 1);
            } else {
                let mut on = fill.is_some();
                if widgets::checkbox(ui, &mut on, &tr("Fill Overlaps with a Colour")).changed() {
                    *fill = on.then_some([0.0, 0.0, 0.0, 0.0]);
                }
                if let Some(c) = fill {
                    cmyk_fields(app, ui, "pn-kofill", c);
                }
            }
        }
        Mix::Depth { step } => {
            percent32(ui, &tr("Darker per Layer"), "pn-step", step, (0.0, 1.0));
        }
        Mix::Halftone { shape, pitch, size, angles } => {
            let labels = [tr("Dots"), tr("Lines"), tr("Crosses")];
            let at = match shape {
                Dot::Round => 0,
                Dot::Line => 1,
                Dot::Cross => 2,
            };
            if let Some(i) = seg(ui, "pn-seg-7", &[labels[0].as_str(), labels[1].as_str(), labels[2].as_str()], at, fw) {
                *shape = [Dot::Round, Dot::Line, Dot::Cross][i];
            }
            number(ui, &tr("Screen Spacing"), "pn-pitch", pitch, " pt", (0.3, 6.0), 1);
            percent32(ui, &tr("Dot Size"), "pn-size", size, (0.1, 1.0));
            for (p, l) in ["Hundreds", "Tens", "Ones"].iter().enumerate() {
                let mut a = f64::from(angles[p]);
                if number(ui, &format!("{} {}", tr("Angle"), tr(l)), &format!("pn-a{p}"), &mut a, "°", (0.0, 180.0), 0) {
                    angles[p] = a as f32;
                }
            }
            hint(ui, &tr("Very fine screens may not print cleanly: keep dots above about 0.3 pt."));
        }
        Mix::Custom { .. } => custom = true,
    }
    if custom {
        custom_fields(app, ui, d);
    }
}

fn custom_fields(app: &mut DesignApp, ui: &mut Ui, d: &mut Design) {
    let fw = widgets::full_width(ui);
    let tr = |s: &'static str| crate::i18n::tr(LANG, s).to_string();
    // Start value: the mode the unchanged overlaps follow.
    let modes: Vec<Mix> = Mix::all().into_iter().filter(|m| !matches!(m, Mix::Custom { .. })).collect();
    let names: Vec<String> = modes.iter().map(|m| tr(m.name())).collect();
    let Mix::Custom { base } = &d.mix else { return };
    let cur = modes.iter().position(|m| same_kind(m, base));
    // Just picked in the gallery: bring the diagram up (it lies below the gallery).
    if std::mem::take(&mut app.page_numbers.scroll_to_custom) {
        ui.scroll_to_cursor(Some(egui::Align::TOP));
    }
    widgets::caption(ui, &tr("Other Overlaps Like"));
    if let Some(i) = widgets::dropdown_list(ui, cur.map_or("", |i| names[i].as_str()), fw, &names, cur) {
        d.mix = Mix::Custom { base: Box::new(modes[i].clone()) };
    }
    if app.page_numbers.region.count_ones() < 2 {
        app.page_numbers.region = 0b101;
    }
    if let Some(r) = preview::venn(ui, d, app.page_numbers.region, fw) {
        app.page_numbers.region = r;
    }
    hint(ui, &tr("Click an overlap to choose its colour."));
    let region = app.page_numbers.region;
    let label =
        [("Hundreds", 0), ("Tens", 1), ("Ones", 2)].iter().filter(|(_, p)| region >> p & 1 == 1).map(|(l, _)| tr(l)).collect::<Vec<_>>().join(" + ");
    widgets::caption(ui, &label);
    let key = region_key(region);
    let now = d.custom.get(&key).cloned();
    let opts = [tr("Like the Others"), tr("Paper"), tr("Own Colour")];
    let at = match now {
        None => 0,
        Some(RegionFill::Paper) => 1,
        Some(RegionFill::Color { .. }) => 2,
    };
    if let Some(i) = seg(ui, "pn-seg-8", &[opts[0].as_str(), opts[1].as_str(), opts[2].as_str()], at, fw) {
        match i {
            0 => {
                d.custom.remove(&key);
            }
            1 => {
                d.custom.insert(key.clone(), RegionFill::Paper);
            }
            _ => {
                let start = designcraft_compose::pagenum::region_color(d, region).map_or([0.0; 4], |c| {
                    let [c, m, y, k] = c.to_cmyk();
                    [c, m, y, k]
                });
                d.custom.insert(key.clone(), RegionFill::Color { cmyk: start });
            }
        }
    }
    if let Some(RegionFill::Color { cmyk }) = d.custom.get_mut(&key) {
        cmyk_fields(app, ui, &format!("pn-region-{key}"), cmyk);
    }
}

// --- applying -----------------------------------------------------------------------------------

fn apply_section(app: &mut DesignApp, ui: &mut Ui, set: &Settings, d: &Design) {
    let fw = widgets::full_width(ui);
    widgets::rule(ui);
    widgets::section(ui, tr(app, "Use for Page Numbers"), true);
    let info = app.session.execute("pageNumbers.settings", &json!({})).unwrap_or_default();
    let markers: Vec<(String, u64)> = info["markers"]
        .as_array()
        .map(|a| a.iter().map(|m| (m["style"].as_str().unwrap_or("").to_string(), m["count"].as_u64().unwrap_or(0))).collect())
        .unwrap_or_default();
    let mine: Vec<&String> = set.styles.iter().filter(|(_, v)| **v == d.name).map(|(k, _)| k).collect();
    if markers.is_empty() {
        hint(
            ui,
            tr(
                app,
                "This document has no page number yet. Put Type › Insert Special Character › Markers › Current Page Number in a text frame on a parent page.",
            ),
        );
        return;
    }
    let shown: Vec<String> = markers.iter().map(|(s, n)| format!("{} ({n})", if s.is_empty() { tr(app, "no character style") } else { s })).collect();
    hint(ui, &format!("{} {}", tr(app, "Page numbers in the document:"), shown.join(", ")));
    if mine.is_empty() {
        if widgets::outline_button(ui, tr(app, "Use This Design"), fw).clicked() {
            run(app, "pageNumbers.assign", json!({"design": d.name}));
        }
    } else {
        let names: Vec<&str> = mine.iter().map(|s| s.as_str()).collect();
        hint(ui, &format!("{} {}", tr(app, "Used by character style:"), names.join(", ")));
        if widgets::outline_button(ui, tr(app, "Stop Using"), fw).clicked() {
            for s in names {
                run(app, "pageNumbers.assign", json!({"design": null, "style": s}));
            }
        }
    }
    if widgets::link_label(ui, tr(app, "Delete Design"), 100.0).clicked() {
        run(app, "pageNumbers.deleteDesign", json!({"name": d.name}));
        app.page_numbers.design = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_values_round_trip_through_the_style() {
        let (base, axes) = split_style("Black {opsz:72,SOFT:50}");
        assert_eq!(base, "Black");
        assert_eq!(join_style(&base, &axes), "Black {SOFT:50,opsz:72}");
        assert_eq!(split_style("Regular"), ("Regular".to_string(), BTreeMap::new()));
    }

    #[test]
    fn the_panel_makes_and_uses_a_design() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.newSample", json!({})).unwrap();
        app.page_numbers.design = None;
        let set = Settings::from_labels(&app.session.active().unwrap().doc.labels);
        let d = new_design(&app, unique_name(&set, "Riso"));
        run(&mut app, "pageNumbers.setDesign", json!({"design": d}));
        run(&mut app, "pageNumbers.assign", json!({"design": d.name}));
        let set = Settings::from_labels(&app.session.active().unwrap().doc.labels);
        assert_eq!(set.designs.len(), 1);
        assert!(set.styles.values().any(|v| *v == d.name), "the sample's page numbers use it");
    }
}
