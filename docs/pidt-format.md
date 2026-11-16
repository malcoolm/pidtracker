# Formát jízdního řádu `.pidt`

Aplikace pracuje offline nad kompaktním jízdním řádem, který `gtfs-prep` předpočítá z
[PID GTFS](https://data.pid.cz/PID_GTFS.zip). Soubor `timetable-<verze>.pidt` má
~1,3 MB (limit `gtfs-prep` je 15 MB); celé GTFS má desítky MB a `shapes.txt` sám 144 MB.

Zdrojový kód:

| Co | Kde |
|---|---|
| Datový model (`Timetable` a podřízené struktury) | `crates/pid-core/src/timetable/mod.rs` |
| Kódování a dekódování souboru | `crates/pid-core/src/timetable/codec.rs` |
| Konstanty pěších přestupů | `crates/pid-core/src/timetable/walk.rs` |
| Převod z GTFS, `manifest.json` | `crates/gtfs-prep/src/lib.rs` |
| Načtení v aplikaci | `crates/app/src/timetable_loader.rs` |

## Soubory v `dist/data/`

```
data/manifest.json
data/timetable-20261115-202611152317.pidt
```

`manifest.json` říká, který soubor načíst:

```json
{
  "bytes": 1330237,
  "feed_end": "2026-11-28",
  "feed_start": "2026-11-15",
  "file": "timetable-20261115-202611152317.pidt",
  "version": "20261115-202611152317"
}
```

Verze je `<feed_start YYYYMMDD>-<vygenerováno YYYYMMDDHHMM UTC>`, takže každé sestavení
dává nový název souboru. Service worker (`crates/app/public/sw.js`) bere `manifest.json`
ze sítě (při výpadku z cache) a `.pidt` z cache (cache-first, `pid-data`). Po úspěšném
načtení aplikace z cache smaže ostatní soubory `.pidt`.

PID GTFS platí jen 14 dní, proto CI sestavuje každé pondělí nová data (viz `README.md`).

## Obálka souboru

```
+--------+------------------------------------------+
| PIDT3  | deflate( postcard( Timetable ) )         |
| 5 B    | raw DEFLATE (RFC 1951), bez zlib hlavičky |
+--------+------------------------------------------+
```

- **Magic** `PIDT` + číslo verze formátu v ASCII. Dekodér přijme jen přesnou shodu.
- **Komprese**: raw DEFLATE (`flate2::DeflateEncoder`, `Compression::best()`).
- **Serializace**: [postcard](https://docs.rs/postcard) (serde), bez názvů polí;
  pořadí polí je dané pořadím v Rust strukturách.

Pravidla postcard, která v souboru platí:

| Typ | Kódování |
|---|---|
| `u8` | 1 bajt |
| `u16`, `u32`, délky | varint (LEB128) |
| `i32` | zigzag + varint |
| `String` | délka (varint) + UTF-8 |
| `Vec<T>` | počet (varint) + prvky |
| `Option<T>` | `0` = None, `1` + hodnota = Some |
| `NaiveDate` | řetězec `YYYY-MM-DD` (serde formát chrono) |

## Datový model

Všechny odkazy mezi tabulkami jsou indexy (`u32`) do příslušného `Vec`. Texty, které se
opakují (názvy zastávek, headsigny, označení nástupišť, dopravci), jsou uložené jednou
v `strings` a odkazované indexem.

### `Timetable`

| Pole | Typ | Význam |
|---|---|---|
| `version` | `String` | stejné jako v `manifest.json` |
| `feed_start` | `NaiveDate` | první den platnosti feedu (`feed_info.txt`) |
| `feed_end` | `NaiveDate` | poslední den platnosti |
| `strings` | `Vec<String>` | internované řetězce |
| `stops` | `Vec<Stop>` | stanoviště (GTFS `location_type` 0) |
| `groups` | `Vec<StopGroup>` | zastávky = skupiny stanovišť |
| `routes` | `Vec<Route>` | linky |
| `patterns` | `Vec<Pattern>` | unikátní sekvence stanovišť linky |
| `profiles` | `Vec<Profile>` | pattern + jízdní doby |
| `trips` | `Vec<Trip>` | jednotlivé spoje |
| `services` | `Vec<Service>` | kalendáře provozu |
| `footpaths` | `Vec<Vec<Footpath>>` | pěší přestupy; index = stanoviště |

### `Stop` – stanoviště

| Pole | Typ | Význam |
|---|---|---|
| `gtfs_id` | `String` | GTFS `stop_id` (např. `U1072Z101P`) |
| `name` | `u32` → `strings` | název zastávky |
| `platform` | `Option<u32>` → `strings` | `platform_code` (A, B, 1, …) |
| `lat_e6`, `lon_e6` | `i32` | souřadnice WGS84 × 10⁶ |
| `group` | `u32` → `groups` | zastávka, do které stanoviště patří |

### `StopGroup` – zastávka

| Pole | Typ | Význam |
|---|---|---|
| `name` | `u32` → `strings` | název |
| `stops` | `Vec<u32>` → `stops` | stanoviště skupiny |

Skupina = stejné `asw_node_id` **a** stejný název; bez uzlu jen stejný název. Uzel PID
totiž může mít víc jmen (v uzlu 1072 jsou Můstek i Václavské náměstí).

### `Route` – linka

| Pole | Typ | Význam |
|---|---|---|
| `gtfs_id` | `String` | GTFS `route_id` |
| `short_name` | `String` | číslo linky (`22`, `C`, `S7`) |
| `route_type` | `u16` | GTFS `route_type` (0 tram, 1 metro, 2 vlak, 3 bus, …; chybějící = 3) |
| `color` | `u32` | barva `0xRRGGBB` (chybějící = `0x00559E`) |

### `Pattern` – trasa

| Pole | Typ | Význam |
|---|---|---|
| `route` | `u32` → `routes` | linka |
| `stops` | `Vec<u32>` → `stops` | stanoviště v pořadí `stop_sequence` |
| `flags` | `Vec<u8>` | příznaky ke každému stanovišti |

Příznaky: `1` = `NO_PICKUP` (nelze nastoupit, GTFS `pickup_type` 1),
`2` = `NO_DROP_OFF` (nelze vystoupit, `drop_off_type` 1).

### `Profile` – jízdní doby

| Pole | Typ | Význam |
|---|---|---|
| `pattern` | `u32` → `patterns` | trasa |
| `offsets` | `Vec<u32>` | odjezd z každého stanoviště v sekundách od odjezdu z prvního |

Spoje se stejnou trasou i jízdními dobami sdílejí jeden profil; liší se jen časem odjezdu.
Používá se odjezdový čas (`departure_time`, jinak `arrival_time`), příjezd zvlášť uložený
není.

### `Trip` – spoj

| Pole | Typ | Význam |
|---|---|---|
| `gtfs_id` | `String` | GTFS `trip_id` (páruje se s GTFS-Realtime) |
| `profile` | `u32` → `profiles` | trasa a jízdní doby |
| `service` | `u32` → `services` | kdy jede |
| `start` | `u32` | odjezd z první zastávky v sekundách od „poledne − 12 h“ provozního dne; může být ≥ 24 h (noční spoje) |
| `headsign` | `u32` → `strings` | cílová zastávka |
| `operator` | `Option<u32>` → `strings` | dopravce z `route_sub_agencies.txt` |

Spoje jsou seřazené podle `(profile, start)`. Spoje s méně než dvěma zastávkami se vynechávají.
Čas odjezdu ze stanoviště *i* = `start + profiles[profile].offsets[i]`.

### `Service` – kalendář

| Pole | Typ | Význam |
|---|---|---|
| `days` | `Vec<u8>` | bitmapa dnů: bit *i* (bajt `i / 8`, bit `i % 8`) = den `feed_start + i` |

Vzniká z `calendar.txt` (dny v týdnu v rozsahu platnosti) a pak se upraví podle
`calendar_dates.txt` (1 = přidat, 2 = odebrat). Dny mimo feed jsou vždy „nejede“.

### `Footpath` – pěší přestup

| Pole | Typ | Význam |
|---|---|---|
| `to` | `u32` → `stops` | cílové stanoviště |
| `walk_s` | `u16` | doba chůze v sekundách |

Vede jen mezi stanovišti **různých** skupin vzdálenými vzdušnou čarou nejvýš 400 m.
Doba chůze = vzdálenost × 1,3 (oklika) / 1 m/s, zaokrouhleno nahoru. Seznam je seřazený
podle `to`. Přestupy uvnitř jedné skupiny se nepředpočítávají; minima přestupu
(2 min, s metrem 4 min) přidává až vyhledávání spojení (`walk.rs`).

## Verze formátu

| Magic | Od verze aplikace | Změna |
|---|---|---|
| `PIDT1` | 0.1.0 | první verze |
| `PIDT2` | 0.3.0 | `Trip.operator` (dopravce) |
| `PIDT3` | 0.6.0 | `Timetable.footpaths` (pěší přestupy pro vyhledávání spojení) |

Postcard nemá názvy polí ani volitelná pole, takže **každá změna struktur vyžaduje nové
číslo v `MAGIC`** (`codec.rs`). Data a aplikace se nasazují spolu, starý soubor ale může
zůstat v cache. Pokud soubor začíná na `PIDT`, ale číslo nesedí, aplikace hlásí „Jízdní
řád je ve starém formátu, stáhne se po připojení.“ a načtení zopakuje.

## Nástroje

```powershell
# sestavení dat z čerstvého GTFS (součást cargo xtask dist --fetch-gtfs)
cargo run -p gtfs-prep -- --help
```
