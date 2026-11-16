# Vánoční Hory (Vanocni Hory)

Písmo [Mountains of Christmas](https://fonts.google.com/specimen/Mountains+of+Christmas)
od Tart Workshop (Font Diner, Inc.), doplněné o česká písmena. Používá se pro vánoční vzhled
PID Trackeru (#4).

## Co je změněno

Původní písmo obsahuje á é í ó ú ý š ž, ale chybí mu **č ď ě ň ř ť ů Č Ď Ě Ň Ř Ť Ů**.
Ta jsou doplněná jako složené glyfy z vlastních tvarů písma:

- základní písmeno + háček (`caron`) nebo kroužek (`ring`) písma, vycentrované nad písmenem
  a ve výšce odvozené od původních š/Š, ž/Ž; u Ř je háček ručně posunutý nad oblouk,
- ď a ť mají háček jako apostrof (`quoteright`) vpravo nahoře, jak je v češtině zvykem.

Jiné glyfy, metriky ani kerning se nemění. Kerning pro nová písmena není.

## Název

Copyright původního písma uvádí vyhrazený název (Reserved Font Name) „Mountains of
Christmas“, upravená verze proto nese název **Vanocni Hory** (rodina `Vanocni Hory`,
PostScript `VanocniHory-Regular` / `VanocniHory-Bold`).

## Licence

Apache License 2.0, viz [`LICENSE.txt`](LICENSE.txt). Copyright (c) 2010 2011, Font Diner, Inc
DBA Tart Workshop. Úpravy popsané výše.

## Znovuvytvoření

Zdroj: `MountainsofChristmas-Regular.ttf` a `-Bold.ttf` z
[google/fonts](https://github.com/google/fonts/tree/main/apache/mountainsofchristmas).
Skript potřebuje Python a `fonttools` (`pip install fonttools brotli`):

```
python -P doplnit-cestinu.py MountainsofChristmas-Bold.ttf VanocniHory-Bold.ttf
python -P doplnit-cestinu.py MountainsofChristmas-Bold.ttf vanocni-hory-700.woff2   # podmnožina pro web
```
