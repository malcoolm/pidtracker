# PID Tracker

Osobní PWA pro evidenci jízd pražskou integrovanou dopravou (PID). Podle polohy předvyplní
zastávku, podle jízdního řádu nabídne linku, jízdy ukládá offline a synchronizuje do
Google tabulky. Ukazuje statistiky po osobách a obdobích.

- Návrh: [`docs/superpowers/specs/2026-10-25-pidtracker-design.md`](docs/superpowers/specs/2026-10-25-pidtracker-design.md)
- Nastavení Google: [`docs/setup-google.md`](docs/setup-google.md)
- Nastavení GitHub Pages: [`docs/setup-github-pages.md`](docs/setup-github-pages.md)
- Testování na iPhonu: [`docs/testing-iphone.md`](docs/testing-iphone.md)

## Struktura

| Crate | Účel |
|---|---|
| `crates/pid-core` | Čistá logika s testy: formát a dotazy jízdního řádu, model jízdy, řádky tabulky, slučování synchronizace, statistiky |
| `crates/gtfs-prep` | CLI/knihovna: PID GTFS → kompaktní `timetable-<verze>.pidt` (~1,2 MB) + `manifest.json` |
| `crates/app` | Leptos (CSR) PWA kompilovaná do WebAssembly; service worker `public/sw.js` |
| `xtask` | `cargo xtask dist` – sestaví složku `dist/` pro GitHub Pages |

## Prerekvizity

```powershell
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
```

## Vývoj

```powershell
# testy (vše kromě UI)
cargo test --workspace --exclude app

# lokální sestavení s daty a test v prohlížeči
$env:PIDTRACKER_GOOGLE_CLIENT_ID = "…apps.googleusercontent.com"
cargo xtask dist --public-url / --fetch-gtfs      # nebo --gtfs .gtfs/feed (už stažené)
python -m http.server 8090 --bind 127.0.0.1 --directory dist
# → http://127.0.0.1:8090/
```

V Git Bash pište `MSYS_NO_PATHCONV=1 cargo xtask dist --public-url / …`, jinak Git Bash
přepíše `/` na cestu `C:/Program Files/Git/`.

Kontrola dat jízdního řádu:

```powershell
cargo run -p gtfs-prep --release -- --fetch --out target/data --probe "Náměstí Míru"
```

## Nasazení

GitHub Actions (`.github/workflows/deploy.yml`) při push do `main`, každé pondělí
(PID GTFS platí jen 14 dní) a ručně: testy → `cargo xtask dist --fetch-gtfs` → GitHub Pages.
Aplikace si novou verzi stáhne sama (autoupdate na bezpečné obrazovce).

## Data

- Jízdní řády: PID open data (<https://pid.cz/o-systemu/opendata/>), soubor
  `https://data.pid.cz/PID_GTFS.zip`.
- Jízdy: Google tabulka uživatele, list `rides` (jeden řádek = jedna jízda, smazané mají
  `deleted = TRUE`) a `people` (e-mail, přezdívka).
- Budoucí přechod na Golemio API: implementovat trait
  `pid_core::timetable::query::DepartureSource`.
