# Jahresbuch – Projektstand und Übergabe

Diese Datei fasst alles zusammen, was in früheren Sessions besprochen und gebaut wurde.
**Neue Session: diese Datei zuerst vollständig lesen.** Stand: 22.11.2026.

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
  **Schnellerer Neubau** (mit dem User am 21.11.2026 vereinbart, ab dem nächsten Update):
  `CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_INCREMENTAL=true cargo build --release -p designcraft -p designcraft-cli`
  (erstes Mal so lang wie sonst, danach schneller; Dauer beim zweiten Mal erfragen). Wünsche möglichst
  sammeln und in einem Rutsch einbauen, damit nicht nach jeder Kleinigkeit neu gebaut werden muss.
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
| Mac `~/Jahresbuch/` | Arbeitsordner, Einstellungen `stil.json` des Jahresbuch-Fensters, Test-Dateien |
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
  (`shadingSpeckle`, `…Color`, `…Tint`, `shadingSpeckleOverText`), Dialogfelder dazu;
  **Rauschen** (`shadingNoise`: Papier-Körnung als Textur, dunkel auf der Fläche, hell auf dem
  Schatten; unter dem Text, nur bei High Quality Display; noch ohne Feld im Absatzschattierungs-Dialog).
- Überlappende Absatzflächen in Textreihenfolge (spätere liegen oben).
- **Objekte im Text wie normale Objekte** (User-Wunsch 21.11.2026: Chat-Fotos „wie normale
  Bild-Objekte“): Auswahlwerkzeug-Klick auf ein verankertes Objekt (z. B. Chat-Foto) wählt es aus
  (Rahmen mit 8 Anfassern), Ziehen an einem Anfasser skaliert es mit Inhalt **proportional**
  (Gegenseite bleibt, Konturen bleiben gleich dick, Text/Blase passen sich an), Entf löscht es,
  Esc oder Klick daneben hebt die Auswahl auf. Befehle `anchored.resize {story,index,width|height}`
  und `anchored.delete`; Werkzeug-Code `crates/tools/src/inline.rs` + `select.rs`.
  **Seit 22.11.2026 wie Bildrahmen** (User-Wunsch: Rahmen und Foto getrennt wie in InDesign): die
  Auswahl steht im Dokument (`selection.set {inline: {story, index, content?}}`); Rahmen-Anfasser
  **beschneiden** (`anchored.frame`, das Foto bleibt, der versetzte Riso-Rahmen der Chat-Foto-Gruppe
  geht mit), mit ⌘ skalieren sie alles (`anchored.resize`); **Inhaltsgreifer** (Kringel in der Mitte)
  oder Doppelklick wählt das Foto im Rahmen: Ziehen verschiebt den Ausschnitt, seine (braunen)
  Anfasser skalieren es (`anchored.content`), Esc zurück zum Rahmen. Eigenschaften-Fenster und
  Steuerleiste: B/H des Rahmens, Skalieren %, Inhalt Skalierung %/X/Y, Einpassen (`anchored.fit`).
  Noch nicht: Mehrfachauswahl, Kopieren.
- `swatch.create {base, tint}` legt Tonwert-Farbfelder an.
- Korn wird am Bildschirm nur bei **High Quality Display** (⌥⌘H) gezeichnet, bei Typical (⌥⌘Z)
  nicht – sonst ruckelt das Tippen. Export/Druck immer mit Korn.
- **Fenster „Jahresbuch“** (Window › Jahresbuch, `crates/ui-egui/src/jahresbuch/`, siehe unten;
  es ersetzt das frühere Menü „Jahresbuch“ mit seinen zwei Dialogen).
  Das Einfügen ruft `jahresbuch/designcraft_import.py` auf
  (Python aus `~/WhatsApp-Export/.venv`), führt dessen Skripte im **offenen Buch** aus:
  Einrichtung (Farbfelder, Formate, Mustervorlage **J-Chat**, basiert auf der ersten Mustervorlage
  des Buchs → Seitenzahl kommt vom Buch), dann der Chat in den **markierten leeren Textrahmen**
  (sonst neue Chat-Seite am Ende); passt er nicht, kommen J-Chat-Seiten mit zwei verketteten Spalten
  ans Ende. Vorhandene Farbfelder/Formate werden nicht überschrieben. Einstellungen in `~/Jahresbuch/stil.json`.
  Einfügen, Fortsetzen und Neu aufbauen sind je **ein** Rückgängig-Schritt; geht unterwegs etwas
  schief, bleibt das Buch unverändert. Während das Hilfsprogramm arbeitet, steht im Fenster
  „Chat wird aufbereitet …“ (das Programm reagiert dann kurz nicht).
- Die End-to-End-Tests des Fensters (`jahresbuch/tests.rs`) laufen nur mit `JAHRESBUCH_TEST_EXPORT`/`…_MEDIEN`
  (synthetische Testdaten, keine echten Chats).

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
- **Abstände** (im Menü als Regler, pt; Vorgaben vom User am 21.11.2026 verkleinert): letzte
  Nachricht → Datum **20** (`vor_datum`; kann nicht kleiner werden als Wechsel − Datum), Datum → erste
  Nachricht **9,5**, Nachrichten derselben Person **12**, Wechsel zur anderen Person **14,5**
  (stil.json `abstand`). Technik: „Chat Sie“ hat Abstand davor = Datum-Abstand, danach = Wechsel −
  Datum, Abstand bei gleichen Formaten = gleiche Person. Foto-Serien: alle Reihen außer der letzten
  „Album X n“, die letzte „Album X n Ende“; eine einzelne Reihe ist eine normale Bildblase.
  Bekannt: Text → Foto derselben Person bekommt den Wechsel-Abstand (andere Formate).
- **Schriftgrößen** (Chat-Gestaltung, Regler in pt, User-Wunsch 22.11.2026; stil.json `schrift`):
  Nachrichtentext **8,4**, Uhrzeit **6**, Datum **10**, Zitat und Hinweise **7,2** (Zitat-Name
  = 6,8/7,2, Hinweis = 7,4/7,2 davon). Emojis (Script-Label „Emoji“) wachsen mit dem Nachrichtentext,
  Datums-Verzierungen („Verzierung“) mit dem Datum, beide samt Anhebung (yOffset). Die Labels setzt
  das Hilfsprogramm erst seit 22.11.2026 – in früher eingefügten Chats wachsen sie nicht mit.
- Änderungen im **Jahresbuch-Fenster** wirken sofort (ein Regler-Zug = ein Rückgängig-Schritt) und
  ändern nur, was sich geändert hat: eigene Anpassungen an anderen Formaten und Farbfeldern bleiben.
- **Blasen:** ihre links hellblau, seine rechts hell-koralle, Schatten in der Gegenfarbe,
  Korn in den Blasen (nicht auf Fotos!), genug Abstand (kein Überlappen), Uhrzeit in der Blase,
  **Rauschen** wie auf der Spalten-Box wahlweise auch auf Blasen und ihren Schatten (Menü: Haken +
  Regler „Rauschen Chatblasen“, Vorgabe aus, Stärke 0,5; stil.json `rauschen_blasen_an`,
  `rauschen.blasen`). „Rauschen“ heißt für den User **die grisselige, fleckige Körnung** wie auf dem
  Ocker (nicht Punkte!). Technik: Absatzattribut `shadingNoise` (0–1) zeichnet dieselbe Art
  Rausch-Textur (Gauß-Rauschen, Zelle 0,65 pt, geglättet): dunkel multipliziert auf der Fläche
  (Deckkraft 0,3 × Stärke), hell auf dem Schatten (Deckkraft = Stärke); unter Text und Fotos;
  nur bei High Quality Display; im PDF als maskiertes Bild. Reaktions-Pillen ohne.
  **Form:** Blasen unten außen **spitz** (ihre unten links, seine unten rechts), sonst 7 pt rund;
  der Schatten folgt der Form (User-Wunsch 21.11.2026, „Richtungseindruck“).
  **keine ✓✓**, kein „bearbeitet“.
- **Schrift Chat:** **Space Grotesk** (Alternative Work Sans, im Menü wählbar).
- **Datum:** **Fraunces** SemiBold, blau, **ohne Jahr**, ohne Box, ~50 % Korn über der Schrift;
  links und rechts eine zufällige Riso-Verzierung aus 14 Arten (keine Wiederholung innerhalb von
  5 Datumszeilen), in den Schattentönen der Blasen, **ohne eigenen Schatten**, mittig zur Schrift.
- **Fotos:** alle Chat-Fotos; Serien als Raster (Reihen als eigene Absätze, keine Überlappung);
  hohe Bilder werden schmaler statt beschnitten; Riso-Rahmen (Kontur + versetzte zweite Kontur)
  **dezent in den Blasen-Schattentönen** „Ich/Sie · Schatten“ (55 %, vom User am 21.11.2026 gewählt);
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

## Fenster „Jahresbuch“ (mit dem User am 22.11.2026 besprochen)

**Stand 22.11.2026 – gebaut** (`crates/ui-egui/src/jahresbuch/`: `modell` Chats im Dokument-Etikett
`jahresbuch.chats`, `gestaltung` Befehle je Chat, `einfuegen` Einfügen/Fortsetzen/Neu aufbauen,
`kalender`, `panel`): Fenster mit Symbol A1 in eigener Spalte ganz links (eingeklappt), Ansichten
„Neuer Chat“ / „Chat“, einklappbare Abschnitte, Änderungen sofort mit ⌘Z (ein Regler-Zug = ein
Schritt), ☰ „Neuen Chat einfügen …“, Zeitraum-Kalender, Komma in Zahlenfeldern, mehrere Chats mit
eigenem Look, „Aussehen wie Chat …“, Personenfarben (in Gruppenchats dazu „Akzent“ für Datum,
Verzierungen und den Schatten der Spalten-Box), Fortsetzen und Neu aufbauen (Hilfsprogramm
`--ersetzen ANFANG ENDE LÄNGE`). Der Chat merkt sich im Etikett die Tage, die **wirklich** im Buch
stehen (erster und letzter Tag mit Nachrichten, vom Hilfsprogramm gemeldet über `--zeitraum-ausgabe`),
und seinen Text (damit Fortsetzen nicht versehentlich einen kopierten Ausschnitt trifft).
Fortsetzen braucht einen Anfang nach dem letzten Tag im Buch; Neu aufbauen setzt nur die Tage des
gewählten Zeitraums neu (gefunden an den Datumszeilen, das Jahr über den Wochentag, ausgehend vom
ersten Tag des Chats), was davor und danach steht, bleibt samt Handarbeit. Ohne gespeicherten
ersten Tag (ältere Chats, „Ganzer Chat“ vor dem 22.11.2026) geht Neu aufbauen nur für den ganzen Chat. Die alten Dialoge und das Menü „Jahresbuch“ sind weg (User-Wunsch 22.11.2026); das
Fenster öffnet sich über **Window › Jahresbuch** in jedem Arbeitsbereich. Die Befehle
`jahresbuch.chatImport`/`chatDesign` bleiben für die Automatik (mit Angaben laufen sie direkt). Ein markierter Rahmen mit Text → der neue Chat kommt auf neue Seiten.
**Noch offen:** Personen umbenennen und zusammenlegen. (Chat-Fotos im Eigenschaften-Fenster: erledigt, siehe oben.)

Ursprünglicher Plan:

- **Andockbares Seitenfenster** „Jahresbuch“ mit eigenem Symbol (wie Ebenen/Farbfelder; in
  `dock.rs` registrieren, Symbol in `icons.rs`), ersetzt die beiden Menü-Dialoge (die Menüpunkte
  öffnen dann das Fenster).
- **Inhalt nach Auswahl:** nichts/leerer Textrahmen markiert → **Chat einfügen** (Export,
  Medienordner, Zeitraum, Knopf); Rahmen oder Textcursor in einem bestehenden Chat → **Chat
  gestalten** (Farben, Schrift, Schriftgrößen, Korn/Rauschen, Abstände) **dieses** Chats. Chat
  erkannt an seinen Absatzformaten.
- **Änderungen wirken sofort** (wie andere Seitenfenster), ⌘Z nimmt zurück; ein Regler-Zug = ein
  Rückgängig-Schritt. Kein Vorschau-Haken/OK mehr.
- **Mehrere Chats je Buch, jeder mit eigenem Look:** je Chat eigene Formate und Farbfelder (z. B.
  Gruppe „Chat 2“) und eigene Chat-Mustervorlage (Spalten-Box in seinen Farben).
- **Gruppenchats kommen vor:** Ich rechts, alle anderen links, **Name über jeder Blase in der Farbe
  der Person** (so hat der User es früher in InDesign gemacht: andere Blasen alle gleich hellgrau,
  eigene grün, Namen je Person farbig). **Entschieden (22.11.2026): Variante B** – Blasen der
  anderen alle gleich hell, **Schatten in der Farbe der Person**, Name in ihrer Farbe (verworfen:
  A alles gleich, C Blase im hellen Ton der Personenfarbe). Beim Bauen trotzdem ein Probebild zeigen. Personenfarben im Jahresbuch-Fenster einzeln einstellbar. Der **Name** hat die **gleiche
  Schriftgröße wie der Nachrichtentext** (wächst mit dem Regler mit) und ist **SemiBold**.
- **Chat fortsetzen** (gewünscht): weiteren Zeitraum hinten an einen bestehenden Chat anhängen,
  ohne Vorhandenes (Handarbeit) anzufassen.
- **Bereich neu aufbauen** (ok): einen Zeitraum neu einfügen, mit Warnung (Handarbeit dort geht
  verloren).
- **Aufbau des Fensters (Entwurf mit dem User abgestimmt 22.11.2026, Screenshots „Entwurf 1–4“):**
  Bedienelemente wie sonst in DesignCraft: Schriftgrößen = Feld mit ▲▼ und Größenliste (wie
  Schriftgröße im Eigenschaften-Fenster); Abstände = ▲▼-Feld in pt; Korn/Rauschen = %-Feld mit
  ›-Knopf, der einen Regler aufklappt (wie Deckkraft); Farben = Farbfelder. Abschnitte **einklappbar**
  mit Dreieck (wie Fenster Typografie): Personen, Schrift, Abstände, Korn und Rauschen, Chat.
  Ansicht „Neuer Chat“: Export + Wählen, Medienordner, Auswahl des Chats im Export, **ein** Feld „Zeitraum“
  mit **Zeitraum-Kalender wie bei Buchungsseiten** (Entwurf 5: 1. Klick Anfang, 2. Klick Ende, Tage
  dazwischen als Band, Monate mit ‹ › blättern, Knöpfe „Ganzer Monat“, „Ganzes Jahr“; Datum über
  chrono, keine neue Bibliothek), Anzahl Nachrichten/Fotos, „Aussehen wie Chat …“/Standard, Ziel, Knopf.
  Ansicht „Chat“: **Name des Chats editierbar**, Zeitraum/Anzahl, dann die Abschnitte; unten Knöpfe
  „Fortsetzen …“ und „Neu aufbauen …“. ☰-Menü des Fensters: **„Neuen Chat einfügen …“**.
  Zahlenfelder sollen **„8,4“ mit Komma** verstehen (DesignCraft kann bisher nur Punkt → in
  `widgets.rs` NumKind::parse ergänzen, mit Test). Platz: eigene **dritte Spalte links** im Dock,
  anfangs **eingeklappt** (nur Symbol); der User dockt es selbst um. Eigenes Symbol zeichnen.
- **Symbol:** Variante **A1** (zwei Sprechblasen, leicht gerundete Ecken, Schwänzchen vorne unten
  links, hinten unten rechts; Linienstil wie die übrigen Panel-Symbole). Entwurf-Code:
  `jb_blasen` mit Radius 2,5 auf dem 20×20-Raster; Ansätze der Spitzen beim Einbau glätten.
- **Personen umbenennen** (gewünscht): im Abschnitt „Personen“ den angezeigten Namen ändern (z. B.
  ein Spitzname statt Kontaktname) und doppelte Personen zusammenlegen (Nummernwechsel). Bis dahin
  heißen die Personen eines neuen Zweier-Chats „Ich“ und wie der Chat im Export (der früher fest
  eingebaute Name ist raus: das Repository ist öffentlich); schon eingefügte Chats behalten ihre Namen.
- Abgelehnt: einzelne Nachrichten weglassen; Bildqualitäts-Prüfung (oft gibt es nichts Besseres).
- **Chat-Fotos wie andere Bilder:** Anfasser gibt es schon; dazu sollen ihre Maße im normalen
  Eigenschaften-Fenster und in der Steuerleiste stehen und dort änderbar sein (nicht im
  Jahresbuch-Fenster).

## Gemerkt für später (dem User von sich aus wieder vorschlagen)

- **Haken pro Art, was ins Buch kommt** (Fotos, Videos, Sprachnachrichten, Sticker, Links,
  „Nachricht gelöscht“): wieder vorschlagen, wenn der User nach Neuerungen fragt.
- **Chat-Statistik als Spaß-Seite** (Nachrichten je Person, häufigste Emojis, aktivster Tag): der
  User will daran **erinnert** werden – nach dem Jahresbuch-Fenster von sich aus ansprechen.

## Offen / nächste Schritte

1. **Das Jahresbuch-Fenster mit dem echten Buch testen** (nach dem Rundumcheck vom 22.11.2026, siehe unten).
2. **Doppelklick-App „DesignCraft Jahresbuch“** fürs Dock (statt Start per Terminal).
3. Noch unbeantwortet: Startet er jedes Jahr mit **neuem leeren Buch** oder einer **Vorlage vom Vorjahr**?
4. Danach: **Foto-Seiten** (alte Vorlagen: 2 Querformate übereinander · 1 quer oben + 2 hoch unten ·
   1 hoch seitenfüllend · 2×2 hoch · 1 randlos; Seitenzahl blau/bei randlos weiß), dann freie Texte.
5. Neue offizielle DesignCraft-Versionen übernehmen: **nur auf Wunsch** des Users (`upstream` mergen).
6. Später evtl.: Erweiterungen dem DesignCraft-Projekt vorschlagen (entscheidet der User).
8. Bekannte Kleinigkeit: Ein in einer Spalte zu hohes Einzelbild rutscht samt Datum in die nächste
   Spalte und lässt Leerraum.

## Testdaten in der Cloud

Eine neue Cloud-Session hat **keine** Chat-Daten. Zum Testen den User um einen kleinen Ausschnitt
als ZIP bitten (Befehl dafür: November 2026 aus `euer_chat_mit_reaktionen.json` herausschneiden,
Fotos verkleinern, Videos als Standbild; < 30 MB).

## Rundumcheck 22.11.2026

Zehn Prüfer (fünf Bereiche × zwei Blickwinkel) fanden 81 mögliche Probleme; Gegenprüfer haben die
wichtigen bestätigt, behoben wurde unter anderem: Fortsetzen erbte die Uhrzeit-Schrift und Handarbeit
des letzten Absatzes; Neu aufbauen löschte alles außerhalb des Zeitraums; Zahlenfelder trugen einen
getippten Wert in den nächsten angeklickten Chat; Fortsetzen konnte Tage doppelt einfügen; ein zu
großer Zeitraum lief nach langem Warten gegen die Grenze von 100.000 Skript-Schritten (jetzt eine
Meldung vorher); Einfügen war mehrere Rückgängig-Schritte ohne Rückweg; Zeilenumbrüche in
Bildunterschriften verschoben den Text; Emojis in Namen; neue Gruppenmitglieder; Reaktionen auf
Fotoserien; ein ausgewähltes Chat-Foto zeigte „Neuer Chat“; Formate des Buchs wie „Datum“ galten als
Chat; Objekte im Text bremsten bei langen Büchern jede Mausbewegung; Sprenkel brauchten ein
Vielfaches an Speicher; PowerPoint-Export malte Sprenkel als Flächen; Emoji-Herkunft in `ASSETS.md`.

**Offen (bewusst noch nicht gemacht):**
- **PDF-Größe:** Die Sprenkel der Spalten-Box liegen zu ~98 % unsichtbar unter der Papierfläche und
  landen trotzdem im PDF (~5 MB je Chat-Seite). Abhilfe ändert den Aufbau der Spalten-Box –
  erst mit dem User abstimmen (Varianten zeigen).
- PDF/X-4 und PDF/A mit Rauschen auf den Blasen (Vorgabe aus): Warnungen bzw. Fehler beim Export.
- Kleinigkeiten: Neu aufbauen mit kürzerem Zeitraum lässt leere Chat-Seiten am Ende; Pfade von
  Export und Medienordner eines Chats im Fenster nicht änderbar; Feld für Rauschen im
  Absatzschattierungs-Dialog; IDML-Export verliert das Chat-Etikett.

