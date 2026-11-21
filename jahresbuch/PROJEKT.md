# Jahresbuch – Projektstand und Übergabe

Diese Datei fasst alles zusammen, was in früheren Sessions besprochen und gebaut wurde.
**Neue Session: diese Datei zuerst vollständig lesen.** Stand: 21.11.2026.

## Wer und wie

- Der User macht jedes Jahr mit seiner Partnerin ein gedrucktes **Jahresbuch**
  über das vergangene Jahr: WhatsApp-Chats im WhatsApp-Look, viele Foto-Seiten, ausformulierte
  Geschichten, sonstiger Quatsch. Früher InDesign (davor LaTeX), jetzt **DesignCraft** (ArtCraft).
- **Kommunikation:** Deutsch, einfache Sprache, **keine Programmier-Vorkenntnisse** voraussetzen
  (Begriffe wie Fork, Branch, Repo erklären oder vermeiden). Anleitungen Schritt für Schritt,
  Befehle zum Kopieren, nach jedem Schritt Screenshot abwarten.
- **Ein Thema nach dem anderen.** Bei Unklarheiten **erst fragen, dann erzeugen** (hat sich mehrfach
  über falsch verstandene Wünsche geärgert). Bei Gestaltungsfragen gern Varianten nebeneinander zeigen.
- **Keine Arbeit in andere Sessions auslagern** (außer der User will es ausdrücklich).
- Rechner: **MacBook Pro mit Intel-Prozessor** (x86_64). Ein Neubau von DesignCraft dauert dort
  ~7–20 Min., ein `git pull` Sekunden. Immer dazusagen, ob nur `git pull` oder auch Neubau nötig ist.
- Privat: Chat-Inhalte, Namen, Nummern nie in Websuchen oder externe Dienste. Die Chat-Daten liegen
  nur auf dem Mac, **nicht** in diesem Repository.

## Wo was liegt

| Ort | Inhalt |
|---|---|
| GitHub `simandee/designcraft`, Branch **`absatz-schatten`** | Fork von `storytold/designcraft` mit unseren Erweiterungen + Ordner `jahresbuch/` |
| Mac `~/DesignCraft-Jahresbuch` | Klon davon; gebaut mit `cargo build --release -p designcraft -p designcraft-cli` |
| Mac `~/DesignCraft-Jahresbuch/target/release/designcraft` | **seine** DesignCraft-Version (das installierte offizielle DesignCraft kann unsere Dateien nicht öffnen) |
| Mac `~/WhatsApp-Export/ausgabe/euer_chat_mit_reaktionen.json` | WhatsApp-Export (45.546 Nachrichten, alle Jahre). Medien unter `ausgabe/AppDomainGroup-group.net.whatsapp.WhatsApp.shared/Message/Media/…` (Pfad = `ausgabe/` + `media_base` + `data`); das Hilfsprogramm sucht sie rekursiv im Medienordner `~/WhatsApp-Export/ausgabe` |
| Mac `~/WhatsApp-Export/.venv` | Python-Umgebung (pillow, emoji, WhatsApp-Chat-Exporter) |
| Mac `~/Jahresbuch/` | Arbeitsordner, Einstellungen `stil.json` des Jahresbuch-Menüs, Test-Dateien |
| Schriften | Space Grotesk, Fraunces, Work Sans in `~/Library/Fonts` (Google Fonts) |

Git: Remote `origin` = Fork, `upstream` = `storytold/designcraft`. Commits mit dem neutralen Namen „Jahresbuch“ (kein echter Name: das Repository ist öffentlich).

## Daten (WhatsApp-Export)

Erzeugt aus einem unverschlüsselten Finder-Backup mit WhatsApp-Chat-Exporter 0.13.1 plus eigenem
Skript für Reaktionen. JSON: `{ chat-jid: { messages: { id: {...} } } }`, nach `timestamp` sortieren.
Wichtige Felder: `from_me`, `timestamp`, `data` (Text **oder** Medienpfad, wenn `media` true!),
`media` (bool), `mime`, `message_type` (0 Text, 1 Bild, 2 Video, 3 Audio, 4 Kontakt, 5 Standort,
7 Link, 8 Dokument, 10 System, 14 gelöscht, 15 Sticker, 51 Selbstlösch-Hinweis, 59 kein Anruf),
`reply` (key_id der zitierten Nachricht), `quoted_data`, `caption`, `reactions` `[{emoji, von: ich|sie}]`.
Anrufe sind nicht enthalten und kommen nicht ins Buch.
Bei 1.965 von 1.996 Medien-Nachrichten ist die Datei vorhanden; der Rest sind vCards (in `data` ein HTML-Link `<a href="…vCards/Name.vcf">Name</a>` → im Buch nur „Kontakt: Name“) oder „The media is missing“ (→ Hinweis „nicht mehr vorhanden“).

## Was gebaut ist

### DesignCraft-Erweiterungen (Rust, im Fork)
- Absatzschattierung: **Schatten** (`shadingShadowOn/Color/Tint/Offset`), **Korn/Sprenkel**
  (`shadingSpeckle`, `…Color`, `…Tint`, `shadingSpeckleOverText`), Dialogfelder dazu.
- Überlappende Absatzflächen in Textreihenfolge (spätere liegen oben).
- `swatch.create {base, tint}` legt Tonwert-Farbfelder an.
- Korn wird am Bildschirm nur bei **High Quality Display** (⌥⌘H) gezeichnet, bei Typical (⌥⌘Z)
  nicht – sonst ruckelt das Tippen. Export/Druck immer mit Korn.
- **Menü „Jahresbuch ▸ WhatsApp-Chat einfließen lassen …“** (`crates/ui-egui/src/jahresbuch_dialog.rs`):
  Export, Medienordner, Zeitraum, 4 Hauptfarben, Chat-Schrift, Regler für Korn/Rauschen/Sprenkel und Abstände,
  Haken „Vorschau“, Knopf „Gestaltung übernehmen“. Ruft `jahresbuch/designcraft_import.py` auf
  (Python aus `~/WhatsApp-Export/.venv`), führt dessen Skripte im **offenen Buch** aus:
  Einrichtung (Farbfelder, Formate, Mustervorlage **J-Chat**, basiert auf der ersten Mustervorlage
  des Buchs → Seitenzahl kommt vom Buch), dann der Chat in den **markierten leeren Textrahmen**
  (sonst neue Chat-Seite am Ende); passt er nicht, kommen J-Chat-Seiten mit zwei verketteten Spalten
  ans Ende. Vorhandene Farbfelder/Formate werden nicht überschrieben. Einstellungen in `~/Jahresbuch/stil.json`.
- Ein End-to-End-Test im Dialog-Modul läuft nur mit `JAHRESBUCH_TEST_EXPORT`/`…_MEDIEN`.

### Hilfsprogramm `jahresbuch/designcraft_import.py`
Baut aus dem Export Befehlsskripte für `designcraft-cli script` bzw. für das Menü. Passt das Layout
ans Seitenformat des Buchs an (entworfen für A4; das Test-Buch des Users ist **170 × 240 mm**).

## Gestaltung der Chat-Seiten (alles vom User so entschieden)

- **Riso-Stil**, zwei Spalten-Boxen pro Seite auf weißer Seite.
- Spalten-Box: Papierfarbe + feines Papier-Rauschen; **nur ein** Schatten, blau („Spalte · Schatten“
  = 100 % Farbe Sie), **4 pt** nach rechts unten versetzt (früher 6 pt + Koralle-Druck darunter; vom
  User am 21.11.2026 reduziert, weil sich die Boxen im 170 × 240-Buch überschnitten), mit Rauschen
  und hellen Sprenkeln. Rauschen/Sprenkel je Fläche einzeln an/aus + Stärke.
- **Farben:** 4 Hauptfarben (Farbe Ich #E8493A Koralle, Farbe Sie #3554E0 Blau, Textfarbe #1E2A5A,
  Papier #F6F0E3); alle Varianten als Tonwert-Farbfelder in der Gruppe „Jahresbuch“
  (Ich/Sie · Blase, · Schatten, · Verzierung, Spalte · Schatten, Papier · Pille).
- **Abstände** (im Menü als Regler, pt; Vorgaben vom User am 21.11.2026 verkleinert): Datum → erste
  Nachricht **9,5**, Nachrichten derselben Person **12**, Wechsel zur anderen Person **14,5**
  (stil.json `abstand`). Technik: „Chat Sie“ hat Abstand davor = Datum-Abstand, danach = Wechsel −
  Datum, Abstand bei gleichen Formaten = gleiche Person. Foto-Serien: alle Reihen außer der letzten
  „Album X n“, die letzte „Album X n Ende“; eine einzelne Reihe ist eine normale Bildblase.
  Bekannt: Text → Foto derselben Person bekommt den Wechsel-Abstand (andere Formate).
- **Vorschau** im Jahresbuch-Menü: Haken „Vorschau“ zeigt Farben, Schrift, Korn, Rauschen und
  Abstände sofort im offenen Dokument; „Gestaltung übernehmen“ macht es fest, Abbrechen nimmt es zurück.
- **Blasen:** ihre links hellblau, seine rechts hell-koralle, Schatten in der Gegenfarbe,
  Korn in den Blasen (nicht auf Fotos!), genug Abstand (kein Überlappen), Uhrzeit in der Blase,
  **keine ✓✓**, kein „bearbeitet“.
- **Schrift Chat:** **Space Grotesk** (Alternative Work Sans, im Menü wählbar).
- **Datum:** **Fraunces** SemiBold, blau, **ohne Jahr**, ohne Box, ~50 % Korn über der Schrift;
  links und rechts eine zufällige Riso-Verzierung aus 14 Arten (keine Wiederholung innerhalb von
  5 Datumszeilen), in den Schattentönen der Blasen, **ohne eigenen Schatten**, mittig zur Schrift.
- **Fotos:** alle Chat-Fotos; Serien als Raster (Reihen als eigene Absätze, keine Überlappung);
  hohe Bilder werden schmaler statt beschnitten; Riso-Rahmen (Kontur + versetzte zweite Kontur);
  Videos als Standbild mit ▶ (auf dem Mac via `qlmanage`).
- **Antworten:** Name der zitierten Person farbig, Zitat in Anführungszeichen (gekürzt),
  bei zitierten Fotos ein Riso-Kamera-Icon.
- **Reaktionen:** Pille unter der Blase, Mitte auf der Blasenkante, nahe der Ecke (wie WhatsApp),
  Hintergrund helles Papier, **kein Schatten**, Kontur in der Farbe dessen, der reagiert hat.
- **Emojis:** **OpenMoji** (Farbe, CC BY-SA 4.0 → ins Impressum: „Emojis: OpenMoji, CC BY-SA 4.0“),
  als SVG im Text; große Emojis ohne Blase.
- Sprachnachricht: ▶ + Wellenform + Dauer; PDF/Dokument: Riso-Dokument-Icon; Sticker frei.
- Seitenzahl: kommt im Buch **von dessen Mustervorlage**, nicht vom Chat-Programm.

## Arbeitsweise im Buch (Wunsch des Users)

Der Chat ist **ein verketteter Text** im großen Buch und wird immer wieder von anderen Seiten
(Fotos, Geschichten) unterbrochen; manchmal steht vor dem Chat auf derselben Seite noch Text.
Deshalb: Chat in verkettete Textrahmen des Buchs, nicht als eigenes PDF.

## Offen / nächste Schritte

1. **Erster Test des Jahresbuch-Menüs auf dem Mac** (war gerade dran: zwei Spaltenrahmen gezogen,
   verketten, linken markieren, Menü aufrufen, Zeitraum Nov. 2026).
2. **Doppelklick-App „DesignCraft Jahresbuch“** fürs Dock (statt Start per Terminal).
3. Noch unbeantwortet: Startet er jedes Jahr mit **neuem leeren Buch** oder einer **Vorlage vom Vorjahr**?
4. Danach: **Foto-Seiten** (alte Vorlagen: 2 Querformate übereinander · 1 quer oben + 2 hoch unten ·
   1 hoch seitenfüllend · 2×2 hoch · 1 randlos; Seitenzahl blau/bei randlos weiß), dann freie Texte.
5. Neue offizielle DesignCraft-Versionen übernehmen: **nur auf Wunsch** des Users (`upstream` mergen).
6. Später evtl.: Erweiterungen dem DesignCraft-Projekt vorschlagen (entscheidet der User).
7. Bekannte Kleinigkeit: Ein in einer Spalte zu hohes Einzelbild rutscht samt Datum in die nächste
   Spalte und lässt Leerraum.

## Testdaten in der Cloud

Eine neue Cloud-Session hat **keine** Chat-Daten. Zum Testen den User um einen kleinen Ausschnitt
als ZIP bitten (Befehl dafür: November 2026 aus `euer_chat_mit_reaktionen.json` herausschneiden,
Fotos verkleinern, Videos als Standbild; < 30 MB).
