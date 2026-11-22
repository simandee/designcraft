#!/usr/bin/env python3
"""WhatsApp-Chat → DesignCraft-Dokument (Riso-Stil) als Befehlsskript für `designcraft-cli script`.

Erzeugt einen verketteten Chat-Text in Zwei-Spalten-Rahmen über beliebig viele Seiten, mit
Absatz- und Zeichenformaten, Farbfeldern, einer Mustervorlage mit den Riso-Spaltenboxen und
Bildern, Emojis und Icons als verankerte Objekte im Textfluss.

Aufruf:
  designcraft_import.py EXPORT.json MEDIENORDNER AUSGABE.jsonl [--stil stil.json] [--von 2021-01-01] [--bis 2021-12-31]
                        [--schrift "Space Grotesk"] [--emoji-font PFAD] [--arbeitsordner ORDNER]
Danach:
  designcraft-cli script AUSGABE.jsonl --save Chat.designcraft --export Chat.pdf
"""
import argparse
import random
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import unicodedata
from datetime import datetime, date
from zoneinfo import ZoneInfo

from PIL import Image, ImageDraw, ImageFont

try:
    import emoji as emoji_lib  # pip install emoji
except ImportError:  # pragma: no cover
    emoji_lib = None

TZ = ZoneInfo("Europe/Berlin")
# Anzeigenamen, solange der Export keine hergibt (eigene Nachrichten tragen keinen Namen).
ICH, SIE = "Ich", "Sie"
AUSBLENDEN = {10, 51, 59}  # Systemmeldungen, Hinweise zu selbstlöschenden Nachrichten, keine Anrufe
WOCHENTAGE = ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"]
MONATE = ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"]

# Seite (pt): A4, zwei Spalten. Die Spaltenboxen liegen 9 pt um die Textspalten.
SEITE_B, SEITE_H = 595.2755905511812, 841.8897637795277
RAND = {"top": 60, "bottom": 77.1, "inside": 57.2, "outside": 57.2}
STEG = 32.2
BOX_RAND = 9
SCHATTEN_VERSATZ = 4  # so weit liegt der blaue Schatten der Spaltenbox nach rechts unten versetzt (pt)
SPALTE_B = (SEITE_B - RAND["inside"] - RAND["outside"] - STEG) / 2
BLASE_EINZUG = 60          # so weit bleibt die andere Seite frei (max. Blasenbreite)
BILD_B = SPALTE_B - BLASE_EINZUG - 7 - 6  # Breite eines Einzelbilds in der Blase
EMOJI_PT, EMOJI_GROSS_PT = 9.5, 24
BILD_MAX_H = 300  # höchstens so hoch wird ein Einzelbild (pt); höhere werden schmaler


def seitenformat(breite, hoehe):
    """Das Chat-Layout an ein anderes Seitenformat anpassen (es ist für A4 entworfen): Ränder,
    Steg und Blasenbreite wachsen und schrumpfen mit der Seite, die Schriftgrößen bleiben."""
    global SEITE_B, SEITE_H, RAND, STEG, SPALTE_B, BLASE_EINZUG, BILD_B, BILD_MAX_H
    fx, fy = breite / 595.2755905511812, hoehe / 841.8897637795277
    SEITE_B, SEITE_H = breite, hoehe
    RAND = {"top": 60 * fy, "bottom": 77.1 * fy, "inside": 57.2 * fx, "outside": 57.2 * fx}
    STEG = 32.2 * fx
    SPALTE_B = (SEITE_B - RAND["inside"] - RAND["outside"] - STEG) / 2
    BLASE_EINZUG = 60 * fx
    BILD_B = SPALTE_B - BLASE_EINZUG - 7 - 6
    BILD_MAX_H = 300 * fy

URL_RE = re.compile(r"https?://\S+")

# Höhe der Verzierungen (pt, inkl. Versatz), damit ihre Mitte auf Höhe der Großbuchstaben-Mitte sitzt
DEKO_HOEHE = {"balken": 6.5, "punkte": 7.5, "rauten": 8.6, "raster": 8.8, "sterne": 11.2, "pillen": 7.8, "kreuze": 10.5,
              "ringe": 12.2, "dreiecke": 8.0, "zickzack": 6.5, "treppe": 11.7, "blumen": 11.0, "konfetti": 8.6, "streifen": 9.0}
DATUM_PT = 10
# Schriftgrößen (pt) der Bestandteile, im Jahresbuch-Menü einstellbar (stil.json "schrift"); die
# übrigen Größen hängen daran: Emojis am Nachrichtentext, Verzierungen am Datum, Zitat-Name und
# Hinweise am Zitat-Text. Gleiche Werte im Jahresbuch-Menü (Rust).
SCHRIFT = {"text": 8.4, "zeit": 6.0, "datum": DATUM_PT, "zitat": 7.2}
ZITAT_NAME, HINWEIS = 6.8 / 7.2, 7.4 / 7.2
# Standard-Abstände (pt), im Jahresbuch-Menü einstellbar (stil.json "abstand"): Datum -> erste
# Nachricht, zwischen Nachrichten derselben Person, bei einem Personenwechsel.
# Dazu "vor_datum": letzte Nachricht -> nächstes Datum (gesamt).
ABSTAND = {"vor_datum": 20, "datum": 9.5, "gleiche_person": 12, "wechsel": 14.5}
ALBUM_REIHEN = 3    # Abstand zwischen den Reihen einer Foto-Serie (pt)
DEKO_MITTE = DATUM_PT * 0.36  # Mitte der Großbuchstaben über der Grundlinie

# Abgeleitete Farben: (Farbfeld, Hauptfarbe, Tonwert). Überschreibbar in stil.json unter "tonwerte".
VARIANTEN = [
    ("Ich · Blase", "Farbe Ich", 0.34), ("Sie · Blase", "Farbe Sie", 0.26),
    ("Ich · Schatten", "Farbe Ich", 0.55), ("Sie · Schatten", "Farbe Sie", 0.55),
    ("Ich · Verzierung", "Farbe Ich", 0.55), ("Sie · Verzierung", "Farbe Sie", 0.55),
    ("Spalte · Schatten", "Farbe Sie", 1.0), ("Papier · Pille", "Papier", 0.4),
]

def blasen_ecken(seite):
    """Ecken einer Chatblase (oben links, oben rechts, unten rechts, unten links): rund, nur unten
    außen spitz – ihre links, seine rechts –, damit die Blase zur Seite des Absenders zeigt.
    Gleiche Angabe wie im Jahresbuch-Menü (Rust)."""
    rund, spitz = {"shape": "rounded", "size": 7}, {"shape": "none", "size": 0}
    return [rund, rund, spitz, rund] if seite == "Ich" else [rund, rund, rund, spitz]


# Farben für weitere Personen in Gruppenchats (in der Reihenfolge, in der sie am meisten schreiben)
PERSONEN_FARBEN = ["#3554E0", "#2E8B57", "#9B4DCA", "#E0952B", "#1F9BB4", "#C2387A", "#6B8E23", "#8B5A2B"]


def farbe_von(key):
    """Hauptfarbe einer Person: im Zweier-Chat „Farbe Ich“/„Farbe Sie“, sonst „<Person> · Farbe“."""
    return "Farbe Ich" if key == "Ich" else "Farbe Sie" if key == "Sie" else f"{key} · Farbe"


def schatten_von(key):
    """Schattenton einer Person (55 % ihrer Farbe)."""
    return f"{key} · Schatten"


DEKO_ARTEN = ["balken", "punkte", "rauten", "raster", "sterne", "pillen", "kreuze", "ringe", "dreiecke", "zickzack",
              "treppe", "blumen", "konfetti", "streifen"]



# ----------------------------------------------------------------------------- Hilfen

def zeit(m):
    return datetime.fromtimestamp(m["timestamp"], TZ)


def datumszeile(d):
    return f"{WOCHENTAGE[d.weekday()]}, {d.day}. {MONATE[d.month - 1]}"


def bereinigen(text):
    """Zeichen aus dem privaten Unicode-Bereich (DesignCraft nutzt sie als Marker, z. B. Seitenzahl)
    und einzelne Surrogate entfernen. Zeilenumbrüche jeder Art werden Zeilenwechsel in der Blase
    (U+2028): ein Absatzende teilte die Nachricht, und CR oder VT verschöben alle Positionen danach."""
    t = "".join(c for c in (text or "") if unicodedata.category(c) not in ("Co", "Cs"))
    return re.sub(r"\r\n|[\r\n\x0b\x0c\x1c-\x1e\x85\u2029]", "\u2028", t)


def umbrechbar(url):
    """Unsichtbare Umbruchstellen in langen Links, damit sie in der Blase umbrechen."""
    return re.sub(r"([/.?&=_-])", "\\1\u200b", url)


_MEDIEN_INDEX = {}


def medium(medienordner, pfad):
    """Datei zum Exporter-Pfad finden, egal in welchem Unterordner sie liegt
    (Videos können als Standbild .mp4.png oder .jpg daneben liegen)."""
    if medienordner not in _MEDIEN_INDEX:
        index = {}
        for ordner, _, dateien in os.walk(medienordner):
            for d in dateien:
                index.setdefault(d, os.path.join(ordner, d))
        _MEDIEN_INDEX[medienordner] = index
    index = _MEDIEN_INDEX[medienordner]
    name = os.path.basename(pfad or "")
    stamm = os.path.splitext(name)[0]
    for kandidat in (name, stamm + ".jpg", name + ".png", stamm + ".png"):
        if name and kandidat in index:
            return os.path.abspath(index[kandidat])
    return None


def bildmasse(p):
    try:
        with Image.open(p) as im:
            return im.size
    except Exception:
        return (4, 3)


def audiodauer(p):
    try:
        out = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", p],
                             capture_output=True, text=True, timeout=20).stdout
        s = round(float(out.strip()))
        return f"{s // 60}:{s % 60:02d}"
    except Exception:
        return ""


def pdfseiten(p):
    try:
        data = open(p, "rb").read()
        n = len(re.findall(rb"/Type\s*/Page[^s]", data))
        return n or None
    except Exception:
        return None


# ----------------------------------------------------------------------------- Emojis als Bilder

class Emojis:
    """Erkennt Emojis im Text und rendert sie als PNG (DesignCraft zeigt Farb-Emojis noch nicht selbst)."""

    def __init__(self, ordner, fontpfad):
        self.ordner = ordner
        os.makedirs(ordner, exist_ok=True)
        self.font = None
        kandidaten = [fontpfad] if fontpfad else []
        kandidaten += ["/System/Library/Fonts/Apple Color Emoji.ttc", "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
                       "/usr/share/fonts/noto/NotoColorEmoji.ttf"]
        for k in kandidaten:
            if k and os.path.isfile(k):
                for groesse in (109, 160, 137, 96, 64):
                    try:
                        self.font = ImageFont.truetype(k, groesse)
                        break
                    except OSError:
                        continue
            if self.font:
                break

    OPENMOJI = os.path.join(os.path.dirname(os.path.abspath(__file__)), "openmoji", "svg")

    def openmoji(self, folge):
        """OpenMoji-SVG (CC BY-SA 4.0) mit engerem Ausschnitt, damit es so groß wirkt wie die Schrift."""
        for mit_fe0f in (True, False):
            name = "-".join(f"{ord(c):X}" for c in folge if mit_fe0f or c != "\ufe0f")
            quelle = os.path.join(self.OPENMOJI, name + ".svg")
            if os.path.isfile(quelle):
                ziel = os.path.join(self.ordner, "openmoji_" + name + ".svg")
                if not os.path.isfile(ziel):
                    svg = open(quelle, encoding="utf-8").read().replace('viewBox="0 0 72 72"', 'viewBox="7 7 58 58"', 1)
                    open(ziel, "w", encoding="utf-8").write(svg)
                return ziel
        return None

    def finden(self, text):
        """Liste (start, ende) der Emoji-Folgen in `text` (Zeichenindizes)."""
        if emoji_lib is not None:
            return [(e["match_start"], e["match_end"]) for e in emoji_lib.emoji_list(text)]
        treffer, i = [], 0
        while i < len(text):
            cp = ord(text[i])
            if cp >= 0x1F000 or 0x2600 <= cp <= 0x27BF or cp in (0x2764, 0x263A, 0x2639):
                j = i + 1
                while j < len(text) and (text[j] in "\u200d\ufe0f\ufe0e" or 0x1F3FB <= ord(text[j]) <= 0x1F3FF
                                          or (text[j - 1] == "\u200d" and ord(text[j]) >= 0x2000)):
                    j += 1
                treffer.append((i, j)); i = j
            else:
                i += 1
        return treffer

    def png(self, folge):
        """Bilddatei für eine Emoji-Folge: OpenMoji (SVG, Vektor) wenn vorhanden, sonst aus der Systemschrift."""
        svg = self.openmoji(folge)
        if svg:
            return svg
        name = "emoji_" + "-".join(f"{ord(c):x}" for c in folge if c != "\ufe0f") + ".png"
        pfad = os.path.join(self.ordner, name)
        if not os.path.isfile(pfad) and self.font is not None:
            im = Image.new("RGBA", (400, 300), (0, 0, 0, 0))
            ImageDraw.Draw(im).text((10, 10), folge, font=self.font, embedded_color=True)
            box = im.getbbox()
            if box:
                im.crop(box).save(pfad)
        return pfad if os.path.isfile(pfad) else None


# ----------------------------------------------------------------------------- Story-Aufbau

class Absatz:
    def __init__(self, stil):
        self.stil = stil
        self.laeufe = []   # (text, zeichenformat|None)
        self.objekte = []  # (zeichenindex im Absatz, objekt-dict)

    def text(self, t, cs=None):
        if t:
            self.laeufe.append((t, cs))

    def objekt(self, obj):
        self.objekte.append((sum(len(t) for t, _ in self.laeufe), obj))
        self.laeufe.append(("*", None))  # Platzhalter, wird durch das verankerte Objekt ersetzt

    @property
    def inhalt(self):
        return "".join(t for t, _ in self.laeufe)


class Importer:
    def __init__(self, args):
        self.a = args
        self.medien = os.path.abspath(args.medien)
        self.arbeit = os.path.abspath(args.arbeitsordner or os.path.join(os.path.dirname(os.path.abspath(args.ausgabe)), "chat_dateien"))
        os.makedirs(self.arbeit, exist_ok=True)
        self.stil = json.load(open(args.stil, encoding="utf-8")) if args.stil and os.path.isfile(args.stil) else {}
        self.emojis = Emojis(os.path.join(self.arbeit, "emojis"), args.emoji_font)
        # Schrift für Chat-Text, Uhrzeit, Zitate und Seitenzahl (Datum bleibt Fraunces)
        self.schrift = getattr(args, "schrift", None) or self.stil.get("schrift_chat") or "Space Grotesk"
        self.absaetze = []
        self.steps = []
        self.deko_verlauf = []
        self.tage = []  # die Tage mit Nachrichten, in der Reihenfolge ihrer Datumszeilen
        # Zieldokument (document.inspect als JSON), wenn in ein bestehendes Buch importiert wird
        dok_pfad = getattr(args, "dokument", None)
        self.dok = json.load(open(dok_pfad, encoding="utf-8")) if dok_pfad else None
        if self.dok and self.dok.get("settings", {}).get("pageWidth"):
            seitenformat(float(self.dok["settings"]["pageWidth"]), float(self.dok["settings"]["pageHeight"]))
        self.parent_index = 0
        self.album_nr = 0
        self.rahmenfarbe = ("Sie · Schatten", "Ich · Schatten")
        self._bbox = None
        # Mehrere Chats in einem Buch: ab dem zweiten bekommen alle Format- und Farbnamen ein Kürzel
        # („Chat 2/…“ bei Formaten, „Chat 2 · …“ bei Farbfeldern), der erste behält die alten Namen.
        self.nummer = int(getattr(args, "nummer", None) or 1)
        self.vorlage = "J-Chat" if self.nummer == 1 else f"J{self.nummer}-Chat"
        # Personen: [{key, name, ids, farbe}]; „Ich“ ist immer dabei. Kommen aus --personen (vom
        # Jahresbuch-Fenster) oder werden aus dem Chat ermittelt.
        self.personen = None
        self.gruppe = False

    # ---------------------------------------------------------------- Befehle
    # Schlüssel, deren Werte Farbfeld-Namen sind (in Formaten und Befehlen)
    FARB_SCHLUESSEL = {"swatch", "base", "fill", "stroke", "shadingColor", "shadingShadowColor", "shadingSpeckleColor", "borderColor",
                       "borderGapColor", "shadingNoiseColor"}

    def n_stil(self, name):
        """Format-Name dieses Chats."""
        if self.nummer == 1 or not isinstance(name, str) or name.startswith("["):
            return name
        return f"Chat {self.nummer}/{name}"

    def n_farbe(self, name):
        """Farbfeld-Name (und Script-Label der Mustervorlage) dieses Chats."""
        if self.nummer == 1 or not isinstance(name, str) or name.startswith("[") or name.startswith("$"):
            return name
        return f"Chat {self.nummer} · {name}"

    def _kuerzel(self, cmd, p):
        """Format- und Farbnamen eines Befehls auf diesen Chat umschreiben."""
        if self.nummer == 1:
            return p
        p = dict(p)
        if cmd.startswith("style."):
            for k in ("name", "basedOn", "nextStyle"):
                if k in p:
                    p[k] = self.n_stil(p[k])
            for teil in ("para", "chars", "attrs"):
                if isinstance(p.get(teil), dict):
                    p[teil] = {k: (self.n_farbe(v) if k in self.FARB_SCHLUESSEL else v) for k, v in p[teil].items()}
        elif cmd == "swatch.create":
            p["name"] = self.n_farbe(p["name"])
            if "base" in p:
                p["base"] = self.n_farbe(p["base"])
        elif cmd == "swatch.newColorGroup":
            p["name"] = f"Jahresbuch – Chat {self.nummer}"
            p["swatches"] = [self.n_farbe(x) for x in p["swatches"]]
        elif cmd in ("object.fill", "object.stroke"):
            p["swatch"] = self.n_farbe(p.get("swatch"))
        elif cmd == "object.label" and p.get("label") not in ("Emoji", "Verzierung"):
            p["label"] = self.n_farbe(p.get("label"))
        elif cmd == "type.char" and isinstance(p.get("attrs"), dict):
            p["attrs"] = {k: (self.n_farbe(v) if k in self.FARB_SCHLUESSEL else v) for k, v in p["attrs"].items()}
        return p

    def s(self, cmd, **p):
        p = self._kuerzel(cmd, p)
        # In ein bestehendes Dokument: vorhandene Farbfelder und Formate bleiben, wie sie sind
        # (so gehen eigene Anpassungen beim erneuten Import nicht verloren).
        if self.dok is not None:
            if cmd == "swatch.create" and p.get("name") in self.dok["swatches"]:
                return None
            if cmd == "style.paragraph.create" and p.get("name") in self.dok["paragraphStyles"]:
                return None
            if cmd == "style.character.create" and p.get("name") in self.dok["characterStyles"]:
                return None
        self.steps.append({"command": cmd, "params": p})
        return len(self.steps) - 1

    # ---------------------------------------------------------------- Text mit Emojis
    def lauf(self, a, text, cs=None, emoji_pt=EMOJI_PT):
        """Text in Absatz `a` schreiben; Emojis werden zu Bildern, Links umbrechbar. Emojis wachsen
        mit dem Nachrichtentext (`emoji_pt` gilt für die Standardgröße)."""
        text = bereinigen(text)
        f = self.groesse("text") / SCHRIFT["text"]
        pos = 0
        for e0, e1 in self.emojis.finden(text):
            self._klartext(a, text[pos:e0], cs)
            png = self.emojis.png(text[e0:e1])
            if png:
                a.objekt({"art": "bild", "pfad": png, "b": emoji_pt * f, "h": emoji_pt * f, "fit": "fitProportionally",
                          "y": round(-1.6 * f, 3), "label": "Emoji"})
            pos = e1
        self._klartext(a, text[pos:], cs)

    def _klartext(self, a, text, cs):
        pos = 0
        for m in URL_RE.finditer(text):
            a.text(text[pos:m.start()], cs)
            a.text(umbrechbar(m.group(0)), "Link")
            pos = m.end()
        a.text(text[pos:], cs)

    # ---------------------------------------------------------------- Nachrichten
    def chat_daten(self):
        """Der gewählte Chat aus dem Export (--chat: seine Kennung; sonst der erste)."""
        daten = json.load(open(self.a.export, encoding="utf-8"))
        jid = getattr(self.a, "chat", None)
        return daten.get(jid) if jid and jid in daten else next(iter(daten.values()))

    def personen_ermitteln(self, msgs, chat_name=None):
        """Personen des Chats: aus --personen oder aus den Absendern. Zweier-Chat: „Ich“ und „Sie“
        (ihr Name: der Absender oder der Name des Chats im Export); Gruppe (mehrere Absender): „Ich“
        und „Person 1“, „Person 2“ … (nach Anzahl Nachrichten)."""
        vorgabe = getattr(self.a, "personen", None)
        if vorgabe:
            self.personen = json.loads(vorgabe) if vorgabe.strip().startswith("[") else json.load(open(vorgabe, encoding="utf-8"))
            # Gruppe: wer seitdem dazugekommen ist, wird eine weitere Person (statt unter Person 1).
            if any(p["key"].startswith("Person ") for p in self.personen):
                bekannt = {i for p in self.personen for i in p.get("ids", [])}
                neu = {}
                for m in msgs:
                    if not m.get("from_me") and m.get("sender") and m["sender"] not in bekannt:
                        neu[m["sender"]] = neu.get(m["sender"], 0) + 1
                n = max(int(p["key"].split()[1]) for p in self.personen if p["key"].startswith("Person "))
                for sender in sorted(neu, key=lambda k: -neu[k]):
                    self.personen.append({"key": f"Person {n + 1}", "name": sender, "ids": [sender],
                                          "farbe": PERSONEN_FARBEN[n % len(PERSONEN_FARBEN)]})
                    n += 1
        else:
            zaehler = {}
            for m in msgs:
                if not m.get("from_me") and m.get("sender"):
                    zaehler[m["sender"]] = zaehler.get(m["sender"], 0) + 1
            andere = sorted(zaehler, key=lambda k: -zaehler[k])
            self.personen = [{"key": "Ich", "name": ICH, "ids": [], "farbe": self.stil.get("farbe_ich", "#E8493A")}]
            if len(andere) > 1:
                for i, sender in enumerate(andere):
                    self.personen.append({"key": f"Person {i + 1}", "name": sender, "ids": [sender],
                                          "farbe": PERSONEN_FARBEN[i % len(PERSONEN_FARBEN)]})
            else:
                self.personen.append({"key": "Sie", "name": andere[0] if andere else (bereinigen(chat_name).strip() or SIE), "ids": andere,
                                      "farbe": self.stil.get("farbe_sie", "#3554E0")})
        self.gruppe = any(p["key"] not in ("Ich", "Sie") for p in self.personen)
        self._nach_id = {i: p["key"] for p in self.personen for i in p.get("ids", [])}

    def person(self, key):
        return next((p for p in self.personen if p["key"] == key), None)

    def nachrichten(self):
        chat = self.chat_daten()
        msgs = sorted(chat["messages"].values(), key=lambda m: m["timestamp"])
        self.personen_ermitteln(msgs, chat.get("name"))
        von = date.fromisoformat(self.a.von) if self.a.von else None
        bis = date.fromisoformat(self.a.bis) if self.a.bis else None
        sichtbar = [m for m in msgs if m.get("message_type") not in AUSBLENDEN
                    and (not von or zeit(m).date() >= von) and (not bis or zeit(m).date() <= bis)]
        byid = {m["key_id"]: m for m in msgs}
        return sichtbar, byid

    def gruppen(self, msgs):
        """Fotos/Videos desselben Absenders kurz hintereinander werden zu Alben gebündelt."""
        def album(m):
            return m.get("message_type") in (1, 2) and m.get("media") and not m.get("caption") and not m.get("reply")
        gr = []
        for m in msgs:
            g = gr[-1] if gr else None
            if (g and album(m) and album(g[-1]) and self.seite(g[-1]) == self.seite(m)
                    and m["timestamp"] - g[-1]["timestamp"] <= 120 and zeit(m).date() == zeit(g[-1]).date()):
                g.append(m)
            else:
                gr.append([m])
        return gr

    def aufbauen(self):
        msgs, byid = self.nachrichten()
        tag = None
        for g in self.gruppen(msgs):
            m = g[0]
            d = zeit(m).date()
            if d != tag:
                self.datum(d); tag = d
            if len(g) > 1:
                n = -(-len(g) // 4)
                groessen = [len(g) // n + (1 if i < len(g) % n else 0) for i in range(n)]
                pos = 0
                for k in groessen:
                    self.album(g[pos:pos + k]); pos += k
            else:
                self.nachricht(m, byid)

    def datum(self, d):
        """Datumszeile mit einer zufälligen (aber pro Datum festen) Riso-Verzierung links und rechts."""
        rnd = random.Random(d.isoformat())
        self.tage.append(d)
        zuletzt = self.deko_verlauf[-5:]
        art = rnd.choice([x for x in DEKO_ARTEN if x not in zuletzt])
        self.deko_verlauf.append(art)
        tausch = rnd.random() < 0.5
        a = Absatz("Datum")
        f = self.groesse("datum") / DATUM_PT  # Verzierungen wachsen mit dem Datum
        y = round((DEKO_MITTE - DEKO_HOEHE[art] / 2) * f, 3)
        a.objekt({"art": "deko", "deko": art, "spiegel": False, "tausch": tausch, "y": y})
        a.text("\u2002" + datumszeile(d) + "\u2002")
        a.objekt({"art": "deko", "deko": art, "spiegel": True, "tausch": tausch, "y": y})
        self.absaetze.append(a)

    def seite(self, m):
        """Wer die Nachricht geschrieben hat (Personen-Schlüssel, zugleich Teil der Formatnamen)."""
        if m.get("from_me"):
            return "Ich"
        if not self.gruppe:
            return "Sie"
        return self._nach_id.get(m.get("sender"), self.personen[1]["key"] if len(self.personen) > 1 else "Sie")

    def namenszeile(self, a, key):
        """Gruppenchat: der Name über der Blase, in der Farbe der Person."""
        if self.gruppe and key != "Ich":
            self.lauf(a, self.person(key)["name"], f"Name {key}")
            a.text("\u2028")

    def zeitlauf(self, a, m):
        a.text("\u2028" + zeit(m).strftime("%H:%M"), "Zeit")

    def reaktionen(self, m, a):
        r = m.get("reactions") or []
        if not r:
            return
        a.stil += " mit Reaktion"
        vons = {x.get("von") for x in r}
        seite = "Ich" if self.seite(m) == "Ich" else "Sie"
        if not self.gruppe:
            von = "von Ihr" if vons == {"sie"} else "von Mir" if vons == {"ich"} else "von Beiden"
        else:
            keys = {"Ich" if v == "ich" else self._nach_id.get(v) for v in vons}
            von = "von Mir" if keys == {"Ich"} else f"von {next(iter(keys))}" if len(keys) == 1 and None not in keys else "von Mehreren"
        p = Absatz(f"Reaktion {seite} {von}")
        for x in r:
            self.lauf(p, x.get("emoji", ""))
        self.absaetze.append(p)

    def zitat(self, a, m, byid):
        q = byid.get(m.get("reply"))
        wer = self.seite(q) if q else ("Sie" if not self.gruppe else self.personen[1]["key"])
        von_mir = wer == "Ich"
        self.lauf(a, self.person(wer)["name"], f"Zitat Name {wer}", emoji_pt=EMOJI_PT * ZITAT_NAME * SCHRIFT["zitat"] / SCHRIFT["text"])
        a.text("\u2028")
        text = bereinigen(m.get("quoted_data") or (q.get("data") if q and not q.get("media") else "") or "")
        if q and q.get("media") and not text:
            typ = q.get("message_type")
            if typ in (1, 2, 15):
                a.objekt({"art": "icon", "icon": "kamera", "farbe": farbe_von(wer)})
                a.text(" Foto" if typ == 1 else " Video" if typ == 2 else " Sticker", "Zitat Text")
            elif typ == 3:
                a.text("Sprachnachricht", "Zitat Text")
            else:
                a.text("Anhang", "Zitat Text")
        else:
            text = " ".join(text.split())
            if len(text) > 36:
                text = text[:35].rstrip() + "…"
            a.text("„", "Zitat Text"); self.lauf(a, text, "Zitat Text", emoji_pt=7.5); a.text("“", "Zitat Text")
        a.text("\u2028")

    def standbild(self, pfad):
        """Für Videos ein Standbild erzeugen (macOS: Vorschau-Dienst, sonst ffmpeg); Bilder bleiben, wie sie sind."""
        if not pfad or os.path.splitext(pfad)[1].lower() not in (".mp4", ".mov", ".m4v", ".3gp"):
            return pfad
        ziel_ordner = os.path.join(self.arbeit, "standbilder")
        os.makedirs(ziel_ordner, exist_ok=True)
        ziel = os.path.join(ziel_ordner, os.path.basename(pfad) + ".png")
        if not os.path.isfile(ziel):
            try:
                if shutil.which("qlmanage"):
                    subprocess.run(["qlmanage", "-t", "-s", "1200", "-o", ziel_ordner, pfad],
                                   capture_output=True, timeout=60)
                elif shutil.which("ffmpeg"):
                    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", pfad, "-frames:v", "1", ziel],
                                   capture_output=True, timeout=60)
            except Exception:
                pass
        return ziel if os.path.isfile(ziel) else None

    def bildobjekt(self, pfad, b, h, video=False):
        pfad = self.standbild(pfad)
        if not pfad:
            return None
        return {"art": "bild", "pfad": pfad, "b": b, "h": h, "fit": "fillProportionally", "video": video, "y": 0,
                "rahmen": self.rahmenfarbe}

    def rahmen_fuer(self, key):
        """Riso-Rahmen eines Fotos: Schattenton der Person, zweite Kontur im Schattenton einer anderen."""
        andere = "Sie" if key == "Ich" else "Ich"
        return (self.schatten(key), self.schatten(andere))

    def schatten(self, key):
        return "Ich · Schatten" if key == "Ich" else "Sie · Schatten" if key == "Sie" else schatten_von(key)

    def album(self, gruppe):
        self.rahmenfarbe = self.rahmen_fuer(self.seite(gruppe[0]))
        """Mehrere Fotos als Raster: jede Reihe ein eigener Absatz ("Album"-Format), die Reihen
        verschmelzen zu einer Blase. So bestimmt jede Reihe ihre eigene Höhe und nichts überlappt."""
        s = self.seite(gruppe[0])
        self.album_nr += 1
        variante = "1" if self.album_nr % 2 else "2"
        kachel = (BILD_B - 3) / 2
        reihen, rest = [], list(gruppe)
        if len(rest) % 2 == 1:
            reihen.append([rest.pop(0)])
        while rest:
            reihen.append(rest[:2]); rest = rest[2:]
        for r, reihe in enumerate(reihen):
            if len(reihen) == 1:
                a = Absatz(f"Bild {s}")  # eine Reihe: eine gewöhnliche Bildblase
            elif r == len(reihen) - 1:
                a = Absatz(f"Album {s} {variante} Ende")
            else:
                a = Absatz(f"Album {s} {variante}")
            if r == 0:
                self.namenszeile(a, s)
            for i, m in enumerate(reihe):
                p = self.standbild(medium(self.medien, m.get("data")))
                if len(reihe) == 1:
                    w, h = bildmasse(p) if p else (4, 3)
                    obj = self.bildobjekt(p, BILD_B, BILD_B * max(0.56, min(h / w, 0.9)), m.get("message_type") == 2) if p else None
                else:
                    obj = self.bildobjekt(p, kachel, kachel, m.get("message_type") == 2) if p else None
                    if i:
                        a.text("\u2002")
                if obj:
                    a.objekt(obj)
                else:
                    a.text("[fehlt]", "Hinweis")
            if r == len(reihen) - 1:
                self.zeitlauf(a, gruppe[-1])
            self.absaetze.append(a)
        # Reaktionen auf jedes Foto der Blase (alle von derselben Seite) unter die Blase
        alle = [x for m in gruppe for x in (m.get("reactions") or [])]
        self.reaktionen({**gruppe[-1], "reactions": alle}, self.absaetze[-1])

    def nachricht(self, m, byid):
        s = self.seite(m)
        self.rahmenfarbe = self.rahmen_fuer(s)
        typ = m.get("message_type")
        a = Absatz(f"Chat {s}")
        self.namenszeile(a, s)
        if m.get("reply"):
            self.zitat(a, m, byid)
        daten = m.get("data") or ""
        if typ == 14:
            a.text("Nachricht gelöscht", "Hinweis")
        elif typ == 4 or "vCards/" in daten:
            # Kontaktkarte: nur der Name (in "data" steht ein HTML-Link auf die .vcf)
            treffer = re.search(r">([^<]+)</a>", daten)
            a.text("Kontakt: ", "Hinweis")
            self.lauf(a, treffer.group(1).strip() if treffer else "Kontaktkarte", "Hinweis", emoji_pt=EMOJI_PT * HINWEIS * SCHRIFT["zitat"] / SCHRIFT["text"])
        elif m.get("media") and daten.strip().lower().startswith("the media is missing"):
            a.text("Anhang nicht mehr vorhanden", "Hinweis")
        elif m.get("media"):
            p = medium(self.medien, m.get("data"))
            if typ == 15 or m.get("sticker"):
                a.stil = f"Sticker {s}"
                if p:
                    a.objekt({"art": "bild", "pfad": p, "b": 70, "h": 70, "fit": "fitProportionally", "y": 0})
            elif typ in (1, 2):
                # Bildblase, wenn nur der Name (Gruppe) davor steht, kein Zitat
                if not m.get("reply"):
                    a.stil = f"Bild {s}"
                p = self.standbild(p)
                if p:
                    # Ganzes Bild zeigen: hohe Bilder (z. B. Screenshots) werden schmaler statt beschnitten
                    w, h = bildmasse(p)
                    verh = max(0.56, h / w)
                    hoehe = min(BILD_B * verh, BILD_MAX_H)
                    a.objekt(self.bildobjekt(p, hoehe / verh, hoehe, typ == 2))
                else:
                    a.text(("Foto" if typ == 1 else "Video") + " nicht mehr vorhanden", "Hinweis")
                if m.get("caption"):
                    a.text("\u2028"); self.lauf(a, m["caption"])
            elif typ == 3:
                a.objekt({"art": "icon", "icon": "play", "farbe": "Textfarbe"})
                a.text("\u2002")
                a.objekt({"art": "icon", "icon": "welle", "farbe": farbe_von(s), "seed": m["key_id"]})
                dauer = audiodauer(p) if p else ""
                if dauer:
                    a.text("\u2002" + dauer, "Zeit")
            else:
                n = pdfseiten(p) if p and (m.get("mime") or "").endswith("pdf") else None
                a.objekt({"art": "icon", "icon": "dokument", "farbe": farbe_von(s)})
                a.text(" PDF-Dokument" if (m.get("mime") or "").endswith("pdf") else " Dokument")
                if n:
                    a.text(f" · {n} Seite{'n' if n != 1 else ''}", "Zitat Text")
        else:
            text = bereinigen(m.get("data") or "").strip()
            if not text:
                return
            if not m.get("reply") and self.nur_emoji(text):
                a.stil = f"Emoji {s}"
                self.lauf(a, text, emoji_pt=EMOJI_GROSS_PT)
            else:
                self.lauf(a, text)
        self.zeitlauf(a, m)
        self.absaetze.append(a)
        self.reaktionen(m, a)

    def nur_emoji(self, text):
        t = re.sub(r"\s", "", text)
        treffer = self.emojis.finden(t)
        return 0 < len(treffer) <= 3 and sum(e - s for s, e in treffer) == len(t)

    # ---------------------------------------------------------------- Dokument
    def farben(self):
        st = self.stil
        return [("Farbe Ich", st.get("farbe_ich", "#E8493A")), ("Farbe Sie", st.get("farbe_sie", "#3554E0")),
                ("Textfarbe", st.get("farbe_text", "#1E2A5A")), ("Papier", st.get("farbe_papier", "#F6F0E3"))]

    def groesse(self, teil):
        """Schriftgröße (pt) eines Bestandteils aus stil.json "schrift"."""
        return float(self.stil.get("schrift", {}).get(teil, SCHRIFT[teil]))

    def abstaende(self):
        """Absatzabstände (pt) aus stil.json "abstand" -> (vor der Blase, nach der Blase, gleiche
        Person, vor dem Datum). Vor der Blase steht der Abstand Datum -> erste Nachricht; bei einem
        Personenwechsel kommen "nach" und "vor" zusammen; dieselbe Person nutzt den Abstand für
        gleiche Formate, der beide ersetzt. Gleiche Rechnung wie im Jahresbuch-Menü (Rust)."""
        a = self.stil.get("abstand", {})
        datum, gleich, wechsel = (float(a.get(k, v)) for k, v in (("datum", ABSTAND["datum"]),
                                  ("gleiche_person", ABSTAND["gleiche_person"]), ("wechsel", ABSTAND["wechsel"])))
        nach = max(0.0, wechsel - datum)
        vor_datum = float(a.get("vor_datum", ABSTAND["vor_datum"]))
        return datum, nach, gleich, max(0.0, vor_datum - nach)

    def rauschen_blasen(self):
        """Stärke des Rauschens auf den Chatblasen (0 = aus), aus stil.json."""
        if not self.stil.get("rauschen_blasen_an", False):
            return 0.0
        return float(self.stil.get("rauschen", {}).get("blasen", 0.5))

    def formate(self):
        R = lambda r: [{"shape": "rounded", "size": r}] * 4
        ecken = blasen_ecken
        vor, nach, gleich, datum_vor = self.abstaende()
        chars = {"fontFamily": self.schrift, "fontStyle": "Regular", "size": self.groesse("text"), "fill": "Textfarbe"}
        blase = {"align": "left", "leftIndent": 7, "rightIndent": BLASE_EINZUG, "spaceBefore": vor, "spaceAfter": nach, "sameParaStyleSpacing": gleich,
                 "hyphenate": True, "keepLinesTogether": True, "keepAllLines": True,
                 "shadingOn": True, "shadingColor": "Sie · Blase", "shadingTint": 1, "shadingOffsets": [3, 6, 4, 6],
                 "shadingWidth": "text", "shadingCorners": ecken("Sie"), "borderMerge": False,
                 "shadingShadowOn": True, "shadingShadowColor": "Ich · Schatten", "shadingShadowTint": 1, "shadingShadowOffset": [2.6, 2.6],
                 "shadingSpeckle": float(self.stil.get("korn_blasen", 0.8)), "shadingSpeckleColor": "Papier", "shadingSpeckleTint": 1,
                 # Papier-Rauschen wie auf der Spalten-Box: dunkel auf der Blase, hell auf ihrem Schatten
                 # (unter dem Text, Fotos liegen darüber)
                 "shadingNoise": self.rauschen_blasen()}
        s = self.s
        if self.gruppe:
            # Gruppenchat (Variante B): alle anderen in derselben hellen Blase, Schatten in ihrer Farbe
            blase = {**blase, "shadingColor": "Andere · Blase"}
        s("style.paragraph.create", name="Chat Sie", para=blase, chars=chars)
        s("style.paragraph.create", name="Chat Ich", basedOn="Chat Sie", para={"align": "right", "leftIndent": BLASE_EINZUG,
          "rightIndent": 7, "shadingColor": "Ich · Blase", "shadingCorners": ecken("Ich"),
          "shadingShadowColor": "Ich · Schatten" if self.gruppe else "Sie · Schatten"})
        keys = [p["key"] for p in self.personen]
        for key in keys:
            if key not in ("Ich", "Sie"):
                s("style.paragraph.create", name=f"Chat {key}", basedOn="Chat Sie", para={"shadingShadowColor": schatten_von(key)})
                s("style.character.create", name=f"Name {key}", chars={"fontStyle": "SemiBold", "fill": farbe_von(key)})
        for seite in keys:
            s("style.paragraph.create", name=f"Bild {seite}", basedOn=f"Chat {seite}", para={"shadingOffsets": [3, 3, 3.5, 3]})
            # Album-Reihen: Abstand zwischen den Reihen klein, Flächen verschmelzen zu einer Blase.
            # Alle Reihen außer der letzten im selben Format (Abstand für gleiche Formate = 3 pt),
            # die letzte ("Ende") bringt den Abstand nach der Blase mit.
            # Zwei Varianten (1/2) wechseln sich je Serie ab und unterscheiden sich unsichtbar im
            # Schattenversatz, damit aufeinanderfolgende Serien nicht zu einer Blase verschmelzen.
            for v, versatz in (("1", 2.6), ("2", 2.6001)):
                s("style.paragraph.create", name=f"Album {seite} {v}", basedOn=f"Bild {seite}",
                  para={"spaceBefore": vor, "spaceAfter": 0, "sameParaStyleSpacing": ALBUM_REIHEN, "borderMerge": True, "keepWithNext": 1,
                        "shadingOffsets": [3, 3, 3.5, 3], "shadingShadowOffset": [versatz, versatz]})
                s("style.paragraph.create", name=f"Album {seite} {v} Ende", basedOn=f"Album {seite} {v}",
                  para={"spaceBefore": ALBUM_REIHEN, "spaceAfter": nach})
                s("style.paragraph.create", name=f"Album {seite} {v} Ende mit Reaktion", basedOn=f"Album {seite} {v} Ende", para={"spaceAfter": 0})
            s("style.paragraph.create", name=f"Emoji {seite}", basedOn=f"Chat {seite}", para={"shadingOn": False})
            s("style.paragraph.create", name=f"Sticker {seite}", basedOn=f"Chat {seite}", para={"shadingOn": False})
            for basis in ("Chat", "Bild", "Emoji", "Sticker"):
                off = [3, 3, 7.5, 3] if basis == "Bild" else [3, 6, 8, 6]
                s("style.paragraph.create", name=f"{basis} {seite} mit Reaktion", basedOn=f"{basis} {seite}",
                  para={"spaceAfter": 0, **({"shadingOffsets": off} if basis in ("Chat", "Bild") else {})})
        pille = {"shadingNoise": 0, "spaceBefore": 4.5, "sameParaStyleSpacing": 4.5, "keepLinesTogether": True, "shadingColor": "Papier · Pille",
                 "shadingTint": 1, "shadingOffsets": [1.5, 3.5, 2, 3.5], "shadingCorners": R(7), "shadingShadowOn": False,
                 "borderOn": True, "borderTint": 1, "borderWeights": [0.8] * 4, "borderOffsets": [1.5, 3.5, 2, 3.5],
                 "borderWidth": "text", "borderCorners": R(7)}
        if self.gruppe:
            vons = [("von Mir", "Farbe Ich"), ("von Mehreren", "Textfarbe")] + [(f"von {k}", farbe_von(k)) for k in keys if k != "Ich"]
        else:
            vons = [("von Ihr", "Farbe Sie"), ("von Mir", "Farbe Ich"), ("von Beiden", "Textfarbe")]
        for seite, einzug in (("Sie", {"leftIndent": 11}), ("Ich", {"rightIndent": 11})):
            for von, kontur in vons:
                s("style.paragraph.create", name=f"Reaktion {seite} {von}", basedOn=f"Chat {seite}",
                  para={**pille, **einzug, "borderColor": kontur})
        s("style.paragraph.create", name="Datum", para={"align": "center", "spaceBefore": datum_vor, "spaceAfter": 0, "keepWithNext": 1,
          "shadingOn": True, "shadingColor": "[None]", "shadingWidth": "text", "shadingOffsets": [3, 4, 3, 4],
          "shadingSpeckle": float(self.stil.get("korn_datum", 0.4)), "shadingSpeckleColor": "Papier", "shadingSpeckleOverText": True},
          chars={"fontFamily": "Fraunces", "fontStyle": "SemiBold", "size": self.groesse("datum"), "fill": "Farbe Sie"})
        # Riso-Schatten der Spaltenboxen (Flächen über die ganze Rahmenhöhe)
        H = SEITE_H - RAND["bottom"] - RAND["top"] + 2 * BOX_RAND
        flaeche = {"shadingOn": True, "shadingWidth": "column", "shadingTop": "leading", "shadingBottom": "descent",
                   "shadingOffsets": [0, 0, 0, 0], "shadingCorners": R(9), "shadingTint": 1,
                   "shadingSpeckleColor": "Papier", "shadingSpeckleTint": 1}
        fl_chars = {"size": 1, "leading": {"kind": "points", "value": H}}
        s("style.paragraph.create", name="Spalte Schatten", chars=fl_chars,
          para={**flaeche, "shadingColor": "Spalte · Schatten", "shadingSpeckle": float(self.stil.get("sprenkel", {}).get("rahmen", 1.0))})
        if self.dok is None:
            # Nur im eigenen Chat-Dokument: im Buch kommt die Seitenzahl von dessen Mustervorlage,
            # und ein Format „Seitenzahl“ des Buchs bleibt unangetastet.
            s("style.paragraph.create", name="Seitenzahl", para={"align": "center"},
              chars={"fontFamily": self.schrift, "fontStyle": "Bold", "size": 18, "fill": "Farbe Ich"})
        z = self.groesse("zitat")
        s("style.character.create", name="Zeit", chars={"size": self.groesse("zeit"), "fontStyle": "Medium"})
        for key in keys:
            s("style.character.create", name=f"Zitat Name {key}", chars={"size": round(z * ZITAT_NAME, 2), "fontStyle": "Bold", "fill": farbe_von(key)})
        s("style.character.create", name="Zitat Text", chars={"size": z, "fontStyle": "Light"})
        s("style.character.create", name="Hinweis", chars={"size": round(z * HINWEIS, 2), "fontStyle": "Light", "fill": "Textfarbe"})
        s("style.character.create", name="Link", chars={"underline": True, "fill": "Textfarbe"})

    def rausch_einstellung(self, flaeche):
        """Stärke des Papier-Rauschens je Fläche (0 = aus … 1 = kräftig), aus stil.json "rauschen"."""
        standard = {"innen": float(self.stil.get("korn", 0.35)), "rahmen": 0.6}
        return float(self.stil.get("rauschen", {}).get(flaeche, standard[flaeche]))

    def rausch_png(self, hell):
        """Rauschen als Bild: dunkel (multipliziert, für helle Flächen) oder hell (weiße Fehlstellen
        unterschiedlicher Deckung, für kräftige Farbflächen wie beim Riso-Druck)."""
        pfad = os.path.join(self.arbeit, "rauschen_hell.png" if hell else "korn.png")
        if not os.path.isfile(pfad):
            rnd = random.Random(11 if hell else 7)
            n = 900
            if hell:
                werte = [max(0, min(255, int(abs(rnd.gauss(0, 90))))) for _ in range(n * n)]
                im = Image.new("LA", (n, n)); im.putdata([(255, a) for a in werte]); im.convert("RGBA").save(pfad)
            else:
                im = Image.new("L", (n, n)); im.putdata([max(0, min(255, int(rnd.gauss(128, 56)))) for _ in range(n * n)])
                im.convert("RGB").save(pfad)
        return pfad

    def rauschen(self, spread, rect, flaeche):
        """Rauschen über einer Fläche: ein eigener Bildrahmen, benannt nach der Fläche. In DesignCraft
        über die Deckkraft (Stärke) und Ausblenden (an/aus) einzeln einstellbar."""
        staerke = self.rausch_einstellung(flaeche)
        name = {"innen": "Rauschen Innenfläche", "rahmen": "Rauschen Rahmen (Blau)"}[flaeche]
        hell = flaeche != "innen"
        k = self.s("frame.create", spread=spread, rect=rect, content="graphic")
        ids = [f"${k}.id"]
        self.s("file.place", path=self.rausch_png(hell), frame=ids[0])
        self.s("object.fit", mode="fillProportionally", ids=ids)
        self.s("object.cornerOptions", shape="rounded", size=9, ids=ids)
        self.s("object.stroke", swatch="[None]", ids=ids)
        if hell:
            self.s("object.opacity", opacity=round(min(1.0, staerke), 3), blend="normal", ids=ids)
        else:
            self.s("object.opacity", opacity=round(0.3 * staerke, 3), blend="multiply", ids=ids)
        self.s("object.label", label=name, ids=ids)
        if staerke <= 0:
            self.s("object.hide", ids=ids)

    def korn_png(self):
        pfad = os.path.join(self.arbeit, "korn.png")
        if not os.path.isfile(pfad):
            import random
            rnd = random.Random(7)
            im = Image.new("L", (900, 900))
            im.putdata([max(0, min(255, int(rnd.gauss(128, 56)))) for _ in range(900 * 900)])
            im.convert("RGB").save(pfad)
        return pfad

    def flaeche(self, spread, rect, stil):
        """Eine Fläche als Absatzschattierung eines leeren Textrahmens (füllt den Rahmen ganz aus)."""
        f = self.s("frame.create", spread=spread, rect=rect, content="text", text="\u00a0")
        self.s("object.textFrameOptions", inset=0, firstBaseline="leading", ids=[f"${f}.id"])
        self.s("text.select", story=f"${f}.story", anchor=0, focus=2)
        self.s("style.paragraph.apply", name=stil)
        self.s("object.label", label=stil, ids=[f"${f}.id"])

    def mustervorlage(self):
        """Riso-Spaltenboxen, Korn und Seitenzahl auf der Mustervorlage A (linke und rechte Seite)."""
        spread = {"kind": "parent", "index": self.parent_index}
        for seite_x in (0.0, SEITE_B):
            links = seite_x == 0.0  # linke Seite: innen = rechts
            x0 = seite_x + (RAND["outside"] if links else RAND["inside"])
            spalten = [(x0, x0 + SPALTE_B), (x0 + SPALTE_B + STEG, x0 + 2 * SPALTE_B + STEG)]
            for sx0, sx1 in spalten:
                box = [sx0 - BOX_RAND, RAND["top"] - BOX_RAND, sx1 + BOX_RAND, SEITE_H - RAND["bottom"] + BOX_RAND]
                # Riso-Schatten: ein blauer Farbauftrag, leicht versetzt, mit kräftigem Papier-Korn
                # (Absatzschattierung in einem leeren Textrahmen, damit Farben und Korn Farbfelder bleiben)
                r = [box[0] + SCHATTEN_VERSATZ, box[1] + SCHATTEN_VERSATZ, box[2] + SCHATTEN_VERSATZ, box[3] + SCHATTEN_VERSATZ]
                self.flaeche(spread, r, "Spalte Schatten")
                self.rauschen(spread, r, "rahmen")
                i = self.s("frame.create", spread=spread, rect=box, content="unassigned")
                ids = [f"${i}.id"]
                self.s("object.fill", swatch="Papier", ids=ids); self.s("object.stroke", swatch="[None]", ids=ids)
                self.s("object.cornerOptions", shape="rounded", size=9, ids=ids)
                self.rauschen(spread, box, "innen")
            # Seitenzahl (nur im eigenen Chat-Dokument; im Buch kommt sie von dessen Mustervorlage),
            # zweimal leicht versetzt in beiden Farben übereinander gedruckt
            for dx, farbe, multi in ([] if self.dok is not None else ((1.6, "Farbe Sie", False), (0, "Farbe Ich", True))):
                nr = self.s("frame.create", spread=spread, content="text", text="\ue000",
                            rect=[seite_x + 200 + dx, SEITE_H - 52 + dx, seite_x + SEITE_B - 200 + dx, SEITE_H - 22 + dx])
                self.s("text.select", story=f"${nr}.story", anchor=0, focus=3)
                self.s("style.paragraph.apply", name="Seitenzahl")
                self.s("type.char", attrs={"fill": farbe})
                if multi:
                    self.ueberdruck(f"${nr}.id")


    # ---------------------------------------------------------------- Objekte im Text
    @staticmethod
    def farbwert(farbe):
        """"Farbfeld@Tonwert" -> (Farbfeld, Tonwert)."""
        name, _, ton = farbe.partition("@")
        return name, float(ton) if ton else 1.0

    def merke(self, rect):
        if self._bbox is not None:
            self._bbox.append(rect)

    def form(self, rect, farbe, ellipse=False, ecke=0):
        self.merke(rect)
        i = self.s("frame.create", rect=rect, content="unassigned", **({"shape": "ellipse"} if ellipse else {}))
        ids = [f"${i}.id"]
        name, ton = self.farbwert(farbe)
        self.s("object.fill", swatch=name, tint=ton, ids=ids); self.s("object.stroke", swatch="[None]", ids=ids)
        if ecke:
            self.s("object.cornerOptions", shape="rounded", size=ecke, ids=ids)
        return f"${i}.id"

    def dreieck(self, x, y, b, farbe):
        i = self.s("frame.create", rect=[x, y, x + b, y + b], shape="polygon", sides=3, content="unassigned")
        ids = [f"${i}.id"]
        self.s("object.fill", swatch=farbe, ids=ids); self.s("object.stroke", swatch="[None]", ids=ids)
        self.s("transform.rotate", angle=-90, ids=ids)
        return f"${i}.id"


    def deko_bauen(self, obj, X, Y):
        """Kleine Riso-Verzierung neben dem Datum aus Formen in den Farbfeldern."""
        # Farben wie die Blasen: heller Blasenton vorne, Schatten in der anderen Farbe
        # Farben wie der Schatten der Chatblasen (kräftige Töne), ohne eigenen Schatten
        BLASE = {"Farbe Ich": "Ich · Verzierung", "Farbe Sie": "Sie · Verzierung"}
        a, b = (BLASE["Farbe Ich"], BLASE["Farbe Sie"]) if not obj.get("tausch") else (BLASE["Farbe Sie"], BLASE["Farbe Ich"])
        sp, art = obj.get("spiegel"), obj["deko"]
        form, ids = self.form, []
        def mit_schatten(rect, farbe, _schatten=None, **kw):
            ids.append(form(rect, farbe, **kw))
        if art == "balken":
            mit_schatten([X, Y, X + 21, Y + 5], b, a, ecke=2.5)
        elif art == "punkte":
            farben = [a, b, a][::-1] if sp else [a, b, a]
            for k, fb in enumerate(farben):
                x = X + k * 8.5
                mit_schatten([x, Y, x + 6, Y + 6], fb, b if fb == a else a, ellipse=True)
        elif art == "rauten":
            for k in range(3):
                x = X + k * 8.5
                i0 = len(ids)
                mit_schatten([x, Y, x + 5, Y + 5], b if k % 2 else a, a if k % 2 else b)
                for ref in ids[i0:]:
                    self.s("transform.rotate", angle=45, ids=[ref])
        elif art == "raster":
            for r in range(3):
                for k in range(5):
                    d = 2.4 if (k + r) % 2 == 0 else 1.6
                    x, y = X + k * 3.6 + (1.8 if r % 2 else 0), Y + r * 3.2
                    ids.append(form([x, y, x + d, y + d], a if r != 1 else b, ellipse=True))
        elif art == "sterne":
            for k in range(2):
                x = X + k * 13
                vorne = a if k == 0 else b
                ids.append(self.polygon([x, Y, x + 10, Y + 10], vorne, sides=8, starInset=45))
        elif art == "pillen":
            ids.append(form([X, Y, X + 15, Y + 3.6], b, ecke=1.8))
            ids.append(form([X + (0 if sp else 5), Y + 4.2, X + (15 if sp else 20), Y + 7.8], a, ecke=1.8))
        elif art == "kreuze":
            for k in range(2):
                x = X + k * 12
                vorne = a if k == 0 else b
                for dx, fb in ((0, vorne),):
                    ids.append(form([x + dx + 3, Y + dx, x + dx + 6, Y + dx + 9], fb, ecke=0.8))
                    ids.append(form([x + dx, Y + dx + 3, x + dx + 9, Y + dx + 6], fb, ecke=0.8))
        elif art == "ringe":
            for k, fb in enumerate((a, b)):
                x = X + k * 6
                self.merke([x - 1.1, Y - 1.1, x + 11.1, Y + 11.1])
                i = self.s("frame.create", rect=[x, Y, x + 10, Y + 10], shape="ellipse", content="unassigned")
                self.s("object.fill", swatch="[None]", ids=[f"${i}.id"])
                name, ton = self.farbwert(fb)
                self.s("object.stroke", swatch=name, tint=ton, weight=2.2, ids=[f"${i}.id"])
                ids.append(f"${i}.id")
        elif art == "dreiecke":
            for k in range(3):
                x = X + k * 8
                vorne = a if k % 2 == 0 else b
                ids.append(self.polygon([x, Y, x + 7, Y + 7], vorne, sides=3))
        elif art == "zickzack":
            for k in range(4):
                x = X + k * 4.6
                i0 = len(ids)
                ids.append(form([x, Y + 2, x + 7, Y + 4.4], a if k % 2 == 0 else b, ecke=1.1))
                self.s("transform.rotate", angle=40 if k % 2 == 0 else -40, ids=[ids[i0]])
        elif art == "treppe":
            for k, laenge in enumerate((20, 14, 8)):
                x0 = X + (20 - laenge) if not sp else X
                mit_schatten([x0, Y + k * 3.8, x0 + laenge, Y + k * 3.8 + 2.6], a if k != 1 else b, b if k != 1 else a, ecke=1.3)
        elif art == "blumen":
            for k in range(2):
                x = X + k * 12
                vorne = a if k == 0 else b
                ids.append(self.polygon([x, Y, x + 10.5, Y + 10.5], vorne, sides=6, starInset=30))
                ids.append(form([x + 3.5, Y + 3.5, x + 7, Y + 7], "Papier", ellipse=True))
        elif art == "konfetti":
            rnd = random.Random(f"{art}{sp}{obj.get('tausch')}")
            for k in range(5):
                x, y = X + k * 4.4, Y + rnd.uniform(0, 5)
                wahl = rnd.choice(("punkt", "quadrat", "dreieck"))
                fb = rnd.choice((a, b))
                if wahl == "punkt":
                    ids.append(form([x, y, x + 3.2, y + 3.2], fb, ellipse=True))
                elif wahl == "quadrat":
                    ids.append(form([x, y, x + 3, y + 3], fb))
                    self.s("transform.rotate", angle=rnd.choice((15, 30, 60)), ids=[ids[-1]])
                else:
                    ids.append(self.polygon([x, y, x + 3.6, y + 3.6], fb, sides=3))
        elif art == "streifen":
            for k in range(6):
                x = X + k * 3.4
                ids.append(form([x, Y, x + 1.8, Y + 9], a if k % 2 == 0 else b, ecke=0.9))
        return ids

    def korn_svg(self, b, h, menge, ox, oy):
        """Papierfarbene Sprenkel als SVG (gleiche Machart wie das Korn der Blasen in DesignCraft)."""
        name = f"korn_{b:.1f}x{h:.1f}_{menge:.2f}_{ox:.1f}_{oy:.1f}.svg"
        pfad = os.path.join(self.arbeit, "korn", name)
        if os.path.isfile(pfad):
            return pfad
        os.makedirs(os.path.dirname(pfad), exist_ok=True)
        rnd = random.Random(name)
        papier = dict(self.farben())["Papier"]
        punkte = []
        schritt, chance = 1.5, menge * 0.33
        y = 0.0
        while y < h:
            x = 0.0
            while x < b:
                if rnd.random() < chance:
                    g = rnd.random()
                    r = 0.45 + g * 0.35 if g > 0.93 else 0.12 + g * 0.28
                    punkte.append(f'<circle cx="{x + rnd.random() * schritt:.2f}" cy="{y + rnd.random() * schritt:.2f}" r="{r:.2f}"/>')
                x += schritt
            y += schritt
        open(pfad, "w").write(f'<svg xmlns="http://www.w3.org/2000/svg" width="{b:.2f}pt" height="{h:.2f}pt" '
                              f'viewBox="0 0 {b:.2f} {h:.2f}"><g fill="{papier}">{"".join(punkte)}</g></svg>')
        return pfad

    def ueberdruck(self, ref):
        """Die vordere Farbe multipliziert sich mit der hinteren, wie zwei Druckfarben übereinander."""
        self.s("object.opacity", opacity=0.92, blend="multiply", ids=[ref])

    def polygon(self, rect, farbe, sides, starInset=0):
        self.merke(rect)
        i = self.s("frame.create", rect=rect, shape="polygon", sides=sides, starInset=starInset, content="unassigned")
        ids = [f"${i}.id"]
        name, ton = self.farbwert(farbe)
        self.s("object.fill", swatch=name, tint=ton, ids=ids); self.s("object.stroke", swatch="[None]", ids=ids)
        return f"${i}.id"

    def objekt_bauen(self, obj):
        """Legt das Objekt außerhalb der Seite an und gibt den Verweis auf seine ID zurück."""
        X, Y = 1400, 100  # Ablage neben der Seite, bevor es verankert wird
        if obj["art"] == "deko":
            self._bbox = []
            teile = self.deko_bauen(obj, X, Y)
            box, self._bbox = self._bbox, None
            menge = float(self.stil.get("korn_deko_extra", 0))
            if box and menge > 0:
                x0, y0 = min(r[0] for r in box) - 1, min(r[1] for r in box) - 1
                x1, y1 = max(r[2] for r in box) + 1, max(r[3] for r in box) + 1
                svg = self.korn_svg(x1 - x0, y1 - y0, menge, x0, y0)
                k = self.s("frame.create", rect=[x0, y0, x1, y1], content="graphic")
                self.s("file.place", path=svg, frame=f"${k}.id")
                self.s("object.fit", mode="fitContentToFrame", ids=[f"${k}.id"])
                self.s("object.stroke", swatch="[None]", ids=[f"${k}.id"])
                teile.append(f"${k}.id")
            g = self.s("object.group", ids=teile) if len(teile) > 1 else None
            ref = f"${g}.id" if g is not None else teile[0]
            f = self.groesse("datum") / DATUM_PT
            if abs(f - 1) > 1e-6:
                self.s("transform.scale", sx=f, sy=f, ids=[ref])
            # Markiert, damit die Chat-Gestaltung sie mit dem Datum skaliert
            self.s("object.label", label="Verzierung", ids=[ref])
            return ref
        if obj["art"] == "bild":
            i = self.s("frame.create", rect=[X, Y, X + obj["b"], Y + obj["h"]], content="graphic")
            ref = f"${i}.id"
            self.s("file.place", path=obj["pfad"], frame=ref)
            self.s("object.fit", mode=obj["fit"], ids=[ref]); self.s("object.stroke", swatch="[None]", ids=[ref])
            if obj.get("label"):
                # Markiert (Emojis), damit die Chat-Gestaltung sie mit der Schrift skaliert
                self.s("object.label", label=obj["label"], ids=[ref])
            if obj["fit"] == "fillProportionally" and not obj.get("emoji"):
                self.s("object.cornerOptions", shape="rounded", size=4, ids=[ref])
            teile = [ref]
            if obj.get("rahmen"):
                # Dezent: die Schattentöne der Blasen (55 %) statt der vollen Farben
                farbe, andere = obj["rahmen"]
                self.s("object.stroke", swatch=farbe, weight=1.6, align="inside", ids=[ref])
                v = 1.3
                k = self.s("frame.create", rect=[X + v, Y + v, X + obj["b"] + v, Y + obj["h"] + v], content="unassigned")
                kid = f"${k}.id"
                self.s("object.fill", swatch="[None]", ids=[kid])
                self.s("object.stroke", swatch=andere, weight=1.2, align="inside", ids=[kid])
                self.s("object.cornerOptions", shape="rounded", size=4, ids=[kid])
                self.ueberdruck(kid)
                teile.append(kid)
            if obj.get("video"):
                d = min(obj["b"], obj["h"]) * 0.28
                cx, cy = X + obj["b"] / 2, Y + obj["h"] / 2
                kreis = self.form([cx - d / 2, cy - d / 2, cx + d / 2, cy + d / 2], "Papier", ellipse=True)
                pfeil = self.dreieck(cx - d * 0.2, cy - d * 0.25, d * 0.5, "Textfarbe")
                teile += [kreis, pfeil]
            if len(teile) > 1:
                g = self.s("object.group", ids=teile)
                return f"${g}.id"
            return ref
        icon, farbe = obj["icon"], obj.get("farbe", "Textfarbe")
        schatten = "Farbe Sie" if farbe == "Farbe Ich" else "Farbe Ich"
        teile = []
        if icon == "kamera":
            for dx, fb in ((1.1, schatten), (0, farbe)):
                teile.append(self.form([X + dx, Y + dx + 1.4, X + dx + 9, Y + dx + 7.4], fb, ecke=1.4))
                teile.append(self.form([X + dx + 2.8, Y + dx, X + dx + 6.2, Y + dx + 2], fb))
                if dx == 0:
                    self.ueberdruck(teile[-1]); self.ueberdruck(teile[-2])
            teile.append(self.form([X + 2.9, Y + 2.8, X + 6.1, Y + 6.0], "Papier", ellipse=True))
        elif icon == "dokument":
            for dx, fb in ((1.1, schatten), (0, farbe)):
                teile.append(self.form([X + dx, Y + dx, X + dx + 8, Y + dx + 10], fb, ecke=1.2))
                if dx == 0:
                    self.ueberdruck(teile[-1])
            for k in range(3):
                teile.append(self.form([X + 1.8, Y + 2.4 + k * 2.2, X + 6.2, Y + 3.2 + k * 2.2], "Papier"))
        elif icon == "play":
            teile.append(self.dreieck(X, Y, 8, farbe))
        elif icon == "welle":
            seed = int(hashlib.md5(obj.get("seed", "x").encode()).hexdigest(), 16)
            for k in range(26):
                h = 2 + (seed >> (k * 3)) % 7
                teile.append(self.form([X + k * 2.2, Y + 5 - h / 2, X + k * 2.2 + 1.3, Y + 5 + h / 2], farbe, ecke=0.6))
        g = self.s("object.group", ids=teile) if len(teile) > 1 else None
        return f"${g}.id" if g is not None else teile[0]

    # ---------------------------------------------------------------- Gesamtablauf
    def einrichtung(self):
        """Farbfelder, Formate und Chat-Mustervorlage."""
        s = self.s
        for name, farbe in self.farben():
            s("swatch.create", name=name, color=farbe)
        # Abgeleitete Varianten als Tonwert-Farbfelder: folgen ihrer Hauptfarbe, einzeln anpassbar
        varianten = []
        for name, basis, ton in VARIANTEN:
            s("swatch.create", name=name, base=basis, tint=self.stil.get("tonwerte", {}).get(name, ton))
            varianten.append(name)
        if self.gruppe:
            # Weitere Personen: Hauptfarbe und Schattenton; alle Blasen der anderen in einem hellen Ton
            for p in self.personen:
                if p["key"] not in ("Ich", "Sie"):
                    s("swatch.create", name=farbe_von(p["key"]), color=p["farbe"])
                    s("swatch.create", name=schatten_von(p["key"]), base=farbe_von(p["key"]), tint=0.55)
                    varianten += [farbe_von(p["key"]), schatten_von(p["key"])]
            s("swatch.create", name="Andere · Blase", base="Textfarbe", tint=0.12)
            varianten.append("Andere · Blase")
        if self.dok is None or self.n_farbe("Farbe Ich") not in self.dok["swatches"]:
            s("swatch.newColorGroup", name="Jahresbuch", swatches=[n for n, _ in self.farben()] + varianten)
        self.formate()
        if self.dok is None:
            self.parent_index = 0
            self.mustervorlage()
        elif not any(pa.get("label") == self.vorlage for pa in self.dok.get("parents", [])):
            # Auf der ersten Mustervorlage des Buchs aufbauen: deren Seitenzahl & Co. zeigen sich mit.
            eltern = self.dok.get("parents", [])
            s("layout.parents.new", prefix=self.vorlage.split("-")[0], name="Chat", basedOn=eltern[0]["label"] if eltern else None)
            s("layout.marginsAndColumns", parent=self.vorlage, margins=RAND, columns=2, gutter=STEG)
            self.parent_index = len(self.dok.get("parents", []))
            self.mustervorlage()

    def inhalt(self, story, versatz=0):
        """Den Chat-Text in `story` formatieren und Bilder, Emojis und Icons verankern; `versatz`:
        wo er im Text beginnt (Bytes), beim Fortsetzen hinter dem vorhandenen Chat."""
        s = self.s
        text = "\n".join(a.inhalt for a in self.absaetze)
        # Byte-Position jedes Zeichens, einmal gezählt (statt jedes Mal den Anfang neu zu kodieren)
        offs = [0]
        for ch in text:
            offs.append(offs[-1] + len(ch.encode()))
        def b(n):
            return versatz + offs[n]
        pos = 0
        einfuegen = []
        for a in self.absaetze:
            inhalt = a.inhalt
            s("text.select", story=story, anchor=b(pos), focus=b(pos + len(inhalt)))
            # clearOverrides: Abweichungen, die der Absatz beim Einfügen vom Nachbarn geerbt hat, fallen weg.
            s("style.paragraph.apply", name=a.stil, clearOverrides=True)
            p = pos
            for t, cs in a.laeufe:
                if cs:
                    s("text.select", story=story, anchor=b(p), focus=b(p + len(t)))
                    s("style.character.apply", name=cs)
                p += len(t)
            einfuegen += [(pos + k, obj) for k, obj in a.objekte]
            pos += len(inhalt) + 1
        # Objekte von hinten nach vorne verankern, damit vordere Positionen gültig bleiben
        for k, obj in sorted(einfuegen, key=lambda x: x[0], reverse=True):
            ref = self.objekt_bauen(obj)
            s("text.select", story=story, anchor=b(k), focus=b(k + 1))
            s("text.delete")
            s("anchored.insert", ids=[ref], story=story, pos=b(k), position="inline", yOffset=obj.get("y", -1.0))
        return text

    def schreiben(self, pfad, schritte):
        os.makedirs(os.path.dirname(os.path.abspath(pfad)) or ".", exist_ok=True)
        with open(pfad, "w", encoding="utf-8") as fh:
            fh.write("\n".join(json.dumps(x, ensure_ascii=False) for x in schritte))

    def einsetzen(self, text, anfang, ende, laenge):
        """`text` (ganze Absätze) anstelle der Bytes anfang..ende des Textes "__STORY__" setzen, der
        laenge Bytes lang ist: in einen leeren Text, hinten an (anfang = ende = laenge, Fortsetzen)
        oder anstelle eines Zeitraums (Neu aufbauen). Der Text davor und dahinter bleibt."""
        s = self.s
        if anfang < ende < laenge and text:
            # Mitten im Chat: die Zeilenschaltung vor dem nächsten Tag bleibt stehen (und mit ihr
            # dessen Absatz samt Handarbeit); ersetzt wird bis davor.
            ende -= 1
            neu = text
        elif ende < laenge:
            neu = text + "\n" if text else ""
        elif anfang == laenge and anfang > 0:
            neu = "\n" + text if text else ""
        elif not text and anfang > 0:
            anfang -= 1  # sonst bliebe hinten ein leerer Absatz
            neu = ""
        else:
            neu = text
        s("text.select", story="__STORY__", anchor=anfang, focus=ende)
        if not neu:
            if ende > anfang:
                s("text.delete")
            return
        s("text.insert", text=neu, raw=True)
        # Eingefügter Text erbt das Zeichenformat vor der Einfügestelle, hinten an einem Chat die
        # Uhrzeit: zurück auf [Ohne], die Zeichenformate setzt inhalt().
        s("text.select", story="__STORY__", anchor=anfang, focus=anfang + len(neu.encode()))
        s("style.character.apply", name="[None]")
        self.inhalt("__STORY__", versatz=anfang + (1 if neu.startswith("\n") else 0))

    def skript(self):
        s = self.s
        if self.dok is not None:
            # Ins bestehende Buch: zwei Skripte. Die Einrichtung läuft zuerst; dann setzt DesignCraft
            # den Text in den gewählten (oder neu angelegten) Rahmen, Platzhalter "__STORY__".
            self.einrichtung()
            einrichtung, self.steps = self.steps, []
            text = "\n".join(a.inhalt for a in self.absaetze)
            self.einsetzen(text, *(getattr(self.a, "ersetzen", None) or (0, 0, 0)))
            self.schreiben(self.a.einrichtung_ausgabe, einrichtung)
            self.schreiben(self.a.ausgabe, self.steps)
            return len(einrichtung) + len(self.steps)
        s("file.new", preset="A4", pages=1, facingPages=True, columns=2, gutter=STEG, margins=RAND, title="Jahresbuch – Chat")
        self.einrichtung()
        text = "\n".join(a.inhalt for a in self.absaetze)
        x0 = RAND["inside"]  # Seite 1 ist eine rechte Seite: innen links
        spalte1 = [x0, RAND["top"], x0 + SPALTE_B, SEITE_H - RAND["bottom"]]
        spalte2 = [x0 + SPALTE_B + STEG, RAND["top"], x0 + 2 * SPALTE_B + STEG, SEITE_H - RAND["bottom"]]
        f = s("frame.create", spread=0, rect=spalte1, content="text", text=text)
        self.inhalt(f"${f}.story")
        # Rahmen verketten: zweite Spalte, dann so viele Seiten wie nötig
        s("text.thread", **{"from": f"${f}.id"}, rect=spalte2, spread=0, autoflow=True)
        self.schreiben(self.a.ausgabe, self.steps)
        return len(self.steps)


def chats_auflisten(export):
    """Die Chats im Export, für die Auswahl im Jahresbuch-Fenster: [{chat, name, nachrichten, von,
    bis, gruppe, personen: [{name, nachrichten}]}], die mit den meisten Nachrichten zuerst."""
    daten = json.load(open(export, encoding="utf-8"))
    out = []
    for jid, chat in daten.items():
        msgs = [m for m in chat.get("messages", {}).values() if m.get("message_type") not in AUSBLENDEN]
        if not msgs:
            continue
        zeiten = sorted(m["timestamp"] for m in msgs)
        sender = {}
        for m in msgs:
            if not m.get("from_me") and m.get("sender"):
                sender[m["sender"]] = sender.get(m["sender"], 0) + 1
        out.append({"chat": jid, "name": chat.get("name") or jid.split("@")[0], "nachrichten": len(msgs),
                    "von": datetime.fromtimestamp(zeiten[0], TZ).date().isoformat(),
                    "bis": datetime.fromtimestamp(zeiten[-1], TZ).date().isoformat(),
                    "gruppe": len(sender) > 1,
                    "personen": [{"name": k, "nachrichten": v} for k, v in sorted(sender.items(), key=lambda kv: -kv[1])]})
    out.sort(key=lambda c: -c["nachrichten"])
    return out


def zaehlen(export, chat, von, bis):
    """Nachrichten und Fotos/Videos eines Chats im Zeitraum (für die Anzeige im Jahresbuch-Fenster)."""
    daten = json.load(open(export, encoding="utf-8"))
    c = daten.get(chat) if chat and chat in daten else next(iter(daten.values()))
    v = date.fromisoformat(von) if von else None
    b = date.fromisoformat(bis) if bis else None
    msgs = [m for m in c.get("messages", {}).values() if m.get("message_type") not in AUSBLENDEN
            and (not v or zeit(m).date() >= v) and (not b or zeit(m).date() <= b)]
    return {"nachrichten": len(msgs), "fotos": sum(1 for m in msgs if m.get("message_type") in (1, 2) and m.get("media"))}


def main():
    if "--liste" in sys.argv:
        # Nur die Chats im Export auflisten: designcraft_import.py --liste EXPORT
        print(json.dumps(chats_auflisten(sys.argv[sys.argv.index("--liste") + 1]), ensure_ascii=False))
        return
    if "--zaehlen" in sys.argv:
        # Nur zählen: designcraft_import.py --zaehlen EXPORT [CHAT] [VON] [BIS]
        rest = sys.argv[sys.argv.index("--zaehlen") + 1:] + [None] * 3
        print(json.dumps(zaehlen(rest[0], rest[1] or None, rest[2] or None, rest[3] or None)))
        return
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("export"); ap.add_argument("medien"); ap.add_argument("ausgabe")
    ap.add_argument("--chat", help="Kennung des Chats im Export (siehe --liste); ohne: der erste")
    ap.add_argument("--personen", help="Personen als JSON (Liste oder Datei): [{key, name, ids, farbe}]")
    ap.add_argument("--nummer", type=int, default=1, help="Nummer des Chats im Buch (ab 2: eigene Formate und Farbfelder)")
    ap.add_argument("--personen-ausgabe", help="Datei, in die die Personen des Chats (JSON) geschrieben werden")
    ap.add_argument("--zeitraum-ausgabe", help="Datei für den ersten und letzten Tag, die wirklich eingefügt werden (JSON {von, bis})")
    ap.add_argument("--stil", default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "stil.json"))
    ap.add_argument("--schrift", help='Schrift für den Chat, z. B. "Space Grotesk" (Standard) oder "Work Sans"')
    ap.add_argument("--von"); ap.add_argument("--bis"); ap.add_argument("--emoji-font"); ap.add_argument("--arbeitsordner")
    ap.add_argument("--dokument", help="Ins bestehende Dokument: document.inspect als JSON-Datei (DesignCraft-Menü Jahresbuch)")
    ap.add_argument("--einrichtung-ausgabe", help="mit --dokument: Datei für das Einrichtungs-Skript")
    ap.add_argument("--ersetzen", type=int, nargs=3, metavar=("ANFANG", "ENDE", "LAENGE"),
                    help="mit --dokument: den Chat anstelle der Bytes ANFANG..ENDE des Textes setzen, der LAENGE Bytes "
                         "lang ist (Fortsetzen: alle drei = LAENGE; Neu aufbauen: der Bereich des Zeitraums)")
    args = ap.parse_args()
    imp = Importer(args)
    imp.aufbauen()
    if not imp.absaetze:
        sys.exit("Im gewählten Zeitraum gibt es keine Nachrichten.")
    if args.personen_ausgabe:
        with open(args.personen_ausgabe, "w", encoding="utf-8") as fh:
            json.dump(imp.personen, fh, ensure_ascii=False)
    if args.zeitraum_ausgabe and imp.tage:
        with open(args.zeitraum_ausgabe, "w", encoding="utf-8") as fh:
            json.dump({"von": imp.tage[0].isoformat(), "bis": imp.tage[-1].isoformat()}, fh)
    n = imp.skript()
    print(f"{len(imp.absaetze)} Absätze, {n} Befehle -> {args.ausgabe}")


if __name__ == "__main__":
    main()
