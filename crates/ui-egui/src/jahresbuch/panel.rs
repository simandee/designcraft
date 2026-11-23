//! Das Jahresbuch-Fenster. Ist ein Chat markiert (sein Textrahmen, der Textcursor darin oder ein
//! Foto im Chat), zeigt es diesen Chat zum Gestalten; sonst „Neuer Chat“ zum Einfügen. Jede
//! Änderung wirkt sofort und ist ein Rückgängig-Schritt (ein Regler-Zug: einer).

use std::sync::{Arc, Mutex};

use chrono::NaiveDate;
use egui::{Color32, Sense, Ui, vec2};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::einfuegen::{self, Auftrag};
use super::gestaltung::{self as g, SCHRIFTEN};
use super::modell::{self, Chat};
use super::{kalender, lies_stil, merke, ordner, standard_python};
use crate::DesignApp;
use crate::app::file_dialogs::Bound;
use crate::theme::Tokens;
use crate::widgets::{self, NumField};

/// Schriftgrößen in der Liste neben den Feldern.
const GROESSEN: &[f64] = &[6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 18.0];
/// Breite der Beschriftungen vor den Feldern.
const LW: f32 = 110.0;

/// Ein Chat im Export, wie ihn `--liste` meldet.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ExportChat {
    pub chat: String,
    pub name: String,
    pub nachrichten: u64,
    #[serde(default)]
    pub von: String,
    #[serde(default)]
    pub bis: String,
    #[serde(default)]
    pub gruppe: bool,
}

/// Nachrichten und Fotos im gewählten Zeitraum, wie sie `--zaehlen` meldet.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Anzahl {
    pub nachrichten: u64,
    pub fotos: u64,
}

/// Eine Arbeit des Hilfsprogramms im Hintergrund (das Lesen eines großen Exports dauert), für
/// einen Schlüssel (was gefragt war); ändert er sich, beginnt sie neu.
pub struct Hintergrund<T> {
    schluessel: String,
    zelle: Option<Arc<Mutex<Option<Result<T, String>>>>>,
    wert: Option<Result<T, String>>,
}

impl<T> Default for Hintergrund<T> {
    fn default() -> Self {
        Self { schluessel: String::new(), zelle: None, wert: None }
    }
}

impl<T: Send + 'static> Hintergrund<T> {
    /// Das Ergebnis für `schluessel`, sobald es da ist; `arbeit` läuft dafür einmal im Hintergrund.
    fn holen(
        &mut self,
        ctx: &egui::Context,
        schluessel: String,
        arbeit: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Option<&Result<T, String>> {
        if self.schluessel != schluessel {
            self.schluessel = schluessel;
            self.wert = None;
            let zelle = Arc::new(Mutex::new(None));
            self.zelle = Some(zelle.clone());
            starten(ctx.clone(), move || {
                let r = arbeit();
                if let Ok(mut z) = zelle.lock() {
                    *z = Some(r);
                }
            });
        }
        if let Some(r) = self.zelle.as_ref().and_then(|z| z.lock().ok().and_then(|mut g| g.take())) {
            self.wert = Some(r);
            self.zelle = None;
        }
        self.wert.as_ref()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn starten(ctx: egui::Context, f: impl FnOnce() + Send + 'static) {
    let _ = std::thread::Builder::new().name("jahresbuch".into()).spawn(move || {
        f();
        ctx.request_repaint();
    });
}

/// Im Browser gibt es kein Hilfsprogramm: die Arbeit meldet das sofort.
#[cfg(target_arch = "wasm32")]
fn starten(_ctx: egui::Context, f: impl FnOnce() + Send + 'static) {
    f();
}

/// Eine Arbeit, die das Programm anhält (Hilfsprogramm und Skripte): sie läuft ein Bild nach dem
/// Klick, damit „wird aufbereitet …“ vorher im Fenster steht.
enum Arbeit {
    Einfuegen(Auftrag),
    Weiter(Auftrag, Box<Chat>, einfuegen::Wohin),
}

/// Was das Fenster sich merkt.
#[derive(Default)]
pub struct Zustand {
    /// „Neuen Chat einfügen …“ gewählt: die Ansicht „Neuer Chat“ auch bei markiertem Chat, bis
    /// sich die Auswahl ändert.
    pub neu_anzeigen: bool,
    zuletzt_markiert: Option<u32>,
    /// Die Felder sind aus ~/Jahresbuch/stil.json gefüllt.
    geladen: bool,
    vorgabe: Map<String, Value>,
    pub export: String,
    pub medien: String,
    pub python: String,
    /// Kennung des gewählten Chats im Export (None: der erste der Liste).
    pub chat: Option<String>,
    pub von: Option<NaiveDate>,
    pub bis: Option<NaiveDate>,
    /// Aussehen wie Chat Nr. … (None: die Vorgabe aus stil.json).
    pub aussehen: Option<u32>,
    liste: Hintergrund<Vec<ExportChat>>,
    anzahl: Hintergrund<Anzahl>,
    /// Die Meldung des letzten Einfügens, das nicht geklappt hat.
    fehler: Option<String>,
    /// Name des Chats, während er im Feld bearbeitet wird: (Nummer, Text).
    name: Option<(u32, String)>,
    /// Fortsetzen oder Neu aufbauen ist aufgeklappt: (Chat, neu aufbauen?, Zeitraum).
    weiter: Option<(u32, bool, Option<NaiveDate>, Option<NaiveDate>)>,
    /// Die bestellte Arbeit und das Bild (egui-Durchgang), in dem sie bestellt wurde.
    arbeit: Option<(Arbeit, u64)>,
}

/// Die Arbeit für das nächste Bild bestellen.
fn bestellen(app: &mut DesignApp, ui: &Ui, arbeit: Arbeit) {
    app.jahresbuch.arbeit = Some((arbeit, ui.ctx().cumulative_pass_nr()));
    app.jahresbuch.fehler = None;
    ui.ctx().request_repaint();
}

/// Der Hinweis, solange eine Arbeit bestellt ist.
fn wird_aufbereitet(app: &DesignApp, ui: &mut Ui) {
    if app.jahresbuch.arbeit.is_some() {
        klein(ui, "⏳ Chat wird aufbereitet … Bei vielen Nachrichten dauert das eine Weile.");
    }
}

/// Die bestellte Arbeit ausführen, sobald das Bild mit dem Hinweis gezeigt ist. War das Fenster
/// länger zu, verfällt sie (sie liefe sonst unerwartet später).
fn erledigen(app: &mut DesignApp, ui: &Ui) {
    let jetzt = ui.ctx().cumulative_pass_nr();
    let Some((arbeit, bestellt)) = app.jahresbuch.arbeit.take_if(|(_, b)| jetzt > *b) else { return };
    if jetzt > bestellt + 3 {
        return;
    }
    app.end_live_preview();
    let ergebnis = match &arbeit {
        Arbeit::Einfuegen(auftrag) => einfuegen::einfuegen(app, auftrag).map(|_| {
            app.jahresbuch.neu_anzeigen = false;
            merke(&[
                ("export", json!(auftrag.export)),
                ("medien", json!(auftrag.medien)),
                ("python", json!(auftrag.python)),
                ("von", json!(auftrag.von)),
                ("bis", json!(auftrag.bis)),
            ]);
        }),
        Arbeit::Weiter(auftrag, chat, wohin) => einfuegen::fortsetzen(app, auftrag, chat, *wohin).map(|_| app.jahresbuch.weiter = None),
    };
    if let Err(e) = ergebnis {
        app.status(e.clone());
        app.jahresbuch.fehler = Some(e);
    }
}

impl Zustand {
    pub(super) fn laden(&mut self) {
        if self.geladen {
            return;
        }
        self.geladen = true;
        self.vorgabe = lies_stil();
        let s = |k: &str| self.vorgabe.get(k).and_then(Value::as_str).map(str::to_string);
        let home = super::home();
        self.export =
            s("export").unwrap_or_else(|| home.join("WhatsApp-Export/ausgabe/euer_chat_mit_reaktionen.json").to_string_lossy().into_owned());
        self.medien = s("medien").unwrap_or_else(|| home.join("WhatsApp-Export/ausgabe").to_string_lossy().into_owned());
        self.python = s("python").unwrap_or_else(standard_python);
        let datum = |k: &str| s(k).and_then(|t| NaiveDate::parse_from_str(&t, "%Y-%m-%d").ok());
        (self.von, self.bis) = (datum("von"), datum("bis"));
    }
}

fn datum_text(d: Option<NaiveDate>) -> String {
    d.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default()
}

/// Das Hilfsprogramm mit `args` aufrufen und seine Ausgabe (JSON) lesen.
fn hilfsprogramm<T: serde::de::DeserializeOwned>(python: &str, args: &[String]) -> Result<T, String> {
    let out = std::process::Command::new(python)
        .arg(ordner().join("designcraft_import.py"))
        .args(args)
        .output()
        .map_err(|e| format!("Python konnte nicht gestartet werden ({python}): {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.lines().last().unwrap_or("Fehler im Hilfsprogramm").to_string());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

fn klein(ui: &mut Ui, s: &str) {
    ui.label(egui::RichText::new(s).size(10.5).color(Tokens::get(ui.ctx()).text_dim));
}

/// Eine Zeile: Beschriftung rechtsbündig in `lw`, dann das Feld.
fn zeile<R>(ui: &mut Ui, label: &str, lw: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        widgets::caption_w(ui, label, lw);
        ui.add_space(4.0);
        add(ui)
    })
    .inner
}

/// Einklappbarer Abschnitt mit Dreieck (wie im Fenster Typografie).
fn klapp(ui: &mut Ui, title: &str, open: bool, body: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(egui::RichText::new(title).size(11.5).color(Tokens::get(ui.ctx()).text_strong))
        .id_salt(("jahresbuch", title))
        .default_open(open)
        .show(ui, body);
}

fn hex_farbe(s: &str) -> Option<Color32> {
    let h = s.trim().trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    (h.len() == 6).then_some(())?;
    Some(Color32::from_rgb(p(0)?, p(2)?, p(4)?))
}

fn farbe_hex(c: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

/// Ein Farbfeld, das beim Klick einen Farbwähler aufklappt. Gibt die neue Farbe zurück.
fn farbfeld(ui: &mut Ui, id: &str, hex: &str, name: &str) -> Option<String> {
    let mut c = hex_farbe(hex).unwrap_or(Color32::BLACK);
    let r = widgets::swatch_chip(ui, 16.0, Some(c), None).on_hover_text("Farbe ändern");
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("{name}: Farbe {hex}")));
    let mut neu = None;
    egui::Popup::menu(&r).id(ui.make_persistent_id(("farbe", id))).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque) {
            neu = Some(farbe_hex(c));
        }
    });
    neu
}

/// Prozentfeld (0 … 1 als 0 … 100 %) mit Pfeil daneben, der einen Regler aufklappt (wie Deckkraft
/// im Eigenschaften-Fenster). Gibt den neuen Wert zurück.
fn prozent(ui: &mut Ui, id: &str, name: &str, v: f64, enabled: bool) -> Option<f64> {
    let t = Tokens::get(ui.ctx());
    let mut neu = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if let Some(x) = NumField::number(id, Some(v * 100.0), "%", 0).name(name).width(51.0).range(0.0, 100.0).enabled(enabled).show(ui) {
            neu = Some(x / 100.0);
        }
        let (r, resp) = ui.allocate_exact_size(vec2(18.0, widgets::FIELD_H), if enabled { Sense::click() } else { Sense::hover() });
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, format!("{name}: Regler")));
        let p = ui.painter();
        p.rect(r, 0.0, if resp.hovered() && enabled { t.hover } else { t.input }, egui::Stroke::new(1.0, t.field_border), egui::StrokeKind::Inside);
        crate::icons::paint(p, r.shrink(4.0), "chevron-right", if enabled { t.icon } else { t.text_disabled });
        egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            ui.spacing_mut().slider_width = 130.0;
            ui.spacing_mut().slider_rail_height = 2.0;
            let mut x = v * 100.0;
            let s = egui::Slider::new(&mut x, 0.0..=100.0)
                .integer()
                .show_value(false)
                .handle_shape(egui::style::HandleShape::Circle)
                .trailing_fill(false);
            if ui.add(s).changed() {
                neu = Some(x / 100.0);
            }
        });
    });
    neu
}

/// Das Fenster zeichnen.
pub fn show(app: &mut DesignApp, ui: &mut Ui) {
    app.jahresbuch.laden();
    erledigen(app, ui);
    let Some(st) = app.session.active() else {
        widgets::selection_band(ui, "Chat");
        klein(ui, "Zuerst das Buch öffnen (oder ein neues Dokument anlegen).");
        return;
    };
    let doc = st.doc.clone();
    let chats = modell::chats(&doc, &app.jahresbuch.vorgabe);
    let markiert_text = modell::markierter_chat(&doc, &st.selection);
    let markiert = markiert_text.map(|(n, _)| n);
    let ziel = leerer_rahmen(&doc, &st.selection);
    let z = &mut app.jahresbuch;
    if markiert != z.zuletzt_markiert {
        z.zuletzt_markiert = markiert;
        z.neu_anzeigen = false;
    }
    let chat = markiert.filter(|_| !z.neu_anzeigen).and_then(|n| chats.iter().find(|c| c.nummer == n)).cloned();
    match chat {
        Some(chat) => {
            let story = modell::text_des_chats(&doc, &chat, markiert_text.map(|(_, sid)| sid)).map_or(0, |sid| sid.0);
            chat_ansicht(app, ui, chat, &chats, story);
        }
        None => neuer_chat(app, ui, &chats, ziel),
    }
    // Ein Regler oder Farbwähler wird noch gezogen: die Vorschau bleibt offen (ein Schritt).
    if ui.input(|i| i.pointer.primary_down()) {
        app.hold_live_preview();
    }
}

/// Wohin ein neuer Chat käme: Some(true) in den markierten leeren Textrahmen, Some(false) auf neue
/// Seiten am Ende, None: auch auf neue Seiten, weil der markierte Textrahmen schon Text hat.
fn leerer_rahmen(doc: &designcraft_doc::Document, sel: &designcraft_doc::Selection) -> Option<bool> {
    for id in &sel.items {
        if let Some(tf) = doc.item(*id).and_then(|i| i.text_frame()) {
            return doc.story(tf.story).is_none_or(|s| s.is_empty()).then_some(true);
        }
    }
    Some(false)
}

// ------------------------------------------------------------------------------- Neuer Chat

fn pfad_feld(app: &mut DesignApp, ui: &mut Ui, ordner: bool) {
    let fw = widgets::full_width(ui);
    ui.horizontal(|ui| {
        let wert = if ordner { &mut app.jahresbuch.medien } else { &mut app.jahresbuch.export };
        ui.add(egui::TextEdit::singleline(wert).desired_width(fw - 72.0).font(egui::FontId::proportional(11.0)));
        if widgets::outline_button(ui, "Wählen …", 64.0).clicked() {
            let setze = move |app: &mut DesignApp, pfad: String| {
                if ordner {
                    app.jahresbuch.medien = pfad
                } else {
                    app.jahresbuch.export = pfad
                }
                Ok(Value::Null)
            };
            let r = if ordner { app.ask_folder("Medienordner wählen", Bound::App, setze) } else { app.ask_open("chatExport", Bound::App, setze) };
            if let Err(e) = r {
                app.status(e);
            }
        }
    });
}

fn neuer_chat(app: &mut DesignApp, ui: &mut Ui, chats: &[Chat], ziel: Option<bool>) {
    let fw = widgets::full_width(ui);
    widgets::selection_band(ui, "Neuer Chat");
    widgets::section(ui, "WhatsApp-Export", true);
    pfad_feld(app, ui, false);
    ui.add_space(4.0);
    widgets::caption(ui, "Medienordner");
    pfad_feld(app, ui, true);
    ui.add_space(4.0);
    widgets::caption(ui, "Chat");
    let ctx = ui.ctx().clone();
    let z = &mut app.jahresbuch;
    let export_da = std::path::Path::new(&z.export).is_file();
    let (python, export) = (z.python.clone(), z.export.clone());
    let liste =
        if export_da { z.liste.holen(&ctx, export.clone(), move || hilfsprogramm(&python, &["--liste".into(), export])).cloned() } else { None };
    let mut gewaehlt: Option<ExportChat> = None;
    match (&liste, export_da) {
        (_, false) => klein(ui, "Den WhatsApp-Export (JSON) wählen."),
        (None, true) => klein(ui, "Chats werden gelesen …"),
        (Some(Err(e)), true) => klein(ui, &format!("Der Export lässt sich nicht lesen: {e}")),
        (Some(Ok(l)), true) => {
            let jetzt = z.chat.as_ref().and_then(|c| l.iter().position(|x| &x.chat == c)).unwrap_or(0);
            let namen: Vec<String> =
                l.iter().map(|c| format!("{} ({} Nachrichten{})", c.name, c.nachrichten, if c.gruppe { ", Gruppe" } else { "" })).collect();
            if let Some(i) = widgets::dropdown_list(ui, namen.get(jetzt).map_or("", String::as_str), fw, &namen, Some(jetzt)) {
                z.chat = l.get(i).map(|c| c.chat.clone());
            }
            gewaehlt = l.get(jetzt).cloned();
        }
    }
    widgets::rule(ui);
    widgets::section(ui, "Zeitraum", true);
    kalender::feld(ui, "neu", &mut z.von, &mut z.bis, fw);
    if let Some(c) = &gewaehlt {
        let (python, export, chat, von, bis) = (z.python.clone(), z.export.clone(), c.chat.clone(), datum_text(z.von), datum_text(z.bis));
        let schluessel = format!("{export}\n{chat}\n{von}\n{bis}");
        let anzahl = z.anzahl.holen(&ctx, schluessel, move || hilfsprogramm(&python, &["--zaehlen".into(), export, chat, von, bis]));
        match anzahl {
            Some(Ok(a)) => klein(ui, &format!("{} Nachrichten, {} Fotos und Videos", a.nachrichten, a.fotos)),
            Some(Err(e)) => klein(ui, e),
            None => klein(ui, "Wird gezählt …"),
        }
        klein(ui, &format!("Im Export: {} bis {}", c.von, c.bis));
    }
    widgets::rule(ui);
    widgets::section(ui, "Aussehen", true);
    let mut namen = vec!["Standard".to_string()];
    namen.extend(chats.iter().map(|c| format!("Wie Chat „{}“", c.name)));
    let jetzt = z.aussehen.and_then(|n| chats.iter().position(|c| c.nummer == n)).map_or(0, |i| i + 1);
    if let Some(i) = widgets::dropdown_list(ui, &namen[jetzt], fw, &namen, Some(jetzt)) {
        z.aussehen = i.checked_sub(1).and_then(|i| chats.get(i)).map(|c| c.nummer);
    }
    klein(ui, "Später hier änderbar, sobald der Chat markiert ist.");
    widgets::rule(ui);
    widgets::section(ui, "Ziel", true);
    match ziel {
        Some(true) => widgets::caption(ui, "Der markierte Textrahmen"),
        Some(false) => {
            widgets::caption(ui, "Neue Chat-Seiten am Ende des Dokuments");
            klein(ui, "Tipp: Erst einen leeren Textrahmen markieren, dann kommt der Chat dort hinein.");
        }
        None => {
            widgets::caption(ui, "Neue Chat-Seiten am Ende des Dokuments");
            klein(ui, "Der markierte Textrahmen hat schon Text; der bleibt, wie er ist.");
        }
    }
    egui::CollapsingHeader::new(egui::RichText::new("Erweitert").size(11.0)).id_salt("jahresbuch-erweitert").show(ui, |ui| {
        widgets::caption(ui, "Python");
        ui.add(egui::TextEdit::singleline(&mut z.python).desired_width(fw).font(egui::FontId::proportional(11.0)));
    });
    ui.add_space(10.0);
    let bereit = export_da && gewaehlt.is_some();
    if knopf(ui, "Chat einfügen", fw, bereit).clicked()
        && let Some(c) = gewaehlt
    {
        let z = &app.jahresbuch;
        let wie = z.aussehen.and_then(|n| chats.iter().find(|c| c.nummer == n));
        let auftrag = Auftrag {
            export: z.export.clone(),
            medien: z.medien.clone(),
            chat: Some(c.chat.clone()),
            name: c.name.clone(),
            von: datum_text(z.von),
            bis: datum_text(z.bis),
            python: z.python.clone(),
            stil: wie.map_or_else(|| z.vorgabe.clone(), |c| c.stil.clone()),
            farben_wie: wie.map(|c| c.personen.clone()).unwrap_or_default(),
        };
        bestellen(app, ui, Arbeit::Einfuegen(auftrag));
    }
    wird_aufbereitet(app, ui);
    if let Some(e) = &app.jahresbuch.fehler {
        ui.add_space(4.0);
        let farbe = ui.visuals().error_fg_color;
        ui.label(egui::RichText::new(format!("⚠ {e}")).size(10.5).color(farbe));
    }
    ui.add_space(8.0);
}

/// Der Hauptknopf: gefüllt in der Akzentfarbe.
fn knopf(ui: &mut Ui, label: &str, w: f32, enabled: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(w, 26.0), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    let fill = if !enabled {
        t.well
    } else if resp.hovered() {
        t.accent
    } else {
        t.accent_strong
    };
    ui.painter().rect_filled(r, 3.0, fill);
    let farbe = if enabled { Color32::WHITE } else { t.text_disabled };
    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, crate::theme::semibold(12.0), farbe);
    resp
}

// ------------------------------------------------------------------------------- Chat gestalten

/// Die geänderte Gestaltung von `chat` zeigen (live; der Schritt endet, wenn der Regler oder das
/// Feld losgelassen ist).
pub(super) fn anwenden(app: &mut DesignApp, alle: &[Chat], chat: &Chat) {
    let Some(st) = app.session.active() else { return };
    // Eine laufende Vorschau spielt ihr Skript immer auf dem Stand davor ab.
    let basis = st.interaction.as_ref().map_or_else(|| st.doc.clone(), |i| i.doc.clone());
    // Nur Geändertes; ein Chat von vor dem Etikett bekommt einmal alles (bringt ältere Formate
    // auf den Stand, etwa die Blasenecken).
    let alt = modell::chats(&basis, &app.jahresbuch.vorgabe)
        .into_iter()
        .find(|c| c.nummer == chat.nummer)
        .filter(|_| modell::im_etikett(&basis, chat.nummer));
    let mut befehle = g::geaenderte_befehle(&basis, alt.as_ref(), chat);
    let auswahl = st.interaction.as_ref().map_or_else(|| st.selection.clone(), |i| i.selection.clone());
    g::schatten_umbauen(&basis, alle, &auswahl, &mut befehle);
    let neu: Vec<Chat> = alle.iter().map(|c| if c.nummer == chat.nummer { chat.clone() } else { c.clone() }).collect();
    befehle.push(modell::speichern(&neu));
    app.live_preview("Chat-Gestaltung", "script.run", json!({"text": g::als_skript(&befehle)}));
}

/// Kennung eines Zahlenfelds von `chat`: je Chat eine eigene, damit ein getippter, noch nicht
/// bestätigter Wert nicht im Chat landet, der als Nächstes angeklickt wird.
fn feld_id(chat: &Chat, pfad: &str) -> String {
    format!("{}-{pfad}", chat.nummer)
}

fn chat_ansicht(app: &mut DesignApp, ui: &mut Ui, mut chat: Chat, alle: &[Chat], story: u64) {
    let fw = widgets::full_width(ui);
    let vorher = chat.clone();
    widgets::selection_band(ui, "Chat");
    // Name: übernommen, wenn das Feld verlassen wird.
    zeile(ui, "Name", 40.0, |ui| {
        let z = &mut app.jahresbuch;
        let id = ui.make_persistent_id(("jahresbuch-name", chat.nummer));
        if z.name.as_ref().is_none_or(|(n, _)| *n != chat.nummer) {
            z.name = Some((chat.nummer, chat.name.clone()));
        }
        if let Some((_, text)) = z.name.as_mut() {
            let r = ui.add(egui::TextEdit::singleline(text).id(id).desired_width(fw - 48.0));
            if r.lost_focus() && text.trim() != chat.name && !text.trim().is_empty() {
                chat.name = text.trim().to_string();
            } else if !r.has_focus() {
                // Außerhalb der Bearbeitung zeigt das Feld den Namen im Dokument (auch nach Rückgängig).
                *text = chat.name.clone();
            }
        }
    });
    let mut info = Vec::new();
    if let (v, b @ Some(_)) | (v @ Some(_), b) =
        (NaiveDate::parse_from_str(&chat.von, "%Y-%m-%d").ok(), NaiveDate::parse_from_str(&chat.bis, "%Y-%m-%d").ok())
    {
        info.push(kalender::text(v, b));
    }
    if !chat.personen.is_empty() {
        info.push(format!("{} Personen", chat.personen.len()));
    }
    klein(ui, &info.join(" · "));

    klapp(ui, "Personen", true, |ui| {
        for p in chat.personen.iter_mut() {
            ui.horizontal(|ui| {
                if let Some(f) = farbfeld(ui, &format!("{}-{}", vorher.nummer, p.key), &p.farbe, &p.name) {
                    p.farbe = f;
                }
                ui.add_space(4.0);
                widgets::caption(ui, &p.name);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    klein(ui, if p.key == "Ich" { "rechts" } else { "links" });
                });
            });
        }
        klein(ui, "Klick aufs Farbfeld: Farbe ändern. Sie färbt Blasen, Schatten und Namen.");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for &(pfad, name, standard) in g::chat_farben(&vorher) {
                let (label, beschreibung) =
                    if name == g::AKZENT.1 { ("Akzent", "Akzent: Datum, Verzierungen und Schatten der Spalten-Box") } else { (name, name) };
                if let Some(f) = farbfeld(ui, &format!("{}-{name}", vorher.nummer), &chat.text(pfad, standard), beschreibung) {
                    chat.setze(pfad, json!(f));
                }
                widgets::caption(ui, label);
                ui.add_space(16.0);
            }
        });
    });

    klapp(ui, "Schrift", true, |ui| {
        let schriften: Vec<String> = SCHRIFTEN.iter().map(|s| s.to_string()).collect();
        let jetzt = chat.text("schrift_chat", SCHRIFTEN[0]);
        if let Some(i) = widgets::dropdown_list(ui, &jetzt, widgets::full_width(ui), &schriften, schriften.iter().position(|s| *s == jetzt)) {
            chat.setze("schrift_chat", json!(schriften[i]));
        }
        ui.add_space(4.0);
        for ((pfad, standard), label) in [
            (g::SCHRIFT_TEXT, "Nachrichtentext"),
            (g::SCHRIFT_ZEIT, "Uhrzeit"),
            (g::SCHRIFT_DATUM, "Datum"),
            (g::SCHRIFT_ZITAT, "Zitat und Hinweise"),
        ] {
            let (v, id) = (chat.zahl(pfad, standard), feld_id(&vorher, pfad));
            if let Some(n) = zeile(ui, label, LW, |ui| {
                NumField::points(&id, Some(v), 2).name(label).width(86.0).spinner().step(0.1).range(4.0, 24.0).presets(GROESSEN).show(ui)
            }) {
                chat.setze(pfad, json!(n));
            }
        }
    });

    klapp(ui, "Abstände", false, |ui| {
        for ((pfad, standard), label) in [
            (g::ABSTAND_VOR_DATUM, "Vor dem Datum"),
            (g::ABSTAND_DATUM, "Nach dem Datum"),
            (g::ABSTAND_GLEICH, "Gleiche Person"),
            (g::ABSTAND_WECHSEL, "Personenwechsel"),
        ] {
            let (v, id) = (chat.zahl(pfad, standard), feld_id(&vorher, pfad));
            if let Some(n) =
                zeile(ui, label, LW, |ui| NumField::points(&id, Some(v), 1).name(label).width(86.0).spinner().step(0.5).range(0.0, 72.0).show(ui))
            {
                chat.setze(pfad, json!(n));
            }
        }
    });

    klapp(ui, "Korn und Rauschen", true, |ui| {
        let regler = |ui: &mut Ui, chat: &mut Chat, label: &str, (pfad, standard): (&str, f64), an: bool| {
            if let Some(v) = zeile(ui, label, LW, |ui| prozent(ui, &feld_id(chat, pfad), label, chat.zahl(pfad, standard), an)) {
                chat.setze(pfad, json!((v * 100.0).round() / 100.0));
            }
        };
        klein(ui, "Blasen");
        regler(ui, &mut chat, "Korn", g::KORN_BLASEN, true);
        let mut an = chat.wert(g::RAUSCHEN_BLASEN_AN).and_then(Value::as_bool).unwrap_or(false);
        let mut neu_an = None;
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(vec2(LW, widgets::FIELD_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.set_min_width(LW);
                if widgets::checkbox(ui, &mut an, "Rauschen").changed() {
                    neu_an = Some(an);
                }
            });
            ui.add_space(4.0);
            let (pfad, standard) = g::RAUSCHEN_BLASEN;
            if let Some(v) = prozent(ui, &feld_id(&vorher, pfad), "Rauschen auf den Blasen", chat.zahl(pfad, standard), an) {
                chat.setze(pfad, json!((v * 100.0).round() / 100.0));
            }
        });
        if let Some(an) = neu_an {
            chat.setze(g::RAUSCHEN_BLASEN_AN, json!(an));
        }
        klein(ui, "Datum");
        regler(ui, &mut chat, "Korn", g::KORN_DATUM, true);
        klein(ui, "Spalten-Box");
        regler(ui, &mut chat, "Rauschen Fläche", g::RAUSCHEN_INNEN, true);
        regler(ui, &mut chat, "Rauschen Schatten", g::RAUSCHEN_RAHMEN, true);
        regler(ui, &mut chat, "Sprenkel Schatten", g::SPRENKEL_RAHMEN, true);
    });
    klapp(ui, "Chat", true, |ui| weiter_bereich(app, ui, &vorher, story));
    ui.add_space(8.0);

    if chat != vorher {
        anwenden(app, alle, &chat);
    }
}

/// Fortsetzen (neue Nachrichten hinten anhängen) und Neu aufbauen (die Tage eines Zeitraums neu
/// setzen, der Rest bleibt), jeweils mit Zeitraum und Bestätigung.
fn weiter_bereich(app: &mut DesignApp, ui: &mut Ui, chat: &Chat, story: u64) {
    let fw = widgets::full_width(ui);
    let datum = |t: &str| NaiveDate::parse_from_str(t, "%Y-%m-%d").ok();
    let offen = app.jahresbuch.weiter.filter(|w| w.0 == chat.nummer);
    if offen.is_none() {
        let half = (fw - 10.0) / 2.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            if widgets::outline_button(ui, "Fortsetzen …", half).on_hover_text("Neue Nachrichten hinten anhängen").clicked() {
                // Ab dem Tag nach dem Ende bis zum Ende jenes Monats.
                let ab = datum(&chat.bis).and_then(|b| b.succ_opt());
                let bis = ab
                    .and_then(|a| a.checked_add_months(chrono::Months::new(1)))
                    .and_then(|m| chrono::Datelike::with_day(&m, 1))
                    .and_then(|m| m.pred_opt());
                app.jahresbuch.weiter = Some((chat.nummer, false, ab, bis));
            }
            if widgets::outline_button(ui, "Neu aufbauen …", half).on_hover_text("Den Text dieses Chats neu setzen").clicked() {
                app.jahresbuch.weiter = Some((chat.nummer, true, datum(&chat.von), datum(&chat.bis)));
            }
        });
        return;
    }
    let Some((_, ersetzen, mut von, mut bis)) = offen else { return };
    if ersetzen {
        ui.label(
            egui::RichText::new("⚠ Die Tage im gewählten Zeitraum werden neu gesetzt; eigene Änderungen dort gehen verloren. Was davor und danach steht, bleibt, ebenso Name, Farben und Gestaltung.")
                .size(10.5)
                .color(ui.visuals().warn_fg_color),
        );
    } else {
        klein(ui, "Neue Nachrichten kommen hinten an den Chat. Was schon da ist, bleibt, wie es ist.");
    }
    widgets::caption(ui, "Zeitraum");
    kalender::feld(ui, &format!("weiter-{}", chat.nummer), &mut von, &mut bis, fw);
    app.jahresbuch.weiter = Some((chat.nummer, ersetzen, von, bis));
    // Fortsetzen braucht einen Anfang nach dem Ende des Chats, sonst stünden Tage doppelt im Buch.
    let ende = datum(&chat.bis);
    let hindernis = match (ersetzen, von, ende) {
        (true, ..) => None,
        (false, None, _) => Some("Ab wann? Bitte im Kalender den ersten neuen Tag wählen.".to_string()),
        (false, Some(v), Some(e)) if v <= e => {
            Some(format!("Der Chat reicht schon bis {}. Bitte einen späteren Anfang wählen (oder „Neu aufbauen“).", kalender::kurz(e)))
        }
        _ => None,
    };
    if let Some(h) = &hindernis {
        ui.label(egui::RichText::new(format!("⚠ {h}")).size(10.5).color(ui.visuals().warn_fg_color));
    }
    ui.add_space(4.0);
    let (ok, abbrechen) = widgets::button_pair(ui, if ersetzen { "Neu aufbauen" } else { "Anhängen" }, "Abbrechen");
    if abbrechen {
        app.jahresbuch.weiter = None;
    }
    if ok && hindernis.is_none() {
        let z = &app.jahresbuch;
        let oder = |a: &str, b: &str| if a.is_empty() { b.to_string() } else { a.to_string() };
        let auftrag = Auftrag {
            export: oder(&chat.export, &z.export),
            medien: oder(&chat.medien, &z.medien),
            chat: (!chat.chat.is_empty()).then(|| chat.chat.clone()),
            name: chat.name.clone(),
            von: datum_text(von),
            bis: datum_text(bis),
            python: z.python.clone(),
            stil: chat.stil.clone(),
            farben_wie: Vec::new(),
        };
        let wohin = if ersetzen { einfuegen::Wohin::Ersetzen(story) } else { einfuegen::Wohin::Anhaengen(story) };
        bestellen(app, ui, Arbeit::Weiter(auftrag, Box::new(chat.clone()), wohin));
    }
    wird_aufbereitet(app, ui);
    if let Some(e) = &app.jahresbuch.fehler {
        let farbe = ui.visuals().error_fg_color;
        ui.label(egui::RichText::new(format!("⚠ {e}")).size(10.5).color(farbe));
    }
}
