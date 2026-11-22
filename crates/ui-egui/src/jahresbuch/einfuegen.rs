//! Einen Chat ins offene Buch einfügen: das Hilfsprogramm baut die Skripte (Einrichtung und
//! Inhalt), dann fließt der Chat in den markierten leeren Textrahmen oder auf neue Chat-Seiten am
//! Ende, und der Chat kommt mit seiner Gestaltung ins Etikett des Dokuments. Fortsetzen hängt einen
//! Zeitraum hinten an, Neu aufbauen setzt die Tage eines Zeitraums neu; der Rest bleibt.

use chrono::{Datelike, NaiveDate, Weekday};
use designcraft_doc::{Story, StoryId};
use serde_json::{Map, Value, json};

use super::modell::{self, Chat, Person};
use super::{gestaltung, home, lies_stil, ordner, skript};
use crate::DesignApp;

/// Was eingefügt werden soll.
#[derive(Clone, Debug, Default)]
pub struct Auftrag {
    pub export: String,
    pub medien: String,
    /// Kennung des Chats im Export (None: der erste).
    pub chat: Option<String>,
    /// Name des Chats im Buch.
    pub name: String,
    /// Zeitraum (JJJJ-MM-TT).
    pub von: String,
    pub bis: String,
    pub python: String,
    /// Die Gestaltung, mit der der Chat beginnt.
    pub stil: Map<String, Value>,
    /// Personen (Farben) eines anderen Chats, dessen Aussehen der neue übernimmt.
    pub farben_wie: Vec<Person>,
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

/// Seite am Ende mit der Chat-Mustervorlage `vorlage` anhängen: (Druckbogen, Spaltenrechtecke).
fn chat_seite_anhaengen(app: &mut DesignApp, vorlage: &str) -> Result<(u64, Vec<Value>), String> {
    app.run("layout.pages.insert", json!({"count": 1, "parent": vorlage}))?;
    let inspect = app.session.execute("document.inspect", &json!({})).map_err(|e| e.to_string())?;
    let spreads = inspect["spreads"].as_array().cloned().unwrap_or_default();
    let sp = spreads.last().ok_or("keine Seiten")?;
    let page = sp["pages"].as_array().and_then(|p| p.last()).ok_or("keine Seite")?;
    Ok((sp["index"].as_u64().unwrap_or(0), page["columnRects"].as_array().cloned().unwrap_or_default()))
}

/// Die Gestaltung, mit der das Hilfsprogramm den Chat baut: die gewählte, mit den Farben des
/// Chats, dessen Aussehen er übernimmt.
fn stil_fuer_hilfsprogramm(a: &Auftrag) -> Map<String, Value> {
    let mut stil = a.stil.clone();
    for (key, feld) in [("Ich", "farbe_ich"), ("Sie", "farbe_sie")] {
        if let Some(p) = a.farben_wie.iter().find(|p| p.key == key) {
            stil.insert(feld.into(), json!(p.farbe));
        }
    }
    stil
}

/// Was mit dem Text `story` eines Chats geschieht.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wohin {
    /// Der Zeitraum kommt hinten an den Text (Fortsetzen): was schon da ist, bleibt.
    Anhaengen(u64),
    /// Die Tage des Zeitraums werden neu gesetzt (Neu aufbauen); was davor und danach steht, bleibt.
    Ersetzen(u64),
}

const WOCHENTAGE: [(&str, Weekday); 7] = [
    ("Montag", Weekday::Mon),
    ("Dienstag", Weekday::Tue),
    ("Mittwoch", Weekday::Wed),
    ("Donnerstag", Weekday::Thu),
    ("Freitag", Weekday::Fri),
    ("Samstag", Weekday::Sat),
    ("Sonntag", Weekday::Sun),
];
const MONATE: [&str; 12] = ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"];

/// Wochentag, Tag und Monat einer Datumszeile, wie das Hilfsprogramm sie setzt („Mittwoch,
/// 11. November“, ohne Jahr; davor und dahinter Verzierungen).
fn datumszeile(text: &str) -> Option<(Weekday, u32, u32)> {
    WOCHENTAGE.iter().find_map(|(name, wochentag)| {
        let rest = &text[text.find(&format!("{name}, "))? + name.len() + 2..];
        let (tag, rest) = rest.split_once(". ")?;
        let monat = MONATE.iter().position(|m| rest.starts_with(m))?;
        Some((*wochentag, tag.parse().ok()?, u32::try_from(monat + 1).ok()?))
    })
}

/// Die Tage im Text `st` eines Chats: (Datum, Byte, an dem seine Datumszeile im Format
/// `datum_stil` beginnt), in Textreihenfolge. Die Zeilen tragen kein Jahr; der Wochentag legt es
/// fest: für die erste Zeile der passende Tag, der `naehe` am nächsten liegt, für jede weitere der
/// erste passende ab der vorigen.
fn tage(st: &Story, datum_stil: &str, naehe: NaiveDate) -> Vec<(NaiveDate, usize)> {
    let mut out: Vec<(NaiveDate, usize)> = Vec::new();
    for (pi, r) in st.para_ranges().into_iter().enumerate() {
        if st.paras.get(pi).is_none_or(|p| p.style != datum_stil) {
            continue;
        }
        let Some((wochentag, tag, monat)) = datumszeile(&st.text[r.clone()]) else { continue };
        let passend = |jahre: std::ops::RangeInclusive<i32>| {
            jahre.filter_map(move |j| NaiveDate::from_ymd_opt(j, monat, tag)).filter(move |d| d.weekday() == wochentag)
        };
        let datum = match out.last() {
            Some(&(vorige, _)) => passend(vorige.year()..=vorige.year() + 30).find(|d| *d >= vorige),
            None => passend(naehe.year() - 30..=naehe.year() + 30).min_by_key(|d| (*d - naehe).num_days().abs()),
        };
        if let Some(d) = datum {
            out.push((d, r.start));
        }
    }
    out
}

/// Die Bytes `[anfang, ende, länge]` des Textes `st` von `chat`, die die Tage `a.von`..=`a.bis`
/// einnehmen: ab der ersten Datumszeile ab `von` (ohne `von`: ab dem Anfang) bis vor die erste
/// Datumszeile nach `bis` (ohne `bis` oder wenn keine folgt: bis zum Ende). Liegt der Zeitraum
/// hinter dem Chat, ist der Bereich leer am Ende, davor leer am Anfang.
fn zeitraum_im_text(st: &Story, chat: &Chat, a: &Auftrag) -> Result<[usize; 3], String> {
    let datum = |t: &str| NaiveDate::parse_from_str(t, "%Y-%m-%d").ok();
    let (von, bis, laenge) = (datum(&a.von), datum(&a.bis), st.len());
    let Some(naehe) = datum(&chat.von).or(von).or(bis) else { return Ok([0, laenge, laenge]) };
    if von.is_none() && bis.is_none() {
        return Ok([0, laenge, laenge]);
    }
    let tage = tage(st, &chat.stil_name("Datum"), naehe);
    if tage.is_empty() && laenge > 0 {
        return Err("Im Text des Chats stehen keine Datumszeilen. Neu aufbauen geht hier nur für den ganzen Chat.".into());
    }
    let anfang = von.map_or(0, |v| tage.iter().find(|t| t.0 >= v).map_or(laenge, |t| t.1));
    let ende = bis.and_then(|b| tage.iter().find(|t| t.0 > b)).map_or(laenge, |t| t.1);
    Ok([anfang, ende.max(anfang), laenge])
}

/// Einen Arbeitsordner anlegen, `f` darin laufen lassen, ihn wieder löschen.
fn im_arbeitsordner<T>(f: impl FnOnce(&std::path::Path) -> Result<T, String>) -> Result<T, String> {
    static LAUF: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let lauf = LAUF.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let arbeit = std::env::temp_dir().join(format!("designcraft-jahresbuch-{}-{lauf}", std::process::id()));
    std::fs::create_dir_all(&arbeit).map_err(|e| e.to_string())?;
    let r = f(&arbeit);
    let _ = std::fs::remove_dir_all(&arbeit);
    r
}

fn pruefen(app: &DesignApp, a: &Auftrag) -> Result<(), String> {
    if app.session.active().is_none() {
        return Err("Zuerst das Buch öffnen (oder ein neues Dokument anlegen).".into());
    }
    if !std::path::Path::new(&a.export).is_file() {
        return Err(format!("WhatsApp-Export nicht gefunden: {}", a.export));
    }
    // Ohne Medienordner fehlte jedes Foto still („nicht mehr vorhanden“).
    if !a.medien.is_empty() && !std::path::Path::new(&a.medien).is_dir() {
        return Err(format!("Medienordner nicht gefunden: {}", a.medien));
    }
    Ok(())
}

/// `f` als ein Rückgängig-Schritt: Einrichtung, Seiten, Text und Etikett zusammen. Geht etwas
/// schief, ist das Dokument danach wieder genau wie vorher (keine halben Chats).
fn als_ein_schritt<T>(app: &mut DesignApp, label: &str, f: impl FnOnce(&mut DesignApp) -> Result<T, String>) -> Result<T, String> {
    app.session.begin_interaction(label).map_err(|e| e.to_string())?;
    match f(app) {
        Ok(v) => app.session.commit_interaction().map(|()| v).map_err(|e| e.to_string()),
        Err(e) => {
            app.session.cancel_interaction();
            Err(e)
        }
    }
}

/// Die Gestaltung von `chat` anwenden und `chats` (mit ihm) im Dokument speichern, als ein
/// Rückgängig-Schritt.
fn speichern_und_gestalten(app: &mut DesignApp, chats: &[Chat], chat: &Chat) -> Result<(), String> {
    let doc = app.session.active().map(|st| st.doc.clone()).ok_or("kein Dokument")?;
    let mut befehle = gestaltung::befehle(&doc, chat);
    befehle.push(modell::speichern(chats));
    skript(app, gestaltung::als_skript(&befehle)).map(|_| ())
}

/// Den Chat einfügen; gibt ihn zurück, wie er im Dokument-Etikett steht.
pub fn einfuegen(app: &mut DesignApp, a: &Auftrag) -> Result<Chat, String> {
    pruefen(app, a)?;
    als_ein_schritt(app, "Chat einfügen", |app| einfuegen_schritt(app, a))
}

fn einfuegen_schritt(app: &mut DesignApp, a: &Auftrag) -> Result<Chat, String> {
    let mut chats = app.session.active().map(|st| modell::chats(&st.doc, &lies_stil())).unwrap_or_default();
    let mut nummer = modell::naechste_nummer(&chats);
    if nummer == 1 && !app.session.active().is_some_and(|st| modell::namen_ohne_kuerzel_frei(&st.doc)) {
        nummer = 2;
    }
    let (personen, story) = im_arbeitsordner(|arbeit| einfuegen_in(app, a, nummer, arbeit, None, None))?;
    let chat = Chat {
        nummer,
        name: if a.name.trim().is_empty() { format!("Chat {nummer}") } else { a.name.trim().to_string() },
        export: a.export.clone(),
        medien: a.medien.clone(),
        chat: a.chat.clone().unwrap_or_default(),
        von: a.von.clone(),
        bis: a.bis.clone(),
        personen,
        story: Some(story),
        stil: a.stil.clone(),
    };
    chats.retain(|c| c.nummer != nummer);
    chats.push(chat.clone());
    speichern_und_gestalten(app, &chats, &chat)?;
    app.status("Jahresbuch: Chat eingefügt.");
    Ok(chat)
}

/// Einen Chat im Buch fortsetzen (`Wohin::Anhaengen`: der Zeitraum von `a` kommt hinten an seinen
/// Text `story`) oder neu aufbauen (`Wohin::Ersetzen`: die Tage des Zeitraums werden neu gesetzt,
/// der Rest des Chats bleibt). Personen, Farben und Gestaltung bleiben die des Chats.
pub fn fortsetzen(app: &mut DesignApp, a: &Auftrag, chat: &Chat, wohin: Wohin) -> Result<Chat, String> {
    pruefen(app, a)?;
    let label = if matches!(wohin, Wohin::Ersetzen(_)) { "Chat neu aufbauen" } else { "Chat fortsetzen" };
    als_ein_schritt(app, label, |app| fortsetzen_schritt(app, a, chat, wohin))
}

fn fortsetzen_schritt(app: &mut DesignApp, a: &Auftrag, chat: &Chat, wohin: Wohin) -> Result<Chat, String> {
    let mut chats = app.session.active().map(|st| modell::chats(&st.doc, &lies_stil())).unwrap_or_default();
    let personen = (!chat.personen.is_empty()).then_some(chat.personen.as_slice());
    let (Wohin::Anhaengen(story) | Wohin::Ersetzen(story)) = wohin;
    if matches!(wohin, Wohin::Anhaengen(_)) && !chat.bis.is_empty() && (a.von.is_empty() || a.von <= chat.bis) {
        return Err(format!("Der Chat reicht schon bis {}: Fortsetzen braucht einen Anfang danach, sonst stünden Tage doppelt im Buch.", chat.bis));
    }
    let bytes = {
        let doc = &app.session.active().ok_or("kein Dokument")?.doc;
        let st = doc.story(StoryId(story)).ok_or("Der Text des Chats fehlt.")?;
        match wohin {
            Wohin::Anhaengen(_) => [st.len(); 3],
            Wohin::Ersetzen(_) => zeitraum_im_text(st, chat, a)?,
        }
    };
    let (neu, story) = im_arbeitsordner(|arbeit| einfuegen_in(app, a, chat.nummer, arbeit, Some((story, bytes)), personen))?;
    let mut chat = chat.clone();
    chat.story = Some(story);
    (chat.export, chat.medien) = (a.export.clone(), a.medien.clone());
    if let Some(c) = &a.chat {
        chat.chat = c.clone();
    }
    // Neue Personen (in Gruppen seitdem dazugekommen) hängt das Hilfsprogramm hinten an.
    if neu.len() > chat.personen.len() {
        chat.personen = neu;
    }
    match wohin {
        Wohin::Ersetzen(_) => {
            // Der Chat umfasst nun seinen bisherigen und den neu aufgebauten Zeitraum (leer: offen).
            let offen = |x: &str, y: &str| x.is_empty() || y.is_empty();
            chat.von = if offen(&chat.von, &a.von) { String::new() } else { chat.von.as_str().min(a.von.as_str()).to_string() };
            chat.bis = if offen(&chat.bis, &a.bis) { String::new() } else { chat.bis.as_str().max(a.bis.as_str()).to_string() };
        }
        Wohin::Anhaengen(_) => {
            if chat.von.is_empty() {
                chat.von = a.von.clone();
            }
            chat.bis = a.bis.clone();
        }
    }
    match chats.iter_mut().find(|c| c.nummer == chat.nummer) {
        Some(c) => *c = chat.clone(),
        None => chats.push(chat.clone()),
    }
    // Nur das Etikett: die Gestaltung steht schon in den Formaten (neue baut das Hilfsprogramm aus
    // ihr), und eigene Anpassungen an den Formaten bleiben so, wie sie sind.
    skript(app, gestaltung::als_skript(&[modell::speichern(&chats)]))?;
    app.status(if matches!(wohin, Wohin::Ersetzen(_)) { "Jahresbuch: Chat neu aufgebaut." } else { "Jahresbuch: Chat fortgesetzt." });
    Ok(chat)
}

/// Hilfsprogramm aufrufen und die Skripte ausführen; gibt die Personen des Chats und seinen Text
/// zurück. `ziel`: None für einen neuen Chat, sonst der Text eines Chats und die Bytes
/// `[anfang, ende, länge]`, an deren Stelle der Zeitraum kommt.
fn einfuegen_in(
    app: &mut DesignApp,
    a: &Auftrag,
    nummer: u32,
    arbeit: &std::path::Path,
    ziel: Option<(u64, [usize; 3])>,
    personen: Option<&[Person]>,
) -> Result<(Vec<Person>, u64), String> {
    let inspect = app.session.execute("document.inspect", &json!({})).map_err(|e| e.to_string())?;
    let laenge = |story: u64| {
        inspect["stories"].as_array().and_then(|a| a.iter().find(|s| s["id"].as_u64() == Some(story))).and_then(|s| s["length"].as_u64())
    };
    let insp_pfad = arbeit.join("dokument.json");
    std::fs::write(&insp_pfad, inspect.to_string()).map_err(|e| e.to_string())?;
    let stil_pfad = arbeit.join("stil.json");
    std::fs::write(&stil_pfad, Value::Object(stil_fuer_hilfsprogramm(a)).to_string()).map_err(|e| e.to_string())?;
    let (einr, inh, pers) = (arbeit.join("einrichtung.jsonl"), arbeit.join("inhalt.jsonl"), arbeit.join("personen.json"));
    let p = |x: &std::path::Path| x.to_string_lossy().into_owned();
    app.status("Jahresbuch: Chat wird aufbereitet …");
    let mut cmd = std::process::Command::new(&a.python);
    cmd.arg(ordner().join("designcraft_import.py"))
        .arg(&a.export)
        .arg(&a.medien)
        .arg(&inh)
        .args(["--dokument", &p(&insp_pfad), "--einrichtung-ausgabe", &p(&einr), "--personen-ausgabe", &p(&pers)])
        .args(["--von", &a.von, "--bis", &a.bis, "--nummer", &nummer.to_string()])
        .args(["--stil", &p(&stil_pfad), "--arbeitsordner", &p(&home().join("Jahresbuch").join("chat_dateien"))]);
    if let Some(c) = &a.chat {
        cmd.args(["--chat", c]);
    }
    if let Some(personen) = personen {
        let pfad = arbeit.join("personen_vorgabe.json");
        std::fs::write(&pfad, serde_json::to_string(personen).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        cmd.args(["--personen", &p(&pfad)]);
    }
    if let Some((_, bytes)) = ziel {
        cmd.arg("--ersetzen").args(bytes.map(|b| b.to_string()));
    }
    let out = cmd.output().map_err(|e| format!("Python konnte nicht gestartet werden ({}): {e}", a.python))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let letzte: Vec<&str> = err.lines().rev().take(4).collect();
        return Err(format!("Das Hilfsprogramm meldet einen Fehler: {}", letzte.into_iter().rev().collect::<Vec<_>>().join(" | ")));
    }
    let personen: Vec<Person> = std::fs::read_to_string(&pers).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    // Ein Skript hat höchstens so viele Schritte: was darüber ginge, gar nicht erst anfangen
    // (es bräche nach langem Warten ab, und Einrichtung und leere Seiten blieben zurück).
    let inhalt = std::fs::read_to_string(&inh).map_err(|e| e.to_string())?;
    let schritte = inhalt.lines().filter(|l| !l.trim().is_empty()).count();
    let hoechstens = designcraft_engine::script::MAX_SCRIPT_STEPS;
    if schritte >= hoechstens {
        return Err(format!(
            "Der Zeitraum ist zu groß für einmal ({schritte} Arbeitsschritte, höchstens {hoechstens}). Bitte einen kürzeren Zeitraum wählen und den Rest danach mit „Fortsetzen …“ anhängen."
        ));
    }
    let vorlage = Chat { nummer, ..Chat::default() }.vorlage();
    // 1. Farbfelder, Formate, Chat-Mustervorlage
    skript(app, std::fs::read_to_string(&einr).map_err(|e| e.to_string())?)?;
    // 2. Ziel: der markierte Rahmen, wenn er leer ist (vorhandener Text, etwa eine Geschichte
    // oder ein anderer Chat, bleibt unangetastet), sonst eine neue Chat-Seite am Ende; beim
    // Fortsetzen und Neuaufbauen der Text des Chats.
    let ziel = match ziel {
        None => markierter_rahmen(&inspect).filter(|(_, story)| laenge(*story).unwrap_or(0) == 0),
        Some((story, _)) => {
            let rahmen = app.run("story.get", json!({"story": story}))?["frames"].as_array().and_then(|f| f.first()).and_then(Value::as_u64);
            Some((rahmen.ok_or("Der Text des Chats hat keinen Rahmen.")?, story))
        }
    };
    let (mut letzter, story) = match ziel {
        Some(ziel) => ziel,
        None => neue_chat_seite(app, &vorlage)?,
    };
    // Der Text kann schon weitere verkettete Rahmen haben: weiter geht es nach dem letzten.
    if let Ok(info) = app.run("story.get", json!({"story": story}))
        && let Some(l) = info["frames"].as_array().and_then(|f| f.last()).and_then(Value::as_u64)
    {
        letzter = l;
    }
    // 3. Der Chat
    skript(app, inhalt.replace("\"__STORY__\"", &story.to_string()))?;
    // 4. Passt er nicht, kommen Chat-Seiten am Ende dazu, zwei Spalten je Seite.
    seiten_bis_alles_passt(app, story, letzter, &vorlage)?;
    Ok((personen, story))
}

/// Eine neue Chat-Seite mit einem Rahmen je Spalte, verkettet: (erster Rahmen, Text).
fn neue_chat_seite(app: &mut DesignApp, vorlage: &str) -> Result<(u64, u64), String> {
    let (spread, spalten) = chat_seite_anhaengen(app, vorlage)?;
    let rect = spalten.first().cloned().ok_or("Chat-Seite ohne Spalten")?;
    let f = app.run("frame.create", json!({"spread": spread, "rect": rect, "content": "text"}))?;
    let erster = f["id"].as_u64().ok_or("kein Rahmen")?;
    let mut von = erster;
    for rect in spalten.iter().skip(1) {
        let r = app.run("text.thread", json!({"from": von, "rect": rect, "spread": spread}))?;
        von = r["to"].as_u64().unwrap_or(von);
    }
    // Beim Verketten leerer Rahmen bekommt der Text eine neue Nummer: die des ersten Rahmens.
    let inspect = app.session.execute("document.inspect", &json!({})).map_err(|e| e.to_string())?;
    let story = inspect["spreads"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|sp| sp["items"].as_array().into_iter().flatten())
        .find(|it| it["id"].as_u64() == Some(erster))
        .and_then(|it| it["story"].as_u64())
        .ok_or("kein Text")?;
    Ok((erster, story))
}

/// Übersatz: Chat-Seiten am Ende anhängen (zwei Spalten je Seite), bis der Text `story` passt.
fn seiten_bis_alles_passt(app: &mut DesignApp, story: u64, letzter: u64, vorlage: &str) -> Result<(), String> {
    let uebersatz = app.run("story.get", json!({"story": story})).map(|v| !v["overset"].is_null()).unwrap_or(false);
    if uebersatz {
        let (spread, spalten) = chat_seite_anhaengen(app, vorlage)?;
        let mut von = letzter;
        for (i, rect) in spalten.iter().enumerate() {
            let letzte_spalte = i + 1 == spalten.len();
            let r = app.run("text.thread", json!({"from": von, "rect": rect, "spread": spread, "autoflow": letzte_spalte}))?;
            von = r["to"].as_u64().unwrap_or(von);
        }
    }
    Ok(())
}
