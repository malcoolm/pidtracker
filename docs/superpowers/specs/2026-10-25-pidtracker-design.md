# PID Tracker – návrh

Datum: 2026-10-25
Stav: návrh ke schválení

## 1. Záměr a proč

Otec a syn chtějí pro vlastní potřebu evidovat, jak jezdí pražskou integrovanou dopravou (PID), a vidět z toho statistiky: kolik a čím jezdí, rozlišeno podle osob a období. Evidence musí být co nejméně obtěžující. Stojím na zastávce, otevřu aplikaci, ta podle polohy a jízdního řádu předvyplní zastávku a linku, stisknu **Start**, při výstupu stisknu **Výstup** a potvrdím předvyplněnou cílovou zastávku.

Aplikace je PWA (instalovatelná webová aplikace), aby nebylo nutné publikovat ji v App Store nebo Google Play. Data se ukládají do Google Sheets, které vlastní otec a sdílí se synem.

## 2. Cíle

- Rychlý záznam jízdy: nástupní zastávka, linka, čas nástupu, výstupní zastávka, čas výstupu, osoba.
- Předvyplnění z polohy a z jízdního řádu, vždy s možností ruční opravy.
- Funkce **Přestoupit**: výstupní zastávka se stane nástupní zastávkou nové jízdy. Každý úsek je samostatná jízda.
- Dodatečná úprava a smazání jízdy (typicky když se zapomene stisknout Výstup).
- Plně offline provoz: záznam bez signálu, synchronizace s Google Sheets, jakmile to jde.
- Statistiky přímo v aplikaci: základní přehled s agregací podle osob a období.
- Celé PID: metro, tramvaje, městské i příměstské autobusy, vlaky, lanovka, přívozy.

## 3. Ne-cíle (pro první verzi)

- Žádné Golemio API ani jiná realtime data (zpoždění, polohy vozidel) pro návrhy zastávek a linek. Ty vycházejí jen ze statických PID open data (GTFS). **Výjimka (rozhodnuto uživatelem 2026-10-25):** evidenční číslo vozu se doplňuje z veřejného GTFS-Realtime feedu poloh vozidel (bez API klíče), viz 7.9.
- Žádný vlastní backend ani server. Jen statický hosting a Google API volaná přímo z prohlížeče.
- Žádné automatické zaznamenávání na pozadí. iOS PWA neumí sledovat polohu na pozadí, Start a Výstup jsou vždy ruční.
- Žádné nativní aplikace a žádné obchody s aplikacemi.
- Žádné multi-tenant řešení pro cizí uživatele. Aplikace je pro dvě konkrétní osoby (obecně pro pár lidí, kteří sdílí jednu tabulku).
- Žádné složité analýzy. Stačí základní přehled.

## 4. Anti-cíle (co by z toho udělalo neúspěch, i kdyby to „fungovalo“)

- Záznam jízdy trvá déle než pár klepnutí, nebo nutí psát.
- Záznam pořízený offline se ztratí nebo se v tabulce zdvojí.
- Úprava nebo smazání se nepropíše do tabulky, případně přepíše cizí jízdu.
- Aplikace nefunguje na iPhonu (Safari, PWA přidaná na plochu).
- Data jízdních řádů jsou tak velká, že první načtení na mobilních datech je nepoužitelné.

## 5. Omezení a rozhodnutí uživatele

- **Jazyk a ekosystém: Rust.** Na vývojovém stroji je jen Rust toolchain (Windows 11). Frontend v Rustu kompilovaném do WebAssembly, předzpracování GTFS jako Rust CLI nástroj.
- **Hosting: GitHub Pages** na účtu uživatele. Původní přání „jeden soubor bez hostingu“ bylo opuštěno, protože PWA potřebuje service worker jako samostatný soubor a Google OAuth potřebuje HTTPS origin. Distribuce je jedna statická složka.
- **Úložiště: Google Sheets.** Tabulka je na Drive otce a je sdílená synovi s právem úprav. Každý se přihlašuje vlastním Google účtem. Při prvním spuštění je vyžadováno přihlášení a zadání tabulky.
- **Data jízdních řádů: pouze PID open data** (https://pid.cz/o-systemu/opendata/, GTFS), předpočítaná a zkonvertovaná do kompaktního formátu, který je součástí distribuce.
- **Offline režim a synchronizace s Google Sheets na pozadí** jsou povinné.
- **Přestup = dvě samostatné jízdy** (přání syna).
- **Osoba = Google účet.** V aplikaci se k e-mailům přiřazují přezdívky.
- **Úpravy:** každý může upravovat a mazat jen svoje jízdy, jízdy druhého jen vidí (návrh přijat).
- **Výstupní zastávka:** přednostně se nabízejí zastávky na trase zvolené linky (návrh přijat).
- **Statistiky** v aplikaci: rozpad po osobách a obdobích. Rozpad podle typu dopravy a top linky jsou přijaté návrhy.

## 6. Uživatelské toky

### 6.1 První spuštění
1. Přihlášení Google účtem (Google Identity Services, OAuth v prohlížeči).
2. Vložení URL nebo ID Google tabulky. Aplikace ověří přístup. Pokud v tabulce chybí listy nebo hlavičky, založí je (to dělá jen ten, kdo má právo úprav, což mají oba).
3. Volitelně nastavení přezdívky pro vlastní e-mail. Přezdívky se ukládají do tabulky, aby je viděli oba.
4. Stažení a uložení dat jízdních řádů pro offline použití.

### 6.2 Záznam jízdy
1. Úvodní obrazovka s tlačítkem „Nová jízda“. Při otevření se rovnou zjistí poloha.
2. Předvyplní se nejbližší zastávka (název a případně označení nástupiště nebo stanoviště). K výběru se nabídnou další zastávky do 100 m, seřazené podle vzdálenosti. Je možné i ruční vyhledání podle názvu (pro případ špatného GPS).
3. Pro zvolenou zastávku, aktuální datum a čas se nabídnou linky, které odtud odjíždějí v časovém okně kolem „teď“ (např. −5 až +30 min), seřazené podle nejbližšího odjezdu. U každé je číslo linky, typ dopravy, směr (cílová zastávka spoje) a čas odjezdu. Předvybraná je ta nejbližší. Lze vybrat jinou nebo zadat linku ručně.
4. **Start** uloží jízdu se stavem „probíhající“ a s časem startu = teď.
5. Obrazovka probíhající jízdy: linka, odkud, uplynulý čas, tlačítko **Výstup**.
6. **Výstup**: zjistí se poloha. Nabídnou se zastávky na trase zvolené linky (spoje) seřazené podle vzdálenosti, ostatní blízké zastávky jsou jako alternativa. Čas výstupu = teď.
7. Potvrzení (**Hotovo**), nebo **Přestoupit**: jízda se uzavře a hned se otevře nová s výstupní zastávkou jako nástupní (krok 3).

Probíhající jízda musí přežít zavření aplikace i restart telefonu (uložena lokálně).

### 6.3 Historie, úprava, smazání
- Seznam jízd (nejnovější nahoře), filtr osoba (já / druhý / oba).
- Detail jízdy: úprava nástupní a výstupní zastávky, linky, času nástupu a výstupu. Uzavření neukončené jízdy.
- Smazání s potvrzením.
- Jízdy druhé osoby jsou jen pro čtení.

### 6.4 Statistiky
- Výběr období: týden / měsíc / rok / vše, s posunem na předchozí a další.
- Výběr osoby: já / druhý / oba (u „oba“ porovnání vedle sebe).
- Ukazatele: počet jízd, celkový čas v MHD, rozpad podle typu dopravy (metro, tramvaj, autobus, vlak, lanovka, přívoz, …), top linky.
- Časová řada počtu jízd v rámci zvoleného období (po dnech, týdnech nebo měsících).
- Počítá se z lokálních dat (kopie tabulky), funguje tedy offline.

## 7. Architektura

```
GitHub Pages (statická složka)
├── index.html          – jediná stránka
├── *.wasm + JS glue    – Rust/WASM aplikace
├── manifest.webmanifest
├── sw.js               – service worker (cache aplikace i dat, offline)
├── icons
└── data/               – předpočítané jízdní řády (verzované)

Telefon (PWA)
├── IndexedDB: jízdy, fronta změn k synchronizaci, nastavení, data jízdních řádů
└── Google Sheets API v4 (přímo z prohlížeče, OAuth token přihlášeného uživatele)

Build (lokálně, později GitHub Actions)
└── gtfs-prep (Rust CLI): stáhne PID GTFS → filtruje/komprimuje → data/
```

### 7.1 Rust workspace (návrh)
- `app` – frontend (WASM). Volba frameworku je na builderovi, doporučuji Leptos (CSR) nebo Yew. Build nástrojem `trunk` (instalovatelný přes cargo).
- `gtfs-prep` – CLI pro předzpracování GTFS.
- `core` – sdílené typy a formát dat jízdních řádů (serializace a deserializace), dotazy (nejbližší zastávky, odjezdy, zastávky na trase), výpočty statistik. Testovatelné běžnými `cargo test` bez prohlížeče.

### 7.2 Data jízdních řádů
Zdroj: PID GTFS z https://pid.cz/o-systemu/opendata/ (celé PID včetně vlaků a příměstských linek).

Co aplikace potřebuje:
- **Zastávky**: ID, název, souřadnice, označení stanoviště nebo nástupiště, příslušnost ke skupině stejného jména (pro hledání).
- **Odjezdy**: pro danou zastávku, datum a časové okno vrátit (linka, typ dopravy, směr/headsign, čas odjezdu, identifikace spoje nebo vzoru trasy).
- **Trasa spoje**: pro výstup potřebujeme zastávky, které následují po nástupní zastávce na daném spoji (stačí vzor trasy, časy nejsou nutné).
- **Kalendář**: které služby (service_id) jedou v daný den, včetně výjimek (calendar_dates).
- **Linky**: krátký název, typ dopravy (route_type), barva.

Požadavky na formát:
- Kompaktní binární formát (např. postcard nebo bincode, případně vlastní) a komprese. Rozumný cíl je **řádově jednotky MB přenosu (ideálně pod ~15 MB komprimovaně)** pro celé PID. Builder ověří reálnou velikost a případně zvolí redukce: deduplikace časů spojů do vzorů (pattern + offsety), vynechání nepotřebných sloupců.
- **Rozhodnuto uživatelem:** pokud se limit nevejde, zkrátí se horizont kalendáře (např. 30 dní, případně i méně) a data se aktualizují automaticky přes GitHub Actions (týdně). Odjezdy vlaků a příměstských linek se nevynechávají.
- Do budoucna se zvažuje přechod na Golemio API. Přístup k jízdnímu řádu v aplikaci proto bude za jasným rozhraním (trait „zdroj odjezdů“), aby šlo zdroj vyměnit. Pro v1 se Golemio neimplementuje.
- Data jsou verzovaná (verze = datum generování nebo platnosti feedu). Service worker je cachuje a aplikace při spuštění online zkontroluje novou verzi a stáhne ji na pozadí.
- Aplikace upozorní, pokud platnost dat brzy končí nebo už skončila (jízdní řády jsou neaktuální).
- Pokud data chybí nebo jsou zastaralá, ruční zadání zastávky a linky musí pořád fungovat.

Časové zóny: GTFS časy jsou v místním čase Europe/Prague a mohou přesahovat 24:00. Výpočty musí správně řešit přechod přes půlnoc a letní/zimní čas.

Aktualizace dat a nasazení: GitHub Actions workflow, který sestaví aplikaci, spustí `gtfs-prep` a nasadí na Pages. Spouští se při push do `main` a pravidelně (týdně) kvůli datům. Lokálně musí jít totéž ručně (`cargo run -p gtfs-prep` + build).

### 7.3 Google přihlášení a Sheets
- Google Identity Services (token client) v prohlížeči. Scope `https://www.googleapis.com/auth/spreadsheets` (plus `openid email` pro zjištění e-mailu).
- Uživatel si v Google Cloud Console vytvoří projekt, OAuth consent screen v režimu **Testing** a OAuth client typu „Web application“ s authorized JavaScript origin = adresa GitHub Pages (a `http://localhost:…` pro vývoj). Oba účty přidá jako test users. Client ID se zadá do konfigurace buildu (není tajné). Součástí dodávky je **návod krok za krokem**.
- Access token platí cca 1 hodinu a obnovuje se tiché přihlášení, je-li to možné. Pokud tiché obnovení nejde, aplikace požádá o přihlášení až ve chvíli, kdy chce synchronizovat. **Záznam jízdy nesmí nikdy čekat na přihlášení.**
- **Rozhodnuto uživatelem:** bez backendu. Na iPhonu je přijatelné, že synchronizace po delší pauze vyžaduje klepnutí na „Přihlásit“ (nenápadná výzva u stavu synchronizace, ne blokující dialog). Žádný token-exchange server ani Apps Script.
- Oba účty jsou plnohodnotné Google účty (žádný Family Link).
- Riziko: chování OAuth popupu v iOS standalone PWA. Builder ho ověří hned na začátku (spike na reálném iPhonu) a případně použije redirect flow.

### 7.4 Schéma tabulky
List `rides` (jeden řádek = jedna jízda):

| sloupec | význam |
|---|---|
| id | UUID jízdy (generuje klient) |
| person | e-mail autora |
| board_stop_id, board_stop_name | nástupní zastávka (GTFS stop_id + název v čase záznamu) |
| route_id, route_short_name, route_type | linka |
| headsign | směr |
| trip_id, pattern_id | konkrétní spoj / vzor trasy (pro nabídku výstupních zastávek i na jiném zařízení a při dodatečné úpravě) |
| board_time | čas nástupu (ISO 8601 s offsetem) |
| alight_stop_id, alight_stop_name | výstupní zastávka (prázdné u probíhající) |
| alight_time | čas výstupu |
| board_lat, board_lon, alight_lat, alight_lon | poloha telefonu při startu a výstupu (pro ladění, volitelné) |
| status | `in_progress` / `done` |
| deleted | `TRUE` u smazané jízdy (tombstone) |
| created_at, updated_at | časy vytvoření a poslední změny (ISO 8601) |
| app_version, data_version | verze aplikace a dat jízdních řádů |

List `people`: `email`, `nickname`.

Názvy zastávek a linek se ukládají denormalizovaně, aby tabulka byla čitelná i bez aplikace a přežila změny GTFS ID.

### 7.5 Offline a synchronizace
- Zdroj pravdy pro UI je lokální IndexedDB. Každá změna (vytvoření, úprava, smazání) se zapíše lokálně a přidá do fronty k odeslání.
- **Push:** fronta se odesílá, kdykoli je aplikace online a má token: při startu, při návratu do popředí (`visibilitychange`), při události `online` a periodicky, když je aplikace otevřená. Upsert podle `id`: aplikace drží mapování id → číslo řádku a před zápisem ho ověří, protože pořadí řádků se mohlo změnit. Nová jízda = append.
- **Pull:** při synchronizaci se načte celý list `rides` (objem je malý: tisíce řádků) a sloučí se lokálně podle `id`. Platí novější `updated_at`, smazání je trvalé (tombstone se nevrací).
- Konflikty jsou v praxi minimální, protože každý edituje jen svoje jízdy. Pro souběh vlastních zařízení platí „last write wins“ podle `updated_at`.
- Smazání se v tabulce provede nastavením `deleted = TRUE`, ne odstraněním řádku. Je to robustní proti posunu řádků a umožňuje synchronizaci smazání.
- „Na pozadí“ znamená: bez blokování UI, kdykoli je aplikace otevřená. Background Sync API se může použít tam, kde existuje (Android Chrome). Na iOS není a spoléhat se na něj nelze.
- UI ukazuje stav synchronizace (vše odesláno / N změn čeká / chyba + důvod).

### 7.6 PWA
- `manifest.webmanifest` (název, ikony, `display: standalone`, barvy), instalovatelné na iOS (Safari → Přidat na plochu) i Android.
- Service worker: precache aplikace (index, wasm, js, ikony) a dat jízdních řádů. Aktualizace aplikace s nabídkou „Je dostupná nová verze – načíst“.
- Responsivní UI, primárně mobil na výšku, velká tlačítka pro ovládání jednou rukou. Funguje i na desktopu (statistiky, úpravy).
- UI v češtině.
- Poloha: Geolocation API (vyžaduje HTTPS). Přesnost se zobrazuje, a pokud je nízká, rozšíří se nabídka zastávek nebo se nabídne vyhledávání.

### 7.7 Verze a automatická aktualizace
- Aplikace viditelně zobrazuje **verzi aplikace** (semver + krátký git hash + datum buildu) a **verzi dat jízdních řádů** (datum generování a platnost od–do). Patička nebo obrazovka Nastavení/O aplikaci, verze aplikace i drobně na hlavní obrazovce. Verze se zapisuje i do každé jízdy (`app_version`, `data_version`).
- **Autoupdate:** při spuštění, při návratu do popředí a periodicky, když je aplikace otevřená online, se kontroluje nová verze na GitHub Pages (malý `version.json` načítaný mimo cache a aktualizace service workeru). Nová verze se stáhne na pozadí a aktivuje se automaticky v bezpečném okamžiku: při příštím otevření nebo na úvodní či přehledové obrazovce, nikdy uprostřed rozpracovaného formuláře. Krátce se ukáže hláška „Aktualizováno na verzi X“. Probíhající jízda a nesynchronizované změny jsou v IndexedDB a aktualizaci přežijí.
- Ruční tlačítko „Zkontrolovat aktualizace“ v Nastavení (pro testování).

### 7.8 Cílová zařízení a testování
- Primárně **iPhone 15 a 16, Safari a PWA přidaná na plochu** (aktuální iOS). Android se zatím neplánuje, ale nic nemá být záměrně iOS-only.
- Desktopový prohlížeč slouží pro vývoj (localhost) a pro úpravy a statistiky.
- Testování na reálném iPhonu provádí uživatel. Plán musí obsahovat akceptační kontrolní body s jasným postupem „co vyzkoušet“: nejdřív spike přihlášení a zápisu do Sheets na iPhonu, pak záznam jízdy offline a online, pak synchronizace, statistiky a autoupdate.

## 8. Dodávky
1. Rust workspace v tomto repozitáři (`app`, `gtfs-prep`, `core`), README s postupem buildu, spuštění lokálně a nasazení.
2. Návod na nastavení Google Cloud (OAuth client, test users) a GitHub Pages.
3. Funkční PWA nasaditelná jako statická složka.
4. Testy pro `core` (dotazy do jízdního řádu, kalendář a čas přes půlnoc, slučování synchronizace, statistiky).

## 8a. Rozhodnutí autora návrhu (ke kontrole)
- **Počet osob:** datový model i UI podporují N osob (filtr osob = výběr ze seznamu z listu `people`). Výchozí podoba UI je optimalizovaná pro dvě osoby.
- **Tabulka začíná prázdná.** Aplikace si listy a hlavičky založí sama. Schéma je verzované a aplikace smí přidávat sloupce na konec, nikdy je nepřeskupuje. Uživatel nemá na listech `rides` a `people` stavět vlastní vzorce (vlastní analýzy patří na jiné listy).

## 9. Rozhodnutí ponechaná builderovi
- Konkrétní WASM framework a UI knihovny (doporučení: Leptos CSR + trunk).
- Konkrétní binární formát a kompresní strategie dat jízdních řádů v rámci limitu z 7.2, včetně případného dělení souborů.
- Prostorový index pro hledání nejbližších zastávek (např. mřížka nebo geohash).
- Přesné časové okno pro nabídku linek.
- Vizuální design (střídmý, čitelný, světlý i tmavý režim).
- Struktura IndexedDB a knihovna pro přístup k ní.
- Hostname a cesta na GitHub Pages (`<user>.github.io/pidtracker/` nebo vlastní doména). Aplikace musí fungovat pod podcestou.

## 10. Otevřená rizika
- Velikost dat celého PID (řešení: kratší horizont, viz 7.2).
- OAuth v iOS standalone PWA (popup vs. redirect). Ověřit spikem jako první krok.
- GitHub Pages na bezplatném účtu vyžaduje veřejný repozitář. Zdrojový kód je veřejný, osobní data jsou jen v Google Sheets.

## 7.9 Číslo vozu (doplněno 2026-10-25)

- Jízda má pole `vehicle` (evidenční číslo vozu), v tabulce jako poslední sloupec `vehicle`.
- Po Startu (a znovu při Výstupu, pokud chybí) aplikace stáhne veřejný feed
  `https://api.golemio.cz/v2/vehiclepositions/gtfsrt/vehicle_positions.pb` (~230 kB, bez klíče, CORS)
  a podle `trip_id` zvoleného spoje doplní `label` vozu. Offline nebo bez shody zůstane prázdné.
- Číslo jde vždy ručně přepsat (obrazovka jízdy, úprava jízdy); ruční hodnota má přednost.
- Tramvaje, autobusy a trolejbusy mají ve feedu evidenční čísla. Metro má jen pořadí soupravy
  a vlaky číslo vlaku – u nich se nic nedoplňuje, jen ruční zadání.

## 7.10 Pevná tabulka a dopravce (doplněno 2026-10-25)

- Tabulka s jízdami je pevně nastavená v buildu (`config::SHEET_ID`); onboarding je jen přihlášení Googlem
  s ověřením přístupu. Volba „Změnit tabulku“ byla odstraněna.
- Dopravce spoje se bere ze statického GTFS (`trips.sub_agency_id` + `route_sub_agencies.txt`; realtime feed
  dopravce neobsahuje). Ukládá se do jízdy při Startu (sloupec `operator` za `vehicle`), lze ho ručně upravit.
- Statistiky navíc ukazují počty jízd podle dopravce a 10 nejčastějších vozů.
- Formát jízdního řádu PIDT2 (spoj nese dopravce).
