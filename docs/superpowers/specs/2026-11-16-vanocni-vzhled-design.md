# Vánoční období a barevné schéma (#4)

Story #4 epiku #3 *Vánoční vzhled aplikace*. Navazují #5 ilustrace, #6 sníh a #7 ikona.

## Proč

Pro pobavení a vánoční atmosféru: od prvního adventního víkendu do 28. 12. má aplikace
vánoční barvy a písmo. Tato story položí základ (období, barvy, písmo), na který ilustrace,
sníh a ikona navážou.

## Cíle

- Vánoční období: **od soboty před 1. adventní nedělí do 28. 12. včetně**, počítané pro
  každý rok (1. adventní neděle = první neděle 27. 11.–3. 12.). 2026: od soboty 28. 11.
- Vánoční barvy **ve světlém i tmavém režimu** podle nastavení telefonu (zvolená varianta B
  „světlý i tmavý“, paleta A „Klasické Vánoce“).
- Vánoční písmo **Vanocni Hory** (Mountains of Christmas s doplněnou češtinou,
  `assets/fonts/vanocni-hory/`) na: nadpisy, název v horní liště, položky menu, tlačítka a
  štítky (např. „nejrychlejší“). Za nadpisy obrazovek 🎄 (jako 🎃 o Halloweenu).
- Vynucení pro test parametrem `?vanoce=1` (jako `?halloween=1`).

## Mimo rozsah / nechceme

- Ilustrace (#5), sníh (#6), ikona aplikace (#7).
- Zdobné písmo pro běžný text: časy, zastávky, nástupiště, zpoždění a čísla linek zůstávají
  ve **Fira Sans** (zvoleno po porovnání s Berkshire Swash).
- Nesmí se zhoršit čitelnost: kontrast textu, zpoždění (oranžová) a „stihneš“ (zelená)
  musí zůstat jasně rozlišitelné v obou režimech.

## Přístupy

1. **Obecné sezóny (zvoleno).** `pid-core::season` vrací aktuální sezónu
   (`Halloween` / `Vanoce` / žádná); aplikace podle ní nastaví třídu na `<html>`
   (`halloween` nebo `vanoce`) a CSS přepíše barevné proměnné. Halloween se jen převede na
   stejný mechanismus. Ilustrace a sníh pak jen čtou aktuální sezónu.
2. Zkopírovat halloweenský modul pro Vánoce – rychlé, ale dvě kopie téže logiky a další
   sezóna by přidala třetí.
3. Sezóny jako konfigurace (data, barvy v JSON) – zbytečně obecné pro dvě sezóny ročně.

## Návrh

### Období (`crates/pid-core/src/season.rs`)

- `enum Season { Halloween, Vanoce }`, `fn season(date) -> Option<Season>`.
- `fn advent_saturday(year) -> NaiveDate`: neděle v rozmezí 27. 11.–3. 12. minus 1 den.
- Vánoce: `advent_saturday(rok) <= date <= 28. 12.`; Halloween beze změny (24. 10.–4. 11.).
- Testy: začátky 2026 = 28. 11., 2027 = 27. 11., 2028 = 2. 12., 2029 = 1. 12.,
  2030 = 30. 11.; den před začátkem ne; 28. 12. ano, 29. 12. ne.
- `is_halloween` zůstává (používá ho `xtask` pro ikonu) nebo se nahradí `season()`.

### Aplikace

- `crates/app/src/halloween.rs` → obecný modul sezón: `apply_season()` nastaví třídu
  `halloween`/`vanoce` na `<html>` (druhou odebere) a barvu `meta[name=theme-color]`.
  Vynucení `?halloween=1` / `?vanoce=1`. `HalloweenDeco` se zobrazuje dál jen o Halloweenu.
- Barva `theme-color` o Vánocích: `#1f5b3a` (světlý), `#143823` (tmavý); stačí podle
  aktuálního `prefers-color-scheme` při startu.

### Barvy (`crates/app/style.css`, `html.vanoce` a `@media (prefers-color-scheme: dark)`)

| Proměnná | Světlý | Tmavý |
|---|---|---|
| `--bg` | `#fbf7ef` | `#0f1a14` |
| `--surface` | `#ffffff` | `#182820` |
| `--fg` | `#23302a` | `#f3ede0` |
| `--muted` | `#4f5b52` | `#b9b3a3` |
| `--border` | `#eadfca` | `#2a3d31` |
| `--accent` | `#b3121b` | `#d6303a` |
| `--accent-text` | `#a10f17` | `#ff7b7b` |
| `--accent-fg` | `#ffffff` | `#ffffff` |
| `--accent-soft` | `#f7e6e2` | `#3a1c1c` |
| `--bar-bg` | `#1f5b3a` | `#143823` |
| `--bar-line` | `#d4a33b` | `#d4a33b` |
| nadpisy | `#1f5b3a` | `#e9c46a` |

Ostatní proměnné (`--warn*`, `--ok*`, `--danger*`) zůstávají z běžného vzhledu, pokud
v kontrolním bodu na iPhonu nebude problém s kontrastem. Horní lišta dostane zlatou spodní
linku (`--bar-line`).

### Písmo

- Webová podmnožina `VanocniHory-Bold` (latin + latin-ext, woff2, ~55 kB) do
  `crates/app/public/fonts/` vyrobená skriptem `assets/fonts/vanocni-hory/doplnit-cestinu.py`,
  vedle ní `LICENSE-vanocni-hory.txt` (Apache 2.0).
- `@font-face { font-family: "Vanocni Hory"; font-weight: 700; font-display: swap; }`.
  Prohlížeč písmo stáhne až při použití, mimo Vánoce se nenačítá.
- Použití jen pod `html.vanoce`: `h1–h3`, `.topbar-title`, `.menu-item`, `.menu-link`,
  `.primary`, `.secondary`, štítky (`.jc-label` a obdobné). Ne na časy, zastávky ani
  `.badge`.
- Offline: písmo musí být dostupné i bez sítě – ověřit, že ho service worker uloží
  (precache seznam v `xtask`, nebo cache při prvním použití).

## Na builderovi

- Přesné selektory štítků a tlačítek podle současného CSS; velikost písma Vánočních Hor
  případně o kousek zvětšit (je drobnější než Fira Sans).
- Zda sjednotit `is_halloween` do `season()` i v `xtask` (ikona řeší #7).
- Doplnit kontrolní bod do `docs/testing-iphone.md` (světlý/tmavý režim, `?vanoce=1`).

## Kritéria přijetí (#4, upravená)

- [ ] *Když* je datum od soboty před 1. adventní nedělí do 28. 12. včetně, *pak* má aplikace
  vánoční barvy a písmo; mimo období běžný vzhled. Ověřeno testy pro 2026–2030.
- [ ] Vánoční vzhled jde vynutit parametrem `?vanoce=1`.
- [ ] Vzhled odpovídá nastavení telefonu (světlý / tmavý) a je v obou čitelný.
- [ ] Časy, zastávky a čísla linek zůstávají ve Fira Sans; české znaky ve Vánočních Horách
  se zobrazují správně (bez náhradního písma).
- [ ] Funguje offline (písmo z cache).
