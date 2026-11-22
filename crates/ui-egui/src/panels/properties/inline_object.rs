//! A picture (or other object) in text selected with the Selection tool: its frame's size (which
//! crops the picture), its overall scale, the picture's scale and position inside the frame, and
//! Frame Fitting, as for a frame on the page. A chat photo in the Jahresbuch is such an object.

use designcraft_doc::InlineSel;
use egui::Ui;
use serde_json::{Value, json};

use super::{divider, icon_toggle, section, units};
use crate::DesignApp;
use crate::widgets::{self, NumField};

fn tr<'a>(app: &'a DesignApp, s: &'a str) -> &'a str {
    crate::i18n::tr(&app.ui.language, s)
}

/// Run `command` on the selected object (it is the selection), reporting a failure.
fn run(app: &mut DesignApp, command: &str, params: Value) {
    if let Err(e) = app.run(command, params) {
        app.status(e);
    }
}

pub(super) fn inline_sections(app: &mut DesignApp, ui: &mut Ui, sel: InlineSel) {
    let Some(o) = app.session.active().and_then(|st| st.doc.story(sel.story)?.objects.get(sel.index).cloned()) else { return };
    let unit = units(app);
    let size = o.size();
    let frame = o.picture_rect();
    let content = o.picture_content();
    section(ui, tr(app, "Transform"), true);
    // The picture frame's size: changing it crops the picture (it stays as large as it is).
    let (w, h) = frame.map_or(size, |r| (r.width(), r.height()));
    ui.horizontal(|ui| {
        widgets::caption_w(ui, tr(app, "W:"), 22.0);
        if let Some(v) = NumField::measure("inline-w", Some(w), unit).name("Frame width").spinner().width(84.0).show(ui) {
            run(app, if frame.is_some() { "anchored.frame" } else { "anchored.resize" }, json!({"width": v}));
        }
        widgets::caption_w(ui, tr(app, "H:"), 22.0);
        if let Some(v) = NumField::measure("inline-h", Some(h), unit).name("Frame height").spinner().width(84.0).show(ui) {
            run(app, if frame.is_some() { "anchored.frame" } else { "anchored.resize" }, json!({"height": v}));
        }
    });
    // Scale everything (frame, picture, outline) by a percentage; the field is back at 100 %.
    ui.horizontal(|ui| {
        widgets::caption_w(ui, tr(app, "Scale:"), 44.0);
        if let Some(pct) = NumField::number("inline-scale", Some(100.0), "%", 1).name("Scale").spinner().range(1.0, 1000.0).width(96.0).show(ui)
            && (pct - 100.0).abs() > 1e-9
        {
            run(app, "anchored.resize", json!({"width": size.0 * pct / 100.0}));
        }
    });
    if frame.is_some() {
        crate::rtl::label(
            ui,
            egui::RichText::new(tr(
                app,
                "W and H crop the picture. Double-click it or use its content grabber to move or scale it inside the frame.",
            ))
            .size(10.5)
            .color(crate::theme::Tokens::get(ui.ctx()).text_dim),
        );
    }
    divider(ui);
    if let (Some(f), Some((c, (sx, _)))) = (frame, content) {
        section(ui, tr(app, if sel.content { "Content (selected)" } else { "Content" }), true);
        ui.horizontal(|ui| {
            widgets::caption_w(ui, tr(app, "Scale:"), 44.0);
            if let Some(pct) = NumField::number("inline-content-scale", Some(sx * 100.0), "%", 1)
                .name("Content scale")
                .spinner()
                .range(1.0, 10000.0)
                .width(96.0)
                .show(ui)
            {
                run(app, "anchored.content", json!({"scale": pct / 100.0}));
            }
        });
        // Where the picture's top-left is, from the frame's top-left.
        ui.horizontal(|ui| {
            widgets::caption_w(ui, tr(app, "X:"), 22.0);
            if let Some(v) = NumField::measure("inline-content-x", Some(c.x0 - f.x0), unit).name("Content X offset").spinner().width(84.0).show(ui) {
                run(app, "anchored.content", json!({"dx": v - (c.x0 - f.x0)}));
            }
            widgets::caption_w(ui, tr(app, "Y:"), 22.0);
            if let Some(v) = NumField::measure("inline-content-y", Some(c.y0 - f.y0), unit).name("Content Y offset").spinner().width(84.0).show(ui) {
                run(app, "anchored.content", json!({"dy": v - (c.y0 - f.y0)}));
            }
        });
        divider(ui);
        section(ui, tr(app, "Frame Fitting"), true);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            for (icon, mode, tip) in [
                ("fit-fill", "fillProportionally", "Fill Frame Proportionally"),
                ("fit-prop", "fitProportionally", "Fit Content Proportionally"),
                ("fit-content", "fitContentToFrame", "Fit Content to Frame"),
                ("fit-frame", "fitFrameToContent", "Fit Frame to Content"),
                ("fit-center", "centerContent", "Center Content"),
            ] {
                if icon_toggle(ui, icon, false, tr(app, tip)).clicked() {
                    run(app, "anchored.fit", json!({"mode": mode}));
                }
            }
        });
        divider(ui);
    }
    section(ui, tr(app, "Quick Actions"), true);
    let (a, b) = widgets::button_pair(ui, tr(app, if sel.content { "Select Frame" } else { "Select Content" }), tr(app, "Delete"));
    if a {
        run(app, "selection.set", json!({"inline": {"story": sel.story.0, "index": sel.index, "content": !sel.content}}));
    }
    if b {
        run(app, "anchored.delete", json!({}));
    }
}

/// The Control panel for an object in text: W/H of its frame (crop), its scale, and the picture's
/// scale inside the frame, with the fitting buttons.
pub fn inline_control(app: &mut DesignApp, ui: &mut Ui, sel: InlineSel) {
    let Some(o) = app.session.active().and_then(|st| st.doc.story(sel.story)?.objects.get(sel.index).cloned()) else { return };
    let unit = units(app);
    let size = o.size();
    let frame = o.picture_rect();
    let (w, h) = frame.map_or(size, |r| (r.width(), r.height()));
    let resize = if frame.is_some() { "anchored.frame" } else { "anchored.resize" };
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            widgets::caption_w(ui, tr(app, "W:"), 22.0);
            if let Some(v) = NumField::measure("ctl-inline-w", Some(w), unit).name("Frame width").spinner().width(84.0).show(ui) {
                run(app, resize, json!({"width": v}));
            }
        });
        ui.horizontal(|ui| {
            widgets::caption_w(ui, tr(app, "H:"), 22.0);
            if let Some(v) = NumField::measure("ctl-inline-h", Some(h), unit).name("Frame height").spinner().width(84.0).show(ui) {
                run(app, resize, json!({"height": v}));
            }
        });
    });
    ui.add_space(12.0);
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            widgets::caption_w(ui, tr(app, "Scale:"), 44.0);
            if let Some(pct) =
                NumField::number("ctl-inline-scale", Some(100.0), "%", 1).name("Scale").spinner().range(1.0, 1000.0).width(86.0).show(ui)
                && (pct - 100.0).abs() > 1e-9
            {
                run(app, "anchored.resize", json!({"width": size.0 * pct / 100.0}));
            }
        });
        if let Some((_, (sx, _))) = o.picture_content() {
            ui.horizontal(|ui| {
                widgets::caption_w(ui, tr(app, "Content:"), 44.0);
                if let Some(pct) = NumField::number("ctl-inline-content", Some(sx * 100.0), "%", 1)
                    .name("Content scale")
                    .spinner()
                    .range(1.0, 10000.0)
                    .width(86.0)
                    .show(ui)
                {
                    run(app, "anchored.content", json!({"scale": pct / 100.0}));
                }
            });
        }
    });
    if o.picture_content().is_some() {
        ui.add_space(12.0);
        for (icon, mode, tip) in [
            ("fit-fill", "fillProportionally", "Fill Frame Proportionally"),
            ("fit-prop", "fitProportionally", "Fit Content Proportionally"),
            ("fit-frame", "fitFrameToContent", "Fit Frame to Content"),
            ("fit-center", "centerContent", "Center Content"),
        ] {
            if icon_toggle(ui, icon, false, tr(app, tip)).clicked() {
                run(app, "anchored.fit", json!({"mode": mode}));
            }
        }
    }
}
