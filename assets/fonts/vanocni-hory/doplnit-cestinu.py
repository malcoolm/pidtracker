"""Doplní do Mountains of Christmas česká písmena (č ď ě ň ř ť ů, malá i velká)
jako složené glyfy z vlastního základního písmene a háčku/kroužku/apostrofu.

Použití: python -P patch_moc.py <vstup.ttf> <výstup.ttf | výstup.woff2 (podmnožina pro web)>
"""
import sys

from fontTools.pens.recordingPen import DecomposingRecordingPen
from fontTools.subset import Options, Subsetter
from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._g_l_y_f import Glyph, GlyphComponent

src, dst = sys.argv[1], sys.argv[2]
f = TTFont(src)
glyf, hmtx, cmap = f["glyf"], f["hmtx"], f.getBestCmap()


def bbox(name):
    g = glyf[name]
    g.recalcBounds(glyf)
    return g.xMin, g.yMin, g.xMax, g.yMax


CARON, RING, APOS = cmap[0x2C7], cmap[0x2DA], cmap[0x2019]

# (nový znak, základ, akcent, mezera nad základem)
LOWER_GAP, UPPER_GAP = 60, 55
# Ruční posun akcentu doleva u písmen, jejichž obrys přesahuje doprava (nožička R).
X_TWEAK = {"R": -75}
PLAN = [
    ("č", "c", CARON, LOWER_GAP), ("ě", "e", CARON, LOWER_GAP), ("ň", "n", CARON, LOWER_GAP),
    ("ř", "r", CARON, LOWER_GAP), ("ů", "u", RING, LOWER_GAP - 10),
    ("Č", "C", CARON, UPPER_GAP), ("Ď", "D", CARON, UPPER_GAP), ("Ě", "E", CARON, UPPER_GAP),
    ("Ň", "N", CARON, UPPER_GAP), ("Ř", "R", CARON, UPPER_GAP), ("Ť", "T", CARON, UPPER_GAP),
    ("Ů", "U", RING, UPPER_GAP - 10),
]


def component(name, x, y):
    c = GlyphComponent()
    c.glyphName, c.x, c.y, c.flags = name, int(round(x)), int(round(y)), 0
    return c


def add(ch, base, comps, advance):
    name = f"uni{ord(ch):04X}"
    g = Glyph()
    g.numberOfContours = -1
    g.components = [component(base, 0, 0)] + comps
    glyf[name] = g
    g.recalcBounds(glyf)
    hmtx[name] = (advance, g.xMin)
    for table in f["cmap"].tables:
        if table.isUnicode():
            table.cmap[ord(ch)] = name
    order = f.getGlyphOrder()
    if name not in order:
        f.setGlyphOrder(order + [name])


for ch, base_ch, accent, gap in PLAN:
    base = cmap[ord(base_ch)]
    bx0, _, bx1, by1 = bbox(base)
    ax0, ay0, ax1, _ = bbox(accent)
    dx = (bx0 + bx1) / 2 - (ax0 + ax1) / 2 + X_TWEAK.get(base_ch, 0)
    dy = by1 + gap - ay0
    add(ch, base, [component(accent, dx, dy)], hmtx[base][0])

# ď, ť: háček jako apostrof vpravo nahoře
for ch, base_ch, overlap in (("ď", "d", 25), ("ť", "t", 45)):
    base = cmap[ord(base_ch)]
    _, _, bx1, by1 = bbox(base)
    ax0, _, ax1, ay1 = bbox(APOS)
    dx = bx1 - overlap - ax0
    dy = by1 - ay1 + 10
    advance = max(hmtx[base][0], int(dx + ax1 + 20))
    add(ch, base, [component(APOS, dx, dy)], advance)

# Kontrola: každý nový glyf jde vykreslit (složené odkazy sedí).
gs = f.getGlyphSet()
for ch in "čďěňřťůČĎĚŇŘŤŮ":
    gs[f.getBestCmap()[ord(ch)]].draw(DecomposingRecordingPen(gs))

# Původní písmo má vyhrazený název „Mountains of Christmas“ (Reserved Font Name),
# upravená verze proto nese jiný název.
FAMILY = "Vanocni Hory"
style = f["name"].getName(2, 3, 1, 0x409).toUnicode()
ps = f"VanocniHory-{style}"
names = {
    1: FAMILY,
    3: f"1.003-cz1;{ps}",
    4: f"{FAMILY} {style}",
    6: ps,
    10: "Mountains of Christmas (Tart Workshop) doplněné o česká písmena č ď ě ň ř ť ů Č Ď Ě Ň Ř Ť Ů "
        "složená z vlastních tvarů písma. Upraveno pro PID Tracker.",
}
for nid, val in names.items():
    f["name"].setName(val, nid, 3, 1, 0x409)
    if val.isascii():
        f["name"].setName(val, nid, 1, 0, 0)
f["name"].removeNames(nameID=16)
f["name"].removeNames(nameID=17)
if "DSIG" in f:
    del f["DSIG"]
f["maxp"].recalc(f)

if dst.endswith(".ttf"):
    f.save(dst)
else:
    opts = Options()
    opts.flavor = "woff2"
    opts.layout_features = ["*"]
    opts.name_IDs = ["*"]
    sub = Subsetter(opts)
    text = "".join(chr(c) for c in range(0x20, 0x7F)) + "".join(chr(c) for c in range(0xA0, 0x180)) + "„“‚‘’–—…•€"
    sub.populate(text=text)
    sub.subset(f)
    f.flavor = "woff2"
    f.save(dst)
print("ok", dst)
