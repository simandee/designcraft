//! Layered page numbers (see `designcraft_doc::pagenum`): designs saved with the document, which
//! character styles use them, and rerolling one page.

use designcraft_doc::pagenum::{Design, LABEL, Settings};
use designcraft_doc::story::PAGE_NUMBER;
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_doc, str_param};
use crate::{Result, Session};

fn settings(s: &Session) -> Result<Settings> {
    Ok(Settings::from_labels(&s.doc()?.doc.labels))
}

/// Change the settings as one undoable edit.
fn change(s: &mut Session, f: impl FnOnce(&mut Settings) -> Result<Value>) -> Result<Value> {
    let mut set = settings(s)?;
    let out = f(&mut set)?;
    let json = set.to_json();
    s.edit(|d, _| {
        d.labels.insert(LABEL.to_string(), json);
        Ok(out)
    })
}

/// The character styles of the document's Current Page Number markers, with how many use each
/// ("" is none).
fn marker_styles(d: &designcraft_doc::Document) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for st in d.stories.values() {
        for (at, _) in st.text.match_indices(PAGE_NUMBER) {
            let mut pos = 0;
            let style = st.chars.iter().find(|r| {
                pos += r.len;
                at < pos
            });
            let name = style.map_or(String::new(), |r| r.format.style.clone());
            match out.iter_mut().find(|(n, _)| *n == name) {
                Some(e) => e.1 += 1,
                None => out.push((name, 1)),
            }
        }
    }
    out
}

fn has_style(name: &str) -> bool {
    !name.is_empty() && name != "[None]"
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "pageNumbers.settings", "Layered Page Numbers", [], None,
            "{} → {designs: [design], styles: {characterStyle: designName}, markers: [{style, count}]} (style \"\" = none)", has_doc, |s, _| {
            let set = settings(s)?;
            let markers: Vec<Value> = marker_styles(&s.doc()?.doc).into_iter().map(|(n, c)| json!({"style": n, "count": c})).collect();
            let mut v = serde_json::to_value(&set).unwrap_or_default();
            v["markers"] = json!(markers);
            Ok(v)
        }),
        cmd!(
            "pageNumbers.setDesign",
            "Set Page Number Design",
            [],
            None,
            "{design: {name, fontFamily, fontStyle, inks: [[c,m,y,k]×3], spacing, vertical, registration, top, mix, custom, rerolls}, replace?: name} — add the design, or replace the one named `replace` (or of its own name); renaming keeps the styles that used it",
            has_doc,
            |s, p| {
                let design: Design = serde_json::from_value(p.get("design").cloned().unwrap_or(Value::Null))
                    .map_err(|e| bad("pageNumbers.setDesign", format!("bad design: {e}")))?;
                if design.name.trim().is_empty() {
                    return Err(bad("pageNumbers.setDesign", "the design needs a name"));
                }
                let old = str_param(p, "replace").unwrap_or(&design.name).to_string();
                change(s, |set| {
                    for v in set.styles.values_mut() {
                        if *v == old {
                            v.clone_from(&design.name);
                        }
                    }
                    let name = design.name.clone();
                    match set.designs.iter_mut().position(|d| d.name == old) {
                        Some(i) => set.designs[i] = design,
                        None => set.designs.push(design),
                    }
                    Ok(json!({"name": name}))
                })
            }
        ),
        cmd!(
            "pageNumbers.deleteDesign",
            "Delete Page Number Design",
            [],
            None,
            "{name} — the design and the style links to it go",
            has_doc,
            |s, p| {
                let name = str_param(p, "name").ok_or_else(|| bad("pageNumbers.deleteDesign", "`name` required"))?.to_string();
                change(s, |set| {
                    set.designs.retain(|d| d.name != name);
                    set.styles.retain(|_, d| *d != name);
                    Ok(json!({"name": name}))
                })
            }
        ),
        cmd!(
            "pageNumbers.reroll",
            "Reroll Page Number",
            [],
            None,
            "{design, page: page name (\"712\")} — a new random layout for that page; {design, page, reset: true} goes back to the first",
            has_doc,
            |s, p| {
                let name = str_param(p, "design").ok_or_else(|| bad("pageNumbers.reroll", "`design` required"))?.to_string();
                let page = str_param(p, "page").ok_or_else(|| bad("pageNumbers.reroll", "`page` required"))?.to_string();
                let reset = p.get("reset").and_then(Value::as_bool).unwrap_or(false);
                change(s, |set| {
                    let d =
                        set.designs.iter_mut().find(|d| d.name == name).ok_or_else(|| bad("pageNumbers.reroll", format!("no design `{name}`")))?;
                    let n = if reset {
                        d.rerolls.remove(&page);
                        0
                    } else {
                        let n = d.rerolls.get(&page).map_or(1, |n| n.wrapping_add(1));
                        d.rerolls.insert(page.clone(), n);
                        n
                    };
                    Ok(json!({"page": page, "reroll": n}))
                })
            }
        ),
        cmd!(
            "pageNumbers.assign",
            "Use Page Number Design",
            [],
            None,
            "{design?: name (missing or null: none), style?: character style, newStyle?: name (default \"Page Number\")} — page numbers in `style` are drawn with the design. Without `style`: the style of the document's Current Page Number markers; markers without one get `newStyle`, made if missing",
            has_doc,
            |s, p| {
                let design = str_param(p, "design").map(str::to_string);
                if let Some(d) = &design
                    && settings(s)?.design(d).is_none()
                {
                    return Err(bad("pageNumbers.assign", format!("no design `{d}`")));
                }
                let mut styles: Vec<String> = match str_param(p, "style") {
                    Some(st) => vec![st.to_string()],
                    None => marker_styles(&s.doc()?.doc).into_iter().map(|(n, _)| n).collect(),
                };
                if styles.is_empty() {
                    return Err(bad(
                        "pageNumbers.assign",
                        "the document has no Current Page Number marker (Type › Insert Special Character › Markers)",
                    ));
                }
                // Markers without a character style get one, so the design has a style to hang on.
                if styles.iter().any(|n| !has_style(n)) {
                    let new = str_param(p, "newStyle").unwrap_or("Page Number").to_string();
                    if s.doc()?.doc.styles.char_style(&new).is_none() {
                        s.execute("style.character.create", &json!({"name": new}))?;
                    }
                    s.edit(|d, _| {
                        let st = d.styles.clone();
                        for story in d.stories.values_mut() {
                            let at: Vec<usize> = story.text.match_indices(PAGE_NUMBER).map(|(i, _)| i).collect();
                            if at.is_empty() {
                                continue;
                            }
                            let story = std::sync::Arc::make_mut(story);
                            for i in at {
                                let mut pos = 0;
                                let cur = story.chars.iter().find(|r| {
                                    pos += r.len;
                                    i < pos
                                });
                                if cur.is_none_or(|r| !has_style(&r.format.style)) {
                                    super::style::apply_char_style(story, &st, &new, &(i..i + PAGE_NUMBER.len_utf8()));
                                }
                            }
                        }
                        Ok(())
                    })?;
                    styles.retain(|n| has_style(n));
                    styles.push(new);
                }
                change(s, |set| {
                    for st in &styles {
                        match &design {
                            Some(d) => set.styles.insert(st.clone(), d.clone()),
                            None => set.styles.remove(st),
                        };
                    }
                    Ok(json!({"styles": styles, "design": design}))
                })
            }
        ),
    ]
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    /// A page with a Current Page Number marker, numbered from 712.
    fn page_with_number() -> (Session, u64) {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.execute("layout.section", &json!({"page": 1, "startNumber": 712})).unwrap();
        let r = s.execute("frame.create", &json!({"rect": [72, 72, 400, 200], "content": "text", "text": "x"})).unwrap();
        let sid = r["story"].as_u64().unwrap();
        s.execute("text.select", &json!({"story": sid, "anchor": 0, "focus": 1})).unwrap();
        s.execute("text.insert", &json!({"text": "\u{E000}", "raw": true})).unwrap();
        (s, sid)
    }

    fn page_number_decos(s: &Session, sid: u64) -> usize {
        let cs = s.cache.get(&s.doc().unwrap().doc, designcraft_doc::StoryId(sid), Some(0));
        cs.frames.iter().flat_map(|f| &f.decos).filter(|d| d.page_number).count()
    }

    #[test]
    fn a_design_draws_the_page_number_as_plates_and_undoes() {
        let (mut s, sid) = page_with_number();
        assert_eq!(page_number_decos(&s, sid), 0);
        s.execute("pageNumbers.setDesign", &json!({"design": {"name": "Riso"}})).unwrap();
        let r = s.execute("pageNumbers.assign", &json!({"design": "Riso"})).unwrap();
        assert_eq!(r["styles"], json!(["Page Number"]), "the marker got a character style");
        assert!(page_number_decos(&s, sid) >= 3, "the three digits are drawn as plates");
        let set = s.execute("pageNumbers.settings", &json!({})).unwrap();
        assert_eq!(set["styles"]["Page Number"], json!("Riso"));
        assert_eq!(set["markers"][0]["style"], json!("Page Number"));
        s.execute("pageNumbers.assign", &json!({"design": null})).unwrap();
        assert_eq!(page_number_decos(&s, sid), 0, "unlinked: ordinary text again");
        s.execute("edit.undo", &json!({})).unwrap();
        assert!(page_number_decos(&s, sid) >= 3, "undone");
    }

    /// The plates' own (mixed) colours reach the PDF: each digit and overlap is a filled shape.
    #[test]
    fn the_pdf_gets_the_plates_in_their_colours() {
        let (mut s, _) = page_with_number();
        let fills = |s: &Session| {
            let opts = designcraft_pdf::PdfOptions { compress: false, ..Default::default() };
            let pdf = designcraft_pdf::export_pdf(&s.doc().unwrap().doc, &s.cache, &opts).unwrap();
            let t = String::from_utf8_lossy(&pdf).to_string();
            t.matches(" k ").count() + t.matches(" k\n").count()
        };
        let plain = fills(&s);
        s.execute("pageNumbers.setDesign", &json!({"design": {"name": "Riso", "mix": {"mode": "multiply", "strength": 1.0}}})).unwrap();
        s.execute("pageNumbers.assign", &json!({"design": "Riso"})).unwrap();
        assert!(fills(&s) >= plain + 3, "{} CMYK fills with the design, {plain} without", fills(&s));
    }

    #[test]
    fn rerolling_changes_one_page_and_renaming_keeps_the_link() {
        let (mut s, _) = page_with_number();
        s.execute("pageNumbers.setDesign", &json!({"design": {"name": "A"}})).unwrap();
        s.execute("pageNumbers.assign", &json!({"design": "A"})).unwrap();
        s.execute("pageNumbers.reroll", &json!({"design": "A", "page": "712"})).unwrap();
        s.execute("pageNumbers.reroll", &json!({"design": "A", "page": "712"})).unwrap();
        let set = s.execute("pageNumbers.settings", &json!({})).unwrap();
        assert_eq!(set["designs"][0]["rerolls"]["712"], json!(2));
        s.execute("pageNumbers.setDesign", &json!({"design": {"name": "B"}, "replace": "A"})).unwrap();
        let set = s.execute("pageNumbers.settings", &json!({})).unwrap();
        assert_eq!(set["styles"]["Page Number"], json!("B"));
        assert_eq!(set["designs"].as_array().unwrap().len(), 1);
        assert!(s.execute("pageNumbers.reroll", &json!({"design": "A", "page": "1"})).is_err());
    }
}
