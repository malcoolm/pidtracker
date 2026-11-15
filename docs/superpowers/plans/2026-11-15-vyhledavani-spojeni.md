# PID Tracker 0.6.0: vyhledávání spojení – plán implementace

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Offline vyhledávač spojení (odkud = poloha nebo zastávka, kam = zastávka, odjezd/příjezd, vyloučení druhů dopravy) jako úvodní obrazovka, s pomalým modelem chůze a nastavitelnou rezervou.

**Architecture:** Pěší přestupy mezi uzly předpočítá `gtfs-prep` do nového pole `Timetable::footpaths` (formát `PIDT3`). Algoritmus RAPTOR je čistá logika v novém modulu `pid-core::timetable::router` nad `TimetableIndex`; testuje se na ručně sestaveném malém `Timetable` v unit testech a na fixture `mini` v `gtfs-prep/tests`. Leptos UI v `crates/app` (nová obrazovka `Screen::Search`, detail spojení, nastavení) jen volá router a zobrazuje.

**Tech Stack:** Rust stable (edition 2024), Leptos 0.8 CSR, trunk, chrono/chrono-tz, postcard + deflate, GitHub Pages.

**Spec:** `docs/superpowers/specs/2026-11-15-vyhledavani-spojeni-design.md` (implementátor čte spec i plán).

## Global Constraints

- Jen Rust toolchain, žádný server ani cizí API pro hledání. Hledání funguje offline.
- UI texty a komentáře v kódu česky.
- Model chůze (konstanty v `pid-core/src/timetable/walk.rs`): rychlost **1,0 m/s**, objížďka **× 1,3**, pěší přestup mezi uzly do **400 m** vzdušnou čarou, přístup z polohy do **1 500 m**, minimum přestupu **120 s**, s nástupištěm metra na kterékoli straně **240 s**, přístup z polohy na nástupiště metra **+120 s**, nejvýš **4 přestupy**.
- Přestupní čas A → B = **max(chůze, minimum)**, nikdy součet.
- Rezerva před prvním nástupem: Nastavení, **0–5 min, výchozí 2**; nevztahuje se na přestupy.
- Vyloučitelné druhy: Metro, Tramvaj, Autobus, Trolejbus, Vlak, Přívoz, Lanovka (`RouteKind`). Ukládá se lokálně.
- Seznam: 3–5 spojení na jedno klepnutí, „Další spojení“ přidává pozdější (u příjezdu dřívější). Výkonový cíl **< 300 ms na jedno klepnutí** na iPhonu.
- Čas dotazu jen v rozsahu `feed_start..=feed_end`.
- Během probíhající jízdy ukazuje úvodní obrazovka jízdu (beze změny), vyhledávání je v menu.
- Verze **0.6.0**. CI s `--locked`. Lokální build v Git Bash: `MSYS_NO_PATHCONV=1 cargo xtask dist --public-url / --no-data`. Testy: `cargo test --workspace --exclude app --locked`.
- Commity končí řádkem `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Spoj přes půlnoc** (dotaz 23:55, spoj s časem 24:05 předchozího servisního dne) → najde se a ukáže 00:05 správného dne. Test `finds_trip_after_midnight` (Task 3).
2. **Přestup na nástupiště metra se nestihne pod 4 min** → nabídne se pozdější vlak metra, ne nestihnutelný. Test `metro_transfer_needs_four_minutes` (Task 3).
3. **Nová aplikace se starými daty v cache (offline po nasazení)** → `decode` vrátí chybu, aplikace ukáže stávající hlášku o chybějících datech a nespadne; s novými daty nic dalšího. Test `decode_rejects_old_format` (Task 1).
4. **Žádné spojení** (cíl = odkud, vyloučeno vše, nic nejede, cíl mimo dosah) → prázdný seznam a srozumitelná hláška, ne panika. Testy `same_group_returns_empty`, `all_excluded_returns_empty` (Task 3).
5. **Dotaz u konce platnosti dat** (zítřek už mimo `feed_end`) → hledání nespadne na `service_runs` mimo rozsah a vrátí, co jede. Test `query_on_last_feed_day` (Task 3).

---

### Task 1: Pěší přestupy v datech (`footpaths`, formát PIDT3)

**Files:**
- Create: `crates/pid-core/src/timetable/walk.rs`
- Modify: `crates/pid-core/src/timetable/mod.rs` (pole `footpaths`, `pub mod walk`)
- Modify: `crates/pid-core/src/timetable/codec.rs` (`MAGIC = b"PIDT3"`)
- Modify: `crates/gtfs-prep/src/lib.rs` (výpočet ve `build_timetable`)
- Test: `crates/gtfs-prep/tests/build.rs`, unit testy v `walk.rs`

**Interfaces:**
- Produces:
  - `walk.rs`: `pub const WALK_SPEED_MPS: f64 = 1.0; pub const DETOUR: f64 = 1.3; pub const FOOTPATH_MAX_M: f64 = 400.0; pub const ACCESS_MAX_M: f64 = 1500.0; pub const MIN_TRANSFER_S: u32 = 120; pub const METRO_MIN_TRANSFER_S: u32 = 240; pub const METRO_ACCESS_EXTRA_S: u32 = 120; pub const MAX_TRANSFERS: u8 = 4;`
  - `pub fn walk_seconds(distance_m: f64) -> u32` (= `ceil(distance_m * DETOUR / WALK_SPEED_MPS)`).
  - `pub fn transfer_seconds(walk_s: u32, from_metro: bool, to_metro: bool) -> u32` (= max(walk_s, 240 pokud kterékoli metro, jinak 120)).
  - `Timetable::footpaths: Vec<Vec<Footpath>>` (index = stop), `#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)] pub struct Footpath { pub to: u32, pub walk_s: u16 }` – jen do stanovišť **jiného** uzlu, vzdušně ≤ 400 m, `walk_s` bez minima, obousměrně (A→B i B→A).

- [ ] **Step 1: Unit testy v `walk.rs`**: `walk_seconds(100.0) == 130`, `walk_seconds(0.0) == 0`; `transfer_seconds(30, false, false) == 120`, `transfer_seconds(30, true, false) == 240`, `transfer_seconds(300, false, true) == 300`.
- [ ] **Step 2: Test v `build.rs`** `footpaths_between_nearby_nodes`: ve fixture `mini` má `U1Z2P` (Náměstí Míru B) footpath na `U2Z1P` (Jana Masaryka), `walk_s` v rozmezí 340–380 (≈ 277 m × 1,3), a obráceně; žádný footpath na `U3Z1P` (Ruská, > 400 m); žádný footpath uvnitř uzlu Náměstí Míru. Test `decode_rejects_old_format`: bajty začínající `PIDT2` → `CodecError::BadMagic`.
- [ ] **Step 3: Spustit** `cargo test -p pid-core -p gtfs-prep --locked` → FAIL (chybí `walk`, `footpaths`).
- [ ] **Step 4: Implementovat.** Ve `gtfs-prep` použít mřížku buněk (≈ 0,005° lat × 0,008° lon) a porovnávat jen sousední buňky – plný O(n²) nad ~15 tis. stanovišť je v CI zbytečně pomalý. `haversine_m` z `pid_core::geo`. Stanoviště z `NEEDED` souborů beze změny (žádný nový GTFS soubor).
- [ ] **Step 5: Spustit testy** → PASS. Spustit `MSYS_NO_PATHCONV=1 cargo xtask dist --public-url / --fetch-gtfs` a zapsat do zprávy commitu velikost `.pidt` před a po (spec: očekávaně + stovky kB).
- [ ] **Step 6: Commit** `feat(data): precomputed walking transfers between nearby nodes (PIDT3)`.

### Task 2: Router – jedno hledání (RAPTOR, odjezd i příjezd)

**Files:**
- Create: `crates/pid-core/src/timetable/router.rs` (+ `pub mod router` v `timetable/mod.rs`)
- Modify: `crates/pid-core/src/timetable/query.rs` (`TimetableIndex`: nové pole `metro_stop: Vec<bool>`, `pub fn is_metro_stop(&self, stop: u32) -> bool`; zpřístupnit routeru `stop_patterns`, `pattern_profiles`, `profile_trips` jako `pub(crate)`)
- Test: unit testy v `router.rs` nad ručně sestaveným `Timetable` (helper `fn net() -> TimetableIndex` v `#[cfg(test)]`)

**Interfaces:**
- Consumes: `walk::*`, `Timetable::footpaths` (Task 1), `TimetableIndex`, `gtfs_instant`, `RouteKind::from_gtfs`.
- Produces (vše `pub` v `router.rs`):
  ```rust
  pub struct Access { pub stop: u32, pub walk_s: u32 }          // walk_s = 0 při startu ze zastávky
  pub enum TimeMode { DepartAt, ArriveBy }
  pub struct JourneyQuery {
      pub origins: Vec<Access>, pub target_group: u32,
      pub mode: TimeMode, pub time: DateTime<Tz>,
      pub excluded: Vec<RouteKind>, pub buffer_s: u32,
  }
  pub enum Leg {
      /// from = None → z polohy uživatele
      Walk { from: Option<u32>, to: u32, seconds: u32 },
      Ride { trip: u32, board_pos: u16, alight_pos: u16, board_stop: u32, alight_stop: u32,
             dep: DateTime<Tz>, arr: DateTime<Tz> },
  }
  pub struct Journey { pub legs: Vec<Leg>, pub leave: DateTime<Tz>, pub arrive: DateTime<Tz>,
                       pub transfers: u8, pub walk_s: u32 }
  impl TimetableIndex { pub fn plan_once(&self, q: &JourneyQuery) -> Vec<Journey>; }
  ```
  `plan_once` vrací Pareto množinu (nejdřívější příjezd pro každý počet přestupů 0..=4, jen zlepšující se; u `ArriveBy` nejpozdější odchod). `leave` = odjezd prvního spoje − chůze z polohy − rezerva (− 120 s navíc u nástupiště metra při startu z polohy). `walk_s` = součet chůze všech `Walk` úseků.

Algoritmické upřesnění (to, co signatury neurčují):
- Časy jako epoch sekundy (`i64`): čas zastávky = `gtfs_instant(d, 0).timestamp() + trip.start + offset` pro servisní den `d ∈ {D−1, D, D+1}`, kde D = datum dotazu. `service_runs` mimo `feed_start..feed_end` vrací false – to stačí.
- „Trasa“ RAPTORu = `Profile`. Spoje profilu jsou seřazené podle `start` a mají stejné offsety, takže se nepředjíždějí.
- Dvě pole na kolo: `arr[k][stop]` (příjezd vozidlem) a `ready[k][stop]` (nejdřív lze nastoupit). Po kole: `ready[stop] = arr + transfer_seconds(0, m, m)` na stejném stanovišti; ostatní stanoviště uzlu: `transfer_seconds(walk_seconds(vzdálenost), mA, mB)`; footpaths: `transfer_seconds(walk_s, mA, mB)`. Start: `ready[0][o.stop] = time + o.walk_s + buffer_s (+ METRO_ACCESS_EXTRA_S pokud o.walk_s > 0 a stop je metro)`.
- Cíl: kterékoli stanoviště `target_group`, hodnotí se `arr` (bez minima). Příjezd pěšky přes footpath do cílového uzlu se počítá čistým `walk_s`.
- `NO_PICKUP` na nástupu a `NO_DROP_OFF` na výstupu se respektují. Vyloučené `RouteKind` se nescanují.
- `ArriveBy` = zrcadlový zpětný běh (nejpozdější odjezd, přestupy se odečítají, rezerva a chůze z polohy se odečítají na konci).
- Rekonstrukce přes zpětné ukazatele uložené při zlepšení (`(k, stop) → Ride/Walk`).

- [ ] **Step 1: Testovací síť `net()`**: uzly „Start“ (S1 bus/tram stanoviště), „Uzel“ (H1 tram, H2 nástupiště metra, 50 m od sebe), „Cíl“ (T1 tram, T2 metro). Tram 9 (`route_type 0`): S1 8:00 → H1 8:05 → T1 8:40. Metro (`route_type 1`): H2 → T2 v 8:08, 8:10, 8:20, jízda 10 min. Služba jede každý den. Footpaths prázdné, kromě testu footpath.
- [ ] **Step 2: Testy** (`DepartAt` 7:55, origin `Access { stop: S1, walk_s: 0 }`, `buffer_s: 0`, není-li uvedeno jinak):
  - `pareto_fast_and_direct`: 2 výsledky – (příjezd 8:20, 1 přestup, metro 8:10) a (8:40, 0 přestupů).
  - `metro_transfer_needs_four_minutes`: metro 8:08 se nevybere (8:05 + 4 min > 8:08).
  - `excluded_metro_only_direct`: `excluded: [Metro]` → jen 8:40.
  - `buffer_blocks_tight_first_boarding`: dotaz 7:59, `buffer_s: 120` → tram 8:00 se nestihne, výsledek prázdný.
  - `access_walk_sets_leave`: origin `walk_s: 300`, dotaz 7:50 → `leave == 7:55`, první úsek `Walk { from: None, .. seconds: 300 }`.
  - `arrive_by_latest`: `ArriveBy` 8:35 → spojení s příjezdem 8:30 (metro 8:20) a odchodem 8:00.
  - `finds_trip_after_midnight`: noční spoj se `start` 24:05 předchozího dne, dotaz 23:55 → odjezd 00:05 dalšího kalendářního dne.
  - `same_group_returns_empty`, `all_excluded_returns_empty`, `query_on_last_feed_day` (dotaz na `feed_end`, D+1 mimo rozsah → bez paniky).
  - `footpath_to_target`: footpath T1 → J1 (jiný uzel „Jinde“, 180 s), cíl „Jinde“ → příjezd 8:43 přes tram 9 a `Walk`.
  - `no_pickup_no_dropoff_respected`: pattern s `NO_PICKUP` na S1 → z S1 nelze nastoupit.
- [ ] **Step 3: Spustit** `cargo test -p pid-core router --locked` → FAIL.
- [ ] **Step 4: Implementovat** `plan_once` a `is_metro_stop`.
- [ ] **Step 5: Spustit** → PASS; `cargo clippy -p pid-core --locked -- -D warnings` čisté.
- [ ] **Step 6: Commit** `feat(core): RAPTOR journey planner (depart at / arrive by)`.

### Task 3: Seznam spojení, štítky, přístup z polohy

**Files:**
- Modify: `crates/pid-core/src/timetable/router.rs`
- Test: unit testy v `router.rs`; `crates/gtfs-prep/tests/queries.rs` (nad `mini`); výkonový test v `crates/gtfs-prep/tests/perf.rs`

**Interfaces:**
- Consumes: `plan_once`, `Journey`, `JourneyQuery` (Task 2).
- Produces:
  ```rust
  impl TimetableIndex {
      /// Stanoviště uzlů do ACCESS_MAX_M od bodu, každé se svým walk_seconds.
      pub fn access_from(&self, lat: f64, lon: f64) -> Vec<Access>;
      /// Všechna stanoviště uzlu s walk_s = 0.
      pub fn access_group(&self, group: u32) -> Vec<Access>;
      /// 3–5 spojení (min. `min_count`), seřazená podle `leave`; viz pravidla níže.
      pub fn plan(&self, q: &JourneyQuery, min_count: usize) -> Vec<Journey>;
      /// Posune dřívější spoje na nejpozdější, které stihnou stejné navazující spoje.
      pub fn tighten(&self, j: Journey, buffer_s: u32) -> Journey;
  }
  pub struct Labels { pub fastest: Option<usize>, pub fewest: Option<usize> }
  pub fn labels(js: &[Journey]) -> Labels;
  ```
- Pravidla `plan`: opakuje `plan_once` s časem posunutým na (nejdřívější `leave` z posledního běhu + 60 s), u `ArriveBy` (nejpozdější `arrive` − 60 s); nejvýš 8 běhů; každé spojení projde `tighten`; duplicity (stejná posloupnost `trip`) se sloučí; odstraní se spojení dominované jiným (`leave ≥`, `arrive ≤`, `transfers ≤`, aspoň jedno ostře); skončí, jakmile je ≥ `min_count` spojení, a vrátí nejvýš 5.
- `labels`: `fastest` = nejmenší `arrive − leave`; `fewest` = nejméně přestupů, při shodě dřívější `arrive`. Může to být tentýž index.

- [ ] **Step 1: Testy**:
  - `tighten_moves_first_ride_later`: varianta sítě z Task 2 bez metra 8:08 a 8:10 a s druhým tramem 9 v 8:02 (H1 8:07). `plan_once` v 7:55 vrátí tram 8:00 + metro 8:20; `tighten` z něj udělá tram 8:02 + metro 8:20 a `leave` 8:02.
  - `plan_drops_dominated`: stejná varianta sítě → `plan` nevrátí dvě spojení se stejným vlakem metra 8:20 (zůstane jen to s tramem 8:02).
  - `plan_returns_at_least_three`: dotaz 7:30 nad sítí s 5 metry → ≥ 3 spojení, seřazená podle `leave`, bez duplicit.
  - `labels_fastest_and_fewest`: nad výsledkem `pareto_fast_and_direct` → `fastest` = spojení s metrem, `fewest` = přímý tram.
  - `access_from_within_1500m`: bod 1 400 m od S1 → obsahuje S1 s `walk_s == walk_seconds(1400.0)`; bod 1 600 m → neobsahuje.
  - V `queries.rs`: `plan_on_mini_feed` – Muzeum → Jana Masaryka v pracovní den 7:55 přes metro A (Muzeum 8:03 → Náměstí Míru 8:05) a tram 22 (Náměstí Míru 8:10), 1 přestup, příjezd 8:12.
- [ ] **Step 2: Spustit** → FAIL. **Step 3: Implementovat.** **Step 4: Spustit** → PASS.
- [ ] **Step 5: Výkon.** `crates/gtfs-prep/tests/perf.rs`, test `#[ignore] plan_perf_real_data`: načte `.pidt` z proměnné `PIDT_FILE`, změří `plan` (z polohy u Anděla do Letňan, pracovní den 7:40, `min_count 3`) 20× a vypíše medián. Spustit `PIDT_FILE=dist/data/<soubor>.pidt cargo test -p gtfs-prep --release --test perf -- --ignored --nocapture`. Pokud medián nativně > 60 ms (WASM na iPhonu je zhruba 3–5× pomalejší), přidat předfiltr profilů, které v dny D−1..D+1 nejedou (spočítat jednou na `plan`), a znovu změřit. Výsledek měření dát do zprávy commitu.
- [ ] **Step 6: Commit** `feat(core): journey list, tightening, labels and location access`.

### Task 4: Nastavení plánovače (rezerva, vyloučené druhy)

**Files:**
- Modify: `crates/app/src/util.rs` (`keys::PLANNER_BUFFER_MIN = "pt.planner_buffer_min"`, `keys::PLANNER_EXCLUDED = "pt.planner_excluded"`)
- Modify: `crates/app/src/state.rs` (`AppState::planner_buffer_min: RwSignal<u8>` výchozí 2, `planner_excluded: RwSignal<Vec<RouteKind>>` výchozí prázdné; obojí načtené z localStorage, metody `set_planner_buffer(u8)` a `set_planner_excluded(Vec<RouteKind>)` ukládají)
- Modify: `crates/app/src/screens/settings.rs` (sekce „Vyhledávání spojení“: výběr 0–5 min „Rezerva před prvním nástupem“)

**Interfaces:**
- Produces: `state.planner_buffer_min`, `state.planner_excluded`, `set_planner_buffer`, `set_planner_excluded` (používá Task 5). `RouteKind` musí mít `Serialize`/`Deserialize` (doplnit derive v `pid-core/src/ride.rs`, pokud chybí).

- [ ] **Step 1: Implementovat.** Hodnoty mimo 0–5 z localStorage se ořežou do rozsahu.
- [ ] **Step 2: Ověřit** `MSYS_NO_PATHCONV=1 cargo xtask dist --public-url / --no-data` projde; v `trunk serve` změnit rezervu, obnovit stránku → hodnota zůstane.
- [ ] **Step 3: Commit** `feat(app): planner settings (first-boarding buffer, excluded modes)`.

### Task 5: Obrazovka Spojení (formulář + seznam) jako úvodní

**Files:**
- Create: `crates/app/src/screens/search.rs` (+ export v `screens/mod.rs`)
- Modify: `crates/app/src/state.rs` (`Screen::Search`), `crates/app/src/app.rs` (route), `crates/app/src/screens/home.rs` (`Idle` = `<Search />` + stávající fab; seznam posledních jízd pryč), `crates/app/src/topbar.rs` (menu), `crates/app/style.css`

**Interfaces:**
- Consumes: `plan`, `labels`, `access_from`, `access_group`, `JourneyQuery` (Task 3); `planner_*` (Task 4); `StopInput`, `RouteBadge`, `current_position`, `remember_fix`, `fetch_delays`, `time_display`.
- Produces: `#[component] pub fn Search() -> impl IntoView`; `#[component] fn JourneyCard(j: Journey, labels: ...)`; klepnutí na kartu otevře `JourneyDetail` (Task 6) jako overlay ve stejné obrazovce (stejně jako `TripDetail` v Board).

Chování (podle spec 4.5, náhled `results-list.html` varianta A):
- Odkud: výchozí „📍 Moje poloha“ (poloha přes `current_position`, origins = `access_from`). Rozbalení ukáže uzly z `access_from` seskupené podle uzlu s minutami chůze; klepnutí → origins = `access_group(g)` a v poli název uzlu. Ruční zastávka přes `StopInput`. Bez polohy: pole prázdné, text chyby z `GeoError::message()` upravený na „…zastávku vyberte ručně“.
- Kam: `StopInput`. Odjezd/Příjezd přepínač, čas výchozí „teď“, `<input type="datetime-local">` s `min`/`max` z `feed_start`/`feed_end`.
- Vyloučit: 7 zaškrtávátek v pořadí Metro, Tramvaj, Autobus, Trolejbus, Vlak, Přívoz, Lanovka; změna ukládá `set_planner_excluded`.
- Hledat: `plan(q, 3)`; výpočet spustit přes `spawn_local` po vykreslení „Hledám…“, aby UI nezamrzlo bez odezvy. „Další spojení“: nový `plan` s časem za posledním výsledkem, připojit, odstranit duplicity.
- Karta: `HH:MM → HH:MM`, štítky „nejrychlejší“/„nejméně přestupů“, „N min · K přestup(y/ů) / bez přestupu · pěšky M min“, řada `🚶N` + `RouteBadge` oddělené „›“, odjezd prvního spoje se zpožděním (`delays.at(trip_gtfs_id, stop_gtfs_id)` + `time_display`). Zpoždění: jedno `fetch_delays()` po hledání a pak každých 60 s jen při viditelné stránce, jako v Board.
- Prázdný výsledek: „Spojení nenalezeno. Zkuste jiný čas nebo povolte víc druhů dopravy.“ Cíl = odkud: „Odkud a kam je stejná zastávka.“
- Menu: první položka „Spojení“ → `Screen::Search`. Je-li `current_ride()`, nad ní zvýrazněná „Probíhající jízda“ → `Screen::Home`. Položka „Jízda“ zmizí.

- [ ] **Step 1: Implementovat** podle výše.
- [ ] **Step 2: Ověřit** build (`cargo xtask dist … --fetch-gtfs`, aby byla data) a v `python -m http.server 8090 --bind 127.0.0.1 --directory dist`: hledání z polohy i ze zastávky, příjezd, vyloučení metra se pamatuje po obnovení, „Další spojení“, během probíhající jízdy úvodní obrazovka ukazuje jízdu a menu má obě položky.
- [ ] **Step 3: Commit** `feat(app): journey search as the home screen`.

### Task 6: Detail spojení (časová osa)

**Files:**
- Create: `crates/app/src/screens/journey.rs` (+ export)
- Modify: `crates/app/style.css`

**Interfaces:**
- Consumes: `Journey`, `Leg` (Task 2), `TimetableIndex::trip_route`/`platform`, `Delays`, `time_display`.
- Produces: `#[component] pub fn JourneyDetail(j: Journey, delays: RwSignal<Delays>, on_close: Callback<()>) -> impl IntoView`.

Chování (spec 4.5, náhled `connection-detail.html` varianta A): hlavička s časy, souhrnem a „Vyrazte do N min“ (`leave − teď`, „Vyrazte hned“ při ≤ 0, „Ujelo“ když první spoj už odjel podle zpoždění); svislá osa – `Walk` přerušovaně šedě s „🚶 N min“ (u přestupu „přestup 🚶 N min“), `Ride` v barvě linky s nástupištěm z `platform(board_stop)`, směrem (headsign) a sbaleným „▸ N zastávek“, které rozbalí mezilehlé zastávky s časy (z `trip_route`, úsek `board_pos..=alight_pos`); zpoždění u každého odjezdu ve formátu z `time_display`.

- [ ] **Step 1: Implementovat.**
- [ ] **Step 2: Ověřit** v prohlížeči na spojení s přestupem a s chůzí z polohy; tmavý režim čitelný.
- [ ] **Step 3: Commit** `feat(app): journey detail timeline`.

### Task 7: Vydání 0.6.0 a kontrolní bod na iPhonu

**Files:**
- Modify: `Cargo.toml` (workspace `version = "0.6.0"`), `Cargo.lock`, `docs/testing-iphone.md` (nový „Kontrolní bod H – verze 0.6.0“)

- [ ] **Step 1:** Kontrolní bod H: hledání z polohy venku (trvá pod ~1 s?), z ručně zvolené zastávky, příjezd, vyloučení druhu se pamatuje, rezerva v Nastavení mění „Vyrazte do“, detail spojení se zpožděním, offline hledání (režim letadlo), během jízdy úvodní obrazovka = jízda.
- [ ] **Step 2:** `cargo test --workspace --exclude app --locked` a `cargo clippy --workspace --locked -- -D warnings` čisté; build `cargo xtask dist`.
- [ ] **Step 3: Commit** `chore: release 0.6.0`. Push až po souhlasu uživatele.
