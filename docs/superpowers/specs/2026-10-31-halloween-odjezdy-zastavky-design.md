# PID Tracker – Halloween, odjezdy se zpožděním, jednotné hledání zastávek, SPZ

Datum: 2026-10-31
Stav: návrh ke schválení
Navazuje na: `2026-10-25-pidtracker-design.md` (verze 0.3.0)

## 1. Záměr a proč

Aplikace v praxi funguje, ale v metru zlobí tři věci, a k tomu přibývá pár přání:

1. **V metru jsou nabízeny jen spoje, které už odjely.** Uživatel se k aktuálním nedostane. Příčina je v kódu (viz 4.1), ne v chybějícím signálu.
2. **Pod zemí je nabídnuta špatná zastávka.** Na Můstku (A) aplikace nabídne Václavské náměstí na povrchu. GPS v metru nefunguje a iPhone hlásí odhad z Wi-Fi a mobilní sítě (±100–500 m). Přesnější polohu nezískáme, proto chceme stanice metra v nabídce dopředu.
3. **Hledání zastávek se v každém formuláři chová jinak.** V Nástupu a Výstupu funguje vlastní hledání bez diakritiky, v úpravě jízdy je HTML `<datalist>`, který si Safari filtruje po svém. „Mus“ tam Můstek nenajde.
4. **Přání:** zpoždění z živých dat PID u každého odjezdu, ruční SPZ vozu zobrazená v historii a statistikách, halloweenský vzhled po dobu svátku.

Úspěch: na Můstku A v metru otevřu Nástup, nahoře je Můstek, vidím nejbližší vlaky obou směrů (a případně jejich zpoždění), klepnu Start. V jakémkoli poli se zastávkou napíšu „mus“ a dostanu Můstek.

## 2. Cíle

- Odjezdy na Nástupu: nejbližší budoucí spoj pro každou linku a směr, průběžně aktualizovaný, se zpožděním z GTFS-RT, pokud existuje.
- Stanice metra dopředu, když je poloha nepřesná.
- Jeden společný našeptávač zastávek pro všechna pole se zastávkou v celé aplikaci.
- Ruční SPZ vozu u jízdy, zobrazená v historii a ve statistikách u vozu.
- Halloweenský vzhled („C – Hravá“) zapnutý automaticky jen kolem Halloweenu.
- Opravit `-dirty` v build ID na produkci.

## 3. Ne-cíle a anti-cíle

- **Ne-cíl:** automatické získání SPZ. V živých datech PID SPZ není (ověřeno 2026-10-31: `VehicleDescriptor` obsahuje jen `id` ve tvaru `service-0-8295`, `label` s číslem vozu a JSON s výbavou, pole `license_plate` je prázdné). SPZ je jen ruční.
- **Ne-cíl:** přepínač vzhledu v Nastavení. Uživatel ho výslovně nechce.
- **Ne-cíl:** lepší poloha v metru (Bluetooth beacony, ruční kalibrace a podobně).
- **Anti-cíl:** cokoli, co zablokuje nebo zpomalí záznam jízdy, když nejsou živá data nebo je uživatel offline. Zpoždění i realtime jsou jen bonus a nikdy nesmí bránit Startu.
- **Anti-cíl:** ztráta možnosti ruční opravy. Zastávka, linka, číslo vozu i SPZ musí jít vždy ručně zadat nebo přepsat.
- **Anti-cíl:** halloweenské dekorace, které zhorší čitelnost odjezdů nebo překryjí ovládací prvky.

## 4. Požadavky

### 4.1 Odjezdy na Nástupu

Dnešní stav (`crates/app/src/screens/board.rs`, `crates/pid-core/src/timetable/query.rs`): okno odjezdů je `at − 5 min … at + 30 min` a `next_per_direction` z něj ponechá **první** spoj pro každou dvojici linka + směr. U metra s intervalem 2–4 min to je skoro vždy spoj, který odjel před 3–5 minutami a ukazuje se jako „teď“. `at` se nastaví jen při otevření obrazovky nebo při výběru zastávky.

Požadované chování:

- Pro každou linku a směr se zobrazí **nejbližší spoj, který ještě neodjel**. Pokud zpoždění známe, rozhoduje očekávaný čas (jízdní řád + zpoždění). Spoj podle jízdního řádu odjel před minutou, ale má zpoždění +3 min → ještě neodjel.
- Nedávno odjeté spoje (posledních pár minut) zůstávají dostupné jako záloha pro případ, že uživatel naskočil na poslední chvíli, ale nesmí odsunout nejbližší budoucí spoj z prvního místa. Jak přesně je zpřístupnit (sbalená sekce „Právě odjelo“, řádek na konci a podobně), je na builderovi.
- Čas, ke kterému se odjezdy počítají, se posouvá s reálným časem, dokud je obrazovka otevřená (řádově jednou za 30 s). Zbývající minuty se přepočítávají.
- Předvybraný spoj zůstává první v seznamu. Pokud se seznam posune a uživatelem ručně vybraný spoj zmizí, výběr nesmí tiše přeskočit na jiný spoj bez ohledu na to, co uživatel vybral. Builder zvolí rozumné chování, například výběr podržet, dokud řádek existuje.

Každý řádek odjezdu zobrazí:

- linku, směr,
- čas odjezdu podle jízdního řádu,
- zbývající čas („za 3 min“, „teď“),
- **zpoždění**, pokud ho živá data pro tento spoj obsahují: „+2 min“ (zpoždění), „−1 min“ (náskok). Zpoždění 0 se zobrazí jako „včas“ nebo vůbec, rozhodne builder. Zbývající čas se při známém zpoždění počítá z očekávaného času.

Když živá data nejsou (offline, chyba, spoj ještě nevyjel), řádek ukáže jen jízdní řád, bez chybové hlášky.

### 4.2 Zdroj zpoždění

- Veřejné GTFS-Realtime feedy PID na `api.golemio.cz/v2/vehiclepositions/gtfsrt/` (bez klíče, s CORS, už používáme `vehicle_positions.pb` pro číslo vozu v `crates/pid-core/src/realtime.rs`).
- K dispozici je `trip_updates.pb` (2026-10-31: ~1,3 MB) a `vehicle_positions.pb` (~190 kB). Který feed nebo kombinaci použít a jak zpoždění k zastávce určit (zpoždění na konkrétní zastávce, nebo aktuální zpoždění spoje), je na builderovi. Hlavní kritérium je velikost stahování na mobilních datech a to, aby se seznam ukázal okamžitě z jízdního řádu a zpoždění se doplnila, až dorazí.
- Párování spojů probíhá přes GTFS `trip_id` (stejně jako dnes u čísla vozu).
- Obnovení zpoždění, dokud je obrazovka otevřená: řádově jednou za 30–60 s (moje volba). Při zavření obrazovky se obnovování zastaví.
- Dekódování a výpočet patří do `pid-core` s unit testy (vzor: existující testy v `realtime.rs`), stahování do `crates/app/src/realtime.rs`.

### 4.3 Stanice metra dopředu při nepřesné poloze

- Když je přesnost polohy horší než zhruba 100 m (moje volba prahu, builder ho může doladit), seřadí se kandidáti tak, že **zastávky se stanicí metra** v okolí jsou první (mezi sebou podle vzdálenosti), za nimi ostatní podle vzdálenosti.
- Zastávka „se stanicí metra“ je skupina, ze které odjíždí linka s `route_type` 1 (metro). Jak to předpočítat, rozhodne builder.
- Pro tuto obrazovku se použije poloměr hledání odpovídající přesnosti, aby se Můstek do kandidátů vůbec dostal (dnes `accuracy_m.clamp(MIN_RADIUS_M, MAX_RADIUS_M)`, ověřit, že horní mez stačí na ±500 m).
- Při dobré přesnosti (na povrchu) se řadí jako dnes, čistě podle vzdálenosti.
- Automaticky vybraná zastávka je první kandidát, takže v metru to bude stanice metra.

### 4.4 Jednotný našeptávač zastávek

Jedna komponenta pro **všechna** pole se zastávkou:

- Nástup: výběr zastávky a ruční zadání zastávky.
- Výstup: výběr zastávky a ruční zadání.
- Úprava jízdy: nástupní a výstupní zastávka (dnes `<datalist id="stop-suggestions">` v `crates/app/src/screens/edit.rs`, ten zmizí).
- Případná další pole se zastávkou, která builder najde.

Chování:

- Hledání nezávislé na diakritice a velikosti písmen v obou směrech: „mus“, „Mus“, „můs“ najdou Můstek a „namesti miru“ najde Náměstí Míru. Základ je `pid_core::text::fold` a `search_groups`.
- Shoda na začátku názvu i na začátku libovolného slova („miru“ najde Náměstí Míru), jako dnes `match_rank`.
- **Řazení podle blízkosti:** bez zadaného textu nabídne zastávky v okolí poslední známé polohy. Při zadaném textu se výsledky se stejnou kvalitou shody řadí podle vzdálenosti od poslední známé polohy. Poslední známá poloha je poslední úspěšný fix v aplikaci. Jestli se pamatuje jen v paměti, nebo i mezi spuštěními, rozhodne builder. V úpravě jízdy dává smysl brát polohu uloženou u jízdy (`board_lat/lon`, `alight_lat/lon`), pokud existuje. Bez jakékoli polohy se řadí jen podle kvality shody.
- Návrhy jsou vlastní seznam vykreslený aplikací (ne `<datalist>`), aby se v Safari choval stejně.
- **Ruční zadání zůstává:** když uživatel napíše název, který v seznamu není, a nic nevybere, uloží se jako ručně zadaná zastávka (bez `stop_id`), stejně jako dnes. V úpravě jízdy se nesmí při uložení tiše přepsat ručně zadaný název nejbližší shodou.
- Vizuálně může zůstat styl „chips“ z Nástupu, nebo builder zvolí rozbalovací seznam pod polem. Důležité je jednotné chování všude a ovladatelnost palcem na iPhonu.

Pole linky (`<datalist id="route-suggestions">` / `line-suggestions`) mají stejný problém se Safari. Sjednotit je stejným mechanismem je vítané, ale nepovinné. Rozhodne builder podle pracnosti.

### 4.5 SPZ vozu

- Nové volitelné pole jízdy `license_plate` (SPZ), zadávané ručně.
- Kde zadat: na úvodní obrazovce u probíhající jízdy vedle „Číslo vozu“ (`crates/app/src/screens/home.rs`) a v úpravě jízdy.
- **Předvyplnění (moje volba):** když má jízda číslo vozu a některá dřívější jízda se stejným číslem vozu má SPZ, nabídne se ta SPZ (vozidlo má stále stejnou SPZ). Hledá se ve všech jízdách v tabulce, i v jízdách druhého uživatele, protože jde o tentýž fyzický vůz. Předvyplnění proběhne, kdykoli jízda číslo vozu dostane, ať ručně, nebo později z realtime feedu po Startu, a jen když je SPZ prázdná. Uživatel ji může přepsat. Ručně zadaná hodnota má vždy přednost.
- Zobrazení:
  - **Historie:** v řádku jízdy za číslem vozu, např. „vůz 8758 · 1AB 2345“ (`RideRow` v `components.rs`).
  - **Statistiky:** v seznamu „Nejčastější vozy“ u čísla vozu (např. „8758 · 1AB 2345“). Agregace zůstává podle čísla vozu. Pokud má vůz v různých jízdách různé SPZ, ukáže se ta nejnovější.
- Google Sheet: nový sloupec `license_plate` **na konec** (`HEADERS` v `crates/pid-core/src/sheet.rs`, sloupec AA, `LAST_COL` se posune). Musí fungovat čtení starších tabulek bez tohoto sloupce (vzor: test `old_sheet_without_vehicle_column_parses`) a doplnění hlavičky do existující tabulky.
- Formát se nevaliduje, jen se ořízne o mezery. Normalizace velkých písmen je na builderovi.

### 4.6 Halloweenský vzhled „C – Hravá“

Schválený směr (mockup v `.superpowers/brainstorm/…/halloween-styl.html`, varianta C):

- Tmavě šedé pozadí (~`#2a2b30`), mírně světlejší plochy (~`#36373d`), světlý teplý text (~`#fff3e6`).
- **Oranžové** nadpisy, odkazy, akcenty, vybrané prvky a primární tlačítka (~`#ff8c1a` / `#ffa040`, text na oranžovém tmavý).
- Hravé písmo **jen v nadpisech** (např. „Chalkboard SE“ dostupné na iOS, se systémovým záložním písmem). Seznamy odjezdů a formuláře zůstávají v systémovém písmu kvůli čitelnosti.
- Siluety: **pavučina v rohu, pavouk na vlákně, kostlivci, čarodějnice na koštěti**, případně dýně. Inline SVG nebo CSS, žádné externí obrázky ani fonty (aplikace musí fungovat offline a SW precache se nesmí zbytečně nafouknout). Dekorace jsou `pointer-events: none`, nesmí překrýt obsah ani ovládání a nesmí zhoršit čitelnost (kontrast textu min. WCAG AA). Kde přesně na kterých obrazovkách, rozhodne builder. Mockup ukazuje náladu, není to pixelová předloha.
- Barvy linek (štítky linek) zůstávají podle PID.
- Halloweenský vzhled je vždy tmavý bez ohledu na systémové nastavení iPhonu.

**Kdy platí (rozhodnutí uživatele):** jen kolem Halloweenu, **5. 11. se aplikace sama přepne zpět na původní vzhled** (světlý/tmavý podle systému). Bez přepínače v Nastavení.

- Období: **od 24. 10. do 4. 11. včetně**, každý rok (začátek je moje volba, konec určil uživatel). Rozhoduje místní datum zařízení (Europe/Prague).
- Přepnutí probíhá v aplikaci podle data, nesmí záviset na novém nasazení. Pokud aplikace zůstane otevřená přes půlnoc na 5. 11., stačí přepnutí při příštím otevření nebo návratu do popředí.
- Původní vzhled se nesmí změnit. Halloweenský vzhled je vrstva navíc (např. třída na `<html>`, která přepíše CSS proměnné z `style.css` a zapne dekorace).
- Theme-color v manifestu a `<meta name="theme-color">` řeší builder, pokud je to rozumně možné.
- Pro test musí jít halloweenský vzhled vynutit i mimo období (např. parametrem v URL), aby šel ověřit i po 4. 11.

### 4.7 Opravit `-dirty` build ID

- Příčina: `Cargo.lock` zůstal po zvednutí verze na 0.3.0 se starými verzemi crate. CI ho při `cargo test` přepíše a `git status --porcelain` v `xtask` pak hlásí změnu.
- Commitnout aktualizovaný `Cargo.lock` (oprava je už v pracovním stromu) a do CI přidat `--locked` k `cargo test` i k `cargo xtask dist`, aby příště zastaralý lockfile shodil build.
- Zvednout verzi (např. 0.4.0) a nezapomenout aktualizovat `Cargo.lock`.

## 5. Omezení

- Jen Rust toolchain na Windows (Leptos + WASM, Trunk, `cargo xtask`). Žádný Node v buildu.
- Testování na iPhonu 15/16 v Safari PWA na ploše. Android neřešíme.
- Offline-first: všechno kromě zpoždění musí fungovat bez sítě.
- UI texty česky.
- Sloupce Google Sheetu se smí přidávat jen na konec.
- Logika (řazení odjezdů, metro, hledání, dekódování zpoždění, období Halloweenu, SPZ v řádcích a statistikách) patří do `pid-core` s unit testy, UI do `crates/app`.

## 6. Na builderovi

- Přesná podoba záložních „právě odjelých“ spojů.
- Volba feedu a algoritmu pro zpoždění a frekvence obnovy v daném rozmezí.
- Práh přesnosti pro „metro dopředu“ (výchozí 100 m).
- Vizuální podoba našeptávače a to, jestli sjednotit i pole linky.
- Rozmístění a kresba siluet a hravé písmo nadpisů v rámci 4.6.
- Pořadí implementace a rozdělení do commitů.

## 7. Moje volby (k odsouhlasení)

- Halloween od **24. 10.** (konec 4. 11. určil uživatel).
- Práh přesnosti 100 m pro řazení stanic metra dopředu.
- Obnova zpoždění jednou za 30–60 s.
- Předvyplnění SPZ z dřívější jízdy se stejným číslem vozu.
- Ve statistikách nejnovější SPZ vozu, pokud se liší.
