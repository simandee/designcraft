//! Queries for agents and UIs: document structure, items, pages.

use designcraft_doc::{Content, Item};
use serde_json::{Value, json};

use super::{CommandSpec, always, cmd, has_doc};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "document.inspect", "Inspect Document", [], None, "{} → pages, spreads, items, stories (with overset), styles, swatches, selection", has_doc, inspect),
        cmd!(query "document.label", "Document Label", [], None, "{key} → the text stored under `key` on the document, or null", has_doc, |s, p| {
            let key = super::str_param(p, "key").ok_or_else(|| super::bad("document.label", "`key` required"))?;
            Ok(s.doc()?.doc.labels.get(key).map_or(Value::Null, |v| json!(v)))
        }),
        cmd!(
            "document.setLabel",
            "Set Document Label",
            [],
            None,
            "{key, value?: string (null or missing: remove it)} — free text saved with the document under `key`, for scripts and extensions",
            has_doc,
            |s, p| {
                let key = super::str_param(p, "key")
                    .filter(|k| !k.is_empty())
                    .ok_or_else(|| super::bad("document.setLabel", "`key` required"))?
                    .to_string();
                let value = match p.get("value") {
                    None | Some(Value::Null) => None,
                    Some(Value::String(v)) => Some(v.clone()),
                    // Anything else must not quietly remove the label.
                    Some(_) => return Err(super::bad("document.setLabel", "`value` must be text (or null to remove the label)")),
                };
                s.edit(|d, _| {
                    match &value {
                        Some(v) => d.labels.insert(key.clone(), v.clone()),
                        None => d.labels.remove(&key),
                    };
                    Ok(json!({"key": key, "value": value}))
                })
            }
        ),
        cmd!(query "app.links", "Community & Project Links", [], None,
            "{} → {discord, website, appPage, github, issues} (ArtCraft Discord, website, DesignCraft page and repository)", always, |_, _| Ok(crate::links::all())),
        cmd!(query "document.list", "List Documents", [], None, "{}", always, |s, _| {
            let preview = s.new_document_preview();
            Ok(json!(s
                .documents()
                .iter()
                .enumerate()
                .map(|(i, d)| {
                    let mut v = json!({"index": i, "title": d.title(), "dirty": d.is_dirty(), "active": Some(i) == s.active_index()});
                    // New Document's preview, not a document yet.
                    if preview == Some(d.uid) {
                        v["preview"] = json!(true);
                    }
                    v
                })
                .collect::<Vec<_>>()))
        }),
        cmd!(noundo "tool.select", "Select Tool", [], None, "{tool: selection|directSelection|type|rectangleFrame|…}", always, |s, p| {
            let t = p.get("tool").and_then(Value::as_str).unwrap_or("selection");
            if designcraft_tools::tool_info(t).is_none() && t != "placeGun" {
                return Err(super::bad("tool.select", format!("unknown tool `{t}`")));
            }
            s.set_tool(t);
            Ok(json!({"tool": s.tool_id()}))
        }),
        cmd!(query "tool.list", "List Tools", [], None, "{}", always, |_, _| Ok(serde_json::to_value(designcraft_tools::TOOL_GROUPS).unwrap_or_default())),
        cmd!(query "document.history", "History", [], None, "{}", has_doc, |s, _| {
            let st = s.doc()?;
            Ok(json!({"undo": st.history.undo.iter().map(|e| &e.label).collect::<Vec<_>>(), "redo": st.history.redo.iter().map(|e| &e.label).collect::<Vec<_>>()}))
        }),
    ]
}

fn item_json(it: &Item) -> Value {
    let b = it.bounds();
    let mut v = json!({
        "id": it.id.0, "name": it.name, "kind": it.default_label(), "layer": it.layer.0,
        "bounds": [b.x0, b.y0, b.x1, b.y1], "fill": it.fill.swatch, "stroke": {"swatch": it.stroke.swatch, "weight": it.stroke.weight},
        "locked": it.locked, "hidden": it.hidden,
    });
    match &it.content {
        Content::Text(t) => {
            v["story"] = json!(t.story.0);
            v["columns"] = json!(t.options.columns);
            v["columnRule"] = json!(t.options.column_rule);
            if t.options.column_rule {
                v["columnRuleWeight"] = json!(t.options.column_rule_weight);
                v["columnRuleColor"] = json!(t.options.column_rule_color);
                v["columnRuleTint"] = json!(t.options.column_rule_tint);
                v["columnRuleOffset"] = json!(t.options.column_rule_offset);
                v["columnRuleTopInset"] = json!(t.options.column_rule_top_inset);
                v["columnRuleBottomInset"] = json!(t.options.column_rule_bottom_inset);
            }
        }
        Content::Graphic(g) => v["asset"] = json!(g.asset.0),
        Content::Group { items } => v["children"] = Value::Array(items.iter().map(|c| item_json(c)).collect()),
        Content::Unassigned => {}
    }
    v
}

fn inspect(s: &mut Session, _p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let d = &st.doc;
    let spreads: Vec<Value> = d
        .spreads
        .iter()
        .enumerate()
        .map(|(si, sp)| {
            json!({
                "index": si,
                "pages": sp.pages.iter().enumerate().map(|(pi, p)| {
                    let abs = d.first_page_of_spread(si) + pi;
                    json!({"index": abs, "name": d.page_name(abs), "bounds": [p.x, 0.0, p.x + p.width, p.height], "side": p.side,
                        "parent": p.parent.and_then(|pid| d.parents.iter().find(|x| x.id == pid)).and_then(|x| x.parent.as_ref()).map(|i| i.label()),
                        "margins": p.margins, "columns": p.columns.count, "gutter": p.columns.gutter,
                        "customColumns": p.columns.valid_positions(p.margin_rect().width()).is_some(),
                        "columnRects": p.column_rects().iter().map(|r| [r.x0, r.y0, r.x1, r.y1]).collect::<Vec<_>>()})
                }).collect::<Vec<_>>(),
                "items": sp.items.iter().map(|i| item_json(i)).collect::<Vec<_>>(),
            })
        })
        .collect();
    let stories: Vec<Value> = d
        .stories
        .values()
        .map(|story| {
            let cs = s.cache.get(d, story.id, None);
            let preview: String = story.text.chars().take(80).collect();
            json!({"id": story.id.0, "frames": story.frames.iter().map(|f| f.0).collect::<Vec<_>>(), "length": story.len(), "paragraphs": story.paras.len(),
                "overset": cs.overset_at.is_some(), "lines": cs.line_count(), "preview": preview})
        })
        .collect();
    Ok(json!({
        "title": st.title(), "path": st.path, "dirty": st.is_dirty(), "pageCount": d.page_count(),
        "settings": {"pageWidth": d.settings.page_width, "pageHeight": d.settings.page_height, "facingPages": d.settings.facing_pages, "intent": d.settings.intent},
        "spreads": spreads,
        "parents": d.parents.iter().map(|p| {
            let based_on = p.parent.as_ref().and_then(|i| i.based_on).and_then(|b| d.parents.iter().find(|x| x.id == b)).and_then(|x| x.parent.as_ref()).map(|i| i.label());
            json!({"id": p.id.0, "label": p.parent.as_ref().map(|i| i.label()), "basedOn": based_on, "items": p.items.len()})
        }).collect::<Vec<_>>(),
        "layers": d.layers,
        "stories": stories,
        "paragraphStyles": d.styles.paragraph.iter().map(|p| &p.name).collect::<Vec<_>>(),
        "characterStyles": d.styles.character.iter().map(|p| &p.name).collect::<Vec<_>>(),
        "swatches": d.swatches.iter().map(|w| &w.name).collect::<Vec<_>>(),
        "selection": st.selection,
        "activeLayer": st.active_layer.0,
        "tool": s.tool_id(),
        "canUndo": !st.history.undo.is_empty(),
    }))
}

#[cfg(test)]
mod label_tests {
    use serde_json::{Value, json};

    use crate::Session;

    /// A document label is kept under its key, saved with the file, undone like an edit, and
    /// removed with no value.
    #[test]
    fn document_labels_are_kept_saved_and_undone() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        assert_eq!(s.execute("document.label", &json!({"key": "a"})).unwrap(), Value::Null);
        s.execute("document.setLabel", &json!({"key": "a", "value": "{\"x\":1}"})).unwrap();
        assert_eq!(s.execute("document.label", &json!({"key": "a"})).unwrap(), json!("{\"x\":1}"));
        let saved = designcraft_format::save(&s.doc().unwrap().doc).unwrap();
        assert_eq!(designcraft_format::load(&saved).unwrap().labels.get("a").map(String::as_str), Some("{\"x\":1}"));
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.execute("document.label", &json!({"key": "a"})).unwrap(), Value::Null, "undone");
        s.execute("document.setLabel", &json!({"key": "a", "value": "1"})).unwrap();
        s.execute("document.setLabel", &json!({"key": "a"})).unwrap();
        assert_eq!(s.execute("document.label", &json!({"key": "a"})).unwrap(), Value::Null, "removed");
        assert!(s.execute("document.setLabel", &json!({"value": "1"})).is_err(), "a key is needed");
        s.execute("document.setLabel", &json!({"key": "a", "value": "kept"})).unwrap();
        assert!(s.execute("document.setLabel", &json!({"key": "a", "value": [1]})).is_err(), "not text");
        assert_eq!(s.execute("document.label", &json!({"key": "a"})).unwrap(), json!("kept"), "a refused value leaves the label");
    }
}
