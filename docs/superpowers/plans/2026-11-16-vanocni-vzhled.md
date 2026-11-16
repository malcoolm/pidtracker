# Vánoční období a barevné schéma (#4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Od soboty před 1. adventní nedělí do 28. 12. má aplikace vánoční barvy (světlý i tmavý režim) a vánoční písmo na nadpisech a ovládání.

**Architecture:** `pid-core::season` vrací aktuální sezónu (`Halloween` / `Vanoce`). Aplikace podle ní nastaví třídu `halloween` nebo `vanoce` na `<html>`, CSS pod touto třídou přepíše barevné proměnné a písmo. Halloween přechází na stejný mechanismus beze změny vzhledu.

**Tech Stack:** Rust (chrono), Leptos CSR/WASM, CSS proměnné, woff2 písmo.

**Spec:** `docs/superpowers/specs/2026-11-16-vanocni-vzhled-design.md`

## Global Constraints

- Vánoční období: od soboty před 1. adventní nedělí (neděle v rozmezí 27. 11.–3. 12.) do 28. 12. včetně.
- Vynucení: `?vanoce=1` (a dál `?halloween=1`).
- Písmo „Vanocni Hory“ (weight 700) jen na: `h1–h3`, `.topbar-title`, `.menu-item`, `.menu-link`, `.primary`, `.secondary`, `.jc-label`, `.tag`, `.chip`. Nikdy na časy, zastávky ani `.badge`.
- Barvy přesně podle tabulky ve spec (světlý / tmavý); `--warn*`, `--ok*`, `--danger*` beze změny.
- Za nadpisy `h2` o Vánocích „ 🎄“.
- Commity odkazují na `(#4)`; issue se zavírá až po kontrolním bodu na iPhonu.

## Review Focus

- Přelom roku a období: 29. 12. až 23. 10. žádná sezóna; 1. 1. není Vánoce (období končí 28. 12., nepřetéká do ledna) — test v Task 1.
- Telefon přepne světlý/tmavý režim za běhu: barvy CSS se přepnou samy (media query), `theme-color` se srovná při dalším návratu do popředí — ověřit v kontrolním bodu (Task 3).
- Halloween po refaktoru vypadá jako dřív (tmavý, dekorace, 🎃) — ověřit `?halloween=1` (Task 2).
- Offline první spuštění o Vánocích: písmo je v precache service workeru — ověřit v `dist/sw.js` (Task 3).
- Současné `?halloween=1&vanoce=1`: vyhraje Vánoce (jedna třída najednou) — test v Task 2 není možný (WASM), ověřit ručně.

---

### Task 1: Sezóny v `pid-core`

**Files:**
- Modify: `crates/pid-core/src/season.rs`

**Interfaces:**
- Produces: `pub enum Season { Halloween, Vanoce }` (derive `Clone, Copy, Debug, PartialEq, Eq`), `pub fn season(date: NaiveDate) -> Option<Season>`, `pub fn advent_saturday(year: i32) -> NaiveDate`, `pub fn is_vanoce(date: NaiveDate) -> bool`. `is_halloween` zůstává beze změny (používá ho `xtask`).

- [ ] **Step 1: Write the failing tests** (do `mod tests`)

```rust
#[test]
fn advent_saturday_by_year() {
    let d = |y, m, dd| NaiveDate::from_ymd_opt(y, m, dd).unwrap();
    assert_eq!(advent_saturday(2026), d(2026, 11, 28));
    assert_eq!(advent_saturday(2027), d(2027, 11, 27));
    assert_eq!(advent_saturday(2028), d(2028, 12, 2));
    assert_eq!(advent_saturday(2029), d(2029, 12, 1));
    assert_eq!(advent_saturday(2030), d(2030, 11, 30));
}

#[test]
fn vanoce_window() {
    let d = |y, m, dd| NaiveDate::from_ymd_opt(y, m, dd).unwrap();
    assert!(!is_vanoce(d(2026, 11, 27)));
    assert!(is_vanoce(d(2026, 11, 28)));
    assert!(is_vanoce(d(2026, 12, 24)));
    assert!(is_vanoce(d(2026, 12, 28)));
    assert!(!is_vanoce(d(2026, 12, 29)));
    assert!(!is_vanoce(d(2027, 1, 1)));
    assert!(!is_vanoce(d(2028, 12, 1)));
    assert!(is_vanoce(d(2028, 12, 2)));
}

#[test]
fn season_picks_period() {
    let d = |m, dd| NaiveDate::from_ymd_opt(2026, m, dd).unwrap();
    assert_eq!(season(d(10, 31)), Some(Season::Halloween));
    assert_eq!(season(d(12, 1)), Some(Season::Vanoce));
    assert_eq!(season(d(11, 15)), None);
    assert_eq!(season(d(6, 1)), None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p pid-core season`
Expected: FAIL (nedefinované `advent_saturday`, `is_vanoce`, `season`, `Season`).

- [ ] **Step 3: Implement** `advent_saturday` (první neděle ≥ 27. 11. daného roku minus 1 den), `is_vanoce(date)` = `advent_saturday(date.year()) <= date && date <= 28. 12.`, `season(date)` (Halloween, jinak Vánoce, jinak `None`). Doc komentáře česky jako ve zbytku souboru.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p pid-core season`
Expected: PASS (4 testy včetně `halloween_window`).

- [ ] **Step 5: Commit**

```bash
git add crates/pid-core/src/season.rs
git commit -m "feat(core): Christmas season from the Saturday before Advent to 28 Dec (#4)"
```

### Task 2: Obecný modul sezón v aplikaci (Halloween beze změny)

**Files:**
- Rename: `crates/app/src/halloween.rs` → `crates/app/src/season.rs`
- Modify: `crates/app/src/main.rs` (`mod halloween` → `mod season`), `crates/app/src/app.rs:8,32-35,59-61`, `crates/app/Cargo.toml` (web-sys feature `MediaQueryList`)

**Interfaces:**
- Consumes: `pid_core::season::{season, Season}` (Task 1).
- Produces: `pub fn current_season() -> Option<Season>` (vynucení `?vanoce=1` má přednost před `?halloween=1`, pak datum), `pub fn apply_season() -> Option<Season>` (nastaví třídu `halloween`/`vanoce` na `<html>`, druhou odebere, a `meta[name=theme-color]`), `HalloweenDeco` beze změny. Barvy `theme-color`: běžně `#c40c00` (světlý) / `#252525` (tmavý) — oprava: dnešní `THEME_COLOR = "#00559e"` je zbytek modré z doby před 0.5.0 a neodpovídá liště; Halloween `#2a2b30`; Vánoce `#1f5b3a` / `#143823`. Tmavý režim zjistit `window.match_media("(prefers-color-scheme: dark)")`.

- [ ] **Step 1: Přejmenovat soubor a upravit API** podle Interfaces; v `app.rs` signál `RwSignal<Option<Season>>`, `<Show when=move || season.get() == Some(Season::Halloween)>` pro `HalloweenDeco`, komentář u přepočtu při návratu do popředí zachovat.

- [ ] **Step 2: Clippy**

Run: `cargo clippy -p app --target wasm32-unknown-unknown --locked -- -D warnings`
Expected: bez varování.

- [ ] **Step 3: Sestavit a ověřit Halloween**

Run: `MSYS_NO_PATHCONV=1 cargo --locked xtask dist --public-url / --no-data` a `grep -c halloween dist/app-*.js`
Expected: build „✅ success“; Halloween se vynutí `?halloween=1` (ruční kontrola v prohlížeči nebo v kontrolním bodu, Task 3).

- [ ] **Step 4: Commit**

```bash
git add -A crates/app/src crates/app/Cargo.toml
git commit -m "refactor(app): generic seasonal theme module (#4)"
```

### Task 3: Vánoční barvy, písmo a kontrolní bod

**Files:**
- Create: `crates/app/public/fonts/vanocni-hory-700.woff2`, `crates/app/public/fonts/LICENSE-vanocni-hory.txt`
- Modify: `crates/app/style.css` (za halloweenský blok), `docs/testing-iphone.md` (nový kontrolní bod I)

**Interfaces:**
- Consumes: třída `vanoce` na `<html>` (Task 2).

- [ ] **Step 1: Vyrobit webové písmo**

Run: `python -P assets/fonts/vanocni-hory/doplnit-cestinu.py <MountainsofChristmas-Bold.ttf z google/fonts> crates/app/public/fonts/vanocni-hory-700.woff2` a zkopírovat `assets/fonts/vanocni-hory/LICENSE.txt` → `crates/app/public/fonts/LICENSE-vanocni-hory.txt`.
Expected: „ok …woff2“, soubor ~55 kB.

- [ ] **Step 2: CSS**
  - `@font-face { font-family: "Vanocni Hory"; font-weight: 700; font-display: swap; src: url("fonts/vanocni-hory-700.woff2") format("woff2"); }`
  - `html.vanoce { … }` světlé hodnoty a `@media (prefers-color-scheme: dark) { html.vanoce { … } }` tmavé hodnoty z tabulky ve spec (`--bg`, `--surface`, `--fg`, `--muted`, `--border`, `--accent`, `--accent-text`, `--accent-fg`, `--accent-soft`, `--bar-bg`, `--bar-line`); nadpisová barva jako `--season-head` (`#1f5b3a` / `#e9c46a`).
  - Horní lišta: spodní linka `3px solid var(--bar-line)` jen pod `html.vanoce` (pokud ji `.topbar` už nemá).
  - Písmo: `html.vanoce :is(h1, h2, h3, .topbar-title, .menu-item, .menu-link, .primary, .secondary, .jc-label, .tag, .chip) { font-family: "Vanocni Hory", "Fira Sans", cursive; font-weight: 700; }`, nadpisy `color: var(--season-head)`, `html.vanoce h2::after { content: " 🎄"; }`. Velikost u `.jc-label`, `.tag`, `.chip` o 1–2 px větší, pokud je text drobnější než ve Fira Sans.

- [ ] **Step 3: Sestavit a ověřit precache**

Run: `MSYS_NO_PATHCONV=1 cargo --locked xtask dist --public-url / --no-data` a `grep -c "vanocni-hory-700.woff2" dist/sw.js`
Expected: build „✅ success“, grep `1`.

- [ ] **Step 4: Kontrolní bod I v `docs/testing-iphone.md`** — položky: `?vanoce=1` ve světlém i tmavém režimu (barvy, zlatá linka, písmo, 🎄, čeština Ř/ů/ď v nadpisech a menu), časy/zastávky/čísla linek ve Fira Sans, přepnutí režimu za běhu, `?halloween=1` vypadá jako dřív, offline (režim letadlo) písmo zůstává, bez parametru 16. 11. běžný vzhled.

- [ ] **Step 5: Testy, clippy, commit, push**

Run: `cargo test --workspace --exclude app --locked` a clippy jako v Task 2.
Expected: PASS, bez varování.

```bash
git add crates/app/public/fonts crates/app/style.css docs/testing-iphone.md
git commit -m "feat(app): Christmas colours and font, light and dark (#4)"
git push origin main
```

- [ ] **Step 6: Nástěnka** — #4 zůstává *Rozpracováno* do kontrolního bodu I; po potvrzení uživatele zavřít `gh issue close 4 --comment "…"` a stav *Hotovo*.
