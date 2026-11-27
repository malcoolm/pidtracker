# Vůz na svislé lince trasy – implementační plán (#13)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** V detailu spoje souhrn „za N min · N zastávek“ / „přijíždí“ / „ve stanici“ / „vůz už odjel“ nahoře a značka vozu s piktogramem druhu dopravy a evidenčním číslem na čáře trasy podle ujeté části úseku.

**Architecture:** Čistá logika v `pid-core::realtime` (podíl úseku, stav příjezdu), piktogramy jako komponenta v `components.rs`, zobrazení v `screens/trip.rs` + CSS. Data (poloha, zpoždění, jízdní řád) zůstávají jako dnes.

**Tech Stack:** Rust, Leptos 0.8 (CSR, wasm), inline SVG, CSS.

**Spec:** `docs/superpowers/specs/2026-11-27-vuz-na-trase-design.md`

## Global Constraints

- Souhrn: velký text „za N min · N zastávek“ (skloňování jako `stops_text`), pod ním „<druh> <číslo> <kde> · před N s“.
- Stavy: vůz na mé zastávce → „ve stanici“; vůz už za mou zastávkou → „vůz už odjel“; těsně před mou zastávkou → „přijíždí“.
- Značka: kruh 30 px v barvě linky, bílý okraj 2 px, bílý piktogram 18 px podle `RouteKind`; vedle žlutý štítek s číslem vozu (jen když je číslo známé).
- Poloha starší 2 min: značka i souhrn zašedlé, „poloha před N min“; starší 10 min: vůz se nezobrazí (#12, `fix_freshness`).
- Bez polohy: souhrn jen „za N min“ podle jízdního řádu + „Poloha vozu není k dispozici.“
- Mimo rozsah: detail spojení, směr jízdy, emoji.

## Review Focus

- Moje zastávka je první zastávka trasy (index 0) nebo vůz ještě nevyjel z výchozí: žádný pád ani záporný počet zastávek (test `arrival_state` s `board = 0`).
- Spoj bez „tady stojíš“ (`board = None`, otevřeno z jiné zastávky skupiny): souhrn se nezobrazí, značka ano.
- Podíl úseku u úseku nulové délky (dvě zastávky na stejném místě): 0, ne NaN (test).
- Vůz daleko za mou zastávkou (o několik zastávek): „vůz už odjel“, ne „za -2 zastávky“ (test).
- Velmi dlouhý název zastávky v souhrnu: zalomí se, nepřeteče (CSS, ověřit v prohlížeči).

---

### Task 1: `pid-core` – podíl úseku a stav příjezdu

**Files:**
- Modify: `crates/pid-core/src/realtime.rs`

**Interfaces:**
- Produces:
  - `pub fn segment_progress(stops: &[(f64, f64)], fix: &VehicleFix, i: usize) -> f32` – projekce polohy na úsek `i → i+1`, 0–1; úsek mimo rozsah nebo nulové délky → 0.
  - `pub const ARRIVING_M: f64 = 200.0;`
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum ArrivalState { StopsAway(usize), Arriving, AtStop, Departed }`
  - `pub fn arrival_state(stops: &[(f64, f64)], fix: &VehicleFix, place: VehiclePlace, board: usize) -> ArrivalState` – `AtStop(board)` → `AtStop`; místo za nástupem (`AtStop(i)` s `i > board`, `Between(i)` s `i >= board`) → `Departed`; `Between(board − 1)` a vzdálenost k nástupu ≤ `ARRIVING_M` → `Arriving`; jinak `StopsAway(board − i)` pro `Between(i)` / `AtStop(i)`.

- [ ] **Step 1: Failing testy** v `mod arrival_tests` (trasa 4 zastávky po 500 m na sever, `fix(lat, lon, None)`):
  - `progress_half_way`: vůz uprostřed úseku 1 → `segment_progress(.., 1)` ≈ 0,5 (±0,05).
  - `progress_zero_length_segment`: dvě stejné souřadnice → 0,0 a ne NaN.
  - `state_stops_away`: `Between(0)`, board 2 → `StopsAway(2)`.
  - `state_arriving`: `Between(1)` s vozem 150 m před zastávkou 2, board 2 → `Arriving`; 300 m → `StopsAway(1)`.
  - `state_at_stop`: `AtStop(2)`, board 2 → `AtStop`.
  - `state_departed`: `Between(2)`, `AtStop(3)` při board 2 → `Departed`.
  - `state_board_first`: `AtStop(0)`, board 0 → `AtStop`; `Between(0)`, board 0 → `Departed`.
- [ ] **Step 2:** `cargo test -p pid-core arrival_tests` → FAIL (nedefinované).
- [ ] **Step 3:** Implementovat; `segment_progress` sdílí projekci s `nearest_segment` (vytáhnout pomocnou funkci).
- [ ] **Step 4:** `cargo test -p pid-core` → PASS.
- [ ] **Step 5:** Commit `feat(core): segment progress and arrival state of a vehicle (#13)`.

### Task 2: Aplikace – souhrn, značka s piktogramem

**Files:**
- Modify: `crates/app/src/components.rs` (komponenta `VehicleIcon`), `crates/app/src/screens/trip.rs`, `crates/app/style.css`, `docs/testing-iphone.md`

**Interfaces:**
- Consumes: Task 1 (`segment_progress`, `arrival_state`, `ArrivalState`), `fix_freshness`, `stops_text`, `RouteKind::from_gtfs(row.dep.route_type)`.
- Produces:
  - `#[component] pub fn VehicleIcon(kind: RouteKind, color: u32) -> impl IntoView` – kruh s SVG piktogramem (autobus, tramvaj se sběračem, trolejbus se dvěma tyčemi, metro „M“ v kruhu, vlak, ostatní = autobus).
  - `fn summary_text(state: Option<ArrivalState>, minutes: Option<i64>) -> String` v `trip.rs` + test: `StopsAway(2), Some(3)` → „za 3 min · 2 zastávky“; `StopsAway(1), Some(0)` → „teď · 1 zastávka“; `Arriving` → „přijíždí“; `AtStop` → „ve stanici“; `Departed` → „vůz už odjel“; `None, Some(5)` → „za 5 min“.
  - `fn kind_word(kind: RouteKind) -> &'static str` („autobus“, „tramvaj“, „trolejbus“, „metro“, „vlak“, „vůz“) + test.

- [ ] **Step 1: Failing testy** `summary_text_states` a `kind_word_cs` v `trip.rs` tests.
- [ ] **Step 2:** `cargo test -p app summary_text` → FAIL.
- [ ] **Step 3:** Implementovat obě funkce, `VehicleIcon`, souhrn nad `.trip-line` (minuty z času mé zastávky se zpožděním vůči `at`), značku vozu jako absolutně umístěný prvek v řádku úseku `Between(i)` (`top` = `progress` × výška řádku) nebo u zastávky `AtStop(i)`, štítek s číslem; zašednutí podle `fix_freshness`; projetá část čáry bledá. Odstranit starý řádek `.trip-vehicle` (text `vehicle_text` zůstává pro druhý řádek souhrnu).
- [ ] **Step 4:** `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` a `cargo clippy -p app --target wasm32-unknown-unknown -- -D warnings` → čisté.
- [ ] **Step 5:** Ověřit v prohlížeči (`cargo xtask dist`, server 8090): autobus, tramvaj a trolejbus mají své piktogramy a číslo, značka mezi zastávkami podle polohy, souhrn; posunem hodin zašednutí; dlouhý název zastávky se zalomí; světlý i tmavý režim a `?vanoce=1`.
- [ ] **Step 6:** Kontrolní bod T v `docs/testing-iphone.md`.
- [ ] **Step 7:** Commit `feat(app): vehicle on the route line with type icon and arrival summary (#13)`.
