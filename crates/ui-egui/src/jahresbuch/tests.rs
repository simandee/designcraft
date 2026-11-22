//! Tests des Jahresbuch-Fensters. Die mit einem echten Export laufen nur, wenn einer da ist:
//! JAHRESBUCH_TEST_EXPORT=… JAHRESBUCH_TEST_MEDIEN=… cargo test -p designcraft-ui-egui jahresbuch

use serde_json::{Value, json};

use super::modell;
use crate::DesignApp;

fn app_mit(pages: u32) -> DesignApp {
    let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
    app.run("file.new", json!({"preset": "A4", "pages": pages, "facingPages": true})).unwrap();
    app
}

/// Der Test-Export (Pfad, Medienordner), wenn einer angegeben ist.
fn export() -> Option<(String, String)> {
    Some((std::env::var("JAHRESBUCH_TEST_EXPORT").ok()?, std::env::var("JAHRESBUCH_TEST_MEDIEN").ok()?))
}

fn einfuegen(app: &mut DesignApp, export: &str, medien: &str) -> Value {
    let p = json!({"export": export, "medien": medien, "von": "2026-11-01", "bis": "2026-11-30", "python": "python3"});
    app.run("ui:jahresbuch.chatImport", p.clone()).or_else(|_| super::befehl_einfuegen(app, &p)).unwrap()
}

fn absatzformat<'a>(app: &'a DesignApp, name: &str) -> Option<&'a designcraft_doc::ParagraphStyle> {
    app.session.active()?.doc.styles.paragraph.iter().find(|s| s.name == name)
}

fn gleiche_person(app: &DesignApp, name: &str) -> Option<f64> {
    absatzformat(app, name).and_then(|s| s.para.same_para_style_spacing.flatten())
}

/// Ein Regler-Zug im Fenster zeigt jede Stellung sofort im Dokument und wird beim Loslassen ein
/// einziger Rückgängig-Schritt; Rückgängig bringt den Stand von vorher zurück.
#[test]
fn ein_regler_zug_ist_ein_rueckgaengig_schritt() {
    let mut app = app_mit(1);
    app.run("swatch.create", json!({"name": "Farbe Ich", "color": "#E8493A"})).unwrap();
    app.run("style.paragraph.create", json!({"name": "Chat Sie", "para": {"sameParaStyleSpacing": 12}})).unwrap();
    let chats = modell::chats(&app.session.active().unwrap().doc, &Default::default());
    assert_eq!(chats.len(), 1, "ein Chat von vor dem Etikett wird als Chat 1 erkannt");
    let mut chat = chats[0].clone();
    for v in [16.0, 20.0, 24.0] {
        chat.setze("abstand.gleiche_person", json!(v));
        super::panel::anwenden(&mut app, &chats, &chat);
        app.hold_live_preview();
        app.settle_live_preview();
    }
    assert!(gleiche_person(&app, "Chat Sie").unwrap() > 20.0, "die letzte Stellung ist zu sehen");
    app.settle_live_preview();
    app.settle_live_preview();
    assert!(app.session.active().unwrap().interaction.is_none(), "losgelassen: übernommen");
    let gespeichert = modell::chats(&app.session.active().unwrap().doc, &Default::default());
    assert_eq!(gespeichert[0].zahl("abstand.gleiche_person", 0.0), 24.0, "die Gestaltung steht im Dokument");
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(gleiche_person(&app, "Chat Sie"), Some(12.0), "ein Rückgängig nimmt den ganzen Zug zurück");
}

/// Zwei Rahmen verkettet, der linke markiert: der Chat läuft vom linken in den rechten Rahmen und
/// erst danach auf neue Seiten.
#[test]
fn chat_fuellt_erst_die_verketteten_rahmen() {
    let Some((export, medien)) = export() else { return };
    let mut app = app_mit(1);
    let links = app.run("frame.create", json!({"spread": 0, "rect": [57.2, 60, 281.5, 764.8], "content": "text"})).unwrap();
    let rechts = app.run("text.thread", json!({"from": links["id"], "rect": [313.7, 60, 538, 764.8], "spread": 0})).unwrap()["to"].clone();
    app.run("selection.set", json!({"ids": [links["id"]]})).unwrap();
    einfuegen(&mut app, &export, &medien);
    let doc = app.session.execute("document.inspect", &json!({})).unwrap();
    let story = doc["spreads"][0]["items"].as_array().unwrap().iter().find(|i| i["id"] == links["id"]).unwrap()["story"].clone();
    let rahmen: Vec<Value> = app.run("story.get", json!({"story": story})).unwrap()["frames"].as_array().unwrap().clone();
    assert_eq!(rahmen.get(1), Some(&rechts), "nach dem linken Rahmen kommt der rechte, nicht die neuen Seiten: {rahmen:?}");
}

/// Ohne markierten Rahmen: die neue Chat-Seite hat in beiden Spalten verkettete Rahmen.
#[test]
fn neue_chat_seite_hat_beide_spalten() {
    let Some((export, medien)) = export() else { return };
    let mut app = app_mit(1);
    einfuegen(&mut app, &export, &medien);
    let doc = app.session.execute("document.inspect", &json!({})).unwrap();
    let rahmen: Vec<&Value> = doc["spreads"].as_array().unwrap().iter().flat_map(|sp| sp["items"].as_array().unwrap()).collect();
    let story = rahmen.iter().find(|i| i["story"].is_u64()).unwrap()["story"].clone();
    let kette: Vec<Value> = app.run("story.get", json!({"story": story})).unwrap()["frames"].as_array().unwrap().clone();
    assert!(kette.len() > 2, "der Chat läuft über mehrere Seiten");
    let x0 = |id: &Value| rahmen.iter().find(|i| i["id"] == *id).unwrap()["bounds"][0].as_f64().unwrap();
    let seite_b = doc["spreads"][1]["pages"][0]["bounds"].as_array().map(|b| b[2].as_f64().unwrap() - b[0].as_f64().unwrap()).unwrap_or(595.0);
    let abstand = x0(&kette[1]) - x0(&kette[0]);
    assert!(abstand > 0.0 && abstand < seite_b / 2.0, "zweite Spalte derselben Seite: {abstand} pt weiter rechts");
}

/// Der Chat landet im markierten Rahmen; ein Text daneben bleibt, wie er war.
#[test]
fn chat_fliesst_in_den_markierten_rahmen() {
    let Some((export, medien)) = export() else { return };
    let mut app = app_mit(3);
    app.run("frame.create", json!({"spread": 0, "rect": [60, 80, 500, 300], "content": "text", "text": "Unsere Geschichte"})).unwrap();
    let r = app.run("frame.create", json!({"spread": 1, "rect": [57.2, 60, 281.5, 764.8], "content": "text"})).unwrap();
    app.run("selection.set", json!({"ids": [r["id"]]})).unwrap();
    einfuegen(&mut app, &export, &medien);
    let story = app.run("story.get", json!({"story": r["story"]})).unwrap();
    assert!(story["text"].as_str().unwrap().contains("November"), "der Chat ist im markierten Rahmen");
    assert!(story["overset"].is_null(), "Seiten kamen dazu, bis er passt");
    let doc = app.session.execute("document.inspect", &json!({})).unwrap();
    assert!(doc["parents"].as_array().unwrap().iter().any(|p| p["label"] == "J-Chat"));
    assert!(doc["stories"].as_array().unwrap().iter().any(|s| s["preview"].as_str().is_some_and(|p| p.contains("Unsere Geschichte"))));
}

/// Ein zweiter Chat bekommt eigene Formate, Farbfelder und Mustervorlage; seine Gestaltung ändert
/// den ersten nicht, und jeder Text wird seinem Chat zugeordnet.
#[test]
fn zweiter_chat_hat_seinen_eigenen_look() {
    let Some((export, medien)) = export() else { return };
    let mut app = app_mit(1);
    let eins = einfuegen(&mut app, &export, &medien);
    let zwei = einfuegen(&mut app, &export, &medien);
    assert_eq!((eins["nummer"].as_u64(), zwei["nummer"].as_u64()), (Some(1), Some(2)));
    assert!(absatzformat(&app, "Chat 2/Chat Sie").is_some(), "eigene Formate");
    let doc = app.session.execute("document.inspect", &json!({})).unwrap();
    assert!(doc["parents"].as_array().unwrap().iter().any(|p| p["label"] == "J2-Chat"), "eigene Mustervorlage");
    let vorher = gleiche_person(&app, "Chat Sie");
    super::befehl_gestalten(&mut app, &json!({"chat": 2, "stil": {"abstand": {"gleiche_person": 30}}, "personen": {"Ich": "#00AA00"}})).unwrap();
    assert!(gleiche_person(&app, "Chat 2/Chat Sie").unwrap() > vorher.unwrap(), "Chat 2 ändert sich");
    assert_eq!(gleiche_person(&app, "Chat Sie"), vorher, "Chat 1 bleibt");
    let st = app.session.active().unwrap();
    let zuordnung: Vec<u32> = st.doc.stories.keys().filter_map(|sid| modell::chat_von_text(&st.doc, *sid)).collect();
    assert!(zuordnung.contains(&1) && zuordnung.contains(&2), "beide Texte gehören zu ihrem Chat: {zuordnung:?}");
    let chats = modell::chats(&st.doc, &Default::default());
    assert_eq!(
        chats.iter().find(|c| c.nummer == 2).and_then(|c| c.personen.iter().find(|p| p.key == "Ich")).map(|p| p.farbe.as_str()),
        Some("#00AA00")
    );
}

/// Schriftgrößen: die Formate ändern sich, Emojis wachsen mit dem Nachrichtentext und die
/// Verzierungen mit dem Datum.
#[test]
fn schriftgroessen_skalieren_emojis_und_verzierungen() {
    let Some((export, medien)) = export() else { return };
    let mut app = app_mit(1);
    einfuegen(&mut app, &export, &medien);
    let hoehen = |app: &DesignApp, label: &str| -> Vec<f64> {
        let doc = &app.session.active().unwrap().doc;
        doc.stories.values().flat_map(|st| st.objects.iter()).filter(|o| o.item.label == label).map(|o| o.size().1).collect()
    };
    let (emoji0, deko0) = (hoehen(&app, "Emoji"), hoehen(&app, "Verzierung"));
    assert!(!emoji0.is_empty() && !deko0.is_empty(), "der Test-Chat hat Emojis und Datumszeilen");
    let (text0, datum0) =
        (absatzformat(&app, "Chat Sie").and_then(|s| s.chars.size).unwrap(), absatzformat(&app, "Datum").and_then(|s| s.chars.size).unwrap());
    super::befehl_gestalten(&mut app, &json!({"chat": 1, "stil": {"schrift": {"text": text0 * 1.5, "datum": datum0 * 1.5}}})).unwrap();
    let verhaeltnis = |vorher: &[f64], nachher: &[f64]| nachher.iter().zip(vorher).map(|(n, v)| n / v).collect::<Vec<_>>();
    for f in verhaeltnis(&emoji0, &hoehen(&app, "Emoji")) {
        assert!((f - 1.5).abs() < 1e-3, "Emojis wie der Text um das 1,5-Fache: {f}");
    }
    for f in verhaeltnis(&deko0, &hoehen(&app, "Verzierung")) {
        assert!((f - 1.5).abs() < 1e-3, "Verzierungen wie das Datum um das 1,5-Fache: {f}");
    }
}

/// Fortsetzen hängt den neuen Zeitraum hinten an: der Text davor bleibt Zeichen für Zeichen, das
/// Ende im Etikett rückt weiter, und der angehängte Text erbt weder das Zeichenformat am Ende (die
/// Uhrzeit) noch die Handarbeit am letzten Absatz. Neu aufbauen setzt nur die Tage des gewählten
/// Zeitraums neu: eine Änderung darin verschwindet, eine dahinter bleibt.
#[test]
fn fortsetzen_haengt_an_und_neu_aufbauen_ersetzt() {
    let Some((export, medien)) = export() else { return };
    let mut app = app_mit(1);
    let p = json!({"export": export, "medien": medien, "von": "2026-11-01", "bis": "2026-11-10", "python": "python3"});
    let chat: modell::Chat = serde_json::from_value(super::befehl_einfuegen(&mut app, &p).unwrap()).unwrap();
    let story = {
        let doc = &app.session.active().unwrap().doc;
        *doc.stories.keys().find(|sid| modell::chat_von_text(doc, **sid) == Some(chat.nummer)).unwrap()
    };
    let text = |app: &DesignApp| app.session.active().unwrap().doc.story(story).unwrap().text.clone();
    let einfuegen_bei = |app: &mut DesignApp, pos: usize, t: &str| {
        app.run("text.select", json!({"story": story.0, "anchor": pos, "focus": pos})).unwrap();
        app.run("text.insert", json!({"text": t})).unwrap();
    };
    let vorher = text(&app);
    // Handarbeit am letzten Absatz: ein Einzug.
    app.run("text.select", json!({"story": story.0, "anchor": vorher.len(), "focus": vorher.len()})).unwrap();
    app.run("type.para", json!({"attrs": {"leftIndent": 40}})).unwrap();
    let auftrag = |von: &str, bis: &str| super::einfuegen::Auftrag {
        export: export.clone(),
        medien: medien.clone(),
        von: von.into(),
        bis: bis.into(),
        python: "python3".into(),
        stil: chat.stil.clone(),
        ..Default::default()
    };
    super::einfuegen::fortsetzen(&mut app, &auftrag("2026-11-11", "2026-11-30"), &chat, super::einfuegen::Wohin::Anhaengen(story.0)).unwrap();
    let nachher = text(&app);
    assert!(nachher.starts_with(&vorher) && nachher.len() > vorher.len(), "der neue Zeitraum kommt hinten dazu");
    {
        let st = app.session.active().unwrap().doc.story(story).unwrap();
        let zeit_am_anfang = |r: &std::ops::Range<usize>| st.format_after(r.start).style == "Zeit";
        let (alt, neu): (Vec<_>, Vec<_>) =
            st.para_ranges().into_iter().enumerate().filter(|(_, r)| !r.is_empty()).partition(|(_, r)| r.start < vorher.len());
        assert!(!neu.is_empty());
        assert_eq!(
            neu.iter().any(|(_, r)| zeit_am_anfang(r)),
            alt.iter().any(|(_, r)| zeit_am_anfang(r)),
            "angehängte Absätze beginnen wie die ersten, nicht in der Uhrzeit"
        );
        assert!(neu.iter().all(|(pi, _)| st.paras[*pi].para.left_indent != Some(40.0)), "der Einzug des letzten Absatzes wandert nicht mit");
    }
    let gespeichert = modell::chats(&app.session.active().unwrap().doc, &Default::default());
    assert!(
        chat.bis.as_str() <= "2026-11-10" && gespeichert[0].bis.as_str() > "2026-11-10" && gespeichert[0].bis.as_str() <= "2026-11-30",
        "das Ende rückt auf den letzten angehängten Tag: {} → {}",
        chat.bis,
        gespeichert[0].bis
    );
    assert!(
        super::einfuegen::fortsetzen(&mut app, &auftrag("2026-11-20", "2026-11-30"), &gespeichert[0], super::einfuegen::Wohin::Anhaengen(story.0))
            .is_err(),
        "Fortsetzen mit Tagen, die schon im Buch stehen, wird abgelehnt"
    );
    // Eine Änderung im ersten Zeitraum und eine im angehängten.
    einfuegen_bei(&mut app, nachher.len(), " DAHINTER");
    let zweiter_absatz = nachher.find('\n').unwrap() + 1;
    einfuegen_bei(&mut app, zweiter_absatz, "DARIN ");
    super::einfuegen::fortsetzen(&mut app, &auftrag("2026-11-01", "2026-11-10"), &gespeichert[0], super::einfuegen::Wohin::Ersetzen(story.0))
        .unwrap();
    let neu_aufgebaut = text(&app);
    assert!(!neu_aufgebaut.contains("DARIN") && neu_aufgebaut.contains(" DAHINTER"), "nur der gewählte Zeitraum ist neu gesetzt");
    assert_eq!(neu_aufgebaut.replace(" DAHINTER", ""), nachher, "der Zeitraum steht wieder wie eingefügt, der Rest bleibt");
    let danach = modell::chats(&app.session.active().unwrap().doc, &Default::default());
    assert_eq!((&danach[0].von, &danach[0].bis), (&gespeichert[0].von, &gespeichert[0].bis), "der Chat umfasst weiter beide Zeiträume");
}

/// Der Schatten der Spalten-Box besteht aus zwei schmalen Streifen (nur der sichtbare Rand). Ein
/// großer Schatten von früher wird beim nächsten Gestalten durch sie ersetzt; die Auswahl bleibt.
#[test]
fn der_spalten_schatten_ist_nur_der_sichtbare_rand() {
    let Some((export, medien)) = export() else { return };
    let mut app = app_mit(1);
    einfuegen(&mut app, &export, &medien);
    let schatten = |app: &DesignApp| -> Vec<(u64, f64)> {
        let doc = &app.session.active().unwrap().doc;
        let k = doc.parents.iter().position(|p| p.parent.as_ref().is_some_and(|i| i.label() == "J-Chat")).unwrap();
        doc.parents[k]
            .items
            .iter()
            .filter(|it| it.label.starts_with("Spalte Schatten"))
            .map(|it| (it.id.0, it.bounds().width().min(it.bounds().height())))
            .collect()
    };
    let neu = schatten(&app);
    assert!(!neu.is_empty() && neu.iter().all(|(_, schmal)| *schmal < 20.0), "nur schmale Streifen: {neu:?}");
    // Ein großer Schatten wie in älteren Büchern.
    let k = app.session.active().unwrap().doc.parents.iter().position(|p| p.parent.as_ref().is_some_and(|i| i.label() == "J-Chat")).unwrap();
    let gross = app
        .run("frame.create", json!({"spread": {"kind": "parent", "index": k}, "rect": [100, 100, 300, 600], "content": "text", "text": "x"}))
        .unwrap()["id"]
        .clone();
    app.run("object.label", json!({"label": "Spalte Schatten", "ids": [gross]})).unwrap();
    app.run("selection.set", json!({"ids": []})).unwrap();
    super::befehl_gestalten(&mut app, &json!({"chat": 1, "stil": {"korn_blasen": 0.5}})).unwrap();
    let danach = schatten(&app);
    assert!(!danach.iter().any(|(id, _)| json!(id) == gross), "der große Schatten ist weg");
    assert_eq!(danach.len(), neu.len() + 5, "an seiner Stelle die fünf Teile am Rand");
    assert!(danach.iter().all(|(_, schmal)| *schmal < 20.0));
    assert!(app.session.active().unwrap().selection.is_empty(), "die Auswahl ist wie vorher");
}

/// Ein Buch von früher: der Schatten der Spalten-Box als große Fläche unter der Papierfläche, auf
/// der unteren von zwei Ebenen, während die obere aktiv ist. Beim Gestalten wird er zu den Teilen am
/// sichtbaren Rand: sie decken jeden Punkt, an dem der Schatten zu sehen war, liegen auf seiner
/// Ebene unter der Papierfläche, und die Auswahl bleibt.
#[test]
fn ein_alter_spalten_schatten_wird_zum_sichtbaren_rand() {
    let mut app = app_mit(1);
    app.run("style.paragraph.create", json!({"name": "Chat Sie"})).unwrap();
    app.run("swatch.create", json!({"name": "Spalte · Schatten", "color": "#3554E0"})).unwrap();
    let rund = json!({"shape": "rounded", "size": 9});
    let para = json!({"shadingOn": true, "shadingColor": "Spalte · Schatten", "shadingWidth": "column", "shadingTop": "leading", "shadingBottom": "descent", "shadingCorners": [rund, rund, rund, rund], "shadingSpeckle": 1.0, "shadingSpeckleColor": "[Paper]"});
    app.run(
        "style.paragraph.create",
        json!({"name": "Spalte Schatten", "chars": {"size": 1, "leading": {"kind": "points", "value": 300}}, "para": para}),
    )
    .unwrap();
    app.run("layout.parents.new", json!({"prefix": "J", "name": "Chat"})).unwrap();
    let k = app.session.active().unwrap().doc.parents.iter().position(|p| p.parent.as_ref().is_some_and(|i| i.label() == "J-Chat")).unwrap();
    let auf = json!({"kind": "parent", "index": k});
    // Papierfläche [100,100,300,400], Schatten 4 pt nach rechts unten, darunter.
    let alt = app.run("frame.create", json!({"spread": auf, "rect": [104, 104, 304, 404], "content": "text", "text": "\u{a0}"})).unwrap();
    app.run("object.textFrameOptions", json!({"inset": 0, "firstBaseline": "leading", "ids": [alt["id"]]})).unwrap();
    app.run("text.select", json!({"story": alt["story"], "anchor": 0, "focus": 2})).unwrap();
    app.run("style.paragraph.apply", json!({"name": "Spalte Schatten"})).unwrap();
    app.run("object.label", json!({"label": "Spalte Schatten", "ids": [alt["id"]]})).unwrap();
    let papier = app.run("frame.create", json!({"spread": auf, "rect": [100, 100, 300, 400], "content": "unassigned"})).unwrap()["id"].clone();
    let ebene = app.session.active().unwrap().doc.item(designcraft_doc::ItemId(papier.as_u64().unwrap())).unwrap().layer;
    app.run("layer.new", json!({"name": "Oben"})).unwrap();
    let textrahmen = app.run("frame.create", json!({"spread": 0, "rect": [50, 50, 150, 150], "content": "text", "text": "x"})).unwrap()["id"].clone();
    app.run("selection.set", json!({"ids": [textrahmen]})).unwrap();
    let vorher = app.session.active().unwrap().selection.clone();
    super::befehl_gestalten(&mut app, &json!({"chat": 1, "stil": {"korn_blasen": 0.5}})).unwrap();
    let st = app.session.active().unwrap();
    let items = &st.doc.parents[k].items;
    let stelle = |id: &Value| items.iter().position(|it| json!(it.id.0) == *id);
    assert_eq!(stelle(&alt["id"]), None, "der große Schatten ist weg");
    let teile: Vec<_> = items.iter().filter(|it| it.label == "Spalte Schatten").collect();
    assert_eq!(teile.len(), 5);
    let p = stelle(&papier).unwrap();
    assert!(
        teile.iter().all(|t| t.layer == ebene && items.iter().position(|it| it.id == t.id).unwrap() < p),
        "auf der Ebene des alten Schattens, unter der Papierfläche"
    );
    // Punkte, an denen der alte Schatten zu sehen war: rechte und untere Kante, die drei runden Ecken.
    for (x, y) in [(302.0, 250.0), (200.0, 402.0), (302.0, 112.0), (301.0, 401.0), (110.0, 402.0)] {
        assert!(teile.iter().any(|t| t.bounds().contains(designcraft_geom::Point::new(x, y))), "({x}, {y}) ist abgedeckt");
    }
    assert_eq!(st.selection, vorher, "die Auswahl ist wie vorher");
}
