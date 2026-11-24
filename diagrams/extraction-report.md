# Extraction report – PID Tracker Blueprint Deck

Output: `diagrams/index.html` (Blueprint Deck Kit 0.7.3), config: `diagrams/diagram.config.yaml`.
Generated 2026-11-24. Four diagrams in one file: Platform (journey), Architecture, Data flow, Process.

## Sources read

- `README.md`, `docs/proces.md`, `docs/pidt-format.md`, `docs/setup-google.md`, `docs/setup-github-pages.md`,
  `docs/testing-iphone.md` (installation and checkpoints).
- Specs: `docs/superpowers/specs/2026-10-25-pidtracker-design.md` (original design),
  `2026-10-31-halloween-odjezdy-zastavky-design.md`, `2026-11-15-vzhled-pid-trasa-mimoradnosti-design.md`,
  `2026-11-15-vyhledavani-spojeni-design.md`, `2026-11-16-vanocni-vzhled-design.md` (beginning). Plans under
  `docs/superpowers/plans/` were matched but not used: they repeat the specs as implementation steps.
- Code (see deviation below): `Cargo.toml`, `crates/*/Cargo.toml`, `crates/app/src/**` (config, storage, sync, update,
  alerts, realtime, google/*, screens/*, util), `crates/app/public/sw.js`, `crates/pid-core/src/**` (public API,
  `timetable/walk.rs`, `router.rs` constants, `realtime.rs` URLs, `sheet.rs` HEADERS), `crates/gtfs-prep/src/lib.rs`,
  `xtask/src/main.rs`, `.github/workflows/deploy.yml`, `.github/workflows/release.yml`, `.github/ISSUE_TEMPLATE/*`.
- GitHub issues #1–#7 (`gh issue list`, 2026-11-24) for the roadmap NOW phase.

**Deviation from `sources.include`:** the patterns `src/**`, `services/**` and `apps/**` match nothing in this repo; the
Rust code lives in `crates/*/src`, `xtask/` and `.github/workflows/`. These were read as well, otherwise the technical
diagrams would have had no code evidence. `package.json`, `go.mod`, `pyproject.toml`, `*.proto`, OpenAPI,
docker-compose, k8s and Helm: none exist.

## Structure

Approved by the user on 2026-11-24 (4th diagram = development and release process).

| Diagram | Areas | Why |
|---|---|---|
| Platform (journey) | 1 Set up · 2 Plan a trip · 3 At the stop · 4 On board · 5 Alight · 6 Look back; bottom row: Data & hosting, PID live data, Google (external), Sheet (customer) | The rider's commute in the order of the user flows in spec §6 |
| Architecture | Who uses it (customer) · App screens · Offline core · On the device · Sync & sign-in · Build & hosting · PID live data, Google, PID open data (external) · Ride sheet (customer) | Crates and runtime parts as a person would draw them: devices → UI → core → storage/sync → externals, build at the bottom |
| Data flow | 1 Source · 2 Prepare · 3 Ship · 4 On device · 5 Use · 6 Sync; Live feeds + Live overlay row; Ride sheet | Timetable stream left → right, live data as a separate row, rides synced to the sheet |
| Process | 1 Report · 2 Analyse · 3 Plan · 4 Build · 5 Release · 6 Run; GitHub (external), iPhones (customer), PID data (external) | Sections 1–5 of `docs/proces.md` plus the scheduled runs from `deploy.yml` |

Personas: by role, never by name. Platform: Sheet owner, Co-rider, Older traveller (spec 2026-11-15 search §1), Maintainer.
Architecture and Data flow: Rider, Older traveller, Maintainer. Process: the four roles in `docs/proces.md` (Users,
Product owner, Developers, Process guardian). All are off the map (`personas.on_map: false`).

Tabs per diagram: Overview, Main flow (Process: Happy path), Governance, Operations, Deployment, plus Live data
(not in Process), plus Personas.

## Inventory (id → main source)

Every box carries `sources` in the file; the main ones:

- Platform: signin → spec §6.1/§7.3, onboarding.rs · sheetcheck → spec §7.10, config.rs · timetable → pidt-format.md ·
  autoupdate → spec §7.7, update.rs · nearby → search spec §4.3, walk.rs · search → search spec §4.1/§4.4, router.rs ·
  conndetail → search spec §4.5 · ridefrom (Later) → search spec §3/§6 · board → Halloween spec §4.1, board.rs ·
  tripdetail → look spec §4.5 · alerts → look spec §4.6 · start/inprog/alightpick/done → spec §6.2 · vehicle → spec §7.9 ·
  plate → Halloween spec §4.5 · outbox → spec §7.5, storage.rs · history/stats → spec §6.3/§6.4, look spec §4.8 ·
  syncstatus → look spec §4.1 · gis/sheetsapi/sheet → spec §7.3/§7.4, setup-google.md · feeds → realtime.rs ·
  gtfs/pages → README Data, setup-github-pages.md.
- Architecture: screens → `crates/app/src/screens/*.rs`; core → `crates/pid-core/src/*`; storage → storage.rs, util.rs,
  sw.js; sync → sync.rs, google/auth.rs, reauth.rs, pid-core sync.rs, sheet.rs; build → gtfs-prep lib.rs, xtask, workflows.
- Data flow: prepare/ship → pidt-format.md, gtfs-prep, sw.js; device/use → timetable_loader.rs, storage.rs, query.rs,
  router.rs, alerts.rs; live → board.rs, trip.rs, alerts.rs, realtime.rs; sync → spec §7.5, pid-core sync.rs.
- Process: every step → the matching section of `docs/proces.md`; deploy/weekly/seasonal → deploy.yml; tag → README
  "Vydání verze", release.yml; check → testing-iphone.md; autoupd → update.rs.

Connection evidence (connections have no `sources` field):
- Google: `google/auth.rs` (GIS, userinfo), `google/sheets.rs` (Sheets API v4 base URL).
- Golemio: `pid-core/src/realtime.rs` (three feed URLs), fetched in `app/src/realtime.rs` and `app/src/alerts.rs`.
- PID GTFS: `gtfs-prep/src/lib.rs` (`PID_GTFS_URL`).
- Storage: `storage.rs` (IndexedDB `pidtracker`, stores `rides`, `outbox`), `util.rs` (localStorage keys).
- Service worker → caches: `sw.js` (`app-<build>`, `pid-data`, 3 s network timeout).
- Build: `xtask` calls gtfs-prep and writes `version.json`, seasonal icons; `deploy.yml` runs tests and `xtask dist`.
- Journey edges (Platform) follow the user flow in spec §6.2, not code calls.

## Conflicts

- Original spec §3 excludes realtime data; later specs add delays, vehicle positions and alerts (decision of
  2026-10-25 and the 2026-10-31 / 2026-11-15 specs). The deck follows the later specs and the code.
- Original spec §7.1 names the core crate `core`; the code calls it `pid-core`. The deck uses `pid-core`.
- Original spec §7.2 targets "up to ~15 MB"; pidt-format.md reports ~1.3 MB actual (15 MB is the tool's limit). Both shown as such.
- Spec §7.5 originally offered sheet selection; §7.10 fixes the sheet ID in the build. The deck follows §7.10 and `config.rs`.
- Issues #5, #6, #7 are committed (`41afafe`, `26cf3b2`, `d085964`) but still open on GitHub. In the roadmap they appear as
  "implemented, open until verified on the iPhone". That is an assumption: the commits have no `closes`.

## Gaps

- `overlays.options` is disabled, but `tour.include` lists `options`, so there is no options step. Open questions are a
  tour step with tiles, not an overlay (`overlays.custom` is empty).
- `derive_status_from` (k8s, Helm, docker-compose): no such files. "What runs" comes from `deploy.yml` (GitHub Pages).
- No sizes, user counts or performance measurements beyond what the docs state (router target < 300 ms is a target, not a measurement).
- The real performance of the router on the iPhone and the actual content of the people tab were not verified.

## Status evidence

`status.meaning: scope` → MVP / Next / Later:
- MVP = in the code on `main` and described in a spec (v0.6.0 per `Cargo.toml`).
- Next = open issues #1, #2 (shown in the roadmap; no separate box).
- Later = "Nápady na příště" and out-of-scope items in the specs: ride from a connection (box `ridefrom`), address
  destination, Golemio API via `DepartureSource`, permanent sign-in (roadmap only).

## Branding

- `branding.website: pid.cz`: the page was not fetched. The colours come from the 2026-11-15 look spec §4.3, which
  recorded pid.cz's styles: primary `#C40C00`, secondary (warning orange) `#F59D1A`. Theme `midnight`, shape `rounded`.
- **PID logo deliberately not used:** PID Tracker is a private app, not a PID product. Using PID's logo would suggest it is
  official. Logo = Lucide icon `train-front`. `meta.url` = https://pid.cz (information only, not shown as a link).

## Assumptions

- Personas use roles. The original spec calls the users father and son; the deck says Sheet owner and Co-rider.
- In the Process diagram, all steps are marked MVP (the process is in use); scope status adds little there.
- Tour step counts: 12 for diagrams 1–3 (including 2 roadmap phases), 15 for Process; all within the configured ranges.

## Checks

- `node tools/check.mjs diagrams/index.html`: 0 errors, 0 warnings in all four diagrams.
- Browser check (Chrome): the screen in this environment was only 1024×538, so the 1600×950 check wasn't possible.
  Text fit was checked; four labels the browser flagged were shortened. Tour card coverage and zoom levels could not be
  judged at that size: open the file on a normal screen and press **E → Check**.
