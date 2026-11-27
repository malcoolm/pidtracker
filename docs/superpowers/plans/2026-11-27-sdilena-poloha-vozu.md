# Sdílená poloha vozů – implementační plán (#14)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Jedno sdílené úložiště polohy vozů, stahované po 20 s jen když ho nějaká obrazovka potřebuje, aby detail spoje a detail spojení měly polohu hned a čerstvou.

**Architecture:** `pid-core` jednou dekóduje feed na tabulku `trip_id → VehicleFix` a má čisté pravidlo „kdy stáhnout“. Aplikace má modul `positions` s daty v `AppState`, smyčkou po 5 s a přihlašováním obrazovek (`subscribe` + `on_cleanup`). `trip.rs` a `journey.rs` ztratí vlastní smyčky a čtou ze sdíleného signálu.

**Tech Stack:** Rust, Leptos (CSR, wasm), prost, gloo-net, gloo-timers.

**Spec:** `docs/superpowers/specs/2026-11-27-sdilena-poloha-vozu-design.md`

## Global Constraints

- Interval stahování: **20 s** (`POSITIONS_MAX_AGE_MS = 20_000`); kontrola pravidla každých **5 s**.
- Nestahuje se, když není přihlášená žádná obrazovka nebo je stránka skrytá (`page_hidden()`).
- Při chybě stahování (offline) zůstávají poslední data.
- Chování `vehicle_fix`, `vehicle_for_trip`, `vehicles_for_trips` se nemění; jejich testy zůstávají beze změny.
- Texty a komentáře česky, commity anglicky s `(#14)`, poslední `(closes #14)` až po iPhonu.

## Review Focus

- Otevření a zavření detailu rychle po sobě: počet přihlášených nesmí klesnout pod 0 ani zůstat > 0 (test na `should_fetch` s nulou + kontrola `on_cleanup` v review).
- Dva detaily přihlášené současně (detail spojení otevřený z vyhledávání a pak detail spoje): stahuje se jednou, ne dvakrát (příznak běžícího stahování; test `should_fetch` s `in_flight = true`).
- Offline po předchozím úspěchu: zůstanou poslední data, `trip.rs` neukáže „poloha není k dispozici“ jen kvůli výpadku.
- Čas stažení v budoucnosti (změna hodin telefonu): stáří < 0 se bere jako „čerstvé“, ale nesmí zablokovat stahování navždy (test se `age_ms` záporným → stáhnout).
- Spoj ve feedu dvakrát (duplicitní entita): vyhraje první, jako dosud `vehicle_fix` (test).

---

### Task 1: `pid-core` – tabulka poloh a pravidlo stahování

**Files:**
- Modify: `crates/pid-core/src/realtime.rs`

**Interfaces:**
- Produces:
  - `pub fn parse_vehicle_fixes(feed: &[u8]) -> HashMap<String, VehicleFix>` – všechny entity s `trip_id` a `position`; neplatný feed → prázdná tabulka; duplicitní `trip_id` → první výskyt.
  - `pub const POSITIONS_MAX_AGE_MS: f64 = 20_000.0;`
  - `pub fn should_fetch_positions(subscribers: u32, hidden: bool, in_flight: bool, age_ms: Option<f64>) -> bool` – `true` právě když `subscribers > 0 && !hidden && !in_flight` a (`age_ms` je `None`, nebo `age_ms >= POSITIONS_MAX_AGE_MS`, nebo `age_ms < 0`).

- [ ] **Step 1: Failing testy** v novém `#[cfg(test)] mod store_tests`:
  - `parse_fixes_maps_trips_with_position`: feed se dvěma vozy s polohou a jedním bez `position` → 2 záznamy, správné `label`, `lat`, `stop_sequence`.
  - `parse_fixes_first_duplicate_wins`: dvě entity se stejným `trip_id`, labely `"1"` a `"2"` → `"1"`.
  - `parse_fixes_garbage_is_empty`: `b"\xff\xff"` → prázdná.
  - `should_fetch_rules`: `(0,false,false,None)`→false; `(1,true,false,None)`→false; `(1,false,true,None)`→false; `(1,false,false,None)`→true; `(1,false,false,Some(5_000.0))`→false; `(2,false,false,Some(20_000.0))`→true; `(1,false,false,Some(-1.0))`→true.
- [ ] **Step 2:** `cargo test -p pid-core store_tests` → FAIL (nedefinované funkce).
- [ ] **Step 3:** Implementovat obě funkce a konstantu; `vehicle_fix` a `vehicles_for_trips` přepsat nad `parse_vehicle_fixes` (`vehicle_for_trip` může zůstat, nevyžaduje polohu).
- [ ] **Step 4:** `cargo test -p pid-core` → všechny testy PASS (i původní).
- [ ] **Step 5:** Commit `feat(core): parse vehicle positions once into a trip table, fetch rule (#14)`.

### Task 2: Aplikace – modul `positions` a přechod obrazovek

**Files:**
- Create: `crates/app/src/positions.rs`
- Modify: `crates/app/src/main.rs` (`mod positions;`), `crates/app/src/state.rs` (pole), `crates/app/src/app.rs` (`crate::positions::install(state)` vedle `alerts::install`), `crates/app/src/realtime.rs`, `crates/app/src/screens/trip.rs`, `crates/app/src/screens/journey.rs`, `docs/testing-iphone.md`

**Interfaces:**
- Consumes: Task 1 (`parse_vehicle_fixes`, `should_fetch_positions`).
- Produces:
  - `AppState.positions: RwSignal<Option<Positions>>`, `pub struct Positions { pub fixes: Arc<HashMap<String, VehicleFix>>, pub fetched_ms: f64 }` (v `positions.rs`, `Clone`).
  - `AppState.positions_subs: RwSignal<u32>`, `AppState.positions_in_flight: RwSignal<bool>`.
  - `pub fn install(state: AppState)` – smyčka po 5 000 ms volá `try_fetch`; Effect na `state.foreground` volá `try_fetch`.
  - `pub fn subscribe(state: AppState)` – volá se v těle komponenty: `positions_subs += 1`, `try_fetch(state)`, `on_cleanup` → `positions_subs -= 1` (saturating).
  - `fn try_fetch(state: AppState)` – podle `should_fetch_positions(subs, page_hidden(), in_flight, age)`; nastaví `in_flight`, stáhne `VEHICLE_POSITIONS_URL`, při úspěchu `positions.set(Some(..))` s `js_sys::Date::now()`, vždy `in_flight = false`.

Aplikační kód v Leptosu nemá unit testy (wasm); logika je pokrytá v Tasku 1, chování se ověří v prohlížeči.

- [ ] **Step 1:** Modul `positions.rs`, pole v `AppState`, `install` v `app.rs`.
- [ ] **Step 2:** `trip.rs`: smazat `VEHICLE_REFRESH_MS`, `refresh`, smyčku a foreground Effect pro polohu; zavolat `subscribe(state)`; `vehicle` = `Memo` z `state.positions` (`fixes.get(&trip_id).cloned()`), `loaded` = `positions.is_some()`; `at` dál tiká vlastní smyčkou, pokud ji obrazovka má, jinak se nastaví při změně `positions`.
- [ ] **Step 3:** `journey.rs`: smazat `VEHICLE_REFRESH_MS` a stahovací smyčku z #1; `subscribe(state)` jen když spojení má jízdu; `vehicles` číst z `state.positions` (`label` z fixu). Bez živých dat se číslo nezobrazí.
- [ ] **Step 4:** `realtime.rs`: smazat `fetch_vehicle` a `fetch_vehicles`; `fill_vehicle` nejdřív zkusí `state.positions` (data mladší než `POSITIONS_MAX_AGE_MS`), jinak stáhne jako dosud.
- [ ] **Step 5:** `cargo clippy --workspace --all-targets -- -D warnings` a `cargo test --workspace` → čisté.
- [ ] **Step 6:** Ověřit v prohlížeči (`cargo xtask dist --public-url / --fetch-gtfs`, server 8090): v síti je po otevření detailu jedno stažení a pak po ~20 s; po zavření detailu stahování přestane; detail spoje i spojení ukazují vůz.
- [ ] **Step 7:** Kontrolní bod O v `docs/testing-iphone.md` (poloha hned po otevření, obnova ~20 s, po zavření a v jiné obrazovce se nestahuje, návrat z pozadí, režim letadlo).
- [ ] **Step 8:** Commit `feat(app): shared vehicle positions refreshed every 20 s while needed (#14)`.
