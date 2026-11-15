# PID Tracker – vzhled PID, horní proužek, trasa spoje, zpoždění, mimořádnosti, poznámka, sloučené statistiky

Datum: 2026-11-15
Stav: návrh ke schválení
Navazuje na: `2026-10-31-halloween-odjezdy-zastavky-design.md` (verze 0.4.2)
Náhledy z brainstormingu: `.superpowers/brainstorm/598-1794749330/content/` (`trasa.html`, `zpozdeni.html`, `rozlozeni-v2.html`). Složka není v gitu.

## 1. Záměr a proč

Aplikace funguje. Uživatel chce, aby vypadala jako „od PID“, byla přehlednější a dávala víc informací z živých dat:

1. **Ovládání:** spodní lišta a stavový řádek synchronizace zabírají místo. Navigace se má schovat do menu v horním proužku a stav přihlášení a synchronizace do malé ikonky v témže proužku. Tlačítko „Nová jízda“ má být vždy po ruce, i když je seznam posledních jízd odscrollovaný.
2. **Vzhled:** barvy a písmo převzaté z pid.cz. Světlý a tmavý režim se dál přepínají podle systému.
3. **Zpoždění je zmatené.** Dnes řádek ukazuje čas z jízdního řádu a vedle něj „za 5 min · +2 min“, kde „za 5 min“ už se zpožděním počítá. Z řádku není poznat původní čas, zpoždění a nový čas.
4. **Kde je můj spoj?** Při výběru nové jízdy chce uživatel u každého spoje vidět celou trasu a poslední známou polohu vozu ze živých dat.
5. **Mimořádnosti:** PID je publikuje ve stejném GTFS-RT streamu. Mají se zobrazit v aplikaci a na ty, které se nás týkají, má přijít upozornění (původní přání „notifikace“, upřesněno na hlášku v aplikaci, viz 4.6).
6. **Poznámka k jízdě**, jen při úpravě v Historii.
7. **Statistiky „Oba“** mají být součtem za oba uživatele. Dnes ukazují dva bloky pod sebou.

Úspěch: otevřu aplikaci, nahoře je červený proužek PID s ☰, dole vlevo plovoucí „＋ Nová jízda“. V Nástupu vidím u spoje „~~14:20~~ **14:22**, +2 min · za 5 min“, klepnu na „›“ a vidím zastávky trasy se štítkem vozu mezi Náměstím Míru a I. P. Pavlova. Když se na lince 22, kterou jsem včera jel, objeví mimořádnost, vyjede v aplikaci oranžový pruh a v menu „Mimořádnosti“ je zvýrazněná. Ve Statistikách „Oba“ je jedno číslo jízd za nás dva.

## 2. Cíle

- Horní proužek s menu (☰) a ikonkou stavu synchronizace a přihlášení. Spodní lišta zmizí.
- Plovoucí tlačítko „＋ Nová jízda“ vlevo dole na úvodní obrazovce.
- Barevné schéma a písmo podle pid.cz, světlé i tmavé podle systému.
- Zpoždění zobrazené jako přeškrtnutý původní čas + nový čas + „+N min · za M min“ všude, kde se ukazuje odjezd.
- Detail spoje: seznam zastávek trasy s polohou vozu z GTFS-RT.
- Mimořádnosti z GTFS-RT `alerts.pb`: seznam v menu, značka ⚠ u dotčených linek, hláška v aplikaci při nové mimořádnosti na lince, která nás zajímá.
- Pole „Poznámka“ v Úpravě jízdy, ukládané do tabulky.
- Statistiky „Oba“ jako součet.
- Prozatímní prodloužení přihlášení: tiché obnovení tokenu při klepnutí (4.9).

## 3. Ne-cíle a anti-cíle

- **Ne-cíl:** systémové notifikace (iOS bubliny, Oznamovací centrum, push při zavřené aplikaci). Uživatel zvolil jen hlášku uvnitř aplikace. Push by navíc na iOS vyžadoval vlastní server.
- **Ne-cíl:** mapa. Uživatel zvolil seznam zastávek. Tvar trasy (`shapes.txt`, 144 MB) se dál nepřibaluje.
- **Ne-cíl:** ruční výběr sledovaných linek v Nastavení. Uživatel souhlasil s automatickým pravidlem (4.6).
- **Ne-cíl:** přepínač světlého a tmavého režimu. Dál se řídí systémem.
- **Ne-cíl:** poznámka při zadávání nové jízdy nebo v obrazovce probíhající jízdy. Číslo vozu a SPZ tam zůstávají beze změny.
- **Anti-cíl (trvá):** nic z živých dat (poloha vozu, zpoždění, mimořádnosti) nesmí blokovat nebo zpomalit záznam jízdy, ani když je uživatel offline. Všechno je best effort a bez chybových hlášek.
- **Anti-cíl:** plovoucí tlačítko nesmí trvale zakrývat obsah, který nejde odscrollovat. Poslední položka seznamu musí jít vysunout nad tlačítko.
- **Anti-cíl:** aplikace nesmí být závislá na načtení písma z internetu. Offline musí vypadat stejně.
- **Anti-cíl:** záplava upozornění. Jedna mimořádnost = nejvýš jedna hláška (viz 4.6).

## 4. Požadavky

### 4.1 Horní proužek a menu

Dnešní stav: `crates/app/src/app.rs` vykresluje `SyncBadge` nad obrazovkou a `BottomNav` pod ní (`crates/app/src/components.rs`).

- Horní proužek je přilepený nahoře (sticky) a respektuje bezpečnou zónu iPhonu (`env(safe-area-inset-top)`). Je v něm:
  - vlevo **☰**, které otevře menu;
  - název aplikace „PID Tracker“;
  - vpravo **ikonka stavu**: krátká informace odvozená ze `SyncStatus` (např. „✓ 14:05“, „⟳“, „čeká 2“, „⚠“ při chybě nebo vypršeném přihlášení). Barva odpovídá stavu, jako dnes ok, warn nebo danger.
- Klepnutí na ikonku stavu otevře menu (nebo malé rozbalení pod proužkem, rozhodne builder). Tam je celé dnešní hlášení `SyncBadge`, včetně akčních tlačítek (opakovat synchronizaci, přihlásit znovu).
- Menu je vysouvací panel zleva přes ztmavený obsah (viz `rozlozeni-v2.html`). Obsahuje:
  - nahoře řádek „Přihlášen: <přezdívka> · <stav synchronizace>“;
  - položky **Jízda**, **Historie**, **Statistiky**, **Mimořádnosti** (nové, 4.6), **Nastavení**. Aktivní položka je zvýrazněná červeným pruhem. Pravidla zvýraznění jsou stejná jako dnes v `BottomNav` (Board/Alight → Jízda, Edit → Historie). U položky Mimořádnosti je počet těch, které se nás týkají, pokud je nenulový.
- Menu se zavře klepnutím na položku, na ztmavení nebo na ✕ (☰ se v otevřeném stavu změní na ✕).
- `BottomNav` se odstraní. Dočasná hlášení (`Toast`) a halloweenské dekorace zůstávají.
- Obrazovky s tlačítkem „Zpět“ (Nástup, Výstup, Úprava) si ho ponechají v obsahu, jako dnes.
- Patička s verzí zůstává na konci obsahu.

### 4.2 Plovoucí tlačítko „Nová jízda“

- Na úvodní obrazovce bez probíhající jízdy (`Idle` v `crates/app/src/screens/home.rs`) se dnešní velké tlačítko „Nová jízda“ nahradí plovoucím tlačítkem „＋ Nová jízda“. Je to kulatý obdélník (pill) v hlavní barvě, se stínem, `position: fixed`, **vlevo dole** (uživatel to výslovně chce vlevo), nad bezpečnou zónou (`env(safe-area-inset-bottom)`).
- Je nad veškerým obsahem stránky, ale pod otevřeným menu a pod `Toast`.
- Obsah úvodní obrazovky má dole odsazení, aby šla poslední jízda vysunout nad tlačítko.
- *Moje rozhodnutí:* při probíhající jízdě se tlačítko neukazuje, obrazovka probíhající jízdy zůstává s tlačítkem „Výstup“ jako dnes. Na ostatních obrazovkách (Historie, Statistiky…) tlačítko není.

### 4.3 Barvy a písmo podle pid.cz

Zjištěno ze stylů pid.cz 2026-11-15 (`wp-content/themes/pid/css/style.css`):

- hlavní barva (tlačítka, zvýraznění, horní proužek): **#C40C00**;
- tmavá barva textu a tmavých ploch: **#252525**, sekundární text #4B4B4B / #464646;
- pozadí stránky: **#F8F8F8**, plochy #FFFFFF, oddělovače a okraje #E8E8E8 / #D0D0D0;
- doplňkové barvy webu: oranžová #F59D1A (varování), zelená #00B274 (ok), tmavě červená #7A0200;
- písmo: **Fira Sans** (řezy 400, 500, 700; web používá i 600). Open Sans web používá jen okrajově, nepoužijeme ho.

Požadavky:

- Všechny barvy v `crates/app/style.css` jsou definované jako proměnné (`:root`) a dnešní barvy se nahradí paletou PID. Tmavý režim přes `@media (prefers-color-scheme: dark)` dostane vlastní hodnoty. Podle náhledu: pozadí kolem #141414, plochy #202020, horní proužek #252525 s červenou linkou dole, hlavní barva zesvětlená pro kontrast (např. #E0261A). Konkrétní odstíny ladí builder. Text na hlavní barvě musí mít kontrast aspoň WCAG AA.
- Barvy linek (odznaky `RouteBadge`) se nemění, berou se dál z GTFS `route_color`.
- Halloweenské téma (`halloween.rs`) přepisuje proměnné jako dnes a dál funguje.
- **Fira Sans se přibalí do aplikace** (`crates/app/public/fonts/…`, woff2, podmnožina latin + latin-ext kvůli češtině). Načte se přes `@font-face` s `font-display: swap` a service worker (`public/sw.js`) ho přidá do offline mezipaměti. Žádné načítání z Google Fonts za běhu. Licence SIL OFL: soubor licence se přibalí vedle písma. Jak se soubory získají (stažení z oficiálního zdroje a uložení do repa), je na builderovi. Do repa patří hotové woff2 soubory, ne krok sestavení se stahováním.

### 4.4 Zobrazení zpoždění

Dnešní stav: `DepRow` v `crates/app/src/screens/board.rs` zobrazuje `HH:MM` z jízdního řádu a za ním `<small>` s `when_label(expected)` a `delay_label`.

Nový formát (varianta A z `zpozdeni.html`):

- **Zpoždění ≥ 1 min:** ~~14:20~~ (přeškrtnuto, šedě) **14:22** (tučně, červeně). Pod tím drobně „+2 min · za 5 min“ červeně.
- **Náskok ≥ 1 min:** stejné, nový čas a „−1 min“ v barvě náskoku (dnešní třída `delay early`, *moje rozhodnutí:* zelená/modrá podle palety, ne červená).
- **Živá data pro spoj jsou a zpoždění je pod minutu:** jen **14:24**, pod tím zeleně „včas · za 7 min“.
- **Živá data pro spoj nejsou** (offline, spoj ještě nevyjel): jen **14:24** a pod tím šedě „za 7 min“, bez „včas“.
- Odpočet („za N min“, „teď“) se počítá z očekávaného času, jako dnes (`when_label`).
- Zaokrouhlení zpoždění na celé minuty zůstává (`delay_label`). Nový čas = jízdní řád + zpoždění, zaokrouhleno na minutu stejně, aby „14:20 +2 → 14:22“ vždy sedělo.
- Stejná komponenta času se použije v detailu spoje (4.5).

### 4.5 Detail spoje: trasa a poloha vozu

- Každý řádek odjezdu v Nástupu dostane vpravo samostatný ovládací prvek „›“ (dostatečně velký cíl pro prst). Klepnutí na zbytek řádku spoj dál **vybere** jako dnes. Klepnutí na „›“ otevře **detail spoje** a výběr nemění.
- Detail je nová obrazovka (nebo celoobrazovkový panel, rozhodne builder) s tlačítkem Zpět, které vrátí do Nástupu se zachovanou zastávkou i výběrem. Obsah (viz `trasa.html`):
  - hlavička: odznak linky, „→ cíl“, případně ⚠ s mimořádností linky (4.6);
  - svislá čára v barvě linky se **všemi zastávkami spoje** od první po konečnou (zastávky z `Pattern` daného `Trip`, časy z jízdního řádu daného dne). Builder doplní do `pid-core` funkci, která pro spoj vrátí seznam (zastávka, čas odjezdu).
  - **zastávka uživatele** (ze které se otevřelo) je zvýrazněná („tady stojíš“);
  - u každé zastávky čas ve formátu 4.4. Zpoždění pro jednotlivé zastávky z `trip_updates`, pokud tam jsou, jinak zpoždění spoje (logika jako `Delays::at`);
  - **štítek vozu**: „🚋 vůz 9315 · před 20 s“ umístěný mezi dvěma zastávkami, kde vůz je, nebo u zastávky, kde stojí. Text „před N s/min“ podle časové značky polohy. Emoji podle druhu dopravy, nebo neutrální, rozhodne builder;
  - projeté zastávky (před vozem) jsou šedé;
  - dlouhou trasu lze zkrátit („⋮ dalších 18 zastávek“, rozbalitelné). Při otevření je vidět poloha vozu a zastávka uživatele.
- **Zdroj polohy:** `vehicle_positions.pb` (Golemio GTFS-RT, dnes asi 230 KB, už se používá ve `fill_vehicle`). Entita vozu se najde podle `trip_id` spoje. Umístění na trase: přednostně z `current_stop_sequence` a `current_status` (IN_TRANSIT_TO / STOPPED_AT), pokud je feed vyplňuje. Jinak z GPS polohy vozu, a to promítnutím na nejbližší úsek mezi souřadnicemi sousedních zastávek trasy. Builder nejdřív ověří (např. přes `examples/rt_probe.rs`), co feed PID skutečně obsahuje, a podle toho zvolí metodu.
- Obnovování: jednou za 60 s a jen když je stránka vidět (stejný vzor jako zpoždění v Nástupu, `page_hidden` + `state.foreground`). Po opuštění detailu se stahování zastaví.
- **Bez živých dat** (offline, spoj ještě nevyjel, vůz ve feedu není): trasa podle jízdního řádu bez štítku vozu, s nenápadnou poznámkou („poloha vozu není k dispozici“). Žádná chyba.
- Detail se dá otevřít i u spojů v sekci „Právě odjelo“. U ruční linky („Jiná linka…“) detail není.

### 4.6 Mimořádnosti

**Zdroj:** `https://api.golemio.cz/v2/vehiclepositions/gtfsrt/alerts.pb` (ověřeno 2026-11-15: HTTP 200, asi 2,5 KB, GTFS-RT `Alert`). Obsahuje id (`alert-<uuid>`), `informed_entity` s `route_id` (např. `L135`, tedy stejné id jako `route_id` v našem jízdním řádu), `header_text` a `description_text` v češtině (`cs`), `url` na pid.cz. Pravděpodobně i `active_period`, `cause` a `effect`, které builder ověří. Dekódování do `pid-core/src/realtime.rs` vedle `parse_trip_delays`, s testy.

**Stahování:** při startu aplikace, při návratu do popředí a pak každých 5 minut, dokud je aplikace vidět. *Moje rozhodnutí:* interval 5 min, feed je malý a mimořádnosti se nemění po vteřinách. Offline nebo chyba → ponechá se poslední známý stav, bez hlášky. Poslední stav se uloží lokálně (IndexedDB nebo localStorage), aby seznam fungoval i chvíli offline.

**Plánované mimořádnosti** (`active_period` začíná v budoucnu), *moje rozhodnutí:*
- v seznamu jsou, s označením „plánováno od 18. 11. 10:00“, řazené za aktivní;
- značka ⚠, počet u menu a tečka u ☰ se počítají jen z právě aktivních;
- hláška vyjede hned při prvním výskytu (i u plánované, s datem začátku v textu), ne až při začátku. Jedno id = jedna hláška.
- Mimořádnost bez `active_period` se bere jako aktivní. Skončené (konec v minulosti) se nezobrazují.

**Linky, které nás zajímají** (pravidlo schválené uživatelem):

- linka probíhající jízdy, a
- linky (`route_id`) z jízd v historii za posledních 30 dní, *moje rozhodnutí:* obou uživatelů (sdílená tabulka, jezdí spolu). Smazané jízdy se nepočítají. Ručně zadané linky bez `route_id` se párují přes `route_short_name`, pokud jde jednoznačně dohledat v jízdním řádu.

**Zobrazení:**

- **Menu → Mimořádnosti:** seznam všech aktuálních mimořádností. Ty, které se nás týkají, jsou nahoře a zvýrazněné. Každá položka má odznaky dotčených linek, nadpis a rozbalitelný celý text s odkazem „Více na pid.cz“ (`url`, otevře se v prohlížeči). Prázdný stav: „Žádné mimořádnosti“. Bez dat: „Mimořádnosti nejsou k dispozici (offline)“.
- **Značka ⚠** u řádku odjezdu v Nástupu a v hlavičce detailu spoje, pokud má linka aktivní mimořádnost (jakoukoli, nejen „naši“). Klepnutí na ⚠ ukáže nadpis a text mimořádnosti (detail nebo přechod do seznamu, rozhodne builder).
- Počet „našich“ mimořádností u položky menu (4.1) a malá tečka u ☰ v horním proužku, pokud je nenulový.

**Upozornění (hláška v aplikaci):** uživatel zvolil jen hlášku v aplikaci. Systémové notifikace (Notification API, `showNotification`, povolení, přepínač v Nastavení) se **nedělají**.

- Když se po stažení objeví **nová** mimořádnost (id ještě neviděné), která se týká linky, jež nás zajímá, vyjede pod horním proužkem pruh: „⚠ Nová mimořádnost · <odznaky linek> · <nadpis>“. Klepnutí otevře Mimořádnosti s rozbalenou touto položkou, ✕ pruh zavře. Pruh zůstává, dokud ho uživatel nezavře nebo neotevře. Při víc nových najednou: „⚠ 3 nové mimořádnosti“.
- Pruh je v barvě varování (oranžová #F59D1A z palety) a nesmí překrýt ovládací prvky obrazovky (obsah se posune, nebo je pruh součástí sticky hlavičky). Konkrétní provedení je na builderovi.
- Ochrana proti opakování: ID už oznámených mimořádností se ukládají lokálně a každá se oznámí nejvýš jednou. Při prvním spuštění této verze se dosavadní mimořádnosti označí jako viděné bez hlášky, ať nepřijde dávka najednou.

### 4.7 Poznámka k jízdě

- `Ride` (`crates/pid-core/src/ride.rs`) dostane pole `note: String` (výchozí prázdné, staré záznamy bez pole se načtou s prázdnou poznámkou, `#[serde(default)]`).
- Tabulka: nový sloupec **`note`** jako 28. sloupec (**AB**) za `license_plate`. Upravit `HEADERS`, `LAST_COL` v `crates/pid-core/src/sheet.rs` a převod řádku. Rozšíření mřížky listu (`appendDimension` v `crates/app/src/google/sheets.rs`) už existuje a má pokrýt i AB, builder to ověří. Hlavička se doplní do existujících tabulek stejně jako u `license_plate` ve verzi 0.4.0.
- **Úprava jízdy** (`crates/app/src/screens/edit.rs`): víceřádkové pole „Poznámka“ (textarea) na konci formuláře, uloží se s ostatními změnami.
- **Historie:** *moje rozhodnutí:* u jízdy s poznámkou se v řádku (`RideRow`) zobrazí malá ikonka 📝. Celý text je v Úpravě.
- **Nezobrazuje se** v Nástupu ani u probíhající jízdy. Číslo vozu a SPZ zůstávají tam, kde jsou dnes, beze změny.

### 4.8 Statistiky „Oba“

Dnešní stav: `crates/app/src/screens/stats.rs` při `Who::Both` vykreslí výsledek `compute` pro každou osobu zvlášť (`For each=result`).

- Volba **Oba** zobrazí **jeden** blok se součtem za oba uživatele: počet jízd, minuty, grafy, nejčastější linky, vozy a ostatní metriky, které dnes statistiky mají, spočítané nad sjednocenými jízdami obou.
- Pokud jeli oba stejnou jízdu, každý ji má zapsanou zvlášť. V součtu se počítá dvakrát. *Moje rozhodnutí:* je to součet, ne „společné jízdy“, uživatel řekl „součtem nás obou“.
- Volby pro jednotlivce („Já“ a druhý uživatel) se nemění.
- Implementačně: buď `StatsQuery` s příznakem „sloučit“, nebo `compute` nad jízdami obou s jednou společnou osobou. Rozhodne builder, s testem v `pid-core`.

### 4.9 Tiché obnovení přihlášení

Dnešní stav: Google Identity Services (token client, `gisRequestToken` v `crates/app/src/interop/mod.rs`) vydá přístupový token na 1 hodinu, bez refresh tokenu. Po vypršení `current_token()` vrátí `None`, synchronizace přejde do `SyncStatus::NeedsSignIn` a uživatel musí ručně klepnout „Přihlásit pro synchronizaci“. Uživatel to označil za otravně krátké.

Rozhodnutí uživatele: **prozatímní řešení**, tiché obnovení při klepnutí. Trvalé řešení (Apps Script místo OAuth, nebo vlastní server s refresh tokenem) je mimo tuto verzi.

- Když token chybí, vypršel nebo vyprší do 5 minut a uživatel je dřív přihlášený (známe e-mail), aplikace při **nejbližším klepnutí uživatele** kamkoli do aplikace požádá Google o nový token. Popup se musí otevřít synchronně v rámci gesta, proto se to neděje z časovače.
- Žádost jde s `login_hint` (e-mail) a s `prompt: ''`, takže Google zobrazí výběr účtu nebo souhlas jen tehdy, když ho opravdu potřebuje. Jinak se okno samo zavře. `gisRequestToken` dostane parametr `prompt`.
- Klepnutí, které obnovu spustilo, musí normálně proběhnout (tlačítko Start, Výstup a podobně). Obnova se jen přidá vedle něj, např. posluchačem `click`/`pointerup` ve fázi capture na úrovni dokumentu. Neruší se ani neblokuje.
- Omezení opakování: nejvýš jeden pokus za 2 minuty (*moje rozhodnutí*). Žádný pokus offline (`navigator.onLine == false`) ani během probíhajícího pokusu. Když tichý pokus selže nebo ho uživatel zavře, zůstává dnešní tlačítko „Přihlásit pro synchronizaci“ (v nové podobě ikonky a menu, 4.1) a další tichý pokus přijde po uplynutí limitu.
- *Moje rozhodnutí:* tichá obnova se nespouští klepnutím do textového pole (input, textarea), aby popup neukradl fokus a klávesnici.
- Po úspěšné obnově se spustí synchronizace (`request_sync`), stejně jako po ručním přihlášení.
- **Ověření na iPhonu je součást úkolu:** builder ověří chování popupu v PWA na ploše (iOS otevírá popup jako list přes aplikaci). Pokud by na iOS tichá obnova trvale selhávala nebo nechávala otevřené prázdné okno, funkce se v PWA na iOS vypne a zůstane jen ruční tlačítko. Zjištění se zapíše do kontrolního bodu G.

## 5. Omezení

- Jen Rust (Leptos/WASM), jako dosud. Žádný JS framework ani mapová knihovna. Drobné JS interop přes existující `crates/app/src/interop` je v pořádku.
- Hosting zůstává GitHub Pages, žádný backend.
- Data: Golemio GTFS-RT bez klíče (`vehicle_positions.pb`, `trip_updates.pb`, `alerts.pb`). Trip updates (asi 1,3 MB) se stahují jen při otevřeném Nástupu nebo detailu, jako dnes.
- iPhone (Safari, PWA na ploše) je hlavní cílová platforma. Uživatel testuje podle `docs/testing-iphone.md`. Přibude kontrolní bod G pro tuto verzi.
- Build: `cargo xtask dist`, CI s `--locked`. Nové závislosti jen nutné. Zprávy GTFS-RT jsou v `pid-core/src/realtime.rs` definované ručně přes `prost`. Typ `Alert` (a `TranslatedString`, `TimeRange`, `EntitySelector`) je potřeba doplnit stejným způsobem, nová závislost to není.
- Verze: **0.5.0**.

## 6. Co nechávám builderovi

- Přesné odstíny tmavého režimu a drobné rozměry (velikost proužku, tlačítka, písma), v rámci palety 4.3 a náhledu `rozlozeni-v2.html`.
- Zda je detail spoje samostatná `Screen` varianta, nebo panel.
- Metodu umístění vozu na trase (4.5) po ověření obsahu feedu.
- Úložiště stavu mimořádností a oznámených ID.
- Zda se ikonka stavu rozbalí pod proužkem, nebo otevře menu.
- Chování ⚠ po klepnutí.
- Rozdělení do commitů a pořadí implementace.
- Text kontrolního bodu G v `docs/testing-iphone.md`.
