# Plynulý pohyb vozu – implementační plán (#15)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Značka vozu v detailu spoje se mezi staženími posouvá každou sekundu podle jízdních dob, nikdy za další zastávku, a odhad je poznat (čárkovaný obrys, „odhad, GPS před N s“).

**Architecture:** Čistá funkce odhadu v `pid-core::realtime`; `trip.rs` tiká po 1 s, značku a vyblednutí kreslí z odhadu, souhrn zůstává z GPS. CSS přechod 1 s lineárně.

**Tech Stack:** Rust, Leptos 0.8 (CSR, wasm), CSS.

**Spec:** `docs/superpowers/specs/2026-11-27-plynuly-pohyb-vozu-design.md`

## Global Constraints

- Přepočet každou 1 s, jen s otevřeným detailem a viditelnou stránkou (`page_hidden()`).
- Odhad = od `timestamp` GPS polohy uplynulo ≥ 10 s (`ESTIMATE_AFTER_S`).
- Jízdní doba úseku nejméně 30 s (`MIN_SEGMENT_S`).
- Podíl úseku nejvýš 1: nikdy za další zastávku.
- Text souhrnu při odhadu: „odhad, GPS před N s“ (místo „před N s“).
- Souhrn nahoře (stav příjezdu, počet zastávek) zůstává podle GPS.

## Review Focus

- Poloha bez `timestamp`: žádný odhad, značka stojí (test `None`).
- `timestamp` v budoucnosti (posunuté hodiny telefonu): značka se nehýbe dozadu ani dopředu (test).
- Zastaralá poloha (> 2 min, #12): odhad se nepočítá, značka stojí zašedlá (ruling v Tasku 2, ověřit v prohlížeči).
- Vůz na poslední zastávce trasy: žádný úsek dál, nespadne na indexu (test).
- Jízdní řád s klesajícím časem mezi zastávkami (půlnoc, chyba dat): jízdní doba ≥ 30 s, ne záporná (test).

---

### Task 1: `pid-core` – odhad místa vozu

**Files:**
- Modify: `crates/pid-core/src/realtime.rs`

**Interfaces:**
- Produces:
  - `pub const MIN_SEGMENT_S: i64 = 30;`, `pub const ESTIMATE_AFTER_S: i64 = 10;`
  - `pub fn estimate_place(place: VehiclePlace, progress: f32, stop_times: &[i64], fix_ts: Option<i64>, now_ts: i64) -> (VehiclePlace, f32)` – `stop_times` = unixové časy zastávek trasy (jízdní řád + zpoždění), `progress` = podíl úseku u `Between`. Bez `fix_ts` nebo s `now_ts <= fix_ts` vrací vstup. `Between(i)`: podíl + uplynulé / jízdní doba úseku, nejvýš 1. `AtStop(i)`: do `max(fix_ts, stop_times[i])` stojí (`AtStop(i)`, 0), potom `Between(i)` s podílem od tohoto času, nejvýš 1; poslední zastávka zůstává `AtStop`. Jízdní doba = `max(stop_times[i+1] − stop_times[i], MIN_SEGMENT_S)`.
  - `pub fn is_estimate(fix_ts: Option<i64>, now_ts: i64) -> bool` – `fix_ts` známý a `now_ts − fix_ts ≥ ESTIMATE_AFTER_S`.

- [ ] **Step 1: Failing testy** v `mod estimate_tests` (časy zastávek `[0, 120, 240, 300]`):
  - `between_moves_with_time`: `Between(0)`, 0,25, fix 1000→`stop_times` posunuté o 1000, now = fix + 60 → `Between(0)`, 0,75 (±0,01).
  - `between_capped_at_next_stop`: now = fix + 600 → `Between(0)`, 1,0.
  - `at_stop_waits_for_departure`: `AtStop(1)`, fix = čas(1) − 30, now = čas(1) − 10 → `AtStop(1)`, 0; now = čas(1) + 60 → `Between(1)`, 0,5.
  - `last_stop_stays`: `AtStop(3)` → `AtStop(3)`.
  - `min_segment_duration`: úsek 2→3 má 60 s, ale s časy `[0, 0]` (nulová doba) a now = fix + 15 → podíl 0,5 (doba 30 s).
  - `no_timestamp_or_future_fix`: `fix_ts = None` → vstup beze změny; `now < fix` → vstup beze změny.
  - `estimate_flag`: `is_estimate(Some(100), 109)` = false, `(Some(100), 110)` = true, `(None, 500)` = false.
- [ ] **Step 2:** `cargo test -p pid-core estimate_tests` → FAIL.
- [ ] **Step 3:** Implementovat.
- [ ] **Step 4:** `cargo test -p pid-core` → PASS.
- [ ] **Step 5:** Commit `feat(core): estimate vehicle place between position fetches (#15)`.

### Task 2: Aplikace – plynulá značka a text odhadu

**Files:**
- Modify: `crates/app/src/screens/trip.rs`, `crates/app/style.css`, `docs/testing-iphone.md`

**Interfaces:**
- Consumes: Task 1 (`estimate_place`, `is_estimate`).
- Produces: `vehicle_text(kind, fix, place, now_ts, estimate: bool) -> String` – při `estimate` a čerstvé poloze „… · odhad, GPS před N s“.

- [ ] **Step 1: Failing test** `vehicle_text_estimate`: `vehicle_text(RouteKind::Tram, &fix(1_000), None, 1_025, true)` = „tramvaj 8758 · odhad, GPS před 25 s“; s `false` = „tramvaj 8758 · před 25 s“; zastaralá (1_180, true) = „tramvaj 8758 · poloha před 3 min“ (u zastaralé se odhad neukazuje).
- [ ] **Step 2:** `cargo test -p app vehicle_text` → FAIL.
- [ ] **Step 3:** Implementovat: `TICK_MS` 1 000; `stop_times` (unix) z časů zastávek + zpoždění z `delays` (bez zpoždění jízdní řád); memo odhadu = `estimate_place(place, progress, …, fix.timestamp, at)` jen pro čerstvou polohu (`FixFreshness::Fresh`), jinak GPS místo; tělo seznamu (vyblednutí, mezera, značka) z odhadu, souhrn z GPS; třída `est` na značce při `is_estimate` a čerstvé poloze.
- [ ] **Step 4:** CSS: `.veh-mark { transition: top 1s linear }`, `.trip-gap .rail-fade { transition: height 1s linear }`, `.veh-mark.est .veh-icon { border-style: dashed }`.
- [ ] **Step 5:** `cargo test --workspace`, clippy (workspace i wasm app) → čisté.
- [ ] **Step 6:** Ověřit v prohlížeči: značka se mezi staženími posouvá každou sekundu, po 10 s čárkovaný obrys a „odhad, GPS před N s“, po stažení plný obrys; nedojede za další zastávku; posun hodin o 3 min → zašedlá, nehýbe se.
- [ ] **Step 7:** Kontrolní bod U v `docs/testing-iphone.md`.
- [ ] **Step 8:** Commit `feat(app): smooth estimated vehicle movement in trip detail (#15)`.
