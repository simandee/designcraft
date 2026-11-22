//! Die Chats eines Buchs: was das Jahresbuch-Fenster über jeden weiß (Name, Export, Zeitraum,
//! Personen, Gestaltung), im Dokument unter dem Etikett [`ETIKETT`] gespeichert, und welcher Chat
//! gerade markiert ist.
//!
//! Der erste Chat eines Buchs trägt die Format- und Farbfeld-Namen ohne Kürzel („Chat Sie“,
//! „Farbe Ich“), jeder weitere ein Kürzel („Chat 2/Chat Sie“, „Chat 2 · Farbe Ich“) – genau wie es
//! das Hilfsprogramm beim Einfügen vergibt.

use designcraft_doc::{Document, StoryId};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Das Dokument-Etikett mit den Chats (JSON-Liste von [`Chat`]).
pub const ETIKETT: &str = "jahresbuch.chats";

/// Eine Person im Chat. `key` steckt in den Format- und Farbnamen („Ich“, „Sie“ oder „Person 1“ …),
/// `ids` sind ihre Absender-Kennungen im Export (mehrere, wenn Personen zusammengelegt wurden).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Person {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub ids: Vec<String>,
    pub farbe: String,
}

/// Ein Chat im Buch.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Chat {
    pub nummer: u32,
    pub name: String,
    #[serde(default)]
    pub export: String,
    #[serde(default)]
    pub medien: String,
    /// Kennung des Chats im Export.
    #[serde(default)]
    pub chat: String,
    /// Eingefügter Zeitraum (JJJJ-MM-TT).
    #[serde(default)]
    pub von: String,
    #[serde(default)]
    pub bis: String,
    #[serde(default)]
    pub personen: Vec<Person>,
    /// Der Text des Chats im Buch (Fortsetzen und Neu aufbauen wirken auf ihn, auch wenn gerade
    /// ein kopierter Ausschnitt markiert ist). Fehlt bei Chats von vor dieser Angabe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub story: Option<u64>,
    /// Die Gestaltung, mit denselben Schlüsseln wie `stil.json` des Hilfsprogramms.
    #[serde(default)]
    pub stil: Map<String, Value>,
}

impl Chat {
    /// Format-Name in diesem Chat.
    pub fn stil_name(&self, name: &str) -> String {
        if self.nummer <= 1 { name.to_string() } else { format!("Chat {}/{name}", self.nummer) }
    }

    /// Farbfeld-Name (auch Script-Label auf der Mustervorlage) in diesem Chat.
    pub fn farb_name(&self, name: &str) -> String {
        if self.nummer <= 1 { name.to_string() } else { format!("Chat {} · {name}", self.nummer) }
    }

    /// Die Chat-Mustervorlage.
    pub fn vorlage(&self) -> String {
        if self.nummer <= 1 { "J-Chat".into() } else { format!("J{}-Chat", self.nummer) }
    }

    /// Mehr als zwei Personen.
    pub fn gruppe(&self) -> bool {
        self.personen.iter().any(|p| p.key != "Ich" && p.key != "Sie")
    }

    /// Farbfeld mit der Hauptfarbe einer Person (ohne Kürzel), wie `farbe_von` im Hilfsprogramm.
    pub fn person_farbfeld(key: &str) -> String {
        match key {
            "Ich" => "Farbe Ich".into(),
            "Sie" => "Farbe Sie".into(),
            k => format!("{k} · Farbe"),
        }
    }

    /// Ein Wert der Gestaltung, auch verschachtelt (`rauschen.innen`).
    pub fn wert(&self, pfad: &str) -> Option<&Value> {
        stil_wert(&self.stil, pfad)
    }

    pub fn zahl(&self, pfad: &str, standard: f64) -> f64 {
        self.wert(pfad).and_then(Value::as_f64).unwrap_or(standard)
    }

    pub fn text(&self, pfad: &str, standard: &str) -> String {
        self.wert(pfad).and_then(Value::as_str).unwrap_or(standard).to_string()
    }

    pub fn setze(&mut self, pfad: &str, wert: Value) {
        setze_stil(&mut self.stil, pfad, wert);
    }
}

/// Ein Wert aus einer Gestaltung, auch verschachtelt (`rauschen.innen`).
pub fn stil_wert<'a>(stil: &'a Map<String, Value>, pfad: &str) -> Option<&'a Value> {
    let mut teile = pfad.split('.');
    let mut v = stil.get(teile.next()?)?;
    for t in teile {
        v = v.get(t)?;
    }
    Some(v)
}

pub fn setze_stil(stil: &mut Map<String, Value>, pfad: &str, wert: Value) {
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

/// Die Chats im Dokument. Ein Chat, der eingefügt wurde, bevor es das Etikett gab (er hat die
/// Formate des ersten Chats, steht aber nicht im Etikett), wird mit `vorgabe` (stil.json)
/// ergänzt.
pub fn chats(doc: &Document, vorgabe: &Map<String, Value>) -> Vec<Chat> {
    let mut out: Vec<Chat> = doc.labels.get(ETIKETT).and_then(|t| serde_json::from_str(t).ok()).unwrap_or_default();
    let alter_chat = doc.styles.paragraph.iter().any(|s| s.name == "Chat Sie");
    if alter_chat && !out.iter().any(|c| c.nummer == 1) {
        let farbe = |k: &str, s: &str| stil_wert(vorgabe, k).and_then(Value::as_str).unwrap_or(s).to_string();
        out.insert(
            0,
            Chat {
                nummer: 1,
                name: "Chat".into(),
                personen: vec![
                    Person { key: "Ich".into(), name: "Ich".into(), ids: vec![], farbe: farbe("farbe_ich", "#E8493A") },
                    Person { key: "Sie".into(), name: "Sie".into(), ids: vec![], farbe: farbe("farbe_sie", "#3554E0") },
                ],
                stil: vorgabe.clone(),
                ..Chat::default()
            },
        );
    }
    out.sort_by_key(|c| c.nummer);
    out
}

/// Der Befehl, der `chats` im Dokument speichert.
pub fn speichern(chats: &[Chat]) -> Value {
    json!({"command": "document.setLabel", "params": {"key": ETIKETT, "value": serde_json::to_string(chats).unwrap_or_default()}})
}

/// Die Nummer des Chats, zu dem der Text `sid` gehört (an seinen Absatzformaten erkannt), oder
/// None für Text, der kein Chat ist.
pub fn chat_von_text(doc: &Document, sid: StoryId) -> Option<u32> {
    let st = doc.story(sid)?;
    for p in &st.paras {
        // Ab dem zweiten Chat ist jedes Format mit Kürzel „Chat N/“ eindeutig seins.
        if let Some(rest) = p.style.strip_prefix("Chat ")
            && let Some((n, _)) = rest.split_once('/')
            && let Ok(n) = n.parse::<u32>()
        {
            return Some(n);
        }
        if chat_format(&p.style) {
            return Some(1);
        }
    }
    None
}

/// Ein Absatzformat, wie es nur das Hilfsprogramm für Chats anlegt (ohne Kürzel): eine Art
/// („Chat“, „Bild“, „Album“ …) und eine Person („Ich“, „Sie“, „Person 2“), etwa „Chat Sie“ oder
/// „Album Ich 3 Ende“. Formate des Buchs wie „Datum“ oder „Bild Unterschrift“ zählen nicht.
fn chat_format(name: &str) -> bool {
    let person = |rest: &str| {
        ["Ich", "Sie"].iter().any(|p| rest.strip_prefix(p).is_some_and(|r| r.is_empty() || r.starts_with(' '))) || rest.starts_with("Person ")
    };
    ["Chat ", "Bild ", "Album ", "Emoji ", "Sticker ", "Reaktion "].iter().any(|p| name.strip_prefix(p).is_some_and(person))
}

/// Ob der erste Chat seine Namen ohne Kürzel tragen kann: nicht, wenn das Buch schon eigene
/// Formate oder Farbfelder mit solchen allgemeinen Namen hat (etwa ein Absatzformat „Datum“) –
/// sonst würde der Chat sie übernehmen und seine Gestaltung sie ändern.
pub fn namen_ohne_kuerzel_frei(doc: &Document) -> bool {
    const FORMATE: [&str; 5] = ["Datum", "Zeit", "Link", "Hinweis", "Zitat Text"];
    const FARBFELDER: [&str; 2] = ["Papier", "Textfarbe"];
    let formate = doc.styles.paragraph.iter().map(|s| s.name.as_str()).chain(doc.styles.character.iter().map(|s| s.name.as_str()));
    !formate.into_iter().any(|n| FORMATE.contains(&n)) && !doc.swatches.iter().any(|s| FARBFELDER.contains(&s.name.as_str()))
}

/// Der Text, auf den Fortsetzen und Neu aufbauen von `chat` wirken: der im Etikett vermerkte,
/// solange er noch zu ihm gehört, sonst `markiert` (der markierte Text des Chats).
pub fn text_des_chats(doc: &Document, chat: &Chat, markiert: Option<StoryId>) -> Option<StoryId> {
    chat.story.map(StoryId).filter(|sid| chat_von_text(doc, *sid) == Some(chat.nummer)).or(markiert)
}

/// Der markierte Chat: ein markierter Textrahmen, der Textcursor oder ein ausgewähltes Objekt im
/// Text eines Chats (etwa ein Chat-Foto). (Nummer, Text)
pub fn markierter_chat(doc: &Document, sel: &designcraft_doc::Selection) -> Option<(u32, StoryId)> {
    let mut kandidaten: Vec<StoryId> = sel.text.iter().map(|t| t.story).chain(sel.inline_object().map(|i| i.story)).collect();
    for id in &sel.items {
        if let Some(tf) = doc.item(*id).and_then(|i| i.text_frame()) {
            kandidaten.push(tf.story);
        }
    }
    kandidaten.into_iter().find_map(|sid| chat_von_text(doc, sid).map(|n| (n, sid)))
}

/// Die nächste freie Chat-Nummer im Dokument.
pub fn naechste_nummer(chats: &[Chat]) -> u32 {
    chats.iter().map(|c| c.nummer).max().map_or(1, |n| n + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Namen bekommen ab dem zweiten Chat ein Kürzel; ein Text wird seinem Chat zugeordnet, an
    /// den Formaten mit oder ohne Kürzel.
    #[test]
    fn namen_und_zuordnung() {
        let eins = Chat { nummer: 1, ..Chat::default() };
        let zwei = Chat { nummer: 2, ..Chat::default() };
        assert_eq!(eins.stil_name("Chat Sie"), "Chat Sie");
        assert_eq!(zwei.stil_name("Chat Sie"), "Chat 2/Chat Sie");
        assert_eq!(zwei.farb_name("Farbe Ich"), "Chat 2 · Farbe Ich");
        assert_eq!(zwei.vorlage(), "J2-Chat");
        let mut doc = Document::new(&designcraft_doc::build::NewDocument::default());
        let lid = doc.default_layer();
        let (_, a) = doc
            .add_text_frame(designcraft_doc::SpreadRef::Doc(0), designcraft_geom::Rect::new(0.0, 0.0, 100.0, 100.0), lid, "x", Default::default())
            .unwrap();
        let (_, b) = doc
            .add_text_frame(designcraft_doc::SpreadRef::Doc(0), designcraft_geom::Rect::new(0.0, 200.0, 100.0, 300.0), lid, "y", Default::default())
            .unwrap();
        assert_eq!(chat_von_text(&doc, a), None, "gewöhnlicher Text ist kein Chat");
        doc.story_mut(a).unwrap().paras[0].style = "Datum".into();
        assert_eq!(chat_von_text(&doc, a), None, "ein Format „Datum“ des Buchs macht noch keinen Chat");
        doc.story_mut(a).unwrap().paras[0].style = "Bild Unterschrift".into();
        assert_eq!(chat_von_text(&doc, a), None);
        doc.story_mut(a).unwrap().paras[0].style = "Chat Ich".into();
        doc.story_mut(b).unwrap().paras[0].style = "Chat 3/Datum".into();
        assert_eq!(chat_von_text(&doc, a), Some(1));
        assert_eq!(chat_von_text(&doc, b), Some(3));
        // Ein ausgewähltes Chat-Foto (ein Objekt im Text) markiert seinen Chat.
        let foto =
            designcraft_doc::Selection { inline: Some(designcraft_doc::InlineSel { story: b, index: 0, content: false }), ..Default::default() };
        assert_eq!(markierter_chat(&doc, &foto), Some((3, b)));
    }
}
