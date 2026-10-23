"""Genera crates/ui/ui/icons.slint a partir de Lucide (ISC, https://lucide.dev), una vez; el resultado se versiona.
Uso: python scripts/build-icons.py
Los círculos, rectángulos y líneas de los SVG se pasan a comandos de path, que es lo que entiende Slint (sin SVG en el binario)."""
import re, urllib.request, xml.etree.ElementTree as ET

# nombre en lumen -> nombre en Lucide. Los nombres viejos se conservan (Rust los usa: "shield", "windows", "game"...).
ICONS = {
    "home": "house", "list": "list", "power": "power", "disk": "hard-drive", "sliders": "sliders-horizontal",
    "bolt": "zap", "box": "package", "chip": "cpu", "memory": "memory-stick", "monitor": "monitor",
    "board": "circuit-board", "windows": "app-window", "gear": "settings", "search": "search", "game": "gamepad-2",
    "battery": "battery-full", "megaphone": "megaphone", "bulb": "lightbulb", "bell": "bell", "cloud": "cloud",
    "print": "printer", "bluetooth": "bluetooth", "map": "map", "refresh": "refresh-cw", "globe": "globe",
    "wrench": "wrench", "clock": "clock", "eye": "eye", "shield": "shield",
    # nuevos
    "check": "check", "x": "x", "chevron-right": "chevron-right", "chevron-down": "chevron-down", "chevron-up": "chevron-up",
    "trash": "trash-2", "undo": "undo-2", "shield-check": "shield-check", "shield-alert": "shield-alert",
    "alert": "triangle-alert", "info": "info", "circle-check": "circle-check", "folder": "folder", "file": "file",
    "history": "history", "activity": "activity", "sort": "arrow-up-down", "copy": "copy", "lock": "lock",
    "sparkles": "sparkles", "broom": "brush-cleaning", "arrow-up": "arrow-up", "arrow-down": "arrow-down",
}

def f(v): return ("%g" % float(v))

def to_path(svg):
    root = ET.fromstring(svg)
    out = []
    for e in root.iter():
        t = e.tag.split("}")[1]
        a = e.attrib
        if t == "path": out.append("M0 0 " + a["d"])  # M0 0: un "m" relativo del SVG pasa a ser absoluto
        elif t == "line": out.append(f"M{a['x1']} {a['y1']} L{a['x2']} {a['y2']}")
        elif t in ("polyline", "polygon"):
            p = re.split(r"[ ,]+", a["points"].strip()); pts = [f"{p[i]} {p[i+1]}" for i in range(0, len(p), 2)]
            out.append("M" + " L".join(pts) + (" Z" if t == "polygon" else ""))
        elif t == "circle":
            cx, cy, r = float(a["cx"]), float(a["cy"]), float(a["r"])
            out.append(f"M{f(cx-r)} {f(cy)} A{f(r)} {f(r)} 0 1 0 {f(cx+r)} {f(cy)} A{f(r)} {f(r)} 0 1 0 {f(cx-r)} {f(cy)}")
        elif t == "rect":
            x, y, w, h = (float(a[k]) for k in ("x", "y", "width", "height")); r = float(a.get("rx", 0))
            out.append(f"M{f(x+r)} {f(y)} H{f(x+w-r)} A{f(r)} {f(r)} 0 0 1 {f(x+w)} {f(y+r)} V{f(y+h-r)} A{f(r)} {f(r)} 0 0 1 {f(x+w-r)} {f(y+h)} H{f(x+r)} A{f(r)} {f(r)} 0 0 1 {f(x)} {f(y+h-r)} V{f(y+r)} A{f(r)} {f(r)} 0 0 1 {f(x+r)} {f(y)} Z")
        elif t == "ellipse":
            cx, cy, rx, ry = (float(a[k]) for k in ("cx", "cy", "rx", "ry"))
            out.append(f"M{f(cx-rx)} {f(cy)} A{f(rx)} {f(ry)} 0 1 0 {f(cx+rx)} {f(cy)} A{f(rx)} {f(ry)} 0 1 0 {f(cx-rx)} {f(cy)}")
    return " ".join(out)

lines = []
for k, n in ICONS.items():
    svg = urllib.request.urlopen(f"https://cdn.jsdelivr.net/npm/lucide-static/icons/{n}.svg").read().decode()
    lines.append(f'        kind == "{k}" ? "{to_path(svg)}" :')

open("crates/ui/ui/icons.slint", "w", encoding="utf-8").write(f'''import {{ Tokens }} from "tokens.slint";

// Iconos Lucide (ISC, lucide.dev) como path. GENERADO por scripts/build-icons.py: no editar a mano.
// Caja de 24, trazo fino y redondeado. El tamaño sale de Tokens.icon-*.
export component Icon inherits Path {{
    in property <string> kind;
    in property <color> tint: Tokens.text-secondary;
    width: Tokens.icon-sm;
    height: Tokens.icon-sm;
    viewbox-width: 24;
    viewbox-height: 24;
    stroke: tint;
    stroke-width: Tokens.icon-stroke;
    stroke-line-cap: round;
    stroke-line-join: round;
    commands:
{chr(10).join(lines)}
        "M12 3 L20 6 V12 C20 17 16 20 12 21 C8 20 4 17 4 12 V6 Z"; // por defecto: escudo
}}
''')
print(len(lines), "iconos")
