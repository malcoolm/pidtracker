# PID Tracker

Osobní PWA pro cestování pražskou integrovanou dopravou (PID) a evidenci jízd. Běží offline
nad jízdním řádem PID, živá data bere z PID GTFS-Realtime.

- **Vyhledávání spojení** (domovská obrazovka): odjezd v / příjezd do, z aktuální polohy
  nebo ze zastávky, s pěšími přestupy; nastavitelná rezerva na první nástup a vyloučené
  druhy dopravy. Detail spojení s časovou osou, zpožděním a stavem „stihnu / nestihnu“.
- **Odjezdy ze zastávky** podle polohy, se zpožděním v reálném čase. Vyhledávání zastávek
  bez diakritiky, řazené podle vzdálenosti.
- **Detail spoje**: celá trasa s časy a aktuální polohou vozu.
- **Mimořádnosti PID**: seznam, upozornění v aplikaci, značky na trase.
- **Evidence jízd**: nástup, linka a výstup předvyplněné z polohy a jízdního řádu, číslo
  vozu z živých dat, SPZ, dopravce a poznámka. Jízdy se ukládají offline a synchronizují
  do sdílené Google tabulky.
- **Historie a statistiky** po osobách (i společně „Oba“), obdobích, dopravcích a vozech.
- Vzhled v barvách PID, sezónní halloweenský motiv, automatická aktualizace aplikace.

Dokumentace:

- Návrh: [`docs/superpowers/specs/`](docs/superpowers/specs/) (původní návrh
  [`2026-10-25-pidtracker-design.md`](docs/superpowers/specs/2026-10-25-pidtracker-design.md),
  vyhledávání spojení [`2026-11-15-vyhledavani-spojeni-design.md`](docs/superpowers/specs/2026-11-15-vyhledavani-spojeni-design.md))
- Verze a poznámky k vydání: [GitHub Releases](https://github.com/malcoolm/pidtracker/releases)
- Nastavení Google: [`docs/setup-google.md`](docs/setup-google.md)
- Nastavení GitHub Pages: [`docs/setup-github-pages.md`](docs/setup-github-pages.md)
- Testování na iPhonu: [`docs/testing-iphone.md`](docs/testing-iphone.md)

## Struktura

| Crate | Účel |
|---|---|
| `crates/pid-core` | Čistá logika s testy: formát a dotazy jízdního řádu, vyhledávání spojení (RAPTOR), GTFS-Realtime (zpoždění, poloha vozů, mimořádnosti), model jízdy, řádky tabulky, slučování synchronizace, statistiky |
| `crates/gtfs-prep` | CLI/knihovna: PID GTFS → kompaktní `timetable-<verze>.pidt` (~1,3 MB, formát PIDT3 s pěšími přestupy) + `manifest.json` |
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

### Vydání verze

Po zvýšení `version` v `Cargo.toml` a pushi do `main` označte verzi anotovaným tagem;
zpráva tagu bez prvního řádku (např. „PID Tracker 0.7.0“) se stane poznámkami k vydání:

```
git tag -a v0.7.0 -F poznamky.md
git push origin v0.7.0
```

Workflow `.github/workflows/release.yml` vytvoří GitHub Release a přiloží
`pidtracker-v0.7.0.zip` (obsah `dist/` pro cestu `/pidtracker/`). Jízdní řády v zipu
jsou z data sestavení. K tagu, jehož commit workflow ještě neobsahuje, se build dodá
ručně: `gh workflow run release.yml -f tag=v0.5.0`.

## Data

- Jízdní řády: PID open data (<https://pid.cz/o-systemu/opendata/>), soubor
  `https://data.pid.cz/PID_GTFS.zip`.
- Jízdy: Google tabulka uživatele, list `rides` (jeden řádek = jedna jízda, smazané mají
  `deleted = TRUE`) a `people` (e-mail, přezdívka).
- Číslo vozu: veřejný GTFS-Realtime feed poloh vozidel PID (`pid_core::realtime`), jen tramvaje/autobusy/trolejbusy.
- Budoucí přechod na Golemio API: implementovat trait
  `pid_core::timetable::query::DepartureSource`.
