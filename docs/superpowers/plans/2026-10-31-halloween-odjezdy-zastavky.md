# Halloween, odjezdy se zpožděním, jednotné hledání zastávek, SPZ – implementační plán

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Verze 0.4.0: správné odjezdy na Nástupu se zpožděním z GTFS-RT, stanice metra dopředu při nepřesné poloze, jeden našeptávač zastávek pro celou aplikaci, ruční SPZ vozu, halloweenský vzhled 24. 10.–4. 11. a build bez `-dirty`. Dnes večer nasazeno na GitHub Pages.

**Architecture:** Logika (výběr odjezdů, dekódování zpoždění, řazení metra, hledání podle blízkosti, SPZ, období Halloweenu) jde do `pid-core` s unit testy, případně do testů `gtfs-prep/tests/queries.rs`, které pracují s mini GTFS fixturou. `crates/app` jen volá tyto funkce a vykresluje UI. Halloween je CSS vrstva zapnutá třídou `halloween` na `<html>`.

**Tech Stack:** Rust stable, Leptos 0.8 CSR, trunk, prost (GTFS-RT), chrono/chrono-tz, gloo-net/gloo-timers, GitHub Actions + Pages.

**Spec:** `docs/superpowers/specs/2026-10-31-halloween-odjezdy-zastavky-design.md`

## Zjištění z dat (2026-10-31)

- `vehicle_positions.pb` (~190 kB) **neobsahuje zpoždění**, jen polohu, číslo vozu a výbavu.
- `trip_updates.pb` (`https://api.golemio.cz/v2/vehiclepositions/gtfsrt/trip_updates.pb`, ~1,3 MB, bez gzipu, CORS `*`, `s-maxage=40`) obsahuje pro každý jedoucí spoj `TripUpdate` (entity tag 3): `trip` (1) s `trip_id`, opakované `stop_time_update` (2) s `stop_sequence` (1), `arrival` (2, `StopTimeEvent.delay` tag 1, int32 s), `departure` (3), `stop_id` (4, např. `U872Z1P` = GTFS stop_id ze statických dat). Trip-level `delay` je tag 5.
- Zpoždění se tedy čte z `trip_updates.pb`, obnova jednou za **60 s** (horní mez spec kvůli objemu dat).

## Global Constraints

- UI texty česky. Bez Node v buildu, jen Rust toolchain (trunk, `cargo xtask`).
- Offline-first: bez sítě vše funguje, jen bez zpoždění. Realtime nikdy neblokuje Start ani nezobrazuje chybu.
- Ruční zadání zastávky, linky, čísla vozu i SPZ zůstává všude možné, ruční hodnota má přednost.
- Sloupce Google Sheetu jen na konec: `license_plate` = 27. sloupec (AA).
- Halloween: 24. 10.–4. 11. včetně, každý rok, podle data v Europe/Prague. 5. 11. původní vzhled. Žádný přepínač v Nastavení. Vynucení pro test: `?halloween=1` v URL.
- Metro dopředu, když `accuracy_m > 100.0`.
- Verze 0.4.0, `Cargo.lock` aktuální, CI s `--locked`.

## Review Focus

1. **Spoj se zpožděním, který podle jízdního řádu „už odjel“** (jízdní řád 08:00, teď 08:01, zpoždění +2 min) musí být nahoře jako nadcházející. Test v Tasku 2.
2. **Seznam se posune pod vybraným spojem:** ručně vybraný spoj nesmí tiše přeskočit na jiný, dokud je v seznamu (nadcházející nebo „právě odjelo“). Výběr se klíčuje podle `(trip, stop)`, ne podle indexu. Kontrola v Tasku 5.
3. **Úprava jízdy s ručně zadanou zastávkou:** uložení bez změny textu nesmí měnit `stop_id` ani název, a nový ruční text, který přesně neodpovídá názvu zastávky, se uloží bez `stop_id`. Kontrola v Tasku 6.
4. **Stará tabulka bez sloupce `license_plate`** se načte (prázdná SPZ) a hlavička se doplní. Test v Tasku 7.
5. **Přechod 4. 11. → 5. 11. a hranice 24. 10.:** `is_halloween` na hranicích. Test v Tasku 8. Třída se přepočítá při návratu do popředí.

---

### Task 1: Opravit `-dirty` (Cargo.lock + CI `--locked`)

**Files:**
- Modify: `Cargo.lock` (oprava už je v pracovním stromu: verze crate 0.2.0 → 0.3.0)
- Modify: `.github/workflows/deploy.yml`

- [ ] **Step 1:** V `deploy.yml` přidej `--locked` do `cargo test --workspace --exclude app --locked` a do `cargo xtask dist … --locked`. `xtask` je alias v `.cargo/config.toml`, takže ověř, že `--locked` projde přes alias jako argument cargo a ne jako argument xtasku. Pokud ne, použij krok `cargo metadata --locked --format-version 1 > /dev/null` před buildem.
- [ ] **Step 2:** Spusť `cargo metadata --locked --format-version 1 > /dev/null` a ověř exit 0.
- [ ] **Step 3:** Commit `fix(ci): up-to-date Cargo.lock, --locked in CI (no more -dirty build id)`.

### Task 2: Výběr odjezdů a zpoždění (pid-core)

**Files:**
- Modify: `crates/pid-core/src/timetable/query.rs` (`Departure` + nové funkce)
- Modify: `crates/pid-core/src/realtime.rs` (TripUpdate dekódování)
- Test: `crates/gtfs-prep/tests/queries.rs`, unit testy v `realtime.rs`

**Interfaces:**
- Produces:
  - `Departure` získá pole `pub stop_gtfs_id: String` (GTFS stop_id stanoviště odjezdu).
  - V `realtime.rs`: `pub const TRIP_UPDATES_URL: &str = "https://api.golemio.cz/v2/vehiclepositions/gtfsrt/trip_updates.pb";`
  - `#[derive(Clone, Debug, Default, PartialEq)] pub struct Delays { /* trip_id → TripDelay */ }` s `pub fn at(&self, trip_id: &str, stop_id: &str) -> Option<i32>` (sekundy). Pravidlo: `departure.delay` stop_time_update se shodným `stop_id`, jinak jeho `arrival.delay`, jinak trip-level `delay`, jinak `None`.
  - `pub fn parse_trip_delays(feed: &[u8]) -> Delays` (neplatná data → prázdné `Delays`).
  - V `query.rs`: `pub const DEPARTED_GRACE_S: i64 = 30;`
  - `#[derive(Clone, Debug, PartialEq)] pub struct BoardRow { pub dep: Departure, pub delay_s: Option<i32>, pub expected: DateTime<Tz> }`
  - `pub fn board_rows(deps: Vec<Departure>, delays: &Delays, now: DateTime<Tz>, recent_min: i64) -> (Vec<BoardRow>, Vec<BoardRow>)`. Vrací `(upcoming, recent)`. `expected = dep.time + delay`. Spoj je odjetý, když `expected + DEPARTED_GRACE_S < now`. `upcoming` obsahuje pro každou dvojici (route_id, headsign) první neodjetý spoj podle `expected`, seřazené podle `expected`. `recent` obsahuje odjeté spoje s `expected >= now - recent_min`, pro každou dvojici jen nejpozdější, nejnovější první.
  - `next_per_direction` zůstane (používá ho test), z Boardu se přestane volat.

- [ ] **Step 1: Failing testy v `queries.rs`**

```rust
#[test]
fn board_rows_skip_departed_trip() {
    use pid_core::realtime::Delays;
    use pid_core::timetable::query::board_rows;
    let ix = index();
    let now = prague(2026, 10, 26, 8, 1);
    let d = ix.departures(group(&ix, "Náměstí Míru"), now, 15, 30);
    let (up, recent) = board_rows(d, &Delays::default(), now, 5);
    assert_eq!(up[0].dep.trip_gtfs_id, "T22c");
    assert_eq!(recent[0].dep.trip_gtfs_id, "T22b");
    assert_eq!(up[0].delay_s, None);
}

#[test]
fn board_rows_delayed_trip_is_upcoming() {
    use pid_core::realtime::Delays;
    use pid_core::timetable::query::board_rows;
    let ix = index();
    let now = prague(2026, 10, 26, 8, 1);
    let d = ix.departures(group(&ix, "Náměstí Míru"), now, 15, 30);
    let delays = Delays::from_pairs([("T22b", "U1Z2P", 120)]); // jen pro testy, viz Step 3
    let (up, _) = board_rows(d, &delays, now, 5);
    assert_eq!(up[0].dep.trip_gtfs_id, "T22b");
    assert_eq!(up[0].delay_s, Some(120));
    assert_eq!(up[0].expected, prague(2026, 10, 26, 8, 2));
}

#[test]
fn departure_carries_stop_gtfs_id() {
    let ix = index();
    let d = ix.departures(group(&ix, "Náměstí Míru"), prague(2026, 10, 26, 7, 58), 5, 30);
    assert!(d.iter().any(|x| x.trip_gtfs_id == "T22b" && x.stop_gtfs_id == "U1Z2P"));
}
```

- [ ] **Step 2: Failing unit testy v `realtime.rs`** (zakóduj `FeedMessage` s `trip_update` přes prost jako stávající testy):
  - `delay_from_matching_stop`: stop_time_update `U1Z2P` s departure.delay 90 → `at("T1","U1Z2P") == Some(90)`.
  - `arrival_delay_when_no_departure`: jen arrival.delay 30 → `Some(30)`.
  - `trip_level_delay_fallback`: žádný odpovídající stop, trip delay 60 → `at("T1","XYZ") == Some(60)`.
  - `unknown_trip_or_garbage`: `at("nope", …) == None` a `parse_trip_delays(b"\xff\xff") == Delays::default()`.
- [ ] **Step 3: Implementace.** Rozšiř `FeedEntity` o `#[prost(message, optional, tag = "3")] trip_update: Option<TripUpdate>` a přidej `TripUpdate` (trip 1, stop_time_update 2 repeated, delay 5 optional int32), `StopTimeUpdate` (stop_sequence 1 optional uint32, arrival 2, departure 3, stop_id 4 optional string), `StopTimeEvent` (delay 1 optional int32). `Delays::from_pairs(impl IntoIterator<Item = (&str, &str, i32)>)` je veřejný konstruktor pro testy. `board_rows` a `stop_gtfs_id` naplň v `departures()`.
- [ ] **Step 4:** `cargo test -p pid-core -p gtfs-prep` → PASS.
- [ ] **Step 5:** Commit `feat(core): upcoming departures with GTFS-RT delays`.

### Task 3: Metro dopředu a hledání podle blízkosti (pid-core)

**Files:**
- Modify: `crates/pid-core/src/timetable/query.rs`
- Test: `crates/gtfs-prep/tests/queries.rs`

**Interfaces:**
- Produces:
  - `pub const METRO_FIRST_ACCURACY_M: f64 = 100.0;`
  - `TimetableIndex` získá privátní `group_has_metro: Vec<bool>` (skupina má stanoviště v patternu linky s `route_type == 1`) a `pub fn group_has_metro(&self, group: u32) -> bool`.
  - `nearby_groups(lat, lon, accuracy_m)`: při `accuracy_m > METRO_FIRST_ACCURACY_M` stabilně přeřadí výsledek, metro napřed. Jinak beze změny.
  - Nová metoda v traitu `DepartureSource`: `fn search_groups_near(&self, query: &str, limit: usize, near: Option<(f64, f64)>) -> Vec<NearbyGroup>;`. Při prázdném dotazu a `Some(near)` vrátí `limit` nejbližších skupin. Při prázdném dotazu a `None` vrátí prázdno. Jinak řadí podle `(match_rank, vzdálenost nebo název)`. `distance_m` je vyplněná, když je `near`, jinak 0.0. `search_groups(q, n)` = `search_groups_near(q, n, None)`.

- [ ] **Step 1: Failing testy**

```rust
#[test]
fn metro_first_when_inaccurate() {
    let ix = index();
    // U Jana Masaryka (bez metra), Náměstí Míru se stanicí A je ~250 m.
    assert_eq!(ix.nearby_groups(50.0735, 14.4400, 400.0)[0].name, "Náměstí Míru");
    assert_eq!(ix.nearby_groups(50.0735, 14.4400, 20.0)[0].name, "Jana Masaryka");
}

#[test]
fn search_near_orders_equal_matches_by_distance() {
    let ix = index();
    let near_nm = Some((50.0755, 14.4375));
    let r = ix.search_groups_near("m", 5, near_nm);
    assert_eq!(r[0].name, "Muzeum"); // prefixová shoda má přednost
    assert_eq!(r[1].name, "Náměstí Míru"); // shoda slova, blíž než Jana Masaryka
    assert!(r[1].distance_m < 50.0);
}

#[test]
fn search_near_empty_query_lists_nearest() {
    let r = index().search_groups_near("", 3, Some((50.0735, 14.4400)));
    assert_eq!(r[0].name, "Jana Masaryka");
    assert_eq!(r.len(), 3);
}

#[test]
fn search_diacritics_both_ways() {
    let ix = index();
    assert_eq!(ix.search_groups("RUS", 5)[0].name, "Ruská");
    assert_eq!(ix.search_groups("rusk", 5)[0].name, "Ruská");
    assert_eq!(ix.search_groups("Ruská", 5)[0].name, "Ruská");
    assert_eq!(ix.search_groups("náměstí", 5)[0].name, "Náměstí Míru");
}
```

- [ ] **Step 2:** `cargo test -p gtfs-prep --test queries` → nové testy FAIL.
- [ ] **Step 3:** Implementace podle Interfaces.
- [ ] **Step 4:** `cargo test -p pid-core -p gtfs-prep` → PASS (včetně stávajících testů).
- [ ] **Step 5:** Commit `feat(core): metro stations first on inaccurate fix, proximity-ranked stop search`.

### Task 4: Komponenta `StopInput` a poslední známá poloha (app)

**Files:**
- Create: `crates/app/src/stop_input.rs` (a `mod stop_input;` v `main.rs`)
- Modify: `crates/app/src/state.rs`, `crates/app/src/util.rs` (klíč `LAST_FIX = "pt.last_fix"`), `crates/app/src/interop/mod.rs` (`Fix` derive `Serialize, Deserialize`)
- Modify: `crates/app/style.css` (styl seznamu návrhů)

**Interfaces:**
- Produces:
  - `AppState.last_fix: RwSignal<Option<Fix>>`, načtená z localStorage. `AppState::remember_fix(&self, f: Fix)` ji nastaví a uloží. Každé úspěšné `current_position()` v Board i Alight volá `remember_fix`.
  - ```rust
    #[component]
    pub fn StopInput(
        text: RwSignal<String>,            // co je v poli; bez výběru = ruční název
        selected: RwSignal<Option<u32>>,   // vybraná skupina, None = ruční / nic
        near: Signal<Option<(f64, f64)>>,  // pro řazení podle blízkosti
        #[prop(optional)] candidates: Option<Signal<Vec<NearbyGroup>>>, // zobrazí se, když je pole prázdné
        #[prop(optional, into)] placeholder: Option<String>,
        #[prop(optional)] on_pick: Option<Callback<u32>>,
    ) -> impl IntoView
    ```
  - Chování: `<input type="search">` + pod ním čipy (styl `.chips`/`.chip` jako dnešní `GroupPicker`). Prázdné pole zobrazí `candidates` (když jsou), jinak `search_groups_near("", 8, near)`. S textem `search_groups_near(text, 8, near)`. Klepnutí na čip nastaví `selected = Some(g)` a `text = název skupiny` a zavolá `on_pick`. Psaní nastaví `selected = None`. Vybraný čip má třídu `selected`. Vzdálenost se ukáže jako dnes („· 120 m“), když je > 0.
  - Bez načteného jízdního řádu se zobrazí jen pole pro ruční zadání.
  - `GroupPicker` se odstraní.

- [ ] **Step 1:** Implementuj `StopInput`, `last_fix`, `remember_fix`.
- [ ] **Step 2:** `cargo build -p app --target wasm32-unknown-unknown` → OK. Projde i `cargo clippy -p app --target wasm32-unknown-unknown`, pokud ho projekt používá.
- [ ] **Step 3:** Commit `feat(app): shared StopInput with diacritics-insensitive, proximity-ranked suggestions`.

### Task 5: Nástup – odjezdy se zpožděním, obnova, StopInput

**Files:**
- Modify: `crates/app/src/screens/board.rs`, `crates/app/src/realtime.rs`, `crates/app/style.css`

**Interfaces:**
- Consumes: `board_rows`, `BoardRow`, `Delays`, `parse_trip_delays`, `TRIP_UPDATES_URL` (Task 2), `StopInput`, `remember_fix` (Task 4).
- Produces: `pub async fn fetch_delays() -> Option<Delays>` v `crates/app/src/realtime.rs` (síťová chyba → `None`).

Požadavky:
- `BEFORE_MIN` = 15 (okno pro dotaz), `RECENT_MIN` = 5, `AFTER_MIN` = 30.
- `at` se posouvá: interval 30 s (`gloo_timers::callback::Interval`, zrušit v `on_cleanup`) nastaví `at = now()`. Zbývající minuty se počítají z `expected`.
- Zpoždění: `delays: RwSignal<Delays>`. Načte se hned po otevření a pak každých 60 s, dokud je obrazovka otevřená (zastavit v `on_cleanup`). Chyba sítě nic nemění a nic nehlásí.
- `LineChoice::Departure(usize)` nahraď `LineChoice::Trip { trip: u32, stop: u32 }` (+ `Manual`). Výchozí výběr je první `upcoming`. Ručně vybraný spoj zůstává vybraný, dokud je v `upcoming` nebo `recent`. Když zmizí, výběr přejde na první `upcoming`. Effect, který dnes přepíná na `Manual` při prázdných odjezdech, zachovej.
- Řádek: štítek, směr, `HH:MM` podle jízdního řádu, pod tím „za N min“ / „teď“ podle `expected`, a když `delay_s` je `Some(d)` a `|d| >= 60`, pak `+N min` (třída `delay late`) nebo `−N min` (`delay early`). Při |d| < 60 se nic nepřidá.
- Pod seznamem `upcoming` je sbalovací tlačítko „Právě odjelo (N)“, které rozbalí řádky `recent` ve stejném formátu (jen když N > 0).
- Start bere `Departure` z vybraného řádku (upcoming nebo recent). Zbytek ukládání beze změny.
- Výběr zastávky: `StopInput` s `candidates` = dnešní `candidates`, `near` = `fix` nebo `last_fix`. Text ve `StopInput` bez výběru = ruční zastávka (nahrazuje `manual_stop` + odkaz „Zastávka není v seznamu?“). `on_pick` dělá to, co dnes (`at = now()`, výběr na první spoj).
- `fill_vehicle` po Startu zůstává.

- [ ] **Step 1:** Implementace.
- [ ] **Step 2:** `cargo build -p app --target wasm32-unknown-unknown` → OK.
- [ ] **Step 3:** Ruční kontrola v `trunk serve` (viz README): Nástup zobrazí řádky, „Právě odjelo“ se rozbalí, v DevTools se `trip_updates.pb` stahuje jednou za 60 s a po odchodu z obrazovky už ne.
- [ ] **Step 4:** Commit `feat(app): board shows upcoming departures with live delays, auto-refresh`.

### Task 6: Výstup a Úprava jízdy přes StopInput

**Files:**
- Modify: `crates/app/src/screens/alight.rs`, `crates/app/src/screens/edit.rs`

Požadavky:
- **Výstup:** „Jiné zastávky v okolí“ + ruční pole nahradí jeden `StopInput` (`candidates` = `nearby`, `near` = `fix`/`last_fix`). `Choice::Group(g)` se nastaví při výběru. Psaní textu → `Choice::Manual` s textem z `StopInput`. Čipy „Na trase“ zůstávají. Výběr na trase vyčistí `StopInput` (`text` i `selected`).
- **Úprava:** obě zastávky přes `StopInput`. `near` pro nástup = `board_lat/lon` jízdy, jinak `last_fix`. Pro výstup = `alight_lat/lon`, jinak `last_fix`. `<datalist id="stop-suggestions">` a `stop_query` zmizí. Při uložení:
  - `selected == Some(g)` → `stop_id` = první stanoviště skupiny, název = název skupiny,
  - jinak, když se text změnil → dnešní logika `stop_id_for` (jen **přesná** shoda názvu, jinak prázdné `stop_id`),
  - jinak beze změny (Review Focus 3).
- Pole linky v Úpravě a Nástupu může zůstat `<datalist>` (ve spec je volitelné).

- [ ] **Step 1:** Implementace.
- [ ] **Step 2:** `cargo build -p app --target wasm32-unknown-unknown` → OK.
- [ ] **Step 3:** Ruční kontrola: v Úpravě napiš „mus“, zobrazí se návrh. Ulož jízdu bez změny zastávky a ověř, že `board_stop_id` se nezměnil.
- [ ] **Step 4:** Commit `feat(app): StopInput in alight and edit screens (replaces datalist)`.

### Task 7: SPZ vozu

**Files:**
- Modify: `crates/pid-core/src/ride.rs`, `crates/pid-core/src/sheet.rs`, `crates/pid-core/src/stats.rs`
- Modify: `crates/app/src/screens/home.rs`, `crates/app/src/screens/edit.rs`, `crates/app/src/components.rs`, `crates/app/src/realtime.rs`, `crates/app/src/screens/stats.rs`, `crates/app/src/screens/board.rs` (inicializace pole)

**Interfaces:**
- Produces:
  - `Ride.license_plate: String` s `#[serde(default)]`, prázdné = neznámá.
  - `HEADERS: [&str; 27]` s `"license_plate"` na indexu 26. `LAST_COL = "AA"`. Řádek zapisuje a čte `license_plate` (chybějící sloupec → `""`).
  - `pub fn known_plate(rides: &[Ride], vehicle: &str) -> Option<String>` v `ride.rs`: SPZ z nesmazané jízdy se stejným (oříznutým) číslem vozu a neprázdnou SPZ, s nejnovějším `board_time`. Pro prázdné `vehicle` vrací `None`.
  - `stats::VehicleCount { pub vehicle: String, pub rides: u32, pub plate: Option<String> }` a `PersonStats.by_vehicle: Vec<VehicleCount>`. `plate` = nejnovější neprázdná SPZ toho vozu v rámci jízd v období.
  - `pub fn apply_known_plate(ride: &mut Ride, all: &[Ride]) -> bool` v `ride.rs`: když je `license_plate` prázdná a `known_plate` něco vrátí, nastaví ji a vrátí `true`.

- [ ] **Step 1: Failing testy**
  - `sheet.rs`: `assert_eq!(HEADERS[26], "license_plate"); assert_eq!(LAST_COL, "AA");` a round-trip jízdy se SPZ „1AB 2345“. Test `old_sheet_without_plate_column_parses` (vzor: `old_sheet_without_vehicle_column_parses`) → `license_plate == ""`.
  - `ride.rs`: `known_plate` vrátí SPZ novější ze dvou jízd se stejným vozem, `None` pro neznámý vůz a pro prázdné číslo, a ignoruje smazané jízdy. `apply_known_plate` nepřepíše neprázdnou SPZ.
  - `stats.rs`: `by_vehicle[0] == VehicleCount { vehicle: "8758".into(), rides: 2, plate: Some("1AB 2345".into()) }` (uprav existující testy na nový typ).
- [ ] **Step 2:** `cargo test -p pid-core` → FAIL.
- [ ] **Step 3:** Implementace v `pid-core`. V `sheets.rs` (app) ověř, že doplnění chybějících hlaviček (`headers.iter().filter(...)`, kolem řádku 150) přidá `license_plate` a rozsahy používají `LAST_COL`.
- [ ] **Step 4:** UI:
  - **Home (probíhající jízda):** pole „SPZ“ vedle „Číslo vozu“ (`on:change`, oříznout). Po změně čísla vozu zavolej `apply_known_plate`.
  - **`fill_vehicle`:** po doplnění čísla vozu zavolej `apply_known_plate` se `state.rides`.
  - **Edit:** pole „SPZ“ pod „Číslo vozu“. Při uložení `r.license_plate = trim`. Když je SPZ prázdná a číslo vozu vyplněné, `apply_known_plate`.
  - **RideRow:** `vůz 8758 · 1AB 2345` (SPZ jen když je).
  - **Statistiky:** „Nejčastější vozy“ s popiskem `8758 · 1AB 2345`, když je `plate`.
- [ ] **Step 5:** `cargo test -p pid-core` → PASS. `cargo build -p app --target wasm32-unknown-unknown` → OK.
- [ ] **Step 6:** Commit `feat: manual license plate per vehicle, shown in history and stats`.

### Task 8: Halloweenský vzhled „C – Hravá“

**Files:**
- Create: `crates/pid-core/src/season.rs` (+ `pub mod season;` v `lib.rs`)
- Create: `crates/app/src/halloween.rs` (komponenta dekorací a přepínání třídy)
- Modify: `crates/app/src/app.rs`, `crates/app/style.css`, `crates/app/Cargo.toml` (web-sys features `Element`, `DomTokenList`, `HtmlElement` dle potřeby)

**Interfaces:**
- Produces:
  - `pub fn is_halloween(date: NaiveDate) -> bool` (24. 10.–4. 11. včetně).
  - `pub fn halloween_active() -> bool` v app: `is_halloween(now().date_naive())` nebo URL query obsahuje `halloween=1`.
  - `pub fn apply_season()` nastaví nebo odebere třídu `halloween` na `document.documentElement` a nastaví `<meta name="theme-color">` na `#2a2b30`, případně zpět na `#00559e`. Volá se při startu a v `on_visible`.
  - `#[component] pub fn HalloweenDeco() -> impl IntoView`: inline SVG (pavučina levý horní roh, pavouk na vlákně, dva kostlivci a čarodějnice na koštěti nad spodní navigací, dýně u nadpisu). Vše `position: fixed`, `pointer-events: none`, `aria-hidden="true"`, nízká opacita, aby nezasahovalo do čitelnosti. Vykreslí se v `app.rs` jen při `halloween_active()` (signál, přepočet v `on_visible`). Tvary lze převzít z mockupu `.superpowers/brainstorm/1169-1793477025/content/halloween-styl.html`.

- [ ] **Step 1: Failing test v `season.rs`**

```rust
#[test]
fn halloween_window() {
    let d = |m, dd| NaiveDate::from_ymd_opt(2026, m, dd).unwrap();
    assert!(!is_halloween(d(10, 23)));
    assert!(is_halloween(d(10, 24)));
    assert!(is_halloween(d(10, 31)));
    assert!(is_halloween(d(11, 4)));
    assert!(!is_halloween(d(11, 5)));
    assert!(!is_halloween(d(6, 1)));
}
```

- [ ] **Step 2:** `cargo test -p pid-core season` → FAIL, implementace → PASS.
- [ ] **Step 3: CSS** `html.halloween { … }` přepíše proměnné v obou režimech (selektor musí mít vyšší specificitu než `@media (prefers-color-scheme: dark) :root`): `--bg:#2a2b30; --surface:#36373d; --fg:#fff3e6; --muted:#b9ae9f; --border:#4a4b52; --accent:#ff8c1a; --accent-fg:#2a2b30; --accent-soft:#3f3a35; --warn:#ffd166; --warn-bg:#3a3222; --danger:#ff6b5a; --danger-bg:#3a2522; --ok:#8fd18f; color-scheme: dark;`. Nadpisy `h2, h3` mají `font-family: "Chalkboard SE", "Comic Sans MS", cursive` a barvu `#ffa040`. Text a odkazy zůstávají v systémovém písmu. Zpoždění `+N` má barvu `#ffd166`. Kontrast textu ≥ WCAG AA.
- [ ] **Step 4:** `cargo build -p app --target wasm32-unknown-unknown` → OK. V `trunk serve` zkontroluj vzhled na šířce 390 px: dnes (31. 10.) je halloween aktivní, dekorace nepřekrývají tlačítko Start ani spodní navigaci. Simulace 5. 11. jde přes dočasné volání `is_halloween` s pevným datem, nebo stačí test ze Step 1.
- [ ] **Step 5:** Commit `feat(app): Halloween theme 24.10.–4.11. (spiders, skeletons, witch)`.

### Task 9: Vydání 0.4.0 a nasazení

**Files:**
- Modify: `Cargo.toml` (`version = "0.4.0"`), `Cargo.lock`, `docs/testing-iphone.md` (checkpoint pro novinky)

- [ ] **Step 1:** Verze 0.4.0, `cargo metadata --format-version 1 > /dev/null` aktualizuje lock. Pak `cargo metadata --locked …` → exit 0.
- [ ] **Step 2:** `cargo test --workspace --exclude app --locked` → vše PASS. Lokální build: `MSYS_NO_PATHCONV=1 cargo xtask dist --public-url / --no-data` → OK.
- [ ] **Step 3:** Do `docs/testing-iphone.md` přidej checkpoint 0.4.0: Můstek v metru, zpoždění, „mus“ v Úpravě, SPZ, halloween.
- [ ] **Step 4:** `git status` je čistý po commitu `chore: release 0.4.0`. Push na `main`.
- [ ] **Step 5:** Sleduj běh GitHub Actions (`gh run watch` nebo `gh run list`) až do úspěchu. Pak `curl -s https://malcoolm.github.io/pidtracker/sw.js | head -2` musí ukázat `BUILD_ID = '0.4.0+<hash>.<čas>'` **bez** `-dirty`.
