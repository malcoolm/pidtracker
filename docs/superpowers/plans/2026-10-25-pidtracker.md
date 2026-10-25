# PID Tracker – implementační plán

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** PWA v Rustu/WASM, která podle polohy a předpočítaného jízdního řádu PID pomáhá zaznamenávat jízdy MHD dvou osob, ukládá je offline a synchronizuje s Google Sheets a ukazuje statistiky.

**Architecture:** Cargo workspace se čtyřmi crates. `pid-core` obsahuje čistou, testovatelnou logiku: formát jízdního řádu, dotazy, model jízdy, řádky tabulky, slučování synchronizace a statistiky. `gtfs-prep` převádí PID GTFS na kompaktní binární soubor. `app` je tenké Leptos CSR UI s interop na prohlížeč (geolokace, IndexedDB, Google). `xtask` sestaví distribuční složku pro GitHub Pages. Veškerá logika, kterou jde testovat bez prohlížeče, patří do `pid-core`.

**Tech Stack:** Rust (stable, ověřeno 1.99.1), target `wasm32-unknown-unknown`, Leptos (poslední stabilní, CSR), trunk, serde + postcard, flate2 (rust backend), chrono + chrono-tz, csv + zip + ureq (gtfs-prep), rexie (IndexedDB), gloo-net, uuid (v4 + js), Google Identity Services + Sheets API v4, GitHub Actions + GitHub Pages.

**Spec:** `docs/superpowers/specs/2026-10-25-pidtracker-design.md`

## Zjištění z dat (2026-10-25)

- Zdroj: `https://data.pid.cz/PID_GTFS.zip` (52 MB zip, 144 MB `shapes.txt` nepotřebujeme).
- `feed_info.txt`: platnost **jen 14 dní** (20261025–20261107). Týdenní přegenerování je nutnost.
- `stop_times.txt`: 1 953 251 řádků, 95 803 spojů, ale jen **7 749 unikátních sekvencí zastávek** a **12 563 profilů (sekvence + časové offsety)**. Odhad komprimovaných dat: 2–4 MB, tedy hluboko pod limitem 15 MB. Horizont se nezkracuje, bere se celý feed.
- `stops.txt`: 18 666 stanovišť (`location_type=0`), ostatní typy (stanice, vstupy, uzly) se ignorují. Sloupce: `stop_id, stop_name, stop_lat, stop_lon, …, location_type, parent_station, …, platform_code, asw_node_id, …`.
- `routes.txt`: `route_id, agency_id, route_short_name, route_long_name, route_type, route_url, route_color, …`. Hodnoty `route_type`: 0 tram, 1 metro, 2 vlak, 3 bus, 4 přívoz, 7 lanovka, 11 trolejbus. Pole jsou v uvozovkách a obsahují čárky, parsovat se musí skutečným CSV parserem.
- `stop_times.txt`: `trip_id, arrival_time, departure_time, stop_id, stop_sequence, stop_headsign, pickup_type, drop_off_type, …`. Časy mohou přesáhnout 24:00:00.
- Některé soubory mají UTF-8 BOM (např. `levels.txt`). CSV čtečka musí BOM odstranit u všech souborů.

## Global Constraints

- UI je celé v češtině.
- Cíl: iPhone 15 a 16, Safari a PWA přidaná na plochu (aktuální iOS). Aplikace běží pod podcestou GitHub Pages (`/pidtracker/`). Všechny URL v aplikaci jsou relativní.
- Žádný backend. Google API se volají přímo z prohlížeče. Scope: `https://www.googleapis.com/auth/spreadsheets openid email`.
- **Záznam jízdy nikdy nečeká na přihlášení ani na síť.** Vše se nejdřív zapíše do IndexedDB.
- Každý může upravovat a mazat jen svoje jízdy (`ride.person == přihlášený e-mail`).
- Smazání = `deleted = TRUE`, řádky se z tabulky nikdy neodstraňují. Sloupce se jen přidávají na konec, nikdy se nepřeskupují.
- Zápis do Sheets s `valueInputOption=RAW`, čtení s výchozím `FORMATTED_VALUE` (vše se čte jako řetězce).
- Časy v tabulce jsou ve formátu ISO 8601 s offsetem (RFC 3339), lokální čas Europe/Prague.
- Jízdní řád jen z PID GTFS, žádné Golemio. Přístup přes trait `DepartureSource`, aby šlo zdroj později vyměnit.
- Komprimovaná data jízdního řádu mají nejvýš 15 MB.
- Okruh nejbližších zastávek je 100 m. Okno nabídky odjezdů je −5 až +30 minut od teď.
- Viditelná verze aplikace (`semver+githash.yyyymmddHHMM`) i verze dat. Autoupdate se aktivuje jen na bezpečné obrazovce.
- Nasazení: GitHub Actions → GitHub Pages, při push do `main` a jednou týdně (cron).
- Google Client ID se předává při buildu přes env `PIDTRACKER_GOOGLE_CLIENT_ID` (není tajný).

## Review Focus

1. **Špatné GPS nebo zamítnutá poloha.** Uživatel musí najít zastávku vyhledáním podle názvu bez diakritiky („namesti miru“). Test `search_ignores_diacritics` v Task 6.
2. **Jízda přes půlnoc a změna času.** Noční spoj „24:10:00“ ze sobotní služby se musí nabídnout v neděli v 00:05. GTFS čas v den změny času se počítá jako „poledne − 12 h“. Testy `night_trip_after_midnight` (Task 6) a `gtfs_time_on_dst_end_day` (Task 4).
3. **Zapomenutý Výstup.** Jízda trvající 14 hodin nesmí zkreslit celkový čas ve statistikách (do času se počítají jen jízdy kratší než 6 h, do počtu jízd všechny). Test `forgotten_exit_excluded_from_time` v Task 8.
4. **Uživatel ručně přeřadí nebo upraví řádky v tabulce.** Push musí řádky hledat podle `id` z čerstvého pullu a nerozbitý řádek bez `id` přeskočit bez pádu. Testy `push_uses_fresh_row_numbers` a `parse_rows_skips_garbage` v Task 7.
5. **Prošlá nebo chybějící data jízdního řádu.** Aplikace musí ukázat varování a dovolit ruční zadání zastávky i linky. Test `timetable_status_expiry` v Task 6, ruční zadání v Task 10.

---

## Struktura souborů

```
Cargo.toml                         workspace
.cargo/config.toml                 alias xtask + getrandom wasm_js cfg pro wasm32
.gitignore                         target/, dist/, .gtfs/
crates/pid-core/
  src/lib.rs                       re-exporty
  src/time.rs                      GTFS čas → okamžik (Europe/Prague), období
  src/geo.rs                       haversine, prostorová mřížka
  src/text.rs                      fold diakritiky, vyhledávání
  src/timetable/mod.rs             datové typy Timetable (serde)
  src/timetable/codec.rs           encode/decode (postcard + deflate)
  src/timetable/query.rs           DepartureSource + dotazy, TimetableIndex
  src/ride.rs                      Ride, RideStatus, RouteKind
  src/sheet.rs                     HEADERS, to_row/from_row, parse_rows
  src/sync.rs                      merge_remote, plan_push, Outbox
  src/stats.rs                     Period, StatsQuery, compute
crates/gtfs-prep/
  src/lib.rs                       build_timetable(dir) → Timetable
  src/main.rs                      CLI: --fetch | --gtfs <zip|dir> --out <dir>
  tests/fixtures/mini/*.txt        mini GTFS feed (Task 5)
  tests/build.rs                   test stavby
  tests/queries.rs                 testy dotazů nad fixture
crates/app/
  Cargo.toml, build.rs             verze z env
  index.html, Trunk.toml, style.css
  public/manifest.webmanifest, public/sw.js, public/icons/*
  src/main.rs, src/app.rs          root, Screen enum, globální stav
  src/config.rs                    CLIENT_ID, verze
  src/interop/{geo.rs, gis.rs, sw.rs}
  src/google/{auth.rs, sheets.rs}
  src/storage.rs                   IndexedDB (rides, outbox, kv)
  src/timetable_loader.rs
  src/sync.rs                      sync engine
  src/update.rs                    autoupdate
  src/screens/{onboarding,home,board,running,alight,history,edit,stats,settings}.rs
  src/components/{stop_picker,route_picker,sync_badge,toast}.rs
xtask/src/main.rs                  `cargo xtask dist`
.github/workflows/deploy.yml
docs/setup-google.md, docs/setup-github-pages.md, docs/testing-iphone.md
README.md
```

Rozhodnutí builderu: data jízdního řádu se drží v **Cache Storage** (service worker), ne v IndexedDB. Soubor je neměnný a pojmenovaný verzí, takže cache-first je přirozené. V IndexedDB jsou jen jízdy, outbox a nastavení.

---

### Task 1: Workspace, prázdná PWA s verzí, `cargo xtask dist`

**Files:**
- Create: `Cargo.toml`, `.cargo/config.toml`, `.gitignore`, `crates/pid-core/{Cargo.toml,src/lib.rs}`, `crates/app/{Cargo.toml,build.rs,index.html,Trunk.toml,style.css,src/main.rs,src/app.rs,src/config.rs}`, `crates/app/public/{manifest.webmanifest,sw.js,icons/icon-192.png,icons/icon-512.png,icons/apple-touch-icon.png}`, `xtask/{Cargo.toml,src/main.rs}`

**Interfaces:**
- Produces: `app::config::{APP_VERSION: &str, BUILD_ID: &str, BUILT_AT: &str, GOOGLE_CLIENT_ID: Option<&str>}`. `dist/version.json` = `{"app_version":"0.1.0","build_id":"0.1.0+a1b2c3d.202610251200","built_at":"2026-10-25T12:00:00Z"}`. V `dist/sw.js` je placeholder `__BUILD_ID__` nahrazený skutečným build id.
- `xtask` CLI: `cargo xtask dist [--public-url /pidtracker/] [--gtfs <path> | --fetch-gtfs] [--no-data]`.

- [ ] **Step 1: Prerekvizity.** `rustup target add wasm32-unknown-unknown` a `cargo install trunk --locked`. Ověř `trunk --version`.
- [ ] **Step 2: Workspace.** Members: `crates/pid-core`, `crates/gtfs-prep` (prázdný lib + bin, naplní se v Task 5), `crates/app`, `xtask`. Alias `.cargo/config.toml`: `[alias] xtask = "run -p xtask --"`. Pro wasm32 přidej `rustflags = ['--cfg', 'getrandom_backend="wasm_js"']` (vyžaduje getrandom 0.3 přes uuid).
- [ ] **Step 3: Test xtask.** V `xtask/src/main.rs` napiš unit testy:

```rust
#[test]
fn build_id_format() {
    let id = make_build_id("0.1.0", "a1b2c3d", chrono::Utc.with_ymd_and_hms(2026,10,25,12,0,0).unwrap());
    assert_eq!(id, "0.1.0+a1b2c3d.202610251200");
}
#[test]
fn sw_placeholder_replaced() {
    assert_eq!(patch_sw("const V='__BUILD_ID__';", "X"), "const V='X';");
}
```
- [ ] **Step 4:** `cargo test -p xtask` musí selhat (funkce neexistují).
- [ ] **Step 5: Implementace xtask.** `make_build_id(version, git_hash, now) -> String`, `patch_sw(src, build_id) -> String` a `dist`. `dist` udělá tohle: zjistí git hash (`git rev-parse --short HEAD`, při neuložených změnách přidá `-dirty`). Spustí `trunk build --release --public-url <url>` v `crates/app` s env `PIDTRACKER_BUILD_ID`, `PIDTRACKER_BUILT_AT`. Zapíše `dist/version.json` a opraví `dist/sw.js`. Data (Task 5) zatím přeskočí s `--no-data`. Výstup je v `dist/` v kořeni repa (Trunk.toml `dist = "../../dist"`).
- [ ] **Step 6: App skeleton.** `build.rs` předá `PIDTRACKER_BUILD_ID`/`BUILT_AT` (fallback `"dev"`) přes `cargo:rustc-env` a `cargo:rerun-if-env-changed`. `config.rs` čte `env!`/`option_env!`. `app.rs` vykreslí nadpis „PID Tracker“ a patičku `v{APP_VERSION} ({BUILD_ID})`. `index.html` obsahuje odkazy trunk na `style.css`, `rel="copy-dir" href="public"` (obsah složky public se kopíruje do kořene distribuce, viz Trunk dokumentace, případně použij `copy-file` pro každý soubor). Dále `<link rel="manifest">`, `apple-touch-icon`, `<meta name="apple-mobile-web-app-capable" content="yes">` a `viewport` s `viewport-fit=cover`.
- [ ] **Step 7: Service worker `public/sw.js`.** `const BUILD_ID='__BUILD_ID__'`, cache `app-${BUILD_ID}` a `pid-data`. `install`: `self.skipWaiting()`. `activate`: smaže cache `app-*` kromě aktuální a zavolá `clients.claim()`. `fetch` (jen GET, jen stejný origin): navigace, `version.json` a `data/manifest.json` jsou network-first s fallbackem na cache. `data/*.pidt` je cache-first v `pid-data`. Ostatní (hashované assety) cache-first v `app-*`. Registrace v `main.rs` přes `navigator.serviceWorker.register('sw.js')`.
- [ ] **Step 8: Ikony.** Vygeneruj jednoduché PNG ikony (písmeno „P“ na barvě PID, 192/512/180 px). Libovolný nástroj, výstup se commitne.
- [ ] **Step 9: Ověření.** `cargo test -p xtask` PASS. `cargo xtask dist --no-data` vytvoří `dist/index.html`, `dist/sw.js` (bez `__BUILD_ID__`), `dist/version.json`, `dist/manifest.webmanifest`. `trunk serve` (v `crates/app`) zobrazí na `http://localhost:8080` nadpis a verzi.
- [ ] **Step 10: Commit** `feat: workspace, PWA skeleton with visible version, xtask dist`.

### Task 2: GitHub repozitář, Actions a Pages – **kontrolní bod A (iPhone)**

**Files:**
- Create: `.github/workflows/deploy.yml`, `docs/setup-github-pages.md`, `docs/testing-iphone.md`

**Interfaces:**
- Consumes: `cargo xtask dist` (Task 1). Repo variable `GOOGLE_CLIENT_ID` → env `PIDTRACKER_GOOGLE_CLIENT_ID`.

- [ ] **Step 1: Workflow.** Triggery `push` na `main`, `schedule: cron '30 3 * * 1'` (pondělí 03:30 UTC) a `workflow_dispatch`. Job: checkout, `dtolnay/rust-toolchain@stable` s targetem wasm32, instalace trunk (`taiki-e/install-action` nebo `cargo install trunk --locked` s cache), `Swatinem/rust-cache`, `cargo test --workspace --exclude app`, `cargo xtask dist --public-url /pidtracker/ --fetch-gtfs` (do Task 5 `--no-data`), `actions/upload-pages-artifact` z `dist`, `actions/deploy-pages`. Permissions `pages: write`, `id-token: write`.
- [ ] **Step 2: Dokumentace.** `docs/setup-github-pages.md`: veřejný repo `pidtracker`, Settings → Pages → Source „GitHub Actions“, nastavení proměnné `GOOGLE_CLIENT_ID`. `docs/testing-iphone.md`: jak otevřít v Safari, Sdílet → Přidat na plochu, kde je vidět verze. Každý další kontrolní bod sem přidá svou sekci „Co vyzkoušet“.
- [ ] **Step 3: Vytvoření vzdáleného repa (potvrdit s uživatelem před spuštěním).** `gh repo create pidtracker --public --source . --push` a zapnutí Pages podle návodu.
- [ ] **Step 4: Ověření.** Workflow doběhne zeleně. `https://<user>.github.io/pidtracker/` ukazuje verzi. `.../version.json` vrací JSON.
- [ ] **Step 5: Kontrolní bod A (uživatel, iPhone).** Otevřít URL v Safari, přidat na plochu, spustit z plochy, vidět verzi. Pak v režimu letadlo znovu otevřít: aplikace se načte z cache.
- [ ] **Step 6: Commit** `ci: deploy to GitHub Pages`.

### Task 3: Google přihlášení a Sheets klient – **kontrolní bod B (iPhone, spike OAuth)**

**Files:**
- Create: `crates/app/src/interop/gis.rs`, `crates/app/src/google/{mod.rs,auth.rs,sheets.rs}`, `crates/app/src/screens/onboarding.rs`, `docs/setup-google.md`
- Modify: `crates/app/index.html` (`<script src="https://accounts.google.com/gsi/client" async>`), `crates/pid-core/src/sheet.rs` (jen `parse_sheet_id`)
- Test: `crates/pid-core/src/sheet.rs` (unit testy)

**Interfaces:**
- Produces (`pid-core::sheet`): `pub fn parse_sheet_id(input: &str) -> Option<String>`.
- Produces (`app::google::auth`): `pub struct Token { pub access_token: String, pub expires_at_ms: f64, pub email: String }`; `pub fn current_token() -> Option<Token>` (platný a z localStorage `pt.token`); `pub async fn sign_in(login_hint: Option<&str>) -> Result<Token, AuthError>` (volat jen z uživatelského gesta); `pub fn sign_out()`.
- Produces (`app::google::sheets`): `pub struct SheetsClient { token: String, sheet_id: String }` s metodami `async fn get_values(&self, range: &str) -> Result<Vec<Vec<String>>, SheetsError>`, `async fn append(&self, sheet: &str, rows: Vec<Vec<String>>) -> Result<(), SheetsError>`, `async fn batch_update_values(&self, data: Vec<(String /*range*/, Vec<Vec<String>>)>) -> Result<(), SheetsError>`, `async fn ensure_sheets(&self, sheets: &[(&str, &[&str])]) -> Result<(), SheetsError>` (založí chybějící listy přes `spreadsheets.batchUpdate addSheet` a zapíše chybějící hlavičky do řádku 1. Existující hlavičky jen doplní na konci, nikdy nepřepisuje). `SheetsError::Unauthorized` při HTTP 401/403 s neplatným tokenem, `SheetsError::Forbidden` při chybějícím přístupu k souboru, `SheetsError::Network`, `SheetsError::Other(String)`.

- [ ] **Step 1: Failing test** v `sheet.rs`:

```rust
#[test]
fn parse_sheet_id_variants() {
    let id = "1AbC-dEf_123456789012345678901234567890";
    assert_eq!(parse_sheet_id(&format!("https://docs.google.com/spreadsheets/d/{id}/edit#gid=0")).as_deref(), Some(id));
    assert_eq!(parse_sheet_id(id).as_deref(), Some(id));
    assert_eq!(parse_sheet_id("  https://example.com/x "), None);
    assert_eq!(parse_sheet_id(""), None);
}
```
- [ ] **Step 2:** `cargo test -p pid-core parse_sheet_id` FAIL.
- [ ] **Step 3:** Implementuj `parse_sheet_id`. Vezmi segment za `/spreadsheets/d/`, případně celý vstup, a přijmi ho, pokud odpovídá `[A-Za-z0-9_-]{25,}`.
- [ ] **Step 4:** PASS.
- [ ] **Step 5: GIS interop + auth.** `wasm_bindgen` extern na `google.accounts.oauth2.initTokenClient({client_id, scope, callback, error_callback})` a `requestAccessToken({prompt, hint})`, zabalené do future. Po získání tokenu načti e-mail z `https://openidconnect.googleapis.com/v1/userinfo` a ulož `Token` do localStorage. Pokud chybí `GOOGLE_CLIENT_ID`, zobraz chybu „Chybí Google Client ID v buildu“.
- [ ] **Step 6: Sheets klient** přes `gloo-net`, base `https://sheets.googleapis.com/v4/spreadsheets/{id}`. `append` → `values/{sheet}!A1:append?valueInputOption=RAW&insertDataOption=INSERT_ROWS`. `batch_update_values` → `values:batchUpdate` s `valueInputOption: RAW`.
- [ ] **Step 7: Onboarding obrazovka** (dočasně jako celá app). Krok 1 je „Přihlásit se Googlem“, krok 2 vložení odkazu na tabulku a „Ověřit“: zavolá `ensure_sheets` s `rides` (HEADERS z Task 4, do té doby placeholder `["id"]`) a `people` (`["email","nickname"]`). Spike tlačítko „Test zápisu“ připíše do `people` řádek `[email, ""]`, pokud tam e-mail ještě není. Sheet id se ukládá do localStorage `pt.sheet_id`.
- [ ] **Step 8: `docs/setup-google.md`.** Krok za krokem: Google Cloud projekt, povolit Google Sheets API, OAuth consent screen (External, Testing, test users = oba e-maily, scopes `spreadsheets`, `openid`, `email`), OAuth client „Web application“ s Authorized JavaScript origins `https://<user>.github.io` a `http://localhost:8080`, zkopírovat Client ID do repo variable `GOOGLE_CLIENT_ID` a lokálně do env. Dále založit prázdnou tabulku, nasdílet synovi jako Editor a upozornit na varování „neověřená aplikace“.
- [ ] **Step 9: Kontrolní bod B (uživatel, iPhone, PWA z plochy).** Přihlásit se, ověřit tabulku, „Test zápisu“, řádek se objeví v Sheets. Totéž na synově účtu. Zavřít a otevřít aplikaci do hodiny: zůstává přihlášená.
- [ ] **Step 10: Pokud popup v iOS standalone PWA selže** (nevrátí token, zůstane bílé okno): nahraď `sign_in` implicitním redirect flow. Navigace na `https://accounts.google.com/o/oauth2/v2/auth?client_id=…&redirect_uri=<origin>/pidtracker/&response_type=token&scope=…&login_hint=…&state=<nonce>`. Při startu aplikace se zpracuje `#access_token=…&expires_in=…&state=…`, ověří se `state` a fragment se smaže z URL. Přidej Authorized redirect URI v Google Console (zapsat do `setup-google.md`). Kontrolní bod B zopakovat.
- [ ] **Step 11: Commit** `feat: Google sign-in and Sheets client`.

### Task 4: Model jízdy, řádky tabulky, čas

**Files:**
- Create: `crates/pid-core/src/{ride.rs,time.rs}`
- Modify: `crates/pid-core/src/sheet.rs`, `crates/pid-core/src/lib.rs`

**Interfaces:**
- Produces (`ride.rs`):

```rust
pub enum RideStatus { InProgress, Done }            // "in_progress" | "done"
pub enum RouteKind { Tram, Metro, Train, Bus, Ferry, Funicular, Trolleybus, Other }
impl RouteKind { pub fn from_gtfs(t: u16) -> Self; pub fn label_cs(&self) -> &'static str }
// 0 Tramvaj, 1 Metro, 2 Vlak, 3 Autobus, 4 Přívoz, 7 Lanovka, 11 Trolejbus, jinak "Jiné"
pub struct Ride {
  pub id: String, pub person: String,
  pub board_stop_id: String, pub board_stop_name: String,
  pub route_id: String, pub route_short_name: String, pub route_type: Option<u16>,
  pub headsign: String, pub trip_id: String, pub pattern_id: Option<u32>,
  pub board_time: DateTime<FixedOffset>,
  pub alight_stop_id: Option<String>, pub alight_stop_name: Option<String>,
  pub alight_time: Option<DateTime<FixedOffset>>,
  pub board_lat: Option<f64>, pub board_lon: Option<f64>,
  pub alight_lat: Option<f64>, pub alight_lon: Option<f64>,
  pub status: RideStatus, pub deleted: bool,
  pub created_at: DateTime<FixedOffset>, pub updated_at: DateTime<FixedOffset>,
  pub app_version: String, pub data_version: String,
}
impl Ride { pub fn can_edit(&self, email: &str) -> bool; pub fn duration_minutes(&self) -> Option<i64> }
```
- Produces (`sheet.rs`): `pub const HEADERS: [&str; 24]` v pořadí `id, person, board_stop_id, board_stop_name, route_id, route_short_name, route_type, headsign, trip_id, pattern_id, board_time, alight_stop_id, alight_stop_name, alight_time, board_lat, board_lon, alight_lat, alight_lon, status, deleted, created_at, updated_at, app_version, data_version`; `pub const LAST_COL: &str = "X"`; `pub fn to_row(r: &Ride) -> Vec<String>`; `pub fn from_row(row: &[String]) -> Result<Ride, RowError>`; `pub fn parse_rows(values: &[Vec<String>]) -> ParsedRows` kde `pub struct ParsedRows { pub rides: Vec<(u32 /*1-based sheet row*/, Ride)>, pub skipped: u32 }` (první řádek = hlavičky; mapování sloupců podle názvu v hlavičce, ne podle pozice). Dále `pub fn people_rows(values) -> Vec<(u32, String, String)>`.
- Produces (`time.rs`): `pub const TZ: chrono_tz::Tz = chrono_tz::Europe::Prague;` `pub fn gtfs_instant(service_date: NaiveDate, secs: u32) -> DateTime<Tz>` (poledne − 12 h + secs); `pub fn now_prague(epoch_ms: f64) -> DateTime<Tz>`.

- [ ] **Step 1: Failing tests:**

```rust
#[test] fn gtfs_time_on_dst_end_day() {
    let d = NaiveDate::from_ymd_opt(2026,10,25).unwrap();
    assert_eq!(gtfs_instant(d, 3600).with_timezone(&Utc), Utc.with_ymd_and_hms(2026,10,25,0,0,0).unwrap());
    assert_eq!(gtfs_instant(d, 12*3600).with_timezone(&Utc), Utc.with_ymd_and_hms(2026,10,25,11,0,0).unwrap());
}
#[test] fn gtfs_time_past_midnight() {
    let d = NaiveDate::from_ymd_opt(2026,10,31).unwrap();
    let t = gtfs_instant(d, 24*3600 + 10*60);
    assert_eq!(t.naive_local(), NaiveDate::from_ymd_opt(2026,11,1).unwrap().and_hms_opt(0,10,0).unwrap());
}
#[test] fn row_roundtrip() { let r = sample_ride(); assert_eq!(from_row(&to_row(&r)).unwrap(), r); }
#[test] fn headers_len_matches_last_col() { assert_eq!(HEADERS.len(), 24); assert_eq!(LAST_COL, "X"); }
#[test] fn deleted_and_optionals_serialization() {
    let mut r = sample_ride(); r.deleted = true; r.alight_time = None;
    let row = to_row(&r);
    assert_eq!(row[19], "TRUE"); assert_eq!(row[13], "");
}
#[test] fn parse_rows_skips_garbage() {
    let mut v = vec![HEADERS.iter().map(|s| s.to_string()).collect::<Vec<_>>()];
    v.push(to_row(&sample_ride()));
    v.push(vec!["".into(), "x".into()]);          // bez id
    v.push(vec!["abc".into(), "a@b.cz".into()]); // chybí board_time
    let p = parse_rows(&v);
    assert_eq!(p.rides.len(), 1); assert_eq!(p.rides[0].0, 2); assert_eq!(p.skipped, 2);
}
#[test] fn parse_rows_by_header_name() { /* hlavičky v jiném pořadí + navíc sloupec "poznamka" → ride se načte správně */ }
#[test] fn can_edit_only_own() { let r = sample_ride(); assert!(r.can_edit("tata@gmail.com")); assert!(!r.can_edit("syn@gmail.com")); }
```
(`sample_ride()` je test helper s `person = "tata@gmail.com"`, tramvaj 22, Náměstí Míru → Ruská, 2026-10-26T08:00:00+01:00 → 08:05:00+01:00.)
- [ ] **Step 2:** `cargo test -p pid-core` FAIL.
- [ ] **Step 3:** Implementace. Prázdný řetězec = `None`. `deleted` přijímá `TRUE`/`true`/`1`. Čísla se zapisují přes `to_string()`, časy přes `to_rfc3339()`.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** V onboardingu (Task 3) nahraď placeholder hlaviček `HEADERS`.
- [ ] **Step 6: Commit** `feat(core): ride model, sheet rows, GTFS time`.

### Task 5: Formát jízdního řádu a `gtfs-prep`

**Files:**
- Create: `crates/pid-core/src/timetable/{mod.rs,codec.rs}`, `crates/gtfs-prep/src/{lib.rs,main.rs}`, `crates/gtfs-prep/tests/fixtures/mini/{feed_info,routes,stops,trips,stop_times,calendar,calendar_dates}.txt`, `crates/gtfs-prep/tests/build.rs`
- Modify: `xtask/src/main.rs` (napojení dat), `.github/workflows/deploy.yml` (`--fetch-gtfs`)

**Interfaces:**
- Produces (`pid-core::timetable`), vše `Serialize, Deserialize, PartialEq, Debug`:

```rust
pub struct Timetable {
  pub version: String,              // "20261025-<YYYYMMDDHHMM generování>"
  pub feed_start: NaiveDate, pub feed_end: NaiveDate,
  pub strings: Vec<String>,         // tabulka řetězců (názvy, headsigny, platform kódy)
  pub stops: Vec<Stop>, pub groups: Vec<StopGroup>, pub routes: Vec<Route>,
  pub patterns: Vec<Pattern>, pub profiles: Vec<Profile>, pub trips: Vec<Trip>,
  pub services: Vec<Service>,
}
pub struct Stop { pub gtfs_id: String, pub name: u32, pub platform: Option<u32>, pub lat_e6: i32, pub lon_e6: i32, pub group: u32 }
pub struct StopGroup { pub name: u32, pub stops: Vec<u32> }   // skupina = asw_node_id, jinak stop_name
pub struct Route { pub gtfs_id: String, pub short_name: String, pub route_type: u16, pub color: u32 }
pub struct Pattern { pub route: u32, pub stops: Vec<u32>, pub flags: Vec<u8> }   // bit0 = nelze nastoupit, bit1 = nelze vystoupit
pub struct Profile { pub pattern: u32, pub offsets: Vec<u32> } // departure − departure první zastávky, v sekundách
pub struct Trip { pub gtfs_id: String, pub profile: u32, pub service: u32, pub start: u32, pub headsign: u32 }
pub struct Service { pub days: Vec<u8> }  // bitmapa dnů od feed_start (bit i = feed_start + i)
```
- `codec.rs`: `pub fn encode(t: &Timetable) -> Vec<u8>` (postcard → deflate level 9, prefix magic `b"PIDT1"`); `pub fn decode(bytes: &[u8]) -> Result<Timetable, CodecError>`.
- `gtfs-prep` lib: `pub fn build_timetable(gtfs_dir: &Path, generated_at: DateTime<Utc>) -> anyhow::Result<Timetable>`; `pub fn write_output(t: &Timetable, out_dir: &Path) -> anyhow::Result<OutputInfo>`. Zapíše `timetable-<version>.pidt` a `manifest.json` = `{"version","file","feed_start","feed_end","bytes"}`. CLI `gtfs-prep (--fetch | --gtfs <zip|dir>) --out <dir>` stáhne `https://data.pid.cz/PID_GTFS.zip` do `.gtfs/` a rozbalí jen potřebné soubory (bez `shapes.txt`). Na konci vypíše počty a velikost a skončí chybou, pokud `bytes > 15 * 1024 * 1024`.
- xtask: `--fetch-gtfs`/`--gtfs` → po trunk buildu spustí gtfs-prep lib s `out = dist/data`.

- [ ] **Step 1: Fixture mini feed** (soubory přesně takto, CSV s hlavičkami podle reálného feedu, nepotřebné sloupce lze vynechat):
  - `feed_info`: start 20261025, end 20261107.
  - `routes`: `L22,"22",route_type 0,color 7A0603`; `L991,"A",1,00A562`.
  - `stops` (všechny `location_type=0`, kromě `U1S1`): `U1Z1P "Náměstí Míru" 50.075500,14.437500 platform A node 1`; `U1Z2P "Náměstí Míru" 50.075600,14.437900 B node 1`; `U1Z101P "Náměstí Míru" 50.075300,14.437200 platform 1 node 1`; `U2Z1P "Jana Masaryka" 50.073500,14.440000 node 2`; `U3Z1P "Ruská" 50.070000,14.450000 node 3`; `U4Z101P "Muzeum" 50.079000,14.430000 node 4`; `U1S1 "Náměstí Míru" location_type 1` (musí být ignorována).
  - `calendar`: `WD` po–pá 20261025–20261107; `WE` so–ne 20261025–20261107.
  - `calendar_dates`: `WD,20261028,2`; `WE,20261028,1` (28. 10. je státní svátek).
  - `trips` + `stop_times`: `T22b` (L22, WD, „Nádraží Hostivař“): U1Z2P 08:00:00 → U2Z1P 08:02:00 → U3Z1P 08:05:00. `T22c`: totéž od 08:10:00 (stejné offsety → stejný profil). `T22n` (L22, WE, „Nádraží Hostivař“): U1Z2P 24:10:00 → U2Z1P 24:12:00 → U3Z1P 24:15:00. `TA1` (L991, WD, „Depo Hostivař“): U4Z101P 08:03:00 → U1Z101P 08:05:00 s `pickup_type=1` na U1Z101P.
- [ ] **Step 2: Failing tests** v `tests/build.rs`:

```rust
#[test] fn builds_mini_feed() {
    let t = build_timetable(&fixture(), gen_at()).unwrap();
    assert_eq!(t.stops.len(), 6);                               // U1S1 vynechána
    assert_eq!(t.groups.len(), 4);
    assert_eq!(t.trips.len(), 4);
    assert_eq!(t.patterns.len(), 2);                            // 22 a A
    assert_eq!(t.profiles.len(), 2);                            // T22b/T22c/T22n sdílí profil
    assert_eq!(t.feed_end, NaiveDate::from_ymd_opt(2026,11,7).unwrap());
}
#[test] fn holiday_flips_services() { /* WD bit pro 2026-10-28 = 0, WE bit pro 2026-10-28 = 1, WD bit pro 2026-10-26 = 1 */ }
#[test] fn codec_roundtrip() { let t = build_timetable(&fixture(), gen_at()).unwrap(); assert_eq!(decode(&encode(&t)).unwrap(), t); }
#[test] fn decode_rejects_garbage() { assert!(decode(b"nonsense").is_err()); }
#[test] fn write_output_manifest() { /* do tempdir → manifest.json obsahuje file == "timetable-<version>.pidt" a soubor existuje */ }
```
- [ ] **Step 3:** `cargo test -p gtfs-prep` FAIL.
- [ ] **Step 4: Implementace.** CSV se čte crate `csv` s flexible hlavičkami (sloupce podle jména) a s odstraněním BOM. `stop_times` se čte streamovaně (1,95 M řádků), seskupí se podle `trip_id` a seřadí podle `stop_sequence`. Pattern = (route, sekvence stop indexů, flags) deduplikovaný přes HashMap. Profile = (pattern, offsety) deduplikovaný. Kalendář se rozbalí na bitmapu přes dny feed_start..=feed_end (calendar + exception 1/2). Řetězce se internují.
- [ ] **Step 5:** PASS.
- [ ] **Step 6: Reálná data.** `cargo run -p gtfs-prep --release -- --fetch --out target/data`. Výstup ukáže velikost (očekává se 2–4 MB) a doba běhu má být pod 2 minuty. Velikost zapiš do commit message.
- [ ] **Step 7:** Napoj xtask a workflow (`--fetch-gtfs`). `.gtfs/` je v `.gitignore`.
- [ ] **Step 8: Commit** `feat: compact timetable format and gtfs-prep`.

### Task 6: Dotazy do jízdního řádu (`DepartureSource`)

**Files:**
- Create: `crates/pid-core/src/{geo.rs,text.rs,timetable/query.rs}`, `crates/gtfs-prep/tests/queries.rs`

**Interfaces:**
- Consumes: `Timetable` (Task 5), `gtfs_instant`, `TZ` (Task 4).
- Produces:

```rust
pub struct TimetableIndex { pub tt: Timetable, /* mřížka ~250 m pro stopy, stop→[(pattern, pos)], pattern→[profile], profile→[trip seřazené dle start] */ }
impl TimetableIndex { pub fn new(tt: Timetable) -> Self }
pub struct NearbyGroup { pub group: u32, pub name: String, pub distance_m: f64 }
pub struct Departure { pub trip: u32, pub stop: u32, pub route_short_name: String, pub route_id: String,
  pub route_type: u16, pub color: u32, pub headsign: String, pub time: DateTime<Tz>,
  pub trip_gtfs_id: String, pub pattern: u32 }
pub struct StopRef { pub stop: u32, pub gtfs_id: String, pub name: String, pub distance_m: Option<f64> }
pub enum DataStatus { Valid, ExpiringSoon { days_left: i64 }, Expired }
pub trait DepartureSource {
  fn nearby_groups(&self, lat: f64, lon: f64, accuracy_m: f64) -> Vec<NearbyGroup>;
  fn search_groups(&self, query: &str, limit: usize) -> Vec<NearbyGroup>;  // distance_m = 0
  fn departures(&self, group: u32, at: DateTime<Tz>, before_min: i64, after_min: i64) -> Vec<Departure>;
  fn downstream_stops(&self, trip: u32, board_stop: u32, lat: Option<f64>, lon: Option<f64>) -> Vec<StopRef>;
  fn search_routes(&self, query: &str, limit: usize) -> Vec<(String /*route_id*/, String /*short*/, u16 /*type*/)>;
  fn status(&self, today: NaiveDate) -> DataStatus;
  fn version(&self) -> &str;
}
impl DepartureSource for TimetableIndex { … }
pub fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64;
pub fn fold(s: &str) -> String;  // lowercase + bez české diakritiky
```
Pravidla:
- `nearby_groups`: poloměr `r = accuracy_m.clamp(100.0, 500.0)`. Skupiny, jejichž nejbližší stop je ≤ r, seřazené podle vzdálenosti. Když je výsledek prázdný, vrátí 5 nejbližších do 2 km.
- `departures`: pro všechny stopy skupiny vezme spoje, jejichž služba jede v servisní den `at.date()` nebo `at.date() − 1` (časy ≥ 24 h). Okamžik se počítá přes `gtfs_instant`. Vynechá zastávky s flag „nelze nastoupit“ a poslední zastávku patternu. Okno je `[at − before, at + after]`, řazení podle času.
- `downstream_stops`: zastávky patternu za pozicí nástupní zastávky, kromě těch s flag „nelze vystoupit“. Při zadané poloze se řadí podle vzdálenosti, jinak podle pořadí na trase.
- `status`: `Expired`, pokud `today > feed_end`; `ExpiringSoon`, pokud `feed_end − today ≤ 2`; jinak `Valid`.

- [ ] **Step 1: Failing tests** (`tests/queries.rs`, nad fixture z Task 5; `prague(y,m,d,h,mi)` je helper):

```rust
#[test] fn nearby_finds_namesti_miru_not_ruska() {
    let ix = index(); let g = ix.nearby_groups(50.07555, 14.43760, 20.0);
    assert_eq!(g[0].name, "Náměstí Míru"); assert!(g.iter().all(|x| x.name != "Ruská"));
}
#[test] fn nearby_falls_back_to_nearest_when_empty() { let g = index().nearby_groups(50.0790, 14.4200, 10.0); assert!(!g.is_empty() && g.len() <= 5); }
#[test] fn weekday_departures_in_window() {
    let ix = index(); let g = group(&ix, "Náměstí Míru");
    let d = ix.departures(g, prague(2026,10,26,7,58), 5, 30);
    let lines: Vec<_> = d.iter().map(|x| (x.route_short_name.as_str(), x.time.format("%H:%M").to_string())).collect();
    assert_eq!(lines, vec![("22","08:00".into()), ("22","08:10".into())]); // A je jen výstupní (pickup_type=1)
}
#[test] fn holiday_uses_weekend_service() { let ix = index(); assert!(ix.departures(group(&ix,"Náměstí Míru"), prague(2026,10,28,7,58), 5, 30).is_empty()); }
#[test] fn night_trip_after_midnight() {
    let ix = index(); let d = ix.departures(group(&ix,"Náměstí Míru"), prague(2026,11,1,0,5), 5, 30);
    assert_eq!(d.len(), 1); assert_eq!(d[0].time.format("%Y-%m-%d %H:%M").to_string(), "2026-11-01 00:10");
}
#[test] fn downstream_after_board() {
    let ix = index(); let dep = &ix.departures(group(&ix,"Náměstí Míru"), prague(2026,10,26,7,58), 5, 30)[0];
    let s: Vec<_> = ix.downstream_stops(dep.trip, dep.stop, None, None).into_iter().map(|x| x.name).collect();
    assert_eq!(s, vec!["Jana Masaryka", "Ruská"]);
}
#[test] fn downstream_sorted_by_position_when_located() { /* poloha u Ruské → první "Ruská" */ }
#[test] fn search_ignores_diacritics() { assert_eq!(index().search_groups("namesti mi", 5)[0].name, "Náměstí Míru"); }
#[test] fn search_routes_by_short_name() { assert_eq!(index().search_routes("22", 5)[0].1, "22"); }
#[test] fn timetable_status_expiry() {
    let ix = index(); let d = |m,dd| NaiveDate::from_ymd_opt(2026,m,dd).unwrap();
    assert_eq!(ix.status(d(10,26)), DataStatus::Valid);
    assert_eq!(ix.status(d(11,6)), DataStatus::ExpiringSoon { days_left: 1 });
    assert_eq!(ix.status(d(11,8)), DataStatus::Expired);
}
```
plus unit testy v `pid-core`: `fold("Náměstí Míru Ůž") == "namesti miru uz"` a `haversine_m(50.079, 14.430, 50.0755, 14.4375)` ≈ 662 m ± 5.
- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** Implementace. `search_groups` porovnává `fold(query)` s `fold(name)`. Pořadí: celý název začíná dotazem, pak některé slovo začíná dotazem, pak obsahuje dotaz, a v rámci stejné kategorie abecedně.
- [ ] **Step 4:** PASS. Navíc ověř na reálných datech: `cargo run -p gtfs-prep --release -- --gtfs .gtfs --out target/data --probe "Náměstí Míru"` (přidej flag `--probe <název>`, který vypíše odjezdy teď). Výpis musí vypadat věrohodně.
- [ ] **Step 5: Commit** `feat(core): timetable queries`.

### Task 7: Logika synchronizace

**Files:**
- Create: `crates/pid-core/src/sync.rs`

**Interfaces:**
- Consumes: `Ride`, `parse_rows`, `to_row`, `LAST_COL` (Task 4).
- Produces:

```rust
pub struct MergeReport { pub inserted: u32, pub updated: u32 }
/// local: id → Ride; pending: id čekající na odeslání. Mění local.
pub fn merge_remote(local: &mut HashMap<String, Ride>, remote: &[(u32, Ride)], pending: &HashSet<String>) -> MergeReport;
pub enum SheetWrite { Update { row: u32, ride: Ride }, Append { ride: Ride } }
pub fn plan_push(remote: &[(u32, Ride)], to_push: &[Ride]) -> Vec<SheetWrite>;
pub fn update_range(row: u32) -> String;   // "rides!A{row}:X{row}"
```
Pravidla `merge_remote`: vzdálená jízda, která lokálně chybí, se vloží. Pokud je id v `pending`, lokální verze se nemění, jen `deleted` se zapne, pokud je vzdálená smazaná. Jinak vyhrává novější `updated_at` a smazání je trvalé (`deleted = local.deleted || remote.deleted`). Duplicitní id v tabulce: vyhrává řádek s novějším `updated_at`.

- [ ] **Step 1: Failing tests:**

```rust
#[test] fn inserts_unknown_remote() { … assert_eq!(rep.inserted, 1) }
#[test] fn newer_remote_replaces_when_not_pending() { … }
#[test] fn pending_local_is_kept() { /* remote novější, ale id v pending → lokální zůstává */ }
#[test] fn delete_is_sticky() { /* lokálně deleted, remote nedeleted a novější → deleted zůstává true */ }
#[test] fn push_updates_existing_and_appends_new() {
    let w = plan_push(&[(7, ride("a"))], &[ride("a"), ride("b")]);
    assert!(matches!(&w[0], SheetWrite::Update{row:7, ..})); assert!(matches!(&w[1], SheetWrite::Append{..}));
}
#[test] fn push_uses_fresh_row_numbers() { /* stejné id na řádku 12 po přeřazení → Update{row:12} */ }
#[test] fn update_range_format() { assert_eq!(update_range(5), "rides!A5:X5"); }
```
- [ ] **Step 2:** FAIL. **Step 3:** Implementace. **Step 4:** PASS.
- [ ] **Step 5: Commit** `feat(core): sync merge and push planning`.

### Task 8: Statistiky

**Files:**
- Create: `crates/pid-core/src/stats.rs`

**Interfaces:**
- Produces:

```rust
pub enum Period { Week { iso_year: i32, week: u32 }, Month { year: i32, month: u32 }, Year(i32), All }
impl Period { pub fn containing(kind: PeriodKind, d: NaiveDate) -> Self; pub fn prev(&self) -> Self; pub fn next(&self) -> Self; pub fn label_cs(&self) -> String }
pub enum PeriodKind { Week, Month, Year, All }
pub struct StatsQuery { pub period: Period, pub persons: Vec<String> }   // prázdné = všichni
pub struct PersonStats { pub person: String, pub rides: u32, pub minutes: i64,
  pub by_kind: Vec<(RouteKind, u32)>,        // sestupně podle počtu
  pub top_routes: Vec<(String, u32)>,        // max 5, sestupně, při shodě abecedně
  pub series: Vec<(String, u32)> }           // týden→dny (Po..Ne), měsíc→dny 1..N, rok→měsíce, vše→roky
pub const MAX_COUNTED_MINUTES: i64 = 360;
pub fn compute(rides: &[Ride], q: &StatsQuery) -> Vec<PersonStats>;  // jedna položka na osobu, řazeno podle e-mailu
```
Počítají se jen jízdy `!deleted && status == Done`, přiřazené podle `board_time` v Europe/Prague. `minutes` sčítá jen jízdy s `0 < duration ≤ MAX_COUNTED_MINUTES`. Typ dopravy z `route_type`, chybí-li, `Other`.

- [ ] **Step 1: Failing tests** (fixture: táta v říjnu 2026: tram 20 min, metro 10 min, bus s výstupem po 14 h; plus jedna smazaná jízda a jedna `InProgress`; syn: 1 tram 15 min; táta 1 jízda v listopadu):

```rust
#[test] fn month_per_person() {
    let s = compute(&rides(), &StatsQuery { period: Period::Month{year:2026, month:10}, persons: vec![] });
    let tata = s.iter().find(|p| p.person=="tata@gmail.com").unwrap();
    assert_eq!(tata.rides, 3); assert_eq!(s.len(), 2);
}
#[test] fn forgotten_exit_excluded_from_time() { /* tata.minutes == 30 */ }
#[test] fn person_filter() { /* persons = [syn] → 1 položka, rides 1, minutes 15 */ }
#[test] fn by_kind_and_top_routes() { /* tata.by_kind obsahuje (Tram,1),(Metro,1),(Bus,1); top_routes délka 3 */ }
#[test] fn series_month_has_all_days() { /* series.len() == 31, součet == rides */ }
#[test] fn period_navigation() { assert_eq!(Period::Month{year:2026,month:1}.prev(), Period::Month{year:2025,month:12}); assert_eq!(Period::containing(PeriodKind::Week, d(2026,10,25)), Period::Week{iso_year:2026, week:43}); }
```
- [ ] **Step 2:** FAIL. **Step 3:** Implementace. **Step 4:** PASS.
- [ ] **Step 5: Commit** `feat(core): statistics`.

### Task 9: Lokální úložiště a načítání jízdního řádu v aplikaci

**Files:**
- Create: `crates/app/src/storage.rs`, `crates/app/src/timetable_loader.rs`, `crates/app/src/interop/sw.rs`
- Modify: `crates/app/src/app.rs` (globální stav)

**Interfaces:**
- Produces (`storage`), IndexedDB `pidtracker` v1, stores `rides` (key `id`, hodnota JSON `Ride`), `outbox` (key `id`) a `kv`:

```rust
pub async fn open() -> Result<Db, StorageError>;
impl Db {
  pub async fn all_rides(&self) -> Result<Vec<Ride>, StorageError>;
  pub async fn put_ride(&self, r: &Ride, mark_pending: bool) -> Result<(), StorageError>; // jedna transakce rides+outbox
  pub async fn put_rides_bulk(&self, rs: &[Ride]) -> Result<(), StorageError>;            // z pullu, bez outboxu
  pub async fn pending_ids(&self) -> Result<HashSet<String>, StorageError>;
  pub async fn clear_pending_if_unchanged(&self, id: &str, updated_at: &DateTime<FixedOffset>) -> Result<(), StorageError>;
  pub async fn kv_get<T: DeserializeOwned>(&self, k: &str) -> Result<Option<T>, StorageError>;
  pub async fn kv_set<T: Serialize>(&self, k: &str, v: &T) -> Result<(), StorageError>;
}
```
kv klíče: `sheet_id`, `nicknames` (`HashMap<String,String>`), `last_sync` (RFC 3339), `active_ride_id`, `last_seen_build`.
- Produces (`timetable_loader`): `pub async fn load() -> Result<TimetableIndex, LoadError>`. Načte `data/manifest.json` (SW network-first s fallbackem na cache), pak `data/<file>` (cache-first) a dekóduje. Po úspěchu zavolá `interop::sw::purge_data_cache_except(&file)`. `LoadError::NoData` znamená, že aplikace běží v režimu ručního zadávání.
- App stav (`app.rs`): `AppState { db: Db, tt: RwSignal<Option<Rc<TimetableIndex>>>, rides: RwSignal<Vec<Ride>>, me: RwSignal<Option<String>>, sync: RwSignal<SyncStatus>, screen: RwSignal<Screen> }` poskytnutý přes `provide_context`. `Screen` = `Onboarding | Home | Board { group: Option<u32>, transfer_from: Option<String> } | Running | Alight | History | Edit(String) | Stats | Settings`.

- [ ] **Step 1:** Implementuj `storage` (rexie, případně `indexed_db_futures`, pokud rexie nejde zkompilovat s aktuálním toolchainem) a `timetable_loader`. Tahle vrstva se testuje ručně, logika je v core.
- [ ] **Step 2:** Startup: otevře DB, načte jízdy do signálu a spustí `timetable_loader::load()` na pozadí. UI je použitelné hned, před načtením jízdního řádu ukazuje „Načítám jízdní řád…“. Na Home se zobrazí banner podle `status(today)`: „Jízdní řád končí za N dní“ nebo „Jízdní řád je prošlý – zastávky a linky zadejte ručně“.
- [ ] **Step 3: Ověření.** `cargo xtask dist --gtfs .gtfs` a statický server nad `dist` (`trunk serve` nebo `cargo install miniserve` + `miniserve dist --index index.html`). V devtools Application jsou cache `pid-data` se souborem a IndexedDB `pidtracker`. Po vypnutí sítě (Offline v devtools) a reloadu se jízdní řád načte.
- [ ] **Step 4: Commit** `feat(app): IndexedDB storage and timetable loading`.

### Task 10: Záznam jízdy (Home → Nástup → Jízda → Výstup → Přestup) – **kontrolní bod C**

**Files:**
- Create: `crates/app/src/interop/geo.rs`, `crates/app/src/screens/{home,board,running,alight}.rs`, `crates/app/src/components/{stop_picker,route_picker}.rs`
- Modify: `crates/app/style.css`

**Interfaces:**
- Consumes: `DepartureSource` (Task 6), `Db::put_ride` (Task 9), `Ride` (Task 4), `sync::request_sync()` (Task 11, do té doby no-op stub v `app/src/sync.rs`).
- Produces (`geo`): `pub struct Fix { pub lat: f64, pub lon: f64, pub accuracy_m: f64 }`; `pub async fn current_position() -> Result<Fix, GeoError>` (`enableHighAccuracy: true, timeout: 10000, maximumAge: 30000`); `GeoError::{Denied, Unavailable, Timeout}`.
- `stop_picker`: komponenta `StopPicker(candidates: Signal<Vec<NearbyGroup>>, selected: RwSignal<Option<u32>>)` s vyhledávacím polem (vždy viditelné) a seznamem kandidátů „název · 45 m“. Při hledání nahradí seznam výsledky `search_groups`.
- `route_picker`: výběr z odjezdů (řádek: barevný štítek linky, směr, čas, „za 3 min“) a odkaz „Jiná linka…“ s `search_routes` a volným textem.

Chování:
- **Home:** velké tlačítko „Nová jízda“. Pokud existuje jízda `InProgress` od mě (`active_ride_id`), Home rovnou zobrazí Running. Spodní navigace: Jízda / Historie / Statistiky / Nastavení. Odznak synchronizace (Task 11). Verze drobně dole.
- **Board:** po otevření zavolá `current_position`. Ukáže přesnost („přesnost ±35 m“). Předvybere `nearby_groups[0]`. Odjezdy jsou `departures(group, now, 5, 30)` s předvybraným prvním. Při chybě polohy nebo bez jízdního řádu je vidět hláška a vyhledávání. Bez jízdního řádu se linka zadá ručně (volný text + typ dopravy výběrem). **Start** vytvoří `Ride { id: uuid v4, status: InProgress, board_time: now, created_at = updated_at = now, app_version: BUILD_ID, data_version: tt.version() nebo "" … }`. Jako `board_stop_*` se použije `Departure.stop` (konkrétní stanoviště). Ride se uloží s `mark_pending=true`, nastaví se `active_ride_id`, spustí se `request_sync()` a obrazovka přejde na Running. Celé to proběhne bez čekání na síť.
- **Running:** linka (štítek), směr, odkud, čas nástupu, živě běžící doba (mm:ss, aktualizace 1 s). Tlačítko **Výstup** a menší „Zrušit jízdu“, které ji po potvrzení smaže (`deleted = true`).
- **Alight:** `current_position`. Kandidáti jsou `downstream_stops(trip, board_stop, lat, lon)` (první předvybraný), pod tím „Jiné zastávky v okolí“ (`nearby_groups`) a vyhledávání. Bez tripu (ruční linka) jen okolí a vyhledávání. Tlačítka **Hotovo** a **Přestoupit**: obě uloží `alight_*`, `alight_time = now`, `status = Done`, `updated_at = now`, smažou `active_ride_id` a spustí `request_sync()`. **Přestoupit** pak přejde na `Board { group: skupina výstupní zastávky, transfer_from: Some(id) }` a poloha se znovu nezjišťuje.
- Probíhající jízda přežije zavření aplikace: Running se obnoví z `active_ride_id` + IndexedDB.
- Layout: mobil na výšku, hlavní tlačítka min. 56 px vysoká a dosažitelná palcem (spodní třetina), `env(safe-area-inset-*)`, světlý i tmavý režim přes `prefers-color-scheme`, barvy linek z `Route.color`.

- [ ] **Step 1:** Implementace geo interop a obrazovek podle popisu.
- [ ] **Step 2: Ověření na desktopu.** V Chrome devtools Sensors nastav polohu 50.07555, 14.43760. Board ukáže „Náměstí Míru“ a reálné odjezdy. Projdi Start → Running → Výstup → Přestoupit → Start → Výstup → Hotovo. V IndexedDB jsou 2 jízdy `done` a outbox má 2 id. Reload během Running obnoví Running. Zamítnutá poloha ukáže vyhledávání.
- [ ] **Step 3:** Deploy (push) a **Kontrolní bod C (uživatel, iPhone):** na zastávce zaznamenat skutečnou jízdu s přestupem, z toho jednu část v režimu letadlo. Ověřit předvyplnění zastávek a linek. Zapsat do `docs/testing-iphone.md`.
- [ ] **Step 4: Commit** `feat(app): ride recording flow`.

### Task 11: Synchronizační engine – **kontrolní bod D**

**Files:**
- Create: `crates/app/src/sync.rs` (nahrazuje stub), `crates/app/src/components/sync_badge.rs`

**Interfaces:**
- Consumes: `google::auth::{current_token, sign_in}`, `SheetsClient` (Task 3), `parse_rows`, `people_rows`, `merge_remote`, `plan_push`, `update_range`, `to_row` (Task 4, 7), `Db` (Task 9).
- Produces:

```rust
pub enum SyncStatus { Idle { last: Option<String> }, Running, Pending(u32), NeedsSignIn(u32), Error(String) }
pub fn request_sync();               // debounce 2 s, souběžně běží max. jedna synchronizace
pub fn install_triggers();           // start, visibilitychange→visible, window 'online', interval 60 s (jen když pending > 0)
```
Jeden běh synchronizace:
1. Bez platného tokenu nastav `NeedsSignIn(pending)` a skonči. Nikdy nevolej `sign_in` automaticky.
2. `get_values("rides!A:ZZ")` → `parse_rows` → `merge_remote(local, remote, pending)` → `put_rides_bulk` změněných → refresh signálu `rides`.
3. `to_push` = lokální jízdy s id v `pending` → `plan_push(remote, to_push)`. Updates jdou přes `batch_update_values` (range z `update_range`), appendy jedním `append("rides", rows)`.
4. Po úspěchu `clear_pending_if_unchanged(id, updated_at)` pro každou odeslanou jízdu.
5. `people`: pokud tam můj e-mail není, append `[email, nickname]`. Načti přezdívky do kv `nicknames`.
6. `Unauthorized` → smaž token a nastav `NeedsSignIn`. Síťová chyba → `Pending(n)` (zkusí se znovu). Ostatní chyby → `Error(text)`.
- `sync_badge`: „Synchronizováno 12:04“ / „Čeká 3 změny“ / tlačítko „Přihlásit pro synchronizaci (3)“, které v gestu zavolá `sign_in(Some(me))` a pak `request_sync()` / „Chyba: …“ s možností zkusit znovu.

- [ ] **Step 1:** Implementace podle kroků výše.
- [ ] **Step 2: Ověření (desktop, dva prohlížečové profily = dva účty).** A zaznamená jízdu offline, pak online a jízda se objeví v Sheets. B ji po synchronizaci vidí. A ji upraví (Task 12) a změna se propíše na stejný řádek. Ručně přeřaď řádky v Sheets (seřadit podle sloupce B), A upraví jízdu a zapíše se na správný řádek. Žádné duplicity.
- [ ] **Step 3: Kontrolní bod D (uživatel, iPhone):** jízdy pořízené offline se po připojení odešlou. Po více než hodině nečinnosti se ukáže „Přihlásit pro synchronizaci“ a po klepnutí se dokončí. Synovy jízdy jsou vidět v historii.
- [ ] **Step 4: Commit** `feat(app): background sync with Google Sheets`.

### Task 12: Historie, úprava, smazání

**Files:**
- Create: `crates/app/src/screens/{history,edit}.rs`

**Interfaces:**
- Consumes: `AppState.rides`, `Ride::can_edit`, `Db::put_ride`, `request_sync`, `StopPicker`, `route_picker`, `DepartureSource::search_groups/search_routes`.

Chování:
- **History:** nesmazané jízdy seřazené od nejnovější, seskupené po dnech („Dnes“, „Včera“, „po 26. 10.“). Řádek ukazuje štítek linky, „Odkud → Kam“, časy, dobu a přezdívku osoby. Filtr Já / {přezdívka druhého} / Vše (osoby = unie `person` z jízd). Neukončené jízdy mají štítek „neukončeno“.
- **Edit** (jen `can_edit`, jinak detail jen pro čtení): nástupní a výstupní zastávka přes vyhledávání, linka přes `search_routes` nebo volný text. Při změně linky se vymaže `trip_id` a `pattern_id` a z routy se doplní `route_id`/`route_type`. Časy jsou `<input type="datetime-local">` v Europe/Prague. Validace: výstup musí být po nástupu, jinak hláška a nelze uložit. U neukončené jízdy je tlačítko „Ukončit teď“. **Uložit** nastaví `updated_at = now`, `status = Done`, pokud je vyplněn výstup, a provede `put_ride(…, true)` + `request_sync()`. **Smazat** po potvrzení nastaví `deleted = true` a `updated_at = now` a uloží.

- [ ] **Step 1:** Implementace.
- [ ] **Step 2: Ověření (desktop):** upravit čas výstupu, změnit linku, smazat. V Sheets se změní stejný řádek a smazaná jízda má `deleted=TRUE`. Cizí jízda nejde upravit.
- [ ] **Step 3: Commit** `feat(app): ride history, edit and delete`.

### Task 13: Statistiky v aplikaci

**Files:**
- Create: `crates/app/src/screens/stats.rs`

**Interfaces:**
- Consumes: `stats::{compute, Period, PeriodKind, StatsQuery}`, `RouteKind::label_cs`, kv `nicknames`.

Chování: přepínač Týden / Měsíc / Rok / Vše, šipky ‹ › a popisek období (`label_cs`, např. „říjen 2026“, „týden 43/2026“). Osoby Já / druhý / Oba. Pro každou osobu (u „Oba“ ve sloupcích vedle sebe, na úzkém displeji pod sebou) karty: počet jízd, čas v MHD („5 h 20 min“), rozpad podle typu dopravy (vodorovné pruhy s počtem), top 5 linek. Pod tím sloupcový graf `series` jako jednoduché inline SVG bez knihovny, s osou popisků. Výpočet probíhá z lokálních dat, takže funguje offline.

- [ ] **Step 1:** Implementace.
- [ ] **Step 2: Ověření:** čísla na obrazovce odpovídají ručnímu přepočtu z tabulky za aktuální měsíc a navigace mezi obdobími funguje.
- [ ] **Step 3: Commit** `feat(app): statistics screen`.

### Task 14: Nastavení, verze, autoupdate – **kontrolní bod E**

**Files:**
- Create: `crates/app/src/update.rs`, `crates/app/src/screens/settings.rs`, `crates/app/src/components/toast.rs`

**Interfaces:**
- Consumes: `config::{APP_VERSION, BUILD_ID, BUILT_AT}`, `DepartureSource::{version, status}`, kv `last_seen_build`, `nicknames`, `sheet_id`.
- Produces (`update`): `pub fn install()` (kontrola při startu, při `visibilitychange→visible` a každých 15 min); `pub fn is_safe(screen: &Screen) -> bool` (`Home` bez aktivní jízdy, `History`, `Stats`, `Settings`); `pub async fn check_now() -> UpdateCheck` kde `UpdateCheck::{UpToDate, Available(String), Offline}`.

Chování autoupdate: `fetch("version.json", {cache: "no-store"})`. Pokud `build_id != BUILD_ID`, zavolá `registration.update()` a nastaví `pending_update`. Když je `pending_update` a obrazovka je `is_safe` (kontroluje se i při každé změně obrazovky), proběhne `location.reload()`. Po startu, pokud `kv last_seen_build != BUILD_ID` a předchozí hodnota existovala, ukáže toast „Aktualizováno na verzi {APP_VERSION} ({BUILD_ID})“ na 4 s. Pak se `last_seen_build` uloží.

Settings: přihlášený účet a „Odhlásit“, tabulka (odkaz „Otevřít v Google Sheets“, „Změnit tabulku“), moje přezdívka (uloží se do `people` přes update řádku podle e-mailu), sekce „O aplikaci“: verze aplikace `v{APP_VERSION}`, build `{BUILD_ID}`, sestaveno `{BUILT_AT}`, data jízdního řádu `{version}` a platnost `{feed_start}–{feed_end}`, tlačítko „Zkontrolovat aktualizace“ s výsledkem.

- [ ] **Step 1:** Implementace.
- [ ] **Step 2: Ověření (desktop):** otevři nasazenou verzi, pushni triviální změnu, po deployi se do 15 min (nebo po „Zkontrolovat aktualizace“) aplikace na Home sama přenačte a ukáže toast. Během Running se nepřenačte, až po návratu na Home.
- [ ] **Step 3: Kontrolní bod E (uživatel, iPhone):** verze je vidět v Nastavení. Po novém deployi se PWA z plochy aktualizuje sama. Rozpracovaná jízda aktualizaci přežije.
- [ ] **Step 4: Commit** `feat(app): settings, visible version and auto-update`.

### Task 15: README a dokončení

**Files:**
- Create/Modify: `README.md`, `docs/testing-iphone.md`

- [ ] **Step 1:** README: co to je, struktura workspace, prerekvizity (`rustup target add wasm32-unknown-unknown`, `cargo install trunk --locked`), lokální vývoj (`$env:PIDTRACKER_GOOGLE_CLIENT_ID=...; trunk serve` v `crates/app`, data přes `cargo run -p gtfs-prep --release -- --fetch --out crates/app/public/data` jen pro dev, gitignored), `cargo xtask dist`, nasazení, týdenní aktualizace dat, odkazy na `docs/setup-*.md`. Poznámka k budoucímu Golemio: implementovat `DepartureSource`.
- [ ] **Step 2:** `cargo test --workspace --exclude app` PASS, `cargo clippy --workspace -- -D warnings` čisté, `cargo xtask dist --fetch-gtfs` projde a vypíše velikost dat.
- [ ] **Step 3: Commit** `docs: README and testing guide`.
