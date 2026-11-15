# PID Tracker 0.5.0: vzhled PID, horní proužek, trasa spoje, zpoždění, mimořádnosti, poznámka, statistiky, tiché přihlášení – plán implementace

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Verze 0.5.0 podle návrhu: vzhled a písmo PID, horní proužek s menu místo spodní lišty, plovoucí „＋ Nová jízda“, čitelné zpoždění, detail spoje s polohou vozu, mimořádnosti s hláškou v aplikaci, poznámka k jízdě, sloučené statistiky „Oba“ a tiché obnovení Google tokenu.

**Architecture:** Čistá logika (formát času se zpožděním, trasa spoje, umístění vozu, dekódování a filtrování mimořádností, sloučené statistiky, pravidlo obnovy tokenu, sloupec `note`) jde do `pid-core` s unit testy. Testy nad jízdním řádem jsou v `gtfs-prep/tests/queries.rs` nad fixture `mini`. Leptos UI v `crates/app` to jen zobrazuje. Ověřuje se buildem pro wasm a ruční kontrolou v `trunk serve`. Všechna živá data jsou best effort a nikdy neblokují záznam jízdy.

**Tech Stack:** Rust stable, Leptos 0.8 CSR, trunk, prost (ručně definované zprávy GTFS-RT), chrono/chrono-tz, gloo-net/gloo-timers, web-sys, GitHub Pages.

**Spec:** `docs/superpowers/specs/2026-11-15-vzhled-pid-trasa-mimoradnosti-design.md` (implementátor čte spec i plán).

## Global Constraints

- Jen Rust toolchain (trunk, `cargo xtask`), bez Node v buildu, bez JS frameworku a mapové knihovny. Drobné JS jen v inline modulu `crates/app/src/interop/mod.rs`.
- UI texty česky. Komentáře v kódu česky, jako ve zbytku repa.
- Hosting GitHub Pages, žádný backend. Systémové notifikace (Notification API) se **nedělají**.
- Feedy (bez klíče, s CORS): `…/gtfsrt/vehicle_positions.pb`, `…/gtfsrt/trip_updates.pb`, `…/gtfsrt/alerts.pb` na `https://api.golemio.cz/v2/vehiclepositions`.
- Paleta PID: hlavní #C40C00, text #252525, sekundární text #4B4B4B, pozadí #F8F8F8, plochy #FFFFFF, okraje #E8E8E8 / #D0D0D0, varování #F59D1A, ok #00B274, tmavě červená #7A0200. Tmavý režim: pozadí ~#141414, plochy ~#202020, proužek #252525 s červenou linkou dole, hlavní ~#E0261A.
- Písmo Fira Sans 400/500/700, latin + latin-ext, woff2 přibalené v repu, `font-display: swap`, licence OFL vedle souborů. Žádné Google Fonts za běhu.
- Sloupec `note` = 28. sloupec **AB**. Sloupce se přidávají jen na konec.
- Intervaly: detail spoje 60 s, mimořádnosti 300 s, sledované linky 30 dní, obnova tokenu 5 min před vypršením, nejvýš 1 pokus o obnovu za 120 s. Vše jen při viditelné stránce.
- Verze **0.5.0**. CI s `--locked`. Lokální build v Git Bash: `MSYS_NO_PATHCONV=1 cargo xtask dist --public-url / --no-data`.
- Commity končí řádkem `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Vůz ve feedu bez `current_stop_sequence` a GPS daleko od trasy** (odklon, vůz v depu) → žádný štítek, ne štítek na nesmyslném místě. Test `place_vehicle_far_from_route_is_none` (Task 5).
2. **Mimořádnost bez `route_id`** (jen zastávka nebo dopravce) nebo s nečitelnými daty → aplikace nespadne, mimořádnost je v seznamu, ale nikdy není „naše“. Testy `alert_without_route_is_listed_not_relevant` a `garbage_feed_is_empty` (Task 6).
3. **Spoj přes půlnoc** (noční linka, zpoždění přes 24:00) → časy trasy i „nový čas“ jsou na správném dni a hodině. Testy `night_trip_stops_after_midnight` (Task 4) a `delay_across_midnight` (Task 3).
4. **Stará data bez poznámky** (řádek tabulky jen s 27 sloupci, jízda v IndexedDB bez pole `note`) → poznámka je prázdná a nic se neztratí. Test `old_row_without_note` (Task 1).
5. **Tichá obnova se zavřeným popupem nebo offline** → žádná smyčka popupů a žádné chybové hlášky. Test `refresh_throttled_and_offline` (Task 13).

---

### Task 1: Poznámka k jízdě v datech a v tabulce

**Files:**
- Modify: `crates/pid-core/src/ride.rs` (struct `Ride`, `sample_ride` v testech)
- Modify: `crates/pid-core/src/sheet.rs` (`HEADERS`, `LAST_COL`, `to_row`, parsování, testy)
- Modify: všechna místa, kde se staví `Ride { … }` (`crates/app/src/screens/board.rs` ve `start`, případně testy). Najdeš je přes `cargo build`.

**Interfaces:**
- Produces: `Ride::note: String` (`#[serde(default)]`). `HEADERS: [&str; 28]` s `"note"` na indexu 27, `LAST_COL == "AB"`.

- [ ] **Step 1: Testy** v `sheet.rs`:

```rust
#[test]
fn note_is_column_ab() {
    assert_eq!(HEADERS[27], "note");
    assert_eq!(LAST_COL, "AB");
    assert_eq!(column_letter(HEADERS.len()), LAST_COL);
}
#[test]
fn note_round_trips() {
    let mut r = sample_ride();
    r.note = "plná tramvaj, klimatizace nejela".into();
    let headers: Vec<String> = HEADERS.iter().map(|s| s.to_string()).collect();
    assert_eq!(parse_rows(&[headers, to_row(&r)]).rides[0].1.note, "plná tramvaj, klimatizace nejela");
}
#[test]
fn old_row_without_note() {
    let headers: Vec<String> = HEADERS[..27].iter().map(|s| s.to_string()).collect();
    let mut row = to_row(&sample_ride());
    row.truncate(27);
    let p = parse_rows(&[headers, row]);
    assert_eq!(p.rides[0].1.note, "");
    assert_eq!(p.rides[0].1.license_plate, sample_ride().license_plate);
}
```

Uprav i `plate_is_column_aa`: ověřuje jen `HEADERS[26] == "license_plate"`, `LAST_COL` už ne. V `ride.rs` test, že JSON jízdy bez klíče `note` se deserializuje s `note == ""`.

- [ ] **Step 2:** `cargo test -p pid-core sheet` → FAIL (kompilace).
- [ ] **Step 3:** Přidej pole a sloupec. Ověř, že `ensure_sheets` / `appendDimension` v `crates/app/src/google/sheets.rs` počítá potřebné sloupce z `HEADERS.len()`, ne z konstanty 27, a že se hlavička `note` dopíše do existující tabulky stejně jako `license_plate` v 0.4.0. Pokud ne, oprav to.
- [ ] **Step 4:** `cargo test -p pid-core` → PASS. `cargo build -p app --target wasm32-unknown-unknown` → OK.
- [ ] **Step 5:** Commit `feat(core): ride note in sheet column AB`.

### Task 2: Sloučené statistiky „Oba“ (core)

**Files:**
- Modify: `crates/pid-core/src/stats.rs`

**Interfaces:**
- Produces: `pub fn compute_combined(rides: &[Ride], q: &StatsQuery) -> PersonStats`. Výsledek sečte jízdy všech osob z `q.persons` (prázdné = všichni), `person` je prázdný řetězec.

- [ ] **Step 1: Test** `combined_sums_both_persons`: použij existující testovací `rides()` (táta + syn). Pro `Period::All` platí `compute_combined(..).rides == součet rides z compute(..)`, totéž pro `minutes`. `top_routes` počítá dohromady, takže linka, kterou jeli oba jednou, má počet 2. `series` má stejnou délku jako u jednotlivce a součet hodnot = `rides`.
- [ ] **Step 2:** `cargo test -p pid-core stats` → FAIL.
- [ ] **Step 3:** Vytáhni tělo mapování z `compute` do privátní `fn person_stats(person: String, rides: Vec<&Ride>, all: &[Ride], period: Period) -> PersonStats`. `compute` ji volá pro každou osobu, `compute_combined` jednou pro všechny vyfiltrované jízdy.
- [ ] **Step 4:** `cargo test -p pid-core` → PASS.
- [ ] **Step 5:** Commit `feat(core): combined stats for both persons`.

### Task 3: Formát času se zpožděním (core)

**Files:**
- Modify: `crates/pid-core/src/timetable/query.rs` (vedle `when_label`, testy v `when_tests`)

**Interfaces:**
- Produces:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelayKind { Unknown, OnTime, Late, Early }
#[derive(Clone, Debug, PartialEq)]
pub struct TimeDisplay {
    pub scheduled: String,       // "14:20"
    pub actual: Option<String>,  // Some("14:22") jen u Late/Early
    pub kind: DelayKind,
    pub detail: String,          // "+2 min · za 5 min"
}
pub fn time_display(scheduled: DateTime<Tz>, delay_s: Option<i32>, now: DateTime<Tz>) -> TimeDisplay
```

Pravidla: `|delay| < 60 s` → `OnTime`. Minuty zpoždění = `round(delay/60)`. `actual` = `scheduled + minuty*60` (aby „14:20 +2 → 14:22“ vždy sedělo). Odpočet = `when_label(scheduled + delay_s, now)`. Znak mínus je U+2212 „−“ jako dnes.

- [ ] **Step 1: Testy** (`t(h,m)` = 2026-11-16 v TZ):

```rust
let now = t(14, 17);
assert_eq!(time_display(t(14,20), Some(120), now), TimeDisplay { scheduled: "14:20".into(), actual: Some("14:22".into()), kind: DelayKind::Late, detail: "+2 min · za 5 min".into() });
assert_eq!(time_display(t(14,24), Some(20), now).detail, "včas · za 7 min");
assert_eq!(time_display(t(14,24), Some(20), now).actual, None);
let u = time_display(t(14,24), None, now);
assert_eq!((u.kind, u.detail.as_str(), u.actual), (DelayKind::Unknown, "za 7 min", None));
let e = time_display(t(14,20), Some(-70), now);
assert_eq!((e.kind, e.actual.as_deref(), e.detail.as_str()), (DelayKind::Early, Some("14:19"), "−1 min · za 1 min"));
```

Test `delay_across_midnight`: `time_display(23:58 dne 16. 11., Some(300), now 23:55)` → `actual == Some("00:03")`, `detail == "+5 min · za 8 min"`.
- [ ] **Step 2:** `cargo test -p pid-core when_tests` → FAIL.
- [ ] **Step 3:** Implementuj.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** Commit `feat(core): scheduled/actual time display with delay`.

### Task 4: Trasa spoje z jízdního řádu (core)

**Files:**
- Modify: `crates/pid-core/src/timetable/query.rs` (inherentní metoda na `TimetableIndex`, ne v traitu)
- Test: `crates/gtfs-prep/tests/queries.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct TripStop { pub stop: u32, pub gtfs_id: String, pub name: String, pub lat: f64, pub lon: f64, pub time: DateTime<Tz> }
#[derive(Clone, Debug, PartialEq)]
pub struct TripRoute { pub stops: Vec<TripStop>, /// index zastávky nástupu
    pub board: Option<usize> }
impl TimetableIndex { pub fn trip_route(&self, dep: &Departure) -> TripRoute }
```

Všechny zastávky patternu spoje `dep.trip` v pořadí (i s `NO_PICKUP`/`NO_DROP_OFF`). Den služby a pozici nástupu urči tak, že najdeš den `d ∈ {dep.time−1 den, dep.time, dep.time+1 den}` a pozici `pos` se `stops[pos] == dep.stop`, pro které `gtfs_instant(d, trip.start + offsets[pos]) == dep.time`. Tím fungují i linky, které projedou stejnou zastávku dvakrát, a noční spoje. Časy = `gtfs_instant(d, trip.start + offsets[i])`. Nenalezeno → `board: None` a časy ze dne `dep.time`.

- [ ] **Step 1: Testy:**
  - `trip_route_lists_all_stops`: Náměstí Míru, 2026-10-26 07:58, první odjezd (T22b) → jména `["Náměstí Míru", "Jana Masaryka", "Ruská"]`, časy `["08:00", "08:02", "08:05"]`, `board == Some(0)`.
  - `night_trip_stops_after_midnight`: odjezd z `night_trip_after_midnight` (2026-11-01 00:05) → časy `["00:10", "00:12", "00:15"]` a datum všech je 2026-11-01.
- [ ] **Step 2:** `cargo test -p gtfs-prep --test queries trip_route` → FAIL.
- [ ] **Step 3:** Implementuj.
- [ ] **Step 4:** `cargo test -p pid-core -p gtfs-prep` → PASS.
- [ ] **Step 5:** Commit `feat(core): full trip route with times`.

### Task 5: Poloha vozu ve feedu a umístění na trase (core)

Ověřeno 2026-11-15 na živém feedu: `VehiclePosition` má vždy `position` (2) a `timestamp` (5), `current_stop_sequence` (3) má asi 71 % vozů, `current_status` (4) chybí vždy (výchozí IN_TRANSIT_TO), `stop_id` (7) jen výjimečně.

**Files:**
- Modify: `crates/pid-core/src/realtime.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct VehicleFix { pub label: Option<String>, pub lat: f64, pub lon: f64, pub stop_sequence: Option<u32>, pub timestamp: Option<i64> }
pub fn vehicle_fix(feed: &[u8], trip_id: &str) -> Option<VehicleFix>
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VehiclePlace { AtStop(usize), /// mezi zastávkami i a i+1
    Between(usize) }
pub fn place_vehicle(stops: &[(f64, f64)], fix: &VehicleFix) -> Option<VehiclePlace>
```

Do `VehiclePosition` přidej `position: Option<Position>` (tag 2, `Position { latitude: f32 tag 1, longitude: f32 tag 2 }`), `current_stop_sequence: Option<u32>` (tag 3), `timestamp: Option<u64>` (tag 5). `label` počítej stejně jako dnes ve `vehicle_for_trip` a ten přepiš na `vehicle_fix(..)?.label`, se stejným chováním a projdou stávající testy.

`place_vehicle`: konstanty `AT_STOP_M = 50.0`, `MAX_OFF_ROUTE_M = 300.0`.
1. Když `stop_sequence = Some(s)` a `1 ≤ s ≤ stops.len()`: `next = s−1`. `next == 0` nebo GPS ≤ 50 m od `stops[next]` → `AtStop(next)`, jinak `Between(next−1)`. Předpoklad: PID `stop_sequence` je 1..n bez mezer. Ověř na `.gtfs/stop_times.txt`, pokud je k dispozici (`cargo xtask dist --fetch-gtfs`). Pokud neplatí, tuto větev vynech a vždy použij krok 2.
2. Jinak nejbližší úsek lomené čáry (ekvirektangulární projekce kolem bodu, `crate::geo` pro vzdálenost). Minimum > 300 m → `None`. Bod ≤ 50 m od koncové zastávky úseku → `AtStop`, jinak `Between(i)`.

- [ ] **Step 1: Testy** (trasa tří zastávek na přímce po ~500 m, poloha vždy s `label: None, timestamp: None`):
  - `place_by_stop_sequence`: `stop_sequence: Some(3)`, GPS v půli mezi 2. a 3. → `Between(1)`.
  - `place_by_gps_projection`: `stop_sequence: None`, GPS kousek vedle úseku 1–2 → `Between(0)`. GPS 20 m od 2. zastávky → `AtStop(1)`.
  - `place_vehicle_far_from_route_is_none`: `stop_sequence: None`, GPS 1 km od trasy → `None`.
  - `vehicle_fix_reads_position`: feed sestavený jako ve stávajících testech, s `position`, `current_stop_sequence: 5`, `timestamp` → odpovídající `VehicleFix`. Neznámý trip → `None`.
- [ ] **Step 2:** `cargo test -p pid-core realtime` → FAIL.
- [ ] **Step 3:** Implementuj.
- [ ] **Step 4:** `cargo test -p pid-core` → PASS (včetně stávajících testů `vehicle_for_trip`).
- [ ] **Step 5:** Commit `feat(core): vehicle position and placement on route`.

### Task 6: Mimořádnosti (core)

Ověřeno 2026-11-15: `alerts.pb` má entity s `alert` (tag 5). V alertu je `active_period` (1, často jen `start`), `informed_entity` (5, `route_id` tag 2, např. `L135`), `url` (8), `header_text` (10), `description_text` (11), dál cause/effect (6, 7) a další, které nepotřebujeme.

**Files:**
- Modify: `crates/pid-core/src/realtime.rs` (zprávy protobuf, `ALERTS_URL`)
- Create: `crates/pid-core/src/alerts.rs` (+ `pub mod alerts;` v `lib.rs`)

**Interfaces:**
- Produces:

```rust
pub const ALERTS_URL: &str = "https://api.golemio.cz/v2/vehiclepositions/gtfsrt/alerts.pb"; // realtime.rs
// alerts.rs
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceAlert { pub id: String, pub route_ids: Vec<String>, pub header: String, pub description: String,
    pub url: Option<String>, pub start: Option<i64>, pub end: Option<i64> }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertPhase { Active, Planned, Over }
impl ServiceAlert { pub fn phase(&self, now_s: i64) -> AlertPhase }
#[derive(Clone, Debug, PartialEq)]
pub struct ListedAlert { pub alert: ServiceAlert, pub relevant: bool, pub phase: AlertPhase }
pub fn parse_alerts(feed: &[u8]) -> Vec<ServiceAlert>
pub fn watched_routes(rides: &[Ride], now: DateTime<FixedOffset>, days: i64, resolve_short: impl Fn(&str) -> Option<String>) -> HashSet<String>
pub fn list_alerts(alerts: &[ServiceAlert], watched: &HashSet<String>, now_s: i64) -> Vec<ListedAlert>
pub fn fresh_relevant<'a>(alerts: &'a [ServiceAlert], watched: &HashSet<String>, seen: &HashSet<String>, now_s: i64) -> Vec<&'a ServiceAlert>
```

Pravidla:
- Texty: překlad s `language == "cs"`, jinak první. `route_ids` bez duplicit, v pořadí feedu. `start` = nejmenší start ze všech period, `end` = největší konec (chybějící konec = `None`).
- `phase`: `end < now` → Over, `start > now` → Planned, jinak Active (i bez periody).
- `watched_routes`: nesmazané jízdy s `board_time ≥ now − days` nebo se `status == InProgress`. `route_id`, když není prázdné, jinak `resolve_short(route_short_name)`.
- `list_alerts`: bez Over. Řazení: Active před Planned, uvnitř relevantní (průnik `route_ids` s `watched`) dřív, jinak pořadí feedu.
- `fresh_relevant`: ne v `seen`, ne Over, relevantní.

- [ ] **Step 1: Testy** (feed sestavený přes prost jako v `realtime.rs`):
  - `parses_czech_texts_and_routes`: alert s překlady en+cs, dvěma entitami `L22`, `L22`, `L9` → `header` je česky, `route_ids == ["L22","L9"]`.
  - `garbage_feed_is_empty`: `parse_alerts(b"xyz")` je prázdné.
  - `phase_active_planned_over`: start v budoucnu → Planned, konec v minulosti → Over, bez periody → Active.
  - `alert_without_route_is_listed_not_relevant`: alert bez `route_id` je v `list_alerts` s `relevant == false` a nikdy není ve `fresh_relevant`.
  - `watched_routes_last_30_days`: jízda L22 před 10 dny, L9 před 40 dny, smazaná L5 včera, ručně „136“ bez route_id (resolve → `Some("L136")`) → `{"L22","L136"}`.
  - `fresh_relevant_skips_seen_and_over`.
  - `list_order_active_relevant_first`.
- [ ] **Step 2:** `cargo test -p pid-core alerts` → FAIL.
- [ ] **Step 3:** Implementuj.
- [ ] **Step 4:** `cargo test -p pid-core` → PASS.
- [ ] **Step 5:** Commit `feat(core): GTFS-RT service alerts`.

### Task 7: Barvy a písmo PID

**Files:**
- Create: `crates/app/public/fonts/` (woff2 + `OFL.txt`)
- Modify: `crates/app/index.html` (`<link data-trunk rel="copy-dir" href="public/fonts">`)
- Modify: `crates/app/style.css` (`:root` a dark proměnné, `@font-face`, `font-family`)

- [ ] **Step 1:** Stáhni woff2 z `@fontsource/fira-sans` na jsDelivr (poslední verzi zjistíš z `https://data.jsdelivr.com/v1/packages/npm/@fontsource/fira-sans`): `files/fira-sans-{latin,latin-ext}-{400,500,700}-normal.woff2` (6 souborů) a `LICENSE` jako `OFL.txt`. Ulož do `crates/app/public/fonts/`.
- [ ] **Step 2:** Šest `@font-face` (`font-family: "Fira Sans"`, `font-display: swap`, `unicode-range` podle souborů z balíčku, tj. podle jeho `index.css`). `html, body { font-family: "Fira Sans", -apple-system, …, sans-serif; }`.
- [ ] **Step 3:** Přemapuj proměnné na paletu z Global Constraints: `--accent: #C40C00`, `--accent-fg: #fff`, `--bg: #F8F8F8`, `--surface: #fff`, `--fg: #252525`, `--muted: #4B4B4B`, `--border: #E8E8E8`, `--ok: #00B274` (text ok v tmavším odstínu, pokud nesplní kontrast AA), `--warn: #F59D1A` jako barva plochy pruhu (text na ní #252525). Přidej `--bar-bg` (světle #C40C00, tmavě #252525) a `--bar-line` (tmavě #C40C00). Tmavé hodnoty podle Global Constraints. Ověř kontrast textu na `--accent` (AA ≥ 4,5:1). `html.halloween` dál přepisuje proměnné.
- [ ] **Step 4:** `MSYS_NO_PATHCONV=1 cargo xtask dist --public-url / --no-data` → OK. Ověř, že `dist/fonts/*.woff2` existují a jsou v precache seznamu v `dist/sw.js` (xtask prochází celé `dist`). V `trunk serve` v DevTools → Network: písmo se načítá z `/fonts/`, žádný požadavek na Google Fonts.
- [ ] **Step 5:** Commit `feat(app): PID colors and bundled Fira Sans`.

### Task 8: Horní proužek, menu, plovoucí tlačítko

**Files:**
- Create: `crates/app/src/topbar.rs` (`TopBar`, `Menu`, `StatusChip`). `mod topbar;` v `main.rs`
- Modify: `crates/app/src/app.rs` (místo `<SyncBadge />` a `<BottomNav />` vykreslí `<TopBar />`)
- Modify: `crates/app/src/components.rs` (odstraň `BottomNav`, `SyncBadge` zůstává a vykresluje se v menu)
- Modify: `crates/app/src/state.rs` (`menu_open: RwSignal<bool>`, `Screen::Alerts { open: Option<String> }` pro Task 10, zatím s prázdnou obrazovkou)
- Modify: `crates/app/src/screens/home.rs` (`Idle`: místo `primary huge` je `<button class="fab">"＋ Nová jízda"</button>`)
- Modify: `crates/app/style.css`

**Interfaces:**
- Produces: `#[component] pub fn TopBar() -> impl IntoView`. `AppState::menu_open`. `Screen::Alerts { open: Option<String> }`. Slot pro počet mimořádností: `TopBar` čte `state.alert_count: Memo<usize>` z Task 10. Do té doby přidej do `AppState` signál s hodnotou 0, Task 10 ho nahradí.

Požadavky ze spec 4.1 a 4.2:
- Proužek: `position: sticky; top: 0`, výška + `env(safe-area-inset-top)`, `background: var(--bar-bg)`. Vlevo ☰/✕ (se zapnutým `menu_open` ✕), uprostřed „PID Tracker“, vpravo `StatusChip`.
- `StatusChip` podle `SyncStatus`: `Idle{last:Some(t)}` → „✓ t“, `Idle{last:None}` → „–“, `Running` → „⟳“, `Pending(n)` → „čeká n“, `NeedsSignIn(_)` a `Error(_)` → „⚠“. Klepnutí otevře menu.
- Menu: panel zleva (šířka ~75 %, max 300 px) přes ztmavení. Nahoře „Přihlášen: <přezdívka>“ a `<SyncBadge />` (akční tlačítka zůstávají). Položky Jízda, Historie, Statistiky, Mimořádnosti (+ počet), Nastavení. Zvýraznění podle stejných pravidel jako dřív `BottomNav`. Klepnutí na položku nebo na ztmavení menu zavře. Tečka u ☰, když `alert_count > 0`.
- `.fab`: `position: fixed; left: calc(16px + env(safe-area-inset-left)); bottom: calc(16px + env(safe-area-inset-bottom))`, pill, stín, `var(--accent)`. Z-index: obsah < fab (20) < menu (40) < toast (50). `.home` má dole padding ≥ 96 px. Odstraň `--nav-h` a padding pro spodní lištu z `.app`.
- [ ] **Step 1:** Implementuj.
- [ ] **Step 2:** `cargo build -p app --target wasm32-unknown-unknown` → OK. `cargo clippy -p app --target wasm32-unknown-unknown` bez nových varování.
- [ ] **Step 3:** `trunk serve`, šířka 390 px, světlý i tmavý režim (DevTools → Rendering → prefers-color-scheme): proužek nahoře i při scrollu, menu se otevírá a zavírá, všechny položky přepnou obrazovku, fab je vlevo dole a poslední jízda jde vysunout nad něj. Při probíhající jízdě fab není. Halloween (`?halloween=1`) nepřekrývá fab ani proužek.
- [ ] **Step 4:** Commit `feat(app): top bar with menu and status, floating new-ride button`.

### Task 9: Zpoždění v Nástupu a detail spoje

**Files:**
- Modify: `crates/app/src/components.rs` (nová komponenta `DepTime`)
- Create: `crates/app/src/screens/trip.rs` (`TripDetail`), export v `screens/mod.rs`
- Modify: `crates/app/src/screens/board.rs` (`DepRow` používá `DepTime`, má tlačítko „›“, panel detailu)
- Modify: `crates/app/src/realtime.rs` (`pub async fn fetch_vehicle(trip_id: &str) -> Option<VehicleFix>`)
- Modify: `crates/app/style.css`

**Interfaces:**
- Consumes: `time_display`, `TimeDisplay`, `DelayKind` (Task 3). `TimetableIndex::trip_route` (Task 4). `vehicle_fix`, `place_vehicle`, `VehiclePlace` (Task 5).
- Produces: `#[component] pub fn DepTime(scheduled: DateTime<Tz>, delay_s: Option<i32>, at: DateTime<Tz>) -> impl IntoView`. `#[component] pub fn TripDetail(row: BoardRow, delays: RwSignal<Delays>, on_close: Callback<()>) -> impl IntoView`. Ten má i volitelné `alert: Option<ServiceAlert>`, které Task 10 doplní.

- [ ] **Step 1: `DepTime`** podle spec 4.4. Late/Early: `<s class="sched">14:20</s> <b class="actual late|early">14:22</b>`, pod tím `<small class=…>{detail}</small>`. OnTime/Unknown: `<b>14:24</b>` a `<small class="ok|muted">`. `DepRow` místo dnešního `<span class="dep-time">` použije `DepTime` s `row.dep.time`, `row.delay_s`, `at`. Odstraň `delay_label` z `board.rs`.
- [ ] **Step 2: Tlačítko „›“** jako samostatný `<button class="dep-more" aria-label="Trasa spoje">` vedle řádku (cíl ≥ 44 × 44 px). `on:click` s `stop_propagation` nastaví `detail: RwSignal<Option<BoardRow>>` v `Board`. Výběr spoje se nemění. U „Jiná linka…“ není.
- [ ] **Step 3: `TripDetail`** jako celoobrazovkový panel (`position: fixed; inset: 0`, pod proužkem, z-index pod menu) vykreslený v `Board`, když `detail` je `Some`. Tím zůstane zachovaná zastávka i výběr. Tlačítko Zpět zavolá `on_close`. Obsah podle spec 4.5:
  - `trip_route(&row.dep)`;
  - u zastávky `DepTime` se zpožděním `delays.at(trip_gtfs_id, stop.gtfs_id)`;
  - `board` zvýrazněná s „tady stojíš“;
  - vozidlo: `fetch_vehicle` při otevření a pak každých 60 s, když `!page_hidden()`. Zastav po zavření (vzor `alive` z `Board`);
  - `place_vehicle` → štítek „🚋 vůz {label} · před {N s|N min}“ (věk z `timestamp` a `now()`; bez labelu jen „🚋 vůz“) mezi zastávkami nebo u zastávky;
  - zastávky před vozem šedě;
  - bez polohy poznámka „poloha vozu není k dispozici“.
  - Dlouhá trasa: ukaž rozsah od (vůz nebo nástup) − 2 do (nástup + 5) a zbytek sbal do rozbalitelných řádků „⋮ dalších N zastávek“.
- [ ] **Step 4:** `cargo build -p app --target wasm32-unknown-unknown` → OK.
- [ ] **Step 5:** `trunk serve` s daty (`cargo xtask dist --fetch-gtfs` jednou připraví `public/data`): v Nástupu u tramvaje se zpožděním vidíš přeškrtnutý čas. Klepnutí na řádek vybírá, „›“ otevře detail se štítkem vozu. V DevTools se `vehicle_positions.pb` stahuje jednou za 60 s a po zavření detailu přestane. Zpět vrátí stejný výběr. Offline (DevTools → Offline) detail ukáže trasu bez vozu a bez chyby.
- [ ] **Step 6:** Commit `feat(app): delay display and trip detail with vehicle position`.

### Task 10: Mimořádnosti v aplikaci

**Files:**
- Create: `crates/app/src/alerts.rs` (stahování, úložiště, odvozené signály). `mod alerts;` v `main.rs`, `alerts::install(state)` v `App`
- Create: `crates/app/src/screens/alerts.rs` (`AlertsScreen`). Napojení na `Screen::Alerts { open }` v `app.rs`
- Modify: `crates/app/src/state.rs`, `crates/app/src/util.rs` (`keys::ALERTS = "pt.alerts"`, `keys::ALERTS_SEEN = "pt.alerts_seen"`)
- Modify: `crates/app/src/topbar.rs` (pruh nové mimořádnosti pod proužkem, skutečný `alert_count`)
- Modify: `crates/app/src/screens/board.rs`, `crates/app/src/screens/trip.rs` (⚠)

**Interfaces:**
- Consumes: `parse_alerts`, `list_alerts`, `fresh_relevant`, `watched_routes`, `ServiceAlert`, `ALERTS_URL` (Task 6). `TimetableIndex::search_routes`.
- Produces: `AppState::alerts: RwSignal<Vec<ServiceAlert>>`, `AppState::alert_banner: RwSignal<Vec<String>>` (id nových pro pruh), `AppState::watched: Memo<HashSet<String>>`, `AppState::alert_count: Memo<usize>` (aktivní relevantní). `pub fn route_alert(state: AppState, route_id: &str) -> Option<ServiceAlert>` (první **aktivní** alert linky).

Chování (spec 4.6):
- `install`: stažení při startu, při změně `state.foreground` a každých 300 s, když `!page_hidden()`. Chyba nebo offline → nic se nemění. Úspěch → `alerts.set`, uloží `pt.alerts`. Při startu se nejdřív načte uložený stav.
- `resolve_short` pro `watched_routes`: `search_routes(name, 5)` → jediná přesná shoda `short_name == name` → její `route_id`.
- Seen: `pt.alerts_seen` (JSON pole id). Chybí klíč (první spuštění této verze) → ulož všechna aktuální id bez hlášky. Jinak `fresh_relevant` → přidej id do `alert_banner` a do seen. Seen prořezávej na id přítomná ve feedu.
- Pruh (spec 4.6, „Upozornění“): pod proužkem, `var(--warn)`. Jeden: „⚠ Nová mimořádnost · [odznaky] · nadpis“, víc: „⚠ N nové mimořádnosti“. Klepnutí → `Screen::Alerts { open: první id }` a vyprázdní `alert_banner`, ✕ jen vyprázdní. Plánované: v textu „od 18. 11. 10:00“.
- `AlertsScreen`: `list_alerts`, relevantní zvýrazněné, odznaky linek (barva z jízdního řádu podle `route_id`, jinak neutrální), nadpis, „plánováno od …“ u Planned. Klepnutí rozbalí popis a odkaz „Více na pid.cz“ (`target="_blank"`). `open` je rozbalené a odscrollované. Prázdné: „Žádné mimořádnosti“. Bez dat a offline: „Mimořádnosti nejsou k dispozici (offline)“.
- ⚠ v `DepRow` a v hlavičce `TripDetail`, když `route_alert` vrátí `Some`. Klepnutí na ⚠ v řádku (se `stop_propagation`) → `Screen::Alerts { open: id }`.
- [ ] **Step 1:** Implementuj.
- [ ] **Step 2:** `cargo build -p app --target wasm32-unknown-unknown` → OK.
- [ ] **Step 3:** `trunk serve`: seznam ukazuje aktuální mimořádnosti z feedu. Test hlášky: v DevTools smaž z `pt.alerts_seen` jedno id linky, kterou máš v historii, a přepni záložku pryč a zpět → vyjede pruh. Po zavření se znovu neukáže. Smazání celého klíče → žádný pruh (první spuštění). `alerts.pb` se stahuje ~1× za 5 min.
- [ ] **Step 4:** Commit `feat(app): service alerts list, banner and route markers`.

### Task 11: Poznámka v Úpravě a ikonka v Historii

**Files:**
- Modify: `crates/app/src/screens/edit.rs` (textarea „Poznámka“ na konci formuláře, signál jako `plate`, uložení v `save`, v `ReadOnly` zobrazení textu)
- Modify: `crates/app/src/components.rs` (`RideRow`: „📝“ v `ride-meta`, když `!note.trim().is_empty()`)

**Interfaces:**
- Consumes: `Ride::note` (Task 1).

- [ ] **Step 1:** Implementuj. Nepřidávej poznámku do `Board` ani do `Running`.
- [ ] **Step 2:** `cargo build -p app --target wasm32-unknown-unknown` → OK. V `trunk serve`: poznámka se uloží, po obnovení stránky zůstane, v Historii je 📝.
- [ ] **Step 3:** Commit `feat(app): ride note in edit screen`.

### Task 12: Statistiky „Oba“ jako jeden blok

**Files:**
- Modify: `crates/app/src/screens/stats.rs`

**Interfaces:**
- Consumes: `compute_combined` (Task 2).

- [ ] **Step 1:** `Who::Both` → jeden `PersonCard` z `compute_combined` (`persons` = já + ostatní). `PersonCard` dostane `#[prop(optional)] title: Option<String>`. Pro „Oba“ je nadpis „Oba“, jinak přezdívka jako dnes. Volby „Já“ a druhý uživatel beze změny.
- [ ] **Step 2:** `cargo build -p app --target wasm32-unknown-unknown` → OK. V `trunk serve` „Oba“ ukazuje jeden blok a počet jízd = součet jednotlivců.
- [ ] **Step 3:** Commit `feat(app): combined stats card for both`.

### Task 13: Tiché obnovení Google tokenu

**Files:**
- Create: `crates/pid-core/src/reauth.rs` (+ `pub mod reauth;`)
- Modify: `crates/app/src/interop/mod.rs` (`gisRequestToken(clientId, scope, hint, silent)`: při `silent` `opts.prompt = ''`. Nová `onUserTap(cb)`: listener `click` ve fázi capture na `document`, ignoruje cíle uvnitř `input, textarea, select`, volá `cb` synchronně)
- Modify: `crates/app/src/google/auth.rs` (`sign_in_with(login_hint, silent: bool)`, `sign_in` = `sign_in_with(hint, false)`. `pub fn token_expires_at_ms() -> Option<f64>` čte uložený token i prošlý)
- Create: `crates/app/src/reauth.rs` (`pub fn install(state: AppState)`), volání v `App`

**Interfaces:**
- Produces (core):

```rust
pub const REFRESH_BEFORE_MS: f64 = 300_000.0;
pub const MIN_ATTEMPT_GAP_MS: f64 = 120_000.0;
pub struct RefreshInput { pub now_ms: f64, pub expires_at_ms: Option<f64>, pub signed_in_before: bool,
    pub online: bool, pub in_flight: bool, pub last_attempt_ms: Option<f64> }
pub fn should_refresh(i: &RefreshInput) -> bool
```

`true` jen když: `signed_in_before && online && !in_flight`, poslední pokus je starší než 120 s (nebo žádný), a token chybí nebo `expires_at − now < 300 s`.

- [ ] **Step 1: Testy** v `reauth.rs`: `refresh_when_expired_or_near` (chybí → true, 4 min do konce → true, 10 min → false), `refresh_throttled_and_offline` (pokus před 60 s → false, před 130 s → true, offline → false, in_flight → false, `signed_in_before == false` → false).
- [ ] **Step 2:** `cargo test -p pid-core reauth` → FAIL, implementuj → PASS.
- [ ] **Step 3:** `reauth::install`: `onUserTap` → `should_refresh` (`signed_in_before` = `state.me` je `Some`, `online` = `navigator.onLine`). Pokud `true`: zapiš `last_attempt`, `in_flight = true`, synchronně zavolej `sign_in_with(me, true)`. Úspěch → `request_sync(state)`. Chyba → tiše (bez toastu). Vždy `in_flight = false`. Stav drž v `thread_local` / `Cell` v modulu.
- [ ] **Step 4:** `cargo build -p app --target wasm32-unknown-unknown` → OK. V `trunk serve`: v localStorage v JSON klíče `pt.token` nastav `expires_at_ms` do minulosti, klepni na tlačítko → popup Google se otevře a sám zavře. Tlačítko zároveň udělá svou akci a synchronizace proběhne. Další klepnutí do 2 min popup neotevře. Klepnutí do textového pole neotevře nic.
- [ ] **Step 5:** Commit `feat(app): silent Google token refresh on user tap`.

### Task 14: Vydání 0.5.0

**Files:**
- Modify: `Cargo.toml` (`version = "0.5.0"`), `Cargo.lock`
- Modify: `docs/testing-iphone.md` (kontrolní bod G)

- [ ] **Step 1:** Verze 0.5.0. `cargo metadata --format-version 1 > /dev/null` aktualizuje lock. Pak `cargo metadata --locked --format-version 1 > /dev/null` → exit 0.
- [ ] **Step 2:** Do `docs/testing-iphone.md` přidej „Kontrolní bod G – verze 0.5.0“ s položkami (zaškrtávací, česky, ve stylu bodu F):
  - červený proužek PID, písmo Fira Sans i offline, tmavý režim podle systému;
  - menu ☰ a ikonka stavu;
  - fab vlevo dole a poslední jízda jde vysunout nad něj;
  - přeškrtnutý čas u zpožděného spoje;
  - „›“ otevře trasu se štítkem vozu;
  - Mimořádnosti v menu a pruh při nové mimořádnosti na lince z historie;
  - poznámka v Úpravě se objeví ve sloupci AB tabulky;
  - „Oba“ ve Statistikách jako jeden součet;
  - po hodině klepnutí obnoví přihlášení bez zadávání hesla. Pokud popup v PWA zůstane prázdný nebo nejde zavřít, zapsat sem a vypnout obnovu pro iOS standalone, viz spec 4.9.
- [ ] **Step 3:** `cargo test --workspace --exclude app --locked` → vše PASS. `cargo clippy --workspace --exclude app` a `cargo clippy -p app --target wasm32-unknown-unknown` bez nových varování. `MSYS_NO_PATHCONV=1 cargo xtask dist --public-url / --no-data` → OK.
- [ ] **Step 4:** Commit `chore: release 0.5.0`.
