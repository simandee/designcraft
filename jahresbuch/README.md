# Jahresbuch: WhatsApp-Chat → DesignCraft

Dieses Hilfsprogramm baut aus einem WhatsApp-Export ein fertiges DesignCraft-Dokument mit den
Chat-Seiten im Riso-Stil: Sprechblasen, Zitate, Reaktionen, Fotos, Emojis, Datumszeilen mit
Verzierungen und die Spalten-Boxen auf der Mustervorlage.

## Was hier liegt

| Datei / Ordner | Wofür |
|---|---|
| `designcraft_import.py` | das Hilfsprogramm |
| `stil.json` | deine Einstellungen: Farben, Schrift, Korn und Rauschen je Fläche |
| `openmoji/` | die Emoji-Bilder (OpenMoji, siehe unten) |

Deine Chats und Fotos liegen **nicht** hier, sondern nur auf deinem Mac.

## Was du brauchst

- diese DesignCraft-Version (Branch `absatz-schatten` in diesem Repository); nur sie kennt
  Schatten und Korn der Blasen
- Python 3 mit den Paketen `pillow` und `emoji`
- die Schriften **Space Grotesk** und **Fraunces** (kostenlos bei Google Fonts)
- den WhatsApp-Export als JSON samt Medienordner

## Benutzen (in DesignCraft)

1. Dein Buch öffnen.
2. Einen **leeren Textrahmen** markieren, in dem der Chat beginnen soll (ohne Markierung kommen
   neue Chat-Seiten ans Ende).
3. Fenster **Jahresbuch** (Menü **Window ▸ Jahresbuch**): Export, Medienordner, Chat und Zeitraum
   wählen, **Chat einfügen**.

Der Chat fließt in den Rahmen und durch die damit verketteten Rahmen weiter; passt er nicht,
kommen Seiten mit der Mustervorlage **J-Chat** (zwei Spalten-Boxen) am Ende dazu. Danach ganz
normal weiterarbeiten: Seiten dazwischenschieben, Verkettung lösen und neu ansetzen – der Text
fließt nach. Ist ein Chat markiert (Rahmen, Textcursor oder Chat-Foto), zeigt das Fenster seine
Gestaltung: Farben, Schrift, Abstände, Korn und Rauschen wirken sofort; **Fortsetzen …** hängt einen
weiteren Zeitraum an, **Neu aufbauen …** setzt die Tage eines Zeitraums neu.

## Benutzen (Terminal, eigenes Dokument)

```bash
python3 designcraft_import.py EXPORT.json MEDIENORDNER Chat_2021.jsonl --von 2021-01-01 --bis 2021-12-31
```

```bash
designcraft-cli script Chat_2021.jsonl --save Chat_2021.designcraft --export Chat_2021.pdf
```

Danach `Chat_2021.designcraft` in DesignCraft öffnen und weiterarbeiten.

## Einstellungen (`stil.json`)

- `farbe_ich`, `farbe_sie`, `farbe_text`, `farbe_papier`: die vier Hauptfarben; alle Varianten
  (Blasen, Schatten, Verzierungen …) werden daraus abgeleitet.
- `tonwerte`: einzelne Varianten abweichend einstellen, z. B. `{"Sie · Blase": 0.4}`.
- `schrift_chat`: `"Space Grotesk"` oder `"Work Sans"`.
- `korn_blasen`, `korn_datum`: Korn in den Blasen bzw. auf der Datumszeile (0 = aus, 1 = kräftig).
- `rauschen`: feines Rauschen je Fläche der Spalten-Box: `innen` und `rahmen` (blauer Schatten); `blasen` = Stärke auf den Chatblasen, gilt nur mit `rauschen_blasen_an: true`.
- `sprenkel`: helle Sprenkel auf `rahmen` (blauer Schatten).

In DesignCraft selbst geht alles auch nachträglich: Farbfelder in der Gruppe „Jahresbuch“,
Absatzformate „Chat Sie“/„Chat Ich“/„Datum“/„Spalte Schatten“, Rauschen-Rahmen auf der Mustervorlage.

## Lizenzen

Emojis: OpenMoji – the open-source emoji and icon project, CC BY-SA 4.0 (`openmoji/LICENSE.txt`).
Im Buch gehört dafür eine Zeile ins Impressum, z. B. „Emojis: OpenMoji, CC BY-SA 4.0“.
