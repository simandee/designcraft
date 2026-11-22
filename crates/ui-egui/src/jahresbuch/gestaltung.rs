//! Die Gestaltung eines Chats aufs Dokument: aus seinen Einstellungen ([`Chat::stil`]) die Befehle,
//! die Farbfelder, Formate, Objekte im Text und die Flächen auf der Chat-Mustervorlage anpassen.
//! Was im Dokument fehlt, wird übersprungen.

use designcraft_doc::Document;
use serde_json::{Value, json};

use super::modell::{Chat, chat_von_text};

/// Abstände (pt), Vorgaben wie `ABSTAND` im Hilfsprogramm: (stil.json-Pfad, Standard).
pub const ABSTAND_VOR_DATUM: (&str, f64) = ("abstand.vor_datum", 20.0);
pub const ABSTAND_DATUM: (&str, f64) = ("abstand.datum", 9.5);
pub const ABSTAND_GLEICH: (&str, f64) = ("abstand.gleiche_person", 12.0);
pub const ABSTAND_WECHSEL: (&str, f64) = ("abstand.wechsel", 14.5);
/// Schriftgrößen (pt), Vorgaben wie `SCHRIFT` im Hilfsprogramm.
pub const SCHRIFT_TEXT: (&str, f64) = ("schrift.text", 8.4);
pub const SCHRIFT_ZEIT: (&str, f64) = ("schrift.zeit", 6.0);
pub const SCHRIFT_DATUM: (&str, f64) = ("schrift.datum", 10.0);
pub const SCHRIFT_ZITAT: (&str, f64) = ("schrift.zitat", 7.2);
/// Zitat-Name und Hinweise im Verhältnis zum Zitat-Text (6,8 und 7,4 zu 7,2 pt).
const ZITAT_NAME: f64 = 6.8 / 7.2;
const HINWEIS: f64 = 7.4 / 7.2;
/// Korn und Rauschen (0 … 1): (Pfad, Standard).
pub const KORN_BLASEN: (&str, f64) = ("korn_blasen", 0.8);
pub const KORN_DATUM: (&str, f64) = ("korn_datum", 0.4);
pub const RAUSCHEN_BLASEN: (&str, f64) = ("rauschen.blasen", 0.5);
pub const RAUSCHEN_BLASEN_AN: &str = "rauschen_blasen_an";
pub const RAUSCHEN_INNEN: (&str, f64) = ("rauschen.innen", 0.35);
pub const RAUSCHEN_RAHMEN: (&str, f64) = ("rauschen.rahmen", 0.6);
pub const SPRENKEL_RAHMEN: (&str, f64) = ("sprenkel.rahmen", 1.0);
/// Farben: (Pfad, Farbfeld, Standard).
pub const TEXTFARBE: (&str, &str, &str) = ("farbe_text", "Textfarbe", "#1E2A5A");
pub const PAPIER: (&str, &str, &str) = ("farbe_papier", "Papier", "#F6F0E3");
/// In Gruppenchats färbt „Farbe Sie“ keine Person, sondern Datum, Verzierungen und den Schatten
/// der Spalten-Box: dort ist sie eine eigene Farbe.
pub const AKZENT: (&str, &str, &str) = ("farbe_sie", "Farbe Sie", "#3554E0");

/// Die Farben eines Chats außer denen der Personen.
pub fn chat_farben(chat: &Chat) -> &'static [(&'static str, &'static str, &'static str)] {
    if chat.gruppe() { &[TEXTFARBE, PAPIER, AKZENT] } else { &[TEXTFARBE, PAPIER] }
}
pub const SCHRIFTEN: [&str; 2] = ["Space Grotesk", "Work Sans"];

/// Absatzabstände (pt): (vor der Blase, nach der Blase, gleiche Person, vor dem Datum). Vor der
/// Blase steht der Abstand Datum → erste Nachricht; vor dem Datum der Rest bis zum Abstand letzte
/// Nachricht → Datum (nie weniger als „nach“); bei einem Personenwechsel kommen „nach“ und „vor“
/// zusammen; dieselbe Person nutzt den Abstand für gleiche Formate, der beide ersetzt. Gleiche
/// Rechnung wie `abstaende` im Hilfsprogramm.
pub fn abstaende(chat: &Chat) -> (f64, f64, f64, f64) {
    let w = |(p, s): (&str, f64)| chat.zahl(p, s).max(0.0);
    let (datum, gleich, wechsel) = (w(ABSTAND_DATUM), w(ABSTAND_GLEICH), w(ABSTAND_WECHSEL));
    let nach = (wechsel - datum).max(0.0);
    (datum, nach, gleich, (w(ABSTAND_VOR_DATUM) - nach).max(0.0))
}

/// Ecken einer Chatblase (oben links, oben rechts, unten rechts, unten links): rund, nur unten
/// außen spitz – andere links, ich rechts. Wie `blasen_ecken` im Hilfsprogramm.
fn blasen_ecken(ich: bool) -> Value {
    let (rund, spitz) = (json!({"shape": "rounded", "size": 7}), json!({"shape": "none", "size": 0}));
    if ich { json!([rund, rund, spitz, rund]) } else { json!([rund, rund, rund, spitz]) }
}

/// Die Befehle, die die Gestaltung von `chat` aufs Dokument `doc` bringen (bei einer laufenden
/// Vorschau: das Dokument davor, auf dem sie ihr Skript abspielt; Objekte im Text werden von dort
/// aus skaliert).
pub fn befehle(doc: &Document, chat: &Chat) -> Vec<Value> {
    let mut b = Befehle { doc, chat, out: Vec::new() };
    b.farben_und_schrift();
    b.abstaende();
    b.schriftgroessen();
    b.rauschen_der_spalten_box();
    b.out
}

/// Nur die Befehle, in denen sich `neu` von `alt` (dem Chat, wie er im Etikett von `doc` steht)
/// unterscheidet: eine Änderung im Fenster lässt so eigene Anpassungen an den übrigen Formaten
/// und Farbfeldern stehen. Ohne `alt` alle.
pub fn geaenderte_befehle(doc: &Document, alt: Option<&Chat>, neu: &Chat) -> Vec<Value> {
    let neue = befehle(doc, neu);
    let Some(alt) = alt else { return neue };
    let alte = befehle(doc, alt);
    neue.into_iter().filter(|b| !alte.contains(b)).collect()
}

/// Die Befehle, während sie entstehen, mit dem Dokument, in dem sie laufen.
struct Befehle<'a> {
    doc: &'a Document,
    chat: &'a Chat,
    out: Vec<Value>,
}

impl Befehle<'_> {
    fn b(&mut self, command: &str, params: Value) {
        self.out.push(json!({"command": command, "params": params}));
    }

    /// Das Absatzformat `name` dieses Chats, wenn es im Dokument ist.
    fn pstil(&self, name: &str) -> Option<String> {
        let n = self.chat.stil_name(name);
        self.doc.styles.paragraph.iter().any(|s| s.name == n).then_some(n)
    }

    fn cstil(&self, name: &str) -> Option<String> {
        let n = self.chat.stil_name(name);
        self.doc.styles.character.iter().any(|s| s.name == n).then_some(n)
    }

    fn farbfeld(&self, name: &str) -> Option<String> {
        let n = self.chat.farb_name(name);
        self.doc.swatches.iter().any(|s| s.name == n).then_some(n)
    }

    /// Ein Absatzformat ändern, wenn es da ist.
    fn absatz(&mut self, stil: &str, mut aenderung: Value) {
        if let Some(n) = self.pstil(stil) {
            aenderung["name"] = json!(n);
            self.b("style.paragraph.edit", aenderung);
        }
    }

    /// Farben der Personen, Textfarbe und Papier; Schrift, Korn, Rauschen und Ecken der Blasen.
    fn farben_und_schrift(&mut self) {
        let chat = self.chat;
        for p in &chat.personen {
            if let Some(n) = self.farbfeld(&Chat::person_farbfeld(&p.key)) {
                self.b("swatch.options", json!({"name": n, "color": p.farbe}));
            }
        }
        for &(pfad, name, standard) in chat_farben(chat) {
            if let Some(n) = self.farbfeld(name) {
                self.b("swatch.options", json!({"name": n, "color": chat.text(pfad, standard)}));
            }
        }
        let schrift = chat.text("schrift_chat", SCHRIFTEN[0]);
        self.absatz("Chat Sie", json!({"chars": {"fontFamily": schrift}}));
        for (stil, (pfad, standard)) in [("Chat Sie", KORN_BLASEN), ("Datum", KORN_DATUM), ("Spalte Schatten", SPRENKEL_RAHMEN)] {
            self.absatz(stil, json!({"para": {"shadingSpeckle": chat.zahl(pfad, standard).clamp(0.0, 1.0)}}));
        }
        // Rauschen auf den Blasen (die übrigen Blasenformate erben es von „Chat Sie“)
        let an = chat.wert(RAUSCHEN_BLASEN_AN).and_then(Value::as_bool).unwrap_or(false);
        let rauschen = if an { chat.zahl(RAUSCHEN_BLASEN.0, RAUSCHEN_BLASEN.1).clamp(0.0, 1.0) } else { 0.0 };
        self.absatz("Chat Sie", json!({"para": {"shadingNoise": rauschen}}));
        // Blasen zeigen zum Absender (auch in Chats, die vorher eingefügt wurden)
        for (stil, ich) in [("Chat Sie", false), ("Chat Ich", true)] {
            self.absatz(stil, json!({"para": {"shadingCorners": blasen_ecken(ich)}}));
        }
    }

    /// Abstände (die meisten Blasenformate erben sie von „Chat Sie“); die übrigen Formate werden
    /// an ihren Namen erkannt.
    fn abstaende(&mut self) {
        let (vor, nach, gleich, datum_vor) = abstaende(self.chat);
        self.absatz("Chat Sie", json!({"para": {"spaceBefore": vor, "spaceAfter": nach, "sameParaStyleSpacing": gleich}}));
        self.absatz("Datum", json!({"para": {"spaceBefore": datum_vor, "spaceAfter": 0}}));
        let praefix = self.chat.stil_name("");
        let mut aenderungen = Vec::new();
        for s in &self.doc.styles.paragraph {
            let Some(name) = s.name.strip_prefix(praefix.as_str()) else { continue };
            if self.chat.nummer <= 1 && s.name.starts_with("Chat ") && s.name.contains('/') {
                continue; // Formate eines anderen Chats
            }
            let album = name.starts_with("Album ");
            let para = if name.starts_with("Reaktion ") {
                json!({"shadingNoise": 0.0})
            } else if name.ends_with(" mit Reaktion") && (!album || name.ends_with(" Ende mit Reaktion")) {
                json!({"spaceAfter": 0})
            } else if album && name.ends_with(" Ende") {
                json!({"spaceAfter": nach})
            } else if album && name.ends_with(" Anfang") {
                // Ältere Chats beginnen eine Foto-Serie mit „… Anfang“.
                json!({"spaceBefore": vor})
            } else if album && name.chars().last().is_some_and(|c| c.is_ascii_digit()) {
                json!({"spaceBefore": vor, "spaceAfter": 0})
            } else {
                continue;
            };
            aenderungen.push(json!({"name": s.name, "para": para}));
        }
        for a in aenderungen {
            self.b("style.paragraph.edit", a);
        }
    }

    /// Schriftgrößen; Emojis wachsen mit dem Nachrichtentext, die Verzierungen mit dem Datum.
    fn schriftgroessen(&mut self) {
        let chat = self.chat;
        let g = |(p, s): (&str, f64)| chat.zahl(p, s).clamp(4.0, 24.0);
        let (text, zeit, datum, zitat) = (g(SCHRIFT_TEXT), g(SCHRIFT_ZEIT), g(SCHRIFT_DATUM), g(SCHRIFT_ZITAT));
        for (stil, size) in [("Chat Sie", text), ("Datum", datum)] {
            self.absatz(stil, json!({"chars": {"size": size}}));
        }
        let rund = |v: f64| (v * 100.0).round() / 100.0;
        let mut zeichen = vec![("Zeit".to_string(), zeit), ("Zitat Text".into(), zitat), ("Hinweis".into(), zitat * HINWEIS)];
        zeichen.extend(chat.personen.iter().map(|p| (format!("Zitat Name {}", p.key), zitat * ZITAT_NAME)));
        for (stil, size) in zeichen {
            if let Some(n) = self.cstil(&stil) {
                self.b("style.character.edit", json!({"name": n, "chars": {"size": rund(size)}}));
            }
        }
        for (command, params) in mitwachsende_objekte(self.doc, chat, text, datum) {
            self.b(command, params);
        }
    }

    /// Rauschen der Spalten-Box: Bildrahmen auf der Mustervorlage, nach ihrem Namen gefunden;
    /// Stärke = Deckkraft.
    fn rauschen_der_spalten_box(&mut self) {
        for (label, (pfad, standard), hell) in [("Rauschen Innenfläche", RAUSCHEN_INNEN, false), ("Rauschen Rahmen (Blau)", RAUSCHEN_RAHMEN, true)] {
            let ids = mit_namen(self.doc, &self.chat.farb_name(label));
            if ids.is_empty() {
                continue;
            }
            let v = self.chat.zahl(pfad, standard).clamp(0.0, 1.0);
            if v > 0.0 {
                self.b("object.opacity", json!({"opacity": if hell { v } else { 0.3 * v }, "ids": ids}));
            }
            self.b("object.setFlags", json!({"ids": ids, "hidden": v <= 0.0}));
        }
    }
}

/// Emojis (Script-Label „Emoji“) und Datums-Verzierungen („Verzierung“) in den Texten von `chat`:
/// Befehle, die sie so groß machen und so weit anheben, wie es zum Nachrichtentext `text` und zum
/// Datum `datum` passt – im Verhältnis zur Schriftgröße, die sie in `basis` hatten.
fn mitwachsende_objekte(basis: &Document, chat: &Chat, text: f64, datum: f64) -> Vec<(&'static str, Value)> {
    let stilgroesse = |name: &str, standard: f64| {
        let n = chat.stil_name(name);
        basis.styles.paragraph.iter().find(|s| s.name == n).and_then(|s| s.chars.size).filter(|v| *v > 0.0).unwrap_or(standard)
    };
    let faktor = [("Emoji", text / stilgroesse("Chat Sie", SCHRIFT_TEXT.1)), ("Verzierung", datum / stilgroesse("Datum", SCHRIFT_DATUM.1))];
    let mut out = Vec::new();
    for (sid, story) in &basis.stories {
        if chat_von_text(basis, *sid) != Some(chat.nummer) {
            continue;
        }
        for (k, o) in story.objects.iter().enumerate() {
            let Some(f) = faktor.iter().find(|(l, _)| *l == o.item.label).map(|(_, f)| *f) else { continue };
            if (f - 1.0).abs() < 1e-6 {
                continue;
            }
            let (_, h) = o.size();
            out.push(("anchored.resize", json!({"story": sid.0, "index": k, "height": (h * f).clamp(4.0, 10_000.0)})));
            if let designcraft_doc::AnchorPosition::Inline { y_offset } = o.position {
                out.push(("anchored.options", json!({"story": sid.0, "index": k, "position": "inline", "yOffset": y_offset * f})));
            }
        }
    }
    out
}

/// Objekte (auch auf Mustervorlagen und in Gruppen) mit dem Script-Label `label`.
fn mit_namen(doc: &Document, label: &str) -> Vec<Value> {
    let mut ids = Vec::new();
    for sp in doc.spreads.iter().chain(doc.parents.iter()) {
        for it in &sp.items {
            it.walk(&mut |x| {
                if x.label == label {
                    ids.push(json!(x.id.0));
                }
            });
        }
    }
    ids
}

/// Befehle als Skripttext (eine Zeile je Befehl).
pub fn als_skript(befehle: &[Value]) -> String {
    befehle.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")
}

/// Rundung und Versatz des Spaltenbox-Schattens, wie im Hilfsprogramm (`ECKE`, `SCHATTEN_VERSATZ`).
const ECKE: f64 = 9.0;
const VERSATZ: f64 = 4.0;

/// Der sichtbare Rand eines Spaltenbox-Schattens `s` (die Papierfläche liegt um den Versatz links
/// oben darüber): (Rechteck, runde Ecke 0–3), wie `schatten_teile` im Hilfsprogramm. Drei Eckstücke,
/// groß genug für die ganze Rundung, und dazwischen die rechte und die untere Kante.
fn schatten_teile(s: designcraft_geom::Rect) -> [([f64; 4], Option<usize>); 5] {
    let e2 = 2.0 * ECKE;
    let (bx1, by1) = (s.x1 - VERSATZ, s.y1 - VERSATZ);
    [
        ([s.x1 - e2, s.y0, s.x1, s.y0 + e2], Some(1)),
        ([bx1 - 1.0, s.y0 + e2 - 1.0, s.x1, s.y1 - e2 + 1.0], None),
        ([s.x1 - e2, s.y1 - e2, s.x1, s.y1], Some(2)),
        ([s.x0 + e2 - 1.0, by1 - 1.0, s.x1 - e2 + 1.0, s.y1], None),
        ([s.x0, s.y1 - e2, s.x0 + e2, s.y1], Some(3)),
    ]
}

/// Chats von vor dem 22.11.2026 haben als Schatten der Spalten-Box eine große Fläche voller
/// Sprenkel, die fast ganz unter der Papierfläche liegt: unsichtbar, aber Megabytes im PDF. Diese
/// Befehle (an `out` angehängt, ihre Verweise zählen ab dort) ersetzen sie in allen `chats` durch
/// die Teile am sichtbaren Rand, wie das Hilfsprogramm sie heute anlegt: auf derselben Ebene, an
/// derselben Stelle im Stapel. Im Bild ändert sich nichts; die Auswahl ist danach wie vorher.
pub fn schatten_umbauen(doc: &Document, chats: &[Chat], auswahl: &designcraft_doc::Selection, out: &mut Vec<Value>) {
    let befehl = |command: &str, params: Value| json!({"command": command, "params": params});
    let mut weg: Vec<(u64, Option<designcraft_doc::StoryId>)> = Vec::new();
    for chat in chats {
        let stil = chat.stil_name("Spalte Schatten");
        let vorlage = chat.vorlage();
        let (Some(k), true) = (
            doc.parents.iter().position(|p| p.parent.as_ref().is_some_and(|i| i.label() == vorlage)),
            doc.styles.paragraph.iter().any(|s| s.name == stil),
        ) else {
            continue;
        };
        let label = chat.farb_name("Spalte Schatten");
        // Ausgeblendete oder nicht druckende alte Schatten kommen nicht ins PDF: sie bleiben.
        let alt = doc.parents[k].items.iter().filter(|it| it.label == label && !it.hidden && !it.nonprinting).filter(|it| {
            let r = it.bounds();
            r.width().min(r.height()) > 2.0 * ECKE + 1.0
        });
        for it in alt {
            for (rect, ecke) in schatten_teile(it.bounds()) {
                let f = out.len();
                let id = format!("${f}.id");
                let rund = |i: usize| if ecke == Some(i) { json!({"shape": "rounded", "size": ECKE}) } else { json!({"shape": "none", "size": 0}) };
                out.push(befehl(
                    "frame.create",
                    json!({"spread": {"kind": "parent", "index": k}, "rect": rect, "content": "text", "text": "\u{a0}"}),
                ));
                out.push(befehl("object.textFrameOptions", json!({"inset": 0, "firstBaseline": "leading", "ids": [id]})));
                out.push(befehl("text.select", json!({"story": format!("${f}.story"), "anchor": 0, "focus": 2})));
                out.push(befehl("style.paragraph.apply", json!({"name": stil})));
                out.push(befehl("type.char", json!({"attrs": {"leading": {"kind": "points", "value": rect[3] - rect[1]}}})));
                out.push(befehl("type.para", json!({"attrs": {"shadingCorners": [rund(0), rund(1), rund(2), rund(3)]}})));
                out.push(befehl("object.label", json!({"label": label, "ids": [id]})));
                // Auf die Ebene und die Stelle des alten Schattens (unter Rauschen und Papierfläche).
                out.push(befehl("object.reorder", json!({"ids": [id], "below": it.id.0})));
            }
            weg.push((it.id.0, it.text_frame().map(|t| t.story)));
        }
    }
    if weg.is_empty() {
        return;
    }
    out.push(befehl("edit.clear", json!({"ids": weg.iter().map(|w| w.0).collect::<Vec<_>>()})));
    // Die Auswahl wie vorher (Text im Text eines gelöschten Schattens oder in einer Tabellenzelle: keine).
    let geloescht = |sid: designcraft_doc::StoryId| weg.iter().any(|w| w.1 == Some(sid));
    out.push(match (&auswahl.text, auswahl.inline_object()) {
        (Some(t), _) if t.cell.is_none() && !geloescht(t.story) => befehl("text.select", json!({"story": t.story.0, "anchor": t.anchor, "focus": t.focus})),
        (Some(_), _) => befehl("selection.set", json!({"ids": []})),
        (None, Some(i)) => befehl("selection.set", json!({"inline": {"story": i.story.0, "index": i.index, "content": i.content}})),
        (None, None) => befehl(
            "selection.set",
            json!({"ids": auswahl.items.iter().map(|i| i.0).filter(|i| !weg.iter().any(|w| w.0 == *i)).collect::<Vec<_>>(), "content": auswahl.content}),
        ),
    });
}
