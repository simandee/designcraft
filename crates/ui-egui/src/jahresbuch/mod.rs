//! Das Jahresbuch-Fenster: WhatsApp-Chats ins Buch einfügen und jeden Chat einzeln gestalten.
//!
//! Den Chat baut das Hilfsprogramm `jahresbuch/designcraft_import.py` (Python) als Befehlsskript;
//! das Fenster ruft es auf und führt die Skripte im offenen Dokument aus. Was das Fenster über die
//! Chats eines Buchs weiß, steht im Dokument selbst ([`modell::ETIKETT`]).

pub mod einfuegen;
pub mod gestaltung;
pub mod kalender;
pub mod modell;
pub mod panel;

use std::path::PathBuf;

use serde_json::{Map, Value, json};

use crate::DesignApp;

/// Die Fenster-ID im Andock-Bereich.
pub const PANEL: &str = "jahresbuch";

/// Der Ordner des Hilfsprogramms im Quellcode (die App wird auf dem eigenen Rechner gebaut).
pub fn ordner() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("jahresbuch")
}

/// Deine Einstellungen: ~/Jahresbuch/stil.json (nicht im Programmordner, damit Updates sie nicht
/// berühren). Sie halten den zuletzt benutzten Export und die Gestaltung, mit der ein neuer Chat
/// beginnt. Fehlt die Datei, gelten die Vorgaben aus jahresbuch/stil.json.
pub fn stil_pfad() -> PathBuf {
    home().join("Jahresbuch").join("stil.json")
}

pub fn lies_stil() -> Map<String, Value> {
    let lies = |p: PathBuf| std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str::<Map<String, Value>>(&t).ok());
    lies(stil_pfad()).or_else(|| lies(ordner().join("stil.json"))).unwrap_or_default()
}

/// Einstellungen in ~/Jahresbuch/stil.json ändern (die übrigen bleiben).
pub fn merke(werte: &[(&str, Value)]) {
    let mut stil = lies_stil();
    for (k, v) in werte {
        stil.insert((*k).to_string(), v.clone());
    }
    if let Ok(t) = serde_json::to_string_pretty(&Value::Object(stil)) {
        if let Some(o) = stil_pfad().parent() {
            let _ = std::fs::create_dir_all(o);
        }
        let _ = std::fs::write(stil_pfad(), t);
    }
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// Python: die Umgebung vom WhatsApp-Export, wenn es sie gibt, sonst python3.
pub fn standard_python() -> String {
    let venv = home().join("WhatsApp-Export").join(".venv").join("bin").join("python3");
    if venv.is_file() { venv.to_string_lossy().into_owned() } else { "python3".into() }
}

/// Ein Befehlsskript im offenen Dokument ausführen (ein Rückgängig-Schritt).
pub fn skript(app: &mut DesignApp, text: String) -> Result<Value, String> {
    let r = app.run("script.run", json!({"text": text}))?;
    if let Some(f) = r.get("failed").filter(|f| !f.is_null()) {
        return Err(format!("Ein Schritt ist fehlgeschlagen: {f}"));
    }
    Ok(r)
}

/// Das Jahresbuch-Fenster zeigen; `neu`: mit der Ansicht „Neuer Chat“, auch
/// wenn ein Chat markiert ist.
pub fn zeigen(app: &mut DesignApp, neu: bool) -> Result<Value, String> {
    app.jahresbuch.neu_anzeigen = neu;
    if app.ui.docking_flyout.as_deref() != Some(PANEL) {
        app.run("window.panel", json!({"panel": PANEL}))?;
    }
    Ok(Value::Null)
}

/// Befehl jahresbuch.chatImport: ohne Angaben das Fenster mit „Neuer Chat“; mit
/// `export` (Automatik) fügt es den Chat gleich ein und gibt ihn zurück (wie im Etikett).
/// `python` wählt das Programm, das das Hilfsprogramm ausführt (sonst die Einstellung im Fenster).
pub fn befehl_einfuegen(app: &mut DesignApp, p: &Value) -> Result<Value, String> {
    let Some(export) = p.get("export").and_then(Value::as_str) else { return zeigen(app, true) };
    app.jahresbuch.laden();
    let s = |k: &str| p.get(k).and_then(Value::as_str).map(str::to_string);
    let vorgabe = lies_stil();
    let chats = app.session.active().map(|st| modell::chats(&st.doc, &vorgabe)).unwrap_or_default();
    let wie = p.get("wie").and_then(Value::as_u64).and_then(|n| chats.iter().find(|c| u64::from(c.nummer) == n));
    let auftrag = einfuegen::Auftrag {
        export: export.to_string(),
        medien: s("medien").unwrap_or_else(|| app.jahresbuch.medien.clone()),
        chat: s("chat"),
        name: s("name").unwrap_or_default(),
        von: s("von").unwrap_or_default(),
        bis: s("bis").unwrap_or_default(),
        python: s("python").unwrap_or_else(|| app.jahresbuch.python.clone()),
        stil: wie.map_or(vorgabe.clone(), |c| c.stil.clone()),
        farben_wie: wie.map(|c| c.personen.clone()).unwrap_or_default(),
    };
    app.end_live_preview();
    let chat = einfuegen::einfuegen(app, &auftrag)?;
    serde_json::to_value(&chat).map_err(|e| e.to_string())
}

/// Befehl jahresbuch.chatDesign: ohne Angaben das Fenster (und die Chats des Dokuments); mit
/// `chat` (Nummer) ändert es Name, Gestaltung (`stil`, wie in stil.json, ergänzt) und Farben der
/// Personen (`personen`: {key: Farbe}) dieses Chats, als ein Rückgängig-Schritt.
pub fn befehl_gestalten(app: &mut DesignApp, p: &Value) -> Result<Value, String> {
    let vorgabe = lies_stil();
    let mut chats = app.session.active().map(|st| modell::chats(&st.doc, &vorgabe)).unwrap_or_default();
    let Some(n) = p.get("chat").and_then(Value::as_u64) else {
        zeigen(app, false)?;
        return serde_json::to_value(&chats).map(|c| json!({"chats": c})).map_err(|e| e.to_string());
    };
    let chat = chats.iter_mut().find(|c| u64::from(c.nummer) == n).ok_or_else(|| format!("Im Dokument gibt es keinen Chat {n}."))?;
    let alt = chat.clone();
    let im_etikett = app.session.active().is_some_and(|st| modell::im_etikett(&st.doc, alt.nummer));
    if let Some(name) = p.get("name").and_then(Value::as_str) {
        chat.name = name.to_string();
    }
    if let Some(stil) = p.get("stil").and_then(Value::as_object) {
        for (k, v) in stil {
            match (v.as_object(), chat.stil.get_mut(k).and_then(Value::as_object_mut)) {
                (Some(neu), Some(alt)) => alt.extend(neu.clone()),
                _ => {
                    chat.stil.insert(k.clone(), v.clone());
                }
            }
        }
    }
    if let Some(farben) = p.get("personen").and_then(Value::as_object) {
        for (key, farbe) in farben {
            let person = chat.personen.iter_mut().find(|x| &x.key == key).ok_or_else(|| format!("Im Chat gibt es keine Person „{key}“."))?;
            person.farbe = farbe.as_str().ok_or("Farben als Text, z. B. \"#E8493A\"")?.to_string();
        }
    }
    let chat = chat.clone();
    app.end_live_preview();
    let doc = app.session.active().map(|st| st.doc.clone()).ok_or("kein Dokument")?;
    let mut befehle = gestaltung::geaenderte_befehle(&doc, im_etikett.then_some(&alt), &chat);
    if let Some(st) = app.session.active() {
        gestaltung::schatten_umbauen(&doc, &chats, &st.selection, &mut befehle);
    }
    befehle.push(modell::speichern(&chats));
    skript(app, gestaltung::als_skript(&befehle))?;
    serde_json::to_value(&chat).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
