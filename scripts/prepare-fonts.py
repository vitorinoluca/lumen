"""Genera los fuentes embebidos de lumen (subconjunto latino + cifras tabulares fijas en Inter).
Uso: python scripts/prepare-fonts.py InterVariable.ttf JetBrainsMono-Regular.ttf
Salida: crates/ui/ui/fonts/Inter-lumen.ttf y JetBrainsMono-lumen.ttf. Inter: SIL OFL 1.1. JetBrains Mono: SIL OFL 1.1."""
import sys
from fontTools import subset
from fontTools.ttLib import TTFont

# Basic Latin, Latin-1, Latin Extended-A, puntuación general, flechas, signo menos, euro, símbolos usados en la UI
UNI = "U+0020-007E,U+00A0-017F,U+2010-2027,U+2030-203A,U+20AC,U+2190-2193,U+2212,U+2022,U+00B7,U+2026,U+2191,U+2193"
out = "crates/ui/ui/fonts/"

def run(src, dst, features):
    o = subset.Options()
    o.layout_features = features
    o.name_IDs = [1, 2, 3, 4, 6]
    o.notdef_outline = True
    o.hinting = False
    f = TTFont(src)
    s = subset.Subsetter(o)
    s.populate(unicodes=subset.parse_unicodes(UNI))
    s.subset(f)
    return f

# Inter: se conserva tnum para después aplicarlo de forma permanente.
f = run(sys.argv[1], None, ["tnum", "kern", "calt", "ccmp", "locl", "mark", "mkmk"])
gsub = f["GSUB"].table
cmap = f.getBestCmap()
subs = {}
for fr in gsub.FeatureList.FeatureRecord:
    if fr.FeatureTag == "tnum":
        for li in fr.Feature.LookupListIndex:
            for st in gsub.LookupList.Lookup[li].SubTable:
                st = getattr(st, "ExtSubTable", st)
                subs.update(getattr(st, "mapping", {}))
for t in f["cmap"].tables:
    for cp, g in list(t.cmap.items()):
        if g in subs:
            t.cmap[cp] = subs[g]
f.save(out + "Inter-lumen.ttf")
print("tnum fijado en", len(subs), "glifos")

# JetBrains Mono: ya es monoespaciada; sin ligaduras.
run(sys.argv[2], None, ["kern"]).save(out + "JetBrainsMono-lumen.ttf")
