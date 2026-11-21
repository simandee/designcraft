//! Jahresbuch › WhatsApp-Chat einfügen …: ein WhatsApp-Export (JSON und Medienordner) kommt als
//! Chat im Riso-Stil in den markierten Textrahmen des offenen Buchs (oder auf neue Chat-Seiten am
//! Ende) und läuft durch verkettete Rahmen weiter.
//!
//! Jahresbuch › Chat-Gestaltung …: Farben, Schrift, Korn, Rauschen und Abstände eines schon
//! eingefügten Chats, mit Vorschau im offenen Dokument; OK übernimmt, Abbrechen nimmt zurück.
//!
//! Den Chat baut das Hilfsprogramm `jahresbuch/designcraft_import.py` (Python) als Befehlsskript;
//! dieses Fenster ruft es auf und führt die Skripte im offenen Dokument aus.

use std::path::PathBuf;

use serde_json::{Map, Value, json};

use crate::DesignApp;
use crate::app::file_dialogs::Bound;
use crate::dialogs::Dialog;
use crate::theme::semibold;

/// Das Fenster zum Einfügen.
pub const ID: &str = "jahresbuch";
/// Das Fenster für die Gestaltung eines eingefügten Chats.
pub const GESTALTUNG_ID: &str = "jahresbuchGestaltung";

/// Der Ordner des Hilfsprogramms im Quellcode (die App wird auf dem eigenen Rechner gebaut).
fn ordner() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("jahresbuch")
}

/// Deine Einstellungen: ~/Jahresbuch/stil.json (nicht im Programmordner, damit Updates sie nicht
/// berühren). Fehlt sie, gelten die Vorgaben aus jahresbuch/stil.json.
fn stil_pfad() -> PathBuf {
    home().join("Jahresbuch").join("stil.json")
}

fn lies_stil() -> Map<String, Value> {
    let lies = |p: PathBuf| std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str::<Map<String, Value>>(&t).ok());
    lies(stil_pfad()).or_else(|| lies(ordner().join("stil.json"))).unwrap_or_default()
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// Python: die Umgebung vom WhatsApp-Export, wenn es sie gibt, sonst python3.
fn standard_python() -> String {
    let venv = home().join("WhatsApp-Export").join(".venv").join("bin").join("python3");
    if venv.is_file() { venv.to_string_lossy().into_owned() } else { "python3".into() }
}

/// Felder: (Schlüssel im Fenster, Schlüssel in stil.json, Standard).
const REGLER: [(&str, &str, &str, f64); 6] = [
    ("kornBlasen", "korn_blasen", "Korn in den Blasen", 0.8),
    ("kornDatum", "korn_datum", "Korn auf dem Datum", 0.4),
    ("rauschenInnen", "rauschen.innen", "Rauschen Innenfläche", 0.35),
    ("rauschenRahmen", "rauschen.rahmen", "Rauschen Rahmen (Blau)", 0.6),
    ("sprenkelRahmen", "sprenkel.rahmen", "Sprenkel Rahmen (Blau)", 1.0),
    ("rauschenBlasen", "rauschen.blasen", "Rauschen Chatblasen", 0.5),
];
/// Haken: Rauschen auch auf den Chatblasen und ihren Schatten (nicht auf Fotos).
const RAUSCHEN_BLASEN_AN: (&str, &str) = ("rauschenBlasenAn", "rauschen_blasen_an");

/// Abstände (pt): (Schlüssel im Fenster, Schlüssel in stil.json, Beschriftung, Standard).
const ABSTAENDE: [(&str, &str, &str, f64); 4] = [
    ("abstandVorDatum", "abstand.vor_datum", "Letzte Nachricht → Datum", 20.0),
    ("abstandDatum", "abstand.datum", "Datum → erste Nachricht", 9.5),
    ("abstandGleich", "abstand.gleiche_person", "Nachrichten derselben Person", 12.0),
    ("abstandWechsel", "abstand.wechsel", "Wechsel zur anderen Person", 14.5),
];
/// Welche Gestaltung die Vorschau gerade zeigt.
const VORSCHAU_FELDER: &str = "_vorschauFelder";

const FARBEN: [(&str, &str, &str, &str); 4] = [
    ("farbeIch", "farbe_ich", "Farbe Ich", "#E8493A"),
    ("farbeSie", "farbe_sie", "Farbe Sie", "#3554E0"),
    ("farbeText", "farbe_text", "Textfarbe", "#1E2A5A"),
    ("farbePapier", "farbe_papier", "Papier", "#F6F0E3"),
];

const SCHRIFTEN: [&str; 2] = ["Space Grotesk", "Work Sans"];

/// Ein Wert aus stil.json, auch verschachtelt (`rauschen.innen`).
fn stil_wert<'a>(stil: &'a Map<String, Value>, pfad: &str) -> Option<&'a Value> {
    let mut teile = pfad.split('.');
    let mut v = stil.get(teile.next()?)?;
    for t in teile {
        v = v.get(t)?;
    }
    Some(v)
}

fn setze_stil(stil: &mut Map<String, Value>, pfad: &str, wert: Value) {
    match pfad.split_once('.') {
        Some((a, b)) => {
            let e = stil.entry(a.to_string()).or_insert_with(|| json!({}));
            if !e.is_object() {
                *e = json!({});
            }
            if let Some(o) = e.as_object_mut() {
                o.insert(b.to_string(), wert);
            }
        }
        None => {
            stil.insert(pfad.to_string(), wert);
        }
    }
}

/// Der markierte Textrahmen im offenen Dokument: (Rahmen, Text).
fn markierter_rahmen(inspect: &Value) -> Option<(u64, u64)> {
    let ausgewaehlt: Vec<u64> = inspect["selection"]["items"].as_array()?.iter().filter_map(Value::as_u64).collect();
    for sp in inspect["spreads"].as_array()? {
        for it in sp["items"].as_array().into_iter().flatten() {
            if let (Some(id), Some(story)) = (it["id"].as_u64(), it["story"].as_u64())
                && ausgewaehlt.contains(&id)
            {
                return Some((id, story));
            }
        }
    }
    None
}

/// Das Fenster zum Einfügen öffnen, mit den zuletzt benutzten Einstellungen (stil.json).
pub fn open(app: &mut DesignApp) -> Result<(), String> {
    oeffnen(app, ID)
}

/// Das Gestaltungsfenster öffnen; nur in einem Dokument, das schon einen Chat enthält.
pub fn open_gestaltung(app: &mut DesignApp) -> Result<(), String> {
    let hat_chat = app.session.execute("document.inspect", &json!({})).ok().is_some_and(|i| {
        i["swatches"].as_array().is_some_and(|a| a.iter().any(|v| v == "Farbe Ich"))
    });
    if app.session.active().is_some() && !hat_chat {
        return Err("Im Dokument ist noch kein Chat – erst Jahresbuch ▸ WhatsApp-Chat einfügen …".into());
    }
    oeffnen(app, GESTALTUNG_ID)
}

fn oeffnen(app: &mut DesignApp, id: &str) -> Result<(), String> {
    if app.session.active().is_none() {
        return Err("Zuerst das Buch öffnen (oder ein neues Dokument anlegen).".into());
    }
    let stil = lies_stil();
    let inspect = app.session.execute("document.inspect", &json!({})).map_err(|e| e.to_string())?;
    let mut f = Map::new();
    let s = |k: &str, standard: &str| stil.get(k).and_then(Value::as_str).unwrap_or(standard).to_string();
    let export = s("export", &home().join("WhatsApp-Export/ausgabe/euer_chat_mit_reaktionen.json").to_string_lossy());
    f.insert("export".into(), json!(export));
    f.insert("medien".into(), json!(s("medien", &home().join("WhatsApp-Export/ausgabe").to_string_lossy())));
    f.insert("von".into(), json!(s("von", "2021-01-01")));
    f.insert("bis".into(), json!(s("bis", "2021-12-31")));
    f.insert("python".into(), json!(s("python", &standard_python())));
    f.insert("schrift".into(), json!(s("schrift_chat", SCHRIFTEN[0])));
    for (k, sk, _, standard) in FARBEN {
        f.insert(k.into(), json!(s(sk, standard)));
    }
    for (k, sk, _, standard) in REGLER.iter().chain(ABSTAENDE.iter()) {
        f.insert((*k).into(), json!(stil_wert(&stil, sk).and_then(Value::as_f64).unwrap_or(*standard)));
    }
    f.insert("vorschau".into(), json!(false));
    f.insert(RAUSCHEN_BLASEN_AN.0.into(), json!(stil.get(RAUSCHEN_BLASEN_AN.1).and_then(Value::as_bool).unwrap_or(false)));
    f.insert("ziel".into(), json!(markierter_rahmen(&inspect).is_some()));
    app.ui.dialog = Some(Dialog::new(id, Value::Object(f)));
    Ok(())
}

fn pfad_feld(app: &mut DesignApp, ui: &mut egui::Ui, d: &mut Dialog, key: &str, ordner: bool) {
    ui.horizontal(|ui| {
        let mut s = d.s(key);
        if ui.add(egui::TextEdit::singleline(&mut s).desired_width(380.0)).changed() {
            d.fields.insert(key.into(), json!(s));
        }
        if ui.button("Wählen …").clicked() {
            let k = key.to_string();
            let setze = move |app: &mut DesignApp, pfad: String| {
                if let Some(d) = app.ui.dialog.as_mut().filter(|d| d.id == ID) {
                    d.fields.insert(k.clone(), json!(pfad));
                }
                Ok(Value::Null)
            };
            let r = if ordner {
                app.ask_folder("Medienordner wählen", Bound::Document, setze)
            } else {
                app.ask_open("chatExport", Bound::Document, setze)
            };
            if let Err(e) = r {
                app.status(e);
            }
        }
    });
}

fn kopf(ui: &mut egui::Ui, t: &str) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(t).font(semibold(13.0)));
}

/// Ein Fenster zeichnen: Einfügen oder Gestaltung.
pub fn show(app: &mut DesignApp, ui: &mut egui::Ui, d: &mut Dialog) {
    if d.id == GESTALTUNG_ID {
        show_gestaltung(app, ui, d);
    } else {
        show_einfuegen(app, ui, d);
    }
}

/// Einfügen: Export, Medien, Zeitraum. Die Gestaltung kommt aus den gespeicherten Einstellungen.
fn show_einfuegen(app: &mut DesignApp, ui: &mut egui::Ui, d: &mut Dialog) {
    kopf(ui, "Chat");
    ui.label("WhatsApp-Export (JSON):");
    pfad_feld(app, ui, d, "export", false);
    ui.label("Medienordner:");
    pfad_feld(app, ui, d, "medien", true);
    ui.horizontal(|ui| {
        for (k, l) in [("von", "Von:"), ("bis", "Bis:")] {
            ui.label(l);
            let mut s = d.s(k);
            if ui.add(egui::TextEdit::singleline(&mut s).desired_width(96.0).hint_text("JJJJ-MM-TT")).changed() {
                d.fields.insert(k.into(), json!(s));
            }
            ui.add_space(8.0);
        }
    });
    ui.label(
        egui::RichText::new(if d.b("ziel") {
            "Ziel: der markierte Textrahmen. Der Chat kommt dort hinein und läuft über verkettete Rahmen weiter; bei Bedarf kommen Chat-Seiten am Ende dazu."
        } else {
            "Ziel: neue Chat-Seiten am Ende des Dokuments. (Tipp: Erst einen Textrahmen markieren, dann kommt der Chat dort hinein.)"
        })
        .small(),
    );
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Farben, Schrift, Korn, Rauschen und Abstände: Jahresbuch ▸ Chat-Gestaltung …").small());
    ui.collapsing("Erweitert", |ui| {
        ui.label("Python:");
        let mut s = d.s("python");
        if ui.add(egui::TextEdit::singleline(&mut s).desired_width(380.0)).changed() {
            d.fields.insert("python".into(), json!(s));
        }
        ui.label(egui::RichText::new(format!("Hilfsprogramm: {}", ordner().join("designcraft_import.py").display())).small());
    });
}

/// Gestaltung des Chats im offenen Dokument, mit Vorschau.
fn show_gestaltung(app: &mut DesignApp, ui: &mut egui::Ui, d: &mut Dialog) {
    kopf(ui, "Farben");
    ui.horizontal_wrapped(|ui| {
        for (k, _, l, standard) in FARBEN {
            let mut rgb = hex_rgb(&d.s(k)).or_else(|| hex_rgb(standard)).unwrap_or([0, 0, 0]);
            ui.label(l);
            if egui::color_picker::color_edit_button_srgb(ui, &mut rgb).changed() {
                d.fields.insert(k.into(), json!(format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])));
            }
            ui.add_space(10.0);
        }
    });
    ui.label(egui::RichText::new("Alle Blasen-, Schatten- und Verzierungsfarben werden daraus abgeleitet (Farbgruppe „Jahresbuch“).").small());

    kopf(ui, "Schrift");
    let aktuell = d.s("schrift");
    craft_ui::ComboBox::from_id_salt("jahresbuch_schrift").selected_text(aktuell.clone()).width(200.0).show_ui(ui, |ui| {
        for s in SCHRIFTEN {
            if ui.selectable_label(aktuell == s, s).clicked() {
                d.fields.insert("schrift".into(), json!(s));
            }
        }
    });

    kopf(ui, "Korn und Rauschen (0 = aus)");
    let mut blasen_an = d.b(RAUSCHEN_BLASEN_AN.0);
    if ui.checkbox(&mut blasen_an, "Rauschen auch auf den Chatblasen und ihren Schatten (nicht auf Fotos)").changed() {
        d.fields.insert(RAUSCHEN_BLASEN_AN.0.into(), json!(blasen_an));
    }
    egui::Grid::new("jahresbuch_regler").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
        for (k, _, l, _) in REGLER {
            // Der Regler fürs Blasen-Rauschen gilt nur mit Haken.
            let aktiv = k != "rauschenBlasen" || blasen_an;
            ui.add_enabled(aktiv, egui::Label::new(l));
            let mut v = d.n(k).unwrap_or(0.0);
            if ui.add_enabled(aktiv, egui::Slider::new(&mut v, 0.0..=1.0).step_by(0.05)).changed() {
                d.fields.insert(k.into(), json!(v));
            }
            ui.end_row();
        }
    });

    kopf(ui, "Abstände (pt)");
    egui::Grid::new("jahresbuch_abstaende").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
        for (k, _, l, standard) in ABSTAENDE {
            ui.label(l);
            let mut v = d.n(k).unwrap_or(standard);
            if ui.add(egui::Slider::new(&mut v, 0.0..=30.0).step_by(0.5)).changed() {
                d.fields.insert(k.into(), json!(v));
            }
            ui.end_row();
        }
    });

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let mut an = d.b("vorschau");
        if ui
            .checkbox(&mut an, "Vorschau")
            .on_hover_text("Zeigt die Änderungen sofort im Chat dieses Dokuments. OK übernimmt sie, Abbrechen nimmt alles zurück.")
            .changed()
        {
            d.fields.insert("vorschau".into(), json!(an));
        }
    });
    vorschau(app, d);
}

/// OK im Gestaltungsfenster: die Vorschau zurücknehmen und die Gestaltung fest anwenden (ein
/// Rückgängig-Schritt), die Einstellungen für spätere Chats merken.
pub fn confirm_gestaltung(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    speichere_stil(d);
    gestaltung_uebernehmen(app, d)?;
    app.status("Jahresbuch: Gestaltung übernommen.");
    Ok(Value::Null)
}

fn gestaltung_uebernehmen(app: &mut DesignApp, d: &Dialog) -> Result<(), String> {
    if d.fields.contains_key(crate::dialogs::PREVIEWED) {
        app.session.cancel_interaction();
    }
    gestaltung_anwenden(app, d)
}

fn hex_rgb(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    Some([p(0)?, p(2)?, p(4)?])
}

/// Die Einstellungen in stil.json merken (das Hilfsprogramm liest sie).
fn speichere_stil(d: &Dialog) {
    let mut stil = lies_stil();
    for k in ["export", "medien", "von", "bis", "python"] {
        stil.insert(k.into(), json!(d.s(k)));
    }
    stil.insert("schrift_chat".into(), json!(d.s("schrift")));
    for (k, sk, _, _) in FARBEN {
        stil.insert(sk.into(), json!(d.s(k)));
    }
    for (k, sk, _, _) in REGLER.iter().chain(ABSTAENDE.iter()) {
        setze_stil(&mut stil, sk, json!(d.n(k).unwrap_or(0.0)));
    }
    stil.insert(RAUSCHEN_BLASEN_AN.1.into(), json!(d.b(RAUSCHEN_BLASEN_AN.0)));
    if let Ok(t) = serde_json::to_string_pretty(&Value::Object(stil)) {
        if let Some(o) = stil_pfad().parent() {
            let _ = std::fs::create_dir_all(o);
        }
        let _ = std::fs::write(stil_pfad(), t);
    }
}

/// Absatzabstände (pt) aus den Reglern: (vor der Blase, nach der Blase, gleiche Person, vor dem
/// Datum). Vor der Blase steht der Abstand Datum → erste Nachricht; vor dem Datum der Rest bis
/// zum Abstand letzte Nachricht → Datum (nie weniger als „nach“); bei einem Personenwechsel
/// kommen „nach“ und „vor“ zusammen; dieselbe Person nutzt den Abstand für gleiche Formate, der
/// beide ersetzt. Gleiche Rechnung wie `abstaende` im Hilfsprogramm.
fn abstaende(d: &Dialog) -> (f64, f64, f64, f64) {
    let wert = |k: &str| d.n(k).or_else(|| ABSTAENDE.iter().find(|a| a.0 == k).map(|a| a.3)).unwrap_or(0.0).max(0.0);
    let (datum, gleich, wechsel) = (wert("abstandDatum"), wert("abstandGleich"), wert("abstandWechsel"));
    let nach = (wechsel - datum).max(0.0);
    (datum, nach, gleich, (wert("abstandVorDatum") - nach).max(0.0))
}

/// Farben, Schrift, Korn, Rauschen und Abstände für ein Dokument, das den Chat schon enthält, als
/// Befehlsskript (was im Dokument fehlt, wird übersprungen).
fn gestaltung_skript(app: &mut DesignApp, d: &Dialog) -> Result<String, String> {
    let inspect = app.session.execute("document.inspect", &json!({})).map_err(|e| e.to_string())?;
    let hat = |liste: &str, name: &str| inspect[liste].as_array().is_some_and(|a| a.iter().any(|v| v == name));
    if !hat("swatches", "Farbe Ich") {
        return Err("Im Dokument ist noch kein Chat – erst importieren.".into());
    }
    let mut befehle: Vec<Value> = Vec::new();
    let mut b = |command: &str, params: Value| befehle.push(json!({"command": command, "params": params}));
    for (k, _, name, _) in FARBEN {
        if hat("swatches", name) {
            b("swatch.options", json!({"name": name, "color": d.s(k)}));
        }
    }
    let schrift = d.s("schrift");
    for name in ["Chat Sie", "Seitenzahl"] {
        if hat("paragraphStyles", name) {
            b("style.paragraph.edit", json!({"name": name, "chars": {"fontFamily": schrift}}));
        }
    }
    for (stil, key) in [("Chat Sie", "kornBlasen"), ("Datum", "kornDatum"), ("Spalte Schatten", "sprenkelRahmen")] {
        if hat("paragraphStyles", stil) {
            b("style.paragraph.edit", json!({"name": stil, "para": {"shadingSpeckle": d.n(key).unwrap_or(0.0)}}));
        }
    }
    // Rauschen auf den Blasen (die übrigen Blasenformate erben es von „Chat Sie“; die
    // Reaktions-Pillen bleiben ohne).
    let rauschen = if d.b(RAUSCHEN_BLASEN_AN.0) { d.n("rauschenBlasen").unwrap_or(0.0).clamp(0.0, 1.0) } else { 0.0 };
    if hat("paragraphStyles", "Chat Sie") {
        b("style.paragraph.edit", json!({"name": "Chat Sie", "para": {"shadingNoise": rauschen}}));
    }
    // Blasen zeigen zum Absender: unten außen spitz (auch in Chats, die vorher importiert wurden).
    for seite in ["Sie", "Ich"] {
        let stil = format!("Chat {seite}");
        if hat("paragraphStyles", &stil) {
            b("style.paragraph.edit", json!({"name": stil, "para": {"shadingCorners": blasen_ecken(seite)}}));
        }
    }
    for seite in ["Sie", "Ich"] {
        for von in ["von Ihr", "von Mir", "von Beiden"] {
            let stil = format!("Reaktion {seite} {von}");
            if hat("paragraphStyles", &stil) {
                b("style.paragraph.edit", json!({"name": stil, "para": {"shadingNoise": 0.0}}));
            }
        }
    }
    // Abstände (die übrigen Blasenformate erben sie von „Chat Sie“).
    let (vor, nach, gleich, datum_vor) = abstaende(d);
    let mut abstand = |stil: String, para: Value| {
        if hat("paragraphStyles", &stil) {
            b("style.paragraph.edit", json!({"name": stil, "para": para}));
        }
    };
    abstand("Chat Sie".into(), json!({"spaceBefore": vor, "spaceAfter": nach, "sameParaStyleSpacing": gleich}));
    abstand("Datum".into(), json!({"spaceBefore": datum_vor, "spaceAfter": 0}));
    for seite in ["Sie", "Ich"] {
        for basis in ["Chat", "Bild", "Emoji", "Sticker"] {
            abstand(format!("{basis} {seite} mit Reaktion"), json!({"spaceAfter": 0}));
        }
        for v in ["1", "2"] {
            abstand(format!("Album {seite} {v}"), json!({"spaceBefore": vor, "spaceAfter": 0}));
            abstand(format!("Album {seite} {v} Ende"), json!({"spaceAfter": nach}));
            abstand(format!("Album {seite} {v} Ende mit Reaktion"), json!({"spaceAfter": 0}));
            // Ältere Chats beginnen eine Foto-Serie mit „… Anfang“.
            abstand(format!("Album {seite} {v} Anfang"), json!({"spaceBefore": vor}));
        }
    }
    // Rauschen: Bildrahmen auf der Mustervorlage, nach ihrem Namen gefunden; Stärke = Deckkraft.
    for (label, key, hell) in [("Rauschen Innenfläche", "rauschenInnen", false), ("Rauschen Rahmen (Blau)", "rauschenRahmen", true)] {
        let gefunden = app.session.execute("object.findByLabel", &json!({"label": label})).map_err(|e| e.to_string())?;
        let ids: Vec<Value> = gefunden["ids"].as_array().cloned().unwrap_or_default();
        if ids.is_empty() {
            continue;
        }
        let v = d.n(key).unwrap_or(0.0).clamp(0.0, 1.0);
        let deckkraft = if hell { v } else { 0.3 * v };
        if v > 0.0 {
            b("object.opacity", json!({"opacity": deckkraft, "ids": ids}));
        }
        b("object.setFlags", json!({"ids": ids, "hidden": v <= 0.0}));
    }
    Ok(befehle.iter().map(Value::to_string).collect::<Vec<_>>().join("\n"))
}

/// Ecken einer Chatblase (oben links, oben rechts, unten rechts, unten links): rund, nur unten
/// außen spitz – ihre links, seine rechts. Wie `blasen_ecken` im Hilfsprogramm.
fn blasen_ecken(seite: &str) -> Value {
    let (rund, spitz) = (json!({"shape": "rounded", "size": 7}), json!({"shape": "none", "size": 0}));
    if seite == "Ich" { json!([rund, rund, spitz, rund]) } else { json!([rund, rund, rund, spitz]) }
}

/// Die Gestaltung fest aufs Dokument anwenden (ein Rückgängig-Schritt).
fn gestaltung_anwenden(app: &mut DesignApp, d: &Dialog) -> Result<(), String> {
    let text = gestaltung_skript(app, d)?;
    skript(app, text).map(|_| ())
}

/// Die Felder, die die Gestaltung bestimmen: ändert sich einer, wird die Vorschau neu gezeigt.
fn gestaltungs_felder(d: &Dialog) -> Value {
    let mut f = Map::new();
    let schluessel = FARBEN.iter().map(|x| x.0).chain(REGLER.iter().map(|x| x.0)).chain(ABSTAENDE.iter().map(|x| x.0)).chain(["schrift", RAUSCHEN_BLASEN_AN.0]);
    for k in schluessel {
        f.insert(k.into(), d.fields.get(k).cloned().unwrap_or(Value::Null));
    }
    Value::Object(f)
}

/// Vorschau an: die Gestaltung zeigt sich sofort im offenen Dokument (bei jeder Änderung neu),
/// ohne Rückgängig-Schritt; aus oder Abbrechen: das Dokument ist wieder wie vorher.
fn vorschau(app: &mut DesignApp, d: &mut Dialog) {
    let gezeigt = d.fields.contains_key(crate::dialogs::PREVIEWED);
    if !d.b("vorschau") {
        if gezeigt {
            vorschau_beenden(app, d);
        }
        return;
    }
    let felder = gestaltungs_felder(d);
    if gezeigt && d.fields.get(VORSCHAU_FELDER) == Some(&felder) {
        return;
    }
    d.fields.insert(VORSCHAU_FELDER.into(), felder);
    let text = match gestaltung_skript(app, d) {
        Ok(t) => t,
        Err(e) => {
            d.fields.insert("vorschau".into(), json!(false));
            if gezeigt {
                vorschau_beenden(app, d);
            }
            app.status(e);
            return;
        }
    };
    if !gezeigt && let Err(e) = app.session.begin_interaction("Jahresbuch-Gestaltung") {
        app.status(e.to_string());
        return;
    }
    d.fields.insert(crate::dialogs::PREVIEWED.into(), json!(true));
    if let Err(e) = app.session.preview("script.run", json!({"text": text})) {
        app.status(format!("Vorschau nicht möglich: {e}"));
    }
}

/// Die Vorschau zurücknehmen: das Dokument ist wieder wie beim Einschalten.
fn vorschau_beenden(app: &mut DesignApp, d: &mut Dialog) {
    if d.fields.remove(crate::dialogs::PREVIEWED).is_some() {
        app.session.cancel_interaction();
    }
    d.fields.remove(VORSCHAU_FELDER);
}

/// Ein Befehlsskript aus einer Datei im offenen Dokument ausführen.
fn skript(app: &mut DesignApp, text: String) -> Result<Value, String> {
    let r = app.run("script.run", json!({"text": text}))?;
    if let Some(f) = r.get("failed").filter(|f| !f.is_null()) {
        return Err(format!("Ein Schritt ist fehlgeschlagen: {f}"));
    }
    Ok(r)
}

/// Seite am Ende mit der Chat-Mustervorlage anhängen: (Druckbogen, Spaltenrechtecke).
fn chat_seite_anhaengen(app: &mut DesignApp) -> Result<(u64, Vec<Value>), String> {
    app.run("layout.pages.insert", json!({"count": 1, "parent": "J-Chat"}))?;
    let inspect = app.session.execute("document.inspect", &json!({})).map_err(|e| e.to_string())?;
    let spreads = inspect["spreads"].as_array().cloned().unwrap_or_default();
    let sp = spreads.last().ok_or("keine Seiten")?;
    let page = sp["pages"].as_array().and_then(|p| p.last()).ok_or("keine Seite")?;
    Ok((sp["index"].as_u64().unwrap_or(0), page["columnRects"].as_array().cloned().unwrap_or_default()))
}

/// OK: das Hilfsprogramm baut die Skripte, dann fließt der Chat ins Dokument.
pub fn confirm(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    // Eine laufende Vorschau zuerst zurücknehmen; die Gestaltung kommt nach dem Import fest dazu.
    if d.fields.contains_key(crate::dialogs::PREVIEWED) {
        app.session.cancel_interaction();
    }
    speichere_stil(d);
    let export = d.s("export");
    if !std::path::Path::new(&export).is_file() {
        return Err(format!("WhatsApp-Export nicht gefunden: {export}"));
    }
    let arbeit = std::env::temp_dir().join(format!("designcraft-jahresbuch-{}", std::process::id()));
    std::fs::create_dir_all(&arbeit).map_err(|e| e.to_string())?;
    let inspect = app.session.execute("document.inspect", &json!({})).map_err(|e| e.to_string())?;
    let insp_pfad = arbeit.join("dokument.json");
    std::fs::write(&insp_pfad, inspect.to_string()).map_err(|e| e.to_string())?;
    let (einr, inh) = (arbeit.join("einrichtung.jsonl"), arbeit.join("inhalt.jsonl"));
    app.status("Jahresbuch: Chat wird aufbereitet …");
    let out = std::process::Command::new(d.s("python"))
        .arg(ordner().join("designcraft_import.py"))
        .arg(&export)
        .arg(d.s("medien"))
        .arg(&inh)
        .args(["--dokument", &insp_pfad.to_string_lossy(), "--einrichtung-ausgabe", &einr.to_string_lossy()])
        .args(["--von", &d.s("von"), "--bis", &d.s("bis")])
        .args(["--stil", &stil_pfad().to_string_lossy(), "--arbeitsordner", &home().join("Jahresbuch").join("chat_dateien").to_string_lossy()])
        .output()
        .map_err(|e| format!("Python konnte nicht gestartet werden ({}): {e}", d.s("python")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let letzte: Vec<&str> = err.lines().rev().take(4).collect();
        return Err(format!("Das Hilfsprogramm meldet einen Fehler: {}", letzte.into_iter().rev().collect::<Vec<_>>().join(" | ")));
    }
    // 1. Farbfelder, Formate, Chat-Mustervorlage
    skript(app, std::fs::read_to_string(&einr).map_err(|e| e.to_string())?)?;
    // 2. Ziel: markierter Rahmen, sonst eine neue Chat-Seite am Ende
    let (mut letzter, story) = match markierter_rahmen(&inspect) {
        Some((rahmen, story)) => {
            // Nur in einen leeren Rahmen: vorhandener Text (z. B. eine Geschichte) bleibt unangetastet.
            let laenge = inspect["stories"].as_array().and_then(|a| a.iter().find(|s| s["id"].as_u64() == Some(story))).and_then(|s| s["length"].as_u64());
            if laenge.unwrap_or(0) > 0 {
                return Err("Der markierte Textrahmen enthält schon Text. Bitte einen leeren Rahmen für den Chat markieren.".into());
            }
            (rahmen, story)
        }
        None => {
            // Eine neue Chat-Seite: ein Rahmen je Spalte, verkettet.
            let (spread, spalten) = chat_seite_anhaengen(app)?;
            let rect = spalten.first().cloned().ok_or("Chat-Seite ohne Spalten")?;
            let f = app.run("frame.create", json!({"spread": spread, "rect": rect, "content": "text"}))?;
            let erster = f["id"].as_u64().ok_or("kein Rahmen")?;
            let mut von = erster;
            for rect in spalten.iter().skip(1) {
                let r = app.run("text.thread", json!({"from": von, "rect": rect, "spread": spread}))?;
                von = r["to"].as_u64().unwrap_or(von);
            }
            // Beim Verketten leerer Rahmen bekommt der Text eine neue Nummer.
            let story = app.run("story.get", json!({"story": f["story"]})).ok().and_then(|v| v["id"].as_u64());
            let story = match story {
                Some(st) => st,
                None => {
                    let inspect = app.session.execute("document.inspect", &json!({})).map_err(|e| e.to_string())?;
                    inspect["spreads"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .flat_map(|sp| sp["items"].as_array().into_iter().flatten())
                        .find(|it| it["id"].as_u64() == Some(erster))
                        .and_then(|it| it["story"].as_u64())
                        .ok_or("kein Text")?
                }
            };
            (erster, story)
        }
    };
    // Der Text kann schon weitere verkettete Rahmen haben: weiter geht es nach dem letzten.
    if let Ok(info) = app.run("story.get", json!({"story": story}))
        && let Some(l) = info["frames"].as_array().and_then(|f| f.last()).and_then(Value::as_u64)
    {
        letzter = l;
    }
    // 3. Der Chat
    let text = std::fs::read_to_string(&inh).map_err(|e| e.to_string())?.replace("\"__STORY__\"", &story.to_string());
    skript(app, text)?;
    // 4. Passt er nicht, kommen Chat-Seiten am Ende dazu, zwei Spalten je Seite.
    let uebersatz = app.run("story.get", json!({"story": story})).map(|v| !v["overset"].is_null()).unwrap_or(false);
    if uebersatz {
        let (spread, spalten) = chat_seite_anhaengen(app)?;
        let mut von = letzter;
        for (i, rect) in spalten.iter().enumerate() {
            let letzte_spalte = i + 1 == spalten.len();
            let r = app.run("text.thread", json!({"from": von, "rect": rect, "spread": spread, "autoflow": letzte_spalte}))?;
            von = r["to"].as_u64().unwrap_or(von);
        }
    }
    let _ = std::fs::remove_dir_all(&arbeit);
    app.status("Jahresbuch: Chat eingefügt.");
    gestaltung_anwenden(app, d)?;
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Zwei verkettete Rahmen, der linke markiert: der Chat läuft vom linken in den rechten Rahmen
    /// und erst danach auf neue Seiten.
    #[test]
    fn chat_fuellt_erst_die_verketteten_rahmen() {
        let (Ok(export), Ok(medien)) = (std::env::var("JAHRESBUCH_TEST_EXPORT"), std::env::var("JAHRESBUCH_TEST_MEDIEN")) else { return };
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({"preset": "A4", "pages": 1, "facingPages": true})).unwrap();
        let links = app.run("frame.create", json!({"spread": 0, "rect": [57.2, 60, 281.5, 764.8], "content": "text"})).unwrap();
        let rechts = app.run("text.thread", json!({"from": links["id"], "rect": [313.7, 60, 538, 764.8], "spread": 0})).unwrap()["to"].clone();
        app.run("selection.set", json!({"ids": [links["id"]]})).unwrap();
        open(&mut app).unwrap();
        let mut d = app.ui.dialog.take().unwrap();
        for (k, v) in [("export", export.as_str()), ("medien", medien.as_str()), ("von", "2026-11-01"), ("bis", "2026-11-30"), ("python", "python3")] {
            d.fields.insert(k.into(), json!(v));
        }
        confirm(&mut app, &d).unwrap();
        let doc = app.session.execute("document.inspect", &json!({})).unwrap();
        let story = doc["spreads"][0]["items"].as_array().unwrap().iter().find(|i| i["id"] == links["id"]).unwrap()["story"].clone();
        let rahmen: Vec<Value> = app.run("story.get", json!({"story": story})).unwrap()["frames"].as_array().unwrap().clone();
        assert_eq!(rahmen.get(1), Some(&rechts), "nach dem linken Rahmen kommt der rechte, nicht die neuen Seiten: {rahmen:?}");
    }

    /// Ohne markierten Rahmen: die neue Chat-Seite hat in beiden Spalten verkettete Rahmen, der
    /// Chat füllt erst beide und läuft dann auf weitere Seiten.
    #[test]
    fn neue_chat_seite_hat_beide_spalten() {
        let (Ok(export), Ok(medien)) = (std::env::var("JAHRESBUCH_TEST_EXPORT"), std::env::var("JAHRESBUCH_TEST_MEDIEN")) else { return };
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({"preset": "A4", "pages": 1, "facingPages": true})).unwrap();
        open(&mut app).unwrap();
        let mut d = app.ui.dialog.take().unwrap();
        for (k, v) in [("export", export.as_str()), ("medien", medien.as_str()), ("von", "2026-11-01"), ("bis", "2026-11-30"), ("python", "python3")] {
            d.fields.insert(k.into(), json!(v));
        }
        confirm(&mut app, &d).unwrap();
        let doc = app.session.execute("document.inspect", &json!({})).unwrap();
        // Die ersten beiden Rahmen des Chats: nebeneinander auf derselben Seite (zwei Spalten),
        // nicht der zweite schon auf der nächsten Seite.
        let rahmen: Vec<&Value> = doc["spreads"].as_array().unwrap().iter().flat_map(|sp| sp["items"].as_array().unwrap()).collect();
        let story = rahmen.iter().find(|i| i["story"].is_u64()).unwrap()["story"].clone();
        let kette: Vec<Value> = app.run("story.get", json!({"story": story})).unwrap()["frames"].as_array().unwrap().clone();
        assert!(kette.len() > 2, "der Chat läuft über mehrere Seiten");
        let x0 = |id: &Value| rahmen.iter().find(|i| i["id"] == *id).unwrap()["bounds"][0].as_f64().unwrap();
        let seite_b = doc["spreads"][1]["pages"][0]["bounds"].as_array().map(|b| b[2].as_f64().unwrap() - b[0].as_f64().unwrap()).unwrap_or(595.0);
        let abstand = x0(&kette[1]) - x0(&kette[0]);
        assert!(abstand > 0.0 && abstand < seite_b / 2.0, "zweite Spalte derselben Seite: {abstand} pt weiter rechts");
    }

    /// OK im Gestaltungsfenster übernimmt die Einstellungen, ohne den Chat noch einmal einzufügen.
    #[test]
    fn gestaltung_ok_aendert_nur_die_gestaltung() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({"preset": "A4", "pages": 1})).unwrap();
        assert!(open_gestaltung(&mut app).is_err(), "ohne Chat gibt es nichts zu gestalten");
        app.run("swatch.create", json!({"name": "Farbe Ich", "color": "#E8493A"})).unwrap();
        app.run("style.paragraph.create", json!({"name": "Chat Sie", "para": {"sameParaStyleSpacing": 12}})).unwrap();
        let seiten = app.session.execute("document.inspect", &json!({})).unwrap()["pageCount"].clone();
        open_gestaltung(&mut app).unwrap();
        let mut d = app.ui.dialog.take().unwrap();
        d.fields.insert("abstandGleich".into(), json!(20.0));
        d.fields.insert("vorschau".into(), json!(true));
        vorschau(&mut app, &mut d);
        // Wie OK, nur ohne stil.json im Heimordner zu schreiben.
        gestaltung_uebernehmen(&mut app, &d).unwrap();
        assert!(gleiche_person(&app).unwrap() > 12.0, "die Änderung ist übernommen");
        assert!(app.session.active().unwrap().interaction.is_none(), "keine Vorschau mehr offen");
        let doc = app.session.execute("document.inspect", &json!({})).unwrap();
        assert_eq!(doc["pageCount"], seiten, "kein Chat eingefügt, keine Seiten dazu");
    }

    fn gleiche_person(app: &DesignApp) -> Option<f64> {
        let doc = &app.session.active().unwrap().doc;
        doc.styles.paragraph.iter().find(|s| s.name == "Chat Sie").and_then(|s| s.para.same_para_style_spacing.flatten())
    }

    /// Die Vorschau zeigt eine geänderte Einstellung sofort im Dokument; ausgeschaltet ist das
    /// Dokument wieder genau wie vorher.
    #[test]
    fn vorschau_zeigt_und_nimmt_zurueck() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({"preset": "A4", "pages": 1})).unwrap();
        app.run("swatch.create", json!({"name": "Farbe Ich", "color": "#E8493A"})).unwrap();
        app.run("style.paragraph.create", json!({"name": "Chat Sie", "para": {"sameParaStyleSpacing": 12}})).unwrap();
        let vorher = app.session.active().unwrap().doc.clone();
        open_gestaltung(&mut app).unwrap();
        let mut d = app.ui.dialog.take().unwrap();
        d.fields.insert("abstandGleich".into(), json!(20.0));
        vorschau(&mut app, &mut d);
        assert_eq!(gleiche_person(&app), Some(12.0), "ohne Haken bleibt das Dokument unverändert");
        d.fields.insert("vorschau".into(), json!(true));
        vorschau(&mut app, &mut d);
        let gezeigt = gleiche_person(&app).unwrap();
        assert!(gezeigt > 12.0, "die Vorschau zeigt den größeren Abstand");
        d.fields.insert("abstandGleich".into(), json!(6.0));
        vorschau(&mut app, &mut d);
        assert!(gleiche_person(&app).unwrap() < 12.0, "eine weitere Änderung ersetzt die Vorschau");
        d.fields.insert("vorschau".into(), json!(false));
        vorschau(&mut app, &mut d);
        assert!(std::sync::Arc::ptr_eq(&app.session.active().unwrap().doc, &vorher), "ausgeschaltet: wieder wie vorher");
    }

    /// Ein echter Durchlauf, nur mit einem Export auf diesem Rechner:
    /// JAHRESBUCH_TEST_EXPORT=… JAHRESBUCH_TEST_MEDIEN=… cargo test -p designcraft-ui-egui jahresbuch
    #[test]
    fn chat_fliesst_in_den_markierten_rahmen() {
        let (Ok(export), Ok(medien)) = (std::env::var("JAHRESBUCH_TEST_EXPORT"), std::env::var("JAHRESBUCH_TEST_MEDIEN")) else { return };
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({"preset": "A4", "pages": 3, "facingPages": true})).unwrap();
        app.run("frame.create", json!({"spread": 0, "rect": [60, 80, 500, 300], "content": "text", "text": "Unsere Geschichte"})).unwrap();
        let r = app.run("frame.create", json!({"spread": 1, "rect": [57.2, 60, 281.5, 764.8], "content": "text"})).unwrap();
        app.run("selection.set", json!({"ids": [r["id"]]})).unwrap();
        open(&mut app).unwrap();
        let mut d = app.ui.dialog.take().unwrap();
        for (k, v) in [("export", export.as_str()), ("medien", medien.as_str()), ("von", "2026-11-01"), ("bis", "2026-11-30"), ("python", "python3")] {
            d.fields.insert(k.into(), json!(v));
        }
        confirm(&mut app, &d).unwrap();
        let story = app.run("story.get", json!({"story": r["story"]})).unwrap();
        assert!(story["text"].as_str().unwrap().contains("November"), "the chat is in the selected frame");
        assert!(story["overset"].is_null(), "pages were added until it fits");
        let doc = app.session.execute("document.inspect", &json!({})).unwrap();
        assert!(doc["parents"].as_array().unwrap().iter().any(|p| p["label"] == "J-Chat"));
        assert!(doc["pageCount"].as_u64().unwrap() > 3);
        // The story text frame with "Unsere Geschichte" keeps its text.
        assert!(doc["stories"].as_array().unwrap().iter().any(|s| s["preview"].as_str().is_some_and(|p| p.contains("Unsere Geschichte"))));
    }
}
