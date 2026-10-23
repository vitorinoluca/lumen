"""Valida el contraste WCAG de los pares de color de crates/ui/ui/tokens.slint (lee los hex del archivo)."""
import re, sys

src = open("crates/ui/ui/tokens.slint", encoding="utf-8").read()
tok = {}
for name, d, l in re.findall(r"property <color> ([\w-]+): dark \? #(\w+) : #(\w+);", src):
    tok[name] = (d[:6], l[:6])

def lum(h):
    c = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    c = [x / 12.92 if x <= .03928 else ((x + .055) / 1.055) ** 2.4 for x in c]
    return .2126 * c[0] + .7152 * c[1] + .0722 * c[2]

def ratio(a, b):
    a, b = sorted((lum(a), lum(b)), reverse=True)
    return (a + .05) / (b + .05)

# (primer plano, fondo, mínimo)
pairs = [(f, b, 4.5) for f in ("text-primary", "text-secondary") for b in ("bg-base", "bg-surface", "bg-elevated", "bg-overlay")]
pairs += [(f, b, 4.5) for f in ("accent", "success", "warning", "danger", "info") for b in ("bg-base", "bg-surface", "bg-elevated")]
pairs += [("on-accent", a, 4.5) for a in ("accent", "accent-hover", "accent-active")]
pairs += [(f, f + "-subtle", 4.5) for f in ("success", "warning", "danger", "info")]
pairs += [("accent", "accent-subtle", 4.5), ("text-primary", "accent-subtle", 4.5)]
pairs += [("border-control", b, 3.0) for b in ("bg-surface", "bg-elevated")]
pairs += [("focus-ring", b, 3.0) for b in ("bg-base", "bg-surface")]

bad = 0
for i, theme in enumerate(("oscuro", "claro")):
    for f, b, mn in pairs:
        r = ratio(tok[f][i], tok[b][i])
        if r < mn:
            bad += 1
            print(f"FALLA {theme}: {f} sobre {b} = {r:.2f} (mínimo {mn})")
print("contraste:", "todo cumple" if not bad else f"{bad} pares fallan")
sys.exit(1 if bad else 0)
