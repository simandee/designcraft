//! Zeitraum-Feld mit Kalender wie auf Buchungsseiten: der erste Klick setzt den Anfang, der
//! zweite das Ende; dazwischen liegt ein Band. ‹ › blättern die Monate, „Ganzer Monat“ und
//! „Ganzes Jahr“ wählen den gezeigten Monat oder sein Jahr.

use chrono::{Datelike, Days, Months, NaiveDate};
use egui::{Color32, Sense, Ui, vec2};

use crate::theme::Tokens;
use crate::widgets;

const MONATE: [&str; 12] = ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"];
const TAGE: [&str; 7] = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];

/// Was das Feld zwischen den Bildern behält: der gezeigte Monat (sein Erster) und ob der nächste
/// Klick das Ende setzt.
#[derive(Clone, Copy, Debug, Default)]
struct Blatt {
    monat: Option<NaiveDate>,
    wartet_auf_ende: bool,
}

/// Ein Datum als TT.MM.JJJJ.
pub fn kurz(d: NaiveDate) -> String {
    d.format("%d.%m.%Y").to_string()
}

/// Der Zeitraum als Text, z. B. „05.11.2026 – 18.11.2026“.
pub fn text(von: Option<NaiveDate>, bis: Option<NaiveDate>) -> String {
    match (von, bis) {
        (Some(v), Some(b)) => format!("{} – {}", kurz(v), kurz(b)),
        (Some(v), None) => format!("ab {}", kurz(v)),
        (None, Some(b)) => format!("bis {}", kurz(b)),
        (None, None) => "Ganzer Chat".into(),
    }
}

fn erster(d: NaiveDate) -> NaiveDate {
    d.with_day(1).unwrap_or(d)
}

fn letzter(d: NaiveDate) -> NaiveDate {
    erster(d).checked_add_months(Months::new(1)).and_then(|n| n.checked_sub_days(Days::new(1))).unwrap_or(d)
}

/// Ein Klick auf `tag`: Anfang (und das Ende offen) oder, wenn der Anfang steht, das Ende; liegt
/// es vor dem Anfang, tauschen beide.
fn klick(von: &mut Option<NaiveDate>, bis: &mut Option<NaiveDate>, wartet: &mut bool, tag: NaiveDate) {
    match (*wartet, *von) {
        (true, Some(v)) => {
            (*von, *bis) = if tag < v { (Some(tag), Some(v)) } else { (Some(v), Some(tag)) };
            *wartet = false;
        }
        _ => {
            (*von, *bis) = (Some(tag), None);
            *wartet = true;
        }
    }
}

/// Das Zeitraum-Feld, `w` breit. Gibt true zurück, wenn sich der Zeitraum geändert hat.
pub fn feld(ui: &mut Ui, id: &str, von: &mut Option<NaiveDate>, bis: &mut Option<NaiveDate>, w: f32) -> bool {
    let resp = widgets::dropdown(ui, &text(*von, *bis), w);
    let resp = resp.on_hover_text("Zeitraum wählen: erster Klick Anfang, zweiter Klick Ende");
    let key = ui.make_persistent_id(("jahresbuch-kalender", id));
    let mut blatt: Blatt = ui.ctx().data(|d| d.get_temp(key)).unwrap_or_default();
    let heute = von.or(*bis).unwrap_or_else(|| NaiveDate::from_ymd_opt(2026, 1, 1).unwrap_or_default());
    let mut monat = blatt.monat.unwrap_or_else(|| erster(heute));
    let mut geaendert = false;
    egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        let t = Tokens::get(ui.ctx());
        let cell = vec2(30.0, 24.0);
        ui.horizontal(|ui| {
            ui.set_width(cell.x * 7.0);
            if ui.small_button("‹").on_hover_text("Voriger Monat").clicked() {
                monat = monat.checked_sub_months(Months::new(1)).unwrap_or(monat);
            }
            let titel = format!("{} {}", MONATE[monat.month0() as usize], monat.year());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("›").on_hover_text("Nächster Monat").clicked() {
                    monat = monat.checked_add_months(Months::new(1)).unwrap_or(monat);
                }
                ui.centered_and_justified(|ui| ui.label(egui::RichText::new(titel).strong()));
            });
        });
        ui.add_space(2.0);
        let erster_tag = monat.weekday().num_days_from_monday() as usize;
        let tage = letzter(monat).day() as usize;
        let zeilen = (erster_tag + tage).div_ceil(7);
        let (r, _) = ui.allocate_exact_size(vec2(cell.x * 7.0, cell.y * (zeilen + 1) as f32), Sense::hover());
        for (i, d) in TAGE.iter().enumerate() {
            let c = r.min + vec2(cell.x * (i as f32 + 0.5), cell.y * 0.5);
            ui.painter().text(c, egui::Align2::CENTER_CENTER, *d, egui::FontId::proportional(10.0), t.text_dim);
        }
        // Während das Ende noch fehlt, zeigt das Band bis zum Tag unter dem Zeiger.
        let mut zeiger: Option<NaiveDate> = None;
        let mut zellen = Vec::with_capacity(tage);
        for n in 0..tage {
            let Some(tag) = monat.checked_add_days(Days::new(n as u64)) else { continue };
            let k = erster_tag + n;
            let cr = egui::Rect::from_min_size(r.min + vec2(cell.x * (k % 7) as f32, cell.y * (k / 7 + 1) as f32), cell);
            let resp = ui.interact(cr, key.with(("tag", n)), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, kurz(tag)));
            if resp.hovered() {
                zeiger = Some(tag);
            }
            if resp.clicked() {
                klick(von, bis, &mut blatt.wartet_auf_ende, tag);
                geaendert = true;
            }
            zellen.push((tag, cr, resp.hovered()));
        }
        let (a, b) = match (*von, *bis, blatt.wartet_auf_ende.then_some(zeiger).flatten()) {
            (Some(v), None, Some(z)) => (Some(v.min(z)), Some(v.max(z))),
            (v, b, _) => (v, b),
        };
        let band = t.accent_strong.gamma_multiply(0.35);
        let p = ui.painter();
        for (tag, cr, hover) in zellen {
            let ende = Some(tag) == a || Some(tag) == b;
            if let (Some(a), Some(b)) = (a, b) {
                let streifen = cr.shrink2(vec2(0.0, 2.0));
                if tag > a && tag < b {
                    p.rect_filled(streifen, 0.0, band);
                } else if tag == a && a < b {
                    p.rect_filled(egui::Rect::from_min_max(streifen.center_top(), streifen.right_bottom()), 0.0, band);
                } else if tag == b && a < b {
                    p.rect_filled(egui::Rect::from_min_max(streifen.left_top(), streifen.center_bottom()), 0.0, band);
                }
            }
            if ende {
                p.circle_filled(cr.center(), 11.0, t.accent_strong);
            } else if hover {
                p.circle_stroke(cr.center(), 11.0, egui::Stroke::new(1.0, t.accent_strong));
            }
            let farbe = if ende { Color32::WHITE } else { t.text };
            p.text(cr.center(), egui::Align2::CENTER_CENTER, tag.day().to_string(), egui::FontId::proportional(11.5), farbe);
        }
        ui.label(
            egui::RichText::new(if blatt.wartet_auf_ende { "Jetzt das Ende anklicken." } else { "Erster Klick: Anfang, zweiter Klick: Ende." })
                .size(10.5)
                .color(t.text_dim),
        );
        ui.horizontal(|ui| {
            if widgets::outline_button(ui, "Ganzer Monat", 96.0).clicked() {
                (*von, *bis) = (Some(monat), Some(letzter(monat)));
                blatt.wartet_auf_ende = false;
                geaendert = true;
            }
            if widgets::outline_button(ui, "Ganzes Jahr", 96.0).clicked() {
                (*von, *bis) = (NaiveDate::from_ymd_opt(monat.year(), 1, 1), NaiveDate::from_ymd_opt(monat.year(), 12, 31));
                blatt.wartet_auf_ende = false;
                geaendert = true;
            }
        });
    });
    blatt.monat = Some(monat);
    ui.ctx().data_mut(|d| d.insert_temp(key, blatt));
    geaendert
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Erster Klick Anfang, zweiter Ende; ein Ende vor dem Anfang tauscht beide; ein dritter Klick
    /// beginnt einen neuen Zeitraum.
    #[test]
    fn zwei_klicks_ergeben_einen_zeitraum() {
        let d = |t| NaiveDate::from_ymd_opt(2026, 11, t).unwrap();
        let (mut von, mut bis, mut wartet) = (None, None, false);
        klick(&mut von, &mut bis, &mut wartet, d(18));
        assert_eq!((von, bis), (Some(d(18)), None));
        klick(&mut von, &mut bis, &mut wartet, d(5));
        assert_eq!((von, bis), (Some(d(5)), Some(d(18))), "das frühere Datum wird der Anfang");
        klick(&mut von, &mut bis, &mut wartet, d(20));
        assert_eq!((von, bis), (Some(d(20)), None), "ein neuer Zeitraum beginnt");
        assert_eq!(letzter(d(5)), d(30));
    }
}
