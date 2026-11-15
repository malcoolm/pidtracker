# PID Tracker – vyhledávání spojení

Datum: 2026-11-15
Stav: schváleno uživatelem 2026-11-15
Navazuje na: `2026-11-15-vzhled-pid-trasa-mimoradnosti-design.md` (verze 0.5.0)
Náhledy z brainstormingu: `.superpowers/brainstorm/1635-1794774817/content/` (`results-list.html`, `connection-detail.html`). Složka není v gitu.

## 1. Záměr a proč

Uživatel chce v aplikaci vyhledávač spojení inspirovaný spojeni.dpp.cz: zadá odkud a kam, čas odjezdu nebo příjezdu a dostane spojení, ze kterých vidí to nejrychlejší a to s nejmenším počtem přestupů. Některé druhy dopravy (metro, tramvaj, trolejbus …) jde vyloučit.

Aplikaci používá i starší člověk, proto se chůze (k zastávce, přestupy, do cíle) počítá pomalejším tempem než v běžných plánovačích. Výchozí bod je aktuální poloha.

Vyhledávání se stává úvodní obrazovkou aplikace. Zatím jen radí; zakládání jízd z nalezeného spojení přijde později (viz 3).

Úspěch: otevřu aplikaci, „Odkud“ je „📍 Moje poloha“, napíšu „Letňany“, klepnu Hledat a za okamžik vidím seznam spojení podle času odjezdu. U jednoho je štítek „nejrychlejší“, u jiného „nejméně přestupů“. Klepnu na spojení a vidím časovou osu: pěšky 6 min na Anděl, metro B v 7:44 (+1) z koleje 2, přestup na Florenci 4 min, metro C, pěšky 3 min. Nahoře „Vyrazte do 2 min“. Když si zaškrtnu „bez tramvají“, příště už je vyloučené.

## 2. Cíle

- Hledání spojení přímo v telefonu nad přibaleným jízdním řádem PID, offline.
- Odkud: aktuální poloha (všechny uzly do 1 500 m automaticky) nebo ručně zvolená zastávka.
- Kam: zastávka (uzel).
- Čas odjezdu nebo příjezdu, výchozí „teď“, volitelně datum a čas v rozsahu platnosti dat.
- Výsledky jako jeden seznam podle času odjezdu se štítky „nejrychlejší“ a „nejméně přestupů“.
- Vyloučení druhů dopravy, zapamatované mezi spuštěními.
- Detail spojení jako svislá časová osa se živým zpožděním u odjezdů.
- Chůze odhadnutá ze vzdálenosti, tempem staršího člověka.
- Úvodní obrazovka = vyhledávání; „＋ Nová jízda“ vlevo dole beze změny.
- Nastavitelná rezerva před prvním nástupem (0–5 min, výchozí 2).

## 3. Ne-cíle a anti-cíle

- **Ne-cíl (zatím):** založení jízdy z nalezeného spojení. Uživatel to chce později, návrh na to nemá klást překážky (spojení nese `trip` a stanoviště každého úseku).
- **Ne-cíl:** cíl jako adresa nebo bod na mapě. Kam = zastávka.
- **Ne-cíl:** mapa, pěší navigace po ulicích. Nemáme síť ulic; chůze je odhad ze vzdušné vzdálenosti.
- **Ne-cíl:** plánování podle živých zpoždění. Hledání počítá s jízdním řádem; zpoždění se jen zobrazí u nalezených spojení (uživatel souhlasil).
- **Ne-cíl:** server nebo cizí API pro hledání.
- **Anti-cíl:** spojení, která starší člověk nestihne (přestup počítaný běžným tempem). Raději o spoj později než nestihnutý přestup.
- **Anti-cíl:** během probíhající jízdy schovat obrazovku s „Výstup“ za vyhledávání. Zapomenutý výstup = špatná data v tabulce.

## 4. Návrh

### 4.1 Přístup a proč

Zvažované cesty:

1. **RAPTOR v `pid-core` (zvoleno).** Pracuje přímo nad patterny/profily/spoji, které `Timetable` už má. Počítá po kolech podle počtu přestupů, takže výsledkem je přirozeně sada spojení „k přestupů → nejdřívější příjezd“, ze které se odvodí oba štítky. Vyloučení druhů dopravy = přeskočení tras. Hledání podle příjezdu = zpětný běh. Běží v WASM offline.
2. **CSA (Connection Scan).** Jednodušší kód, ale potřebuje pole všech jednotlivých přejezdů (řádově stovky tisíc za den) navíc v paměti a nejméně přestupů nedává samo. Vyřazeno.
3. **Cizí server/API.** Rozbíjí offline a „bez vlastního serveru“, Golemio hledání spojení nemá. Vyřazeno.

Uživatel zvolil cestu 1.

### 4.2 Data (`gtfs-prep`, `Timetable`)

Jízdní řád už obsahuje vše potřebné kromě pěších přestupů mezi uzly.

- **Nové pole `Timetable::footpaths`**: pro každé stanoviště (stop) seznam `(cílový stop, sekundy chůze)` ke stanovištím **jiných uzlů** do 400 m vzdušnou čarou. Předpočítá `gtfs-prep` při sestavení dat (týdenní CI), aby to telefon nepočítal při každém hledání. Očekávaný nárůst souboru dat: stovky kB; ověřit při implementaci a uvést v PR.
- Přestupy **uvnitř uzlu** (`StopGroup`) se nepředpočítávají, platí pravidlo minimálního přestupního času (4.3).
- Formát `Timetable` se mění → nová verze kodeku; stará data v cache se při načtení odmítnou a stáhnou znovu (stávající mechanismus `manifest.json`).

### 4.3 Model chůze

Konstanty v jednom místě (`pid-core`), aby šly snadno doladit:

- Rychlost **1,0 m/s** (3,6 km/h).
- Vzdušná vzdálenost × **1,3** (objížďky po ulicích).
- Chůze od polohy k zastávce: uzly, jejichž některé stanoviště je do **1 500 m** vzdušnou čarou. Do hledání vstupují všechna jejich stanoviště, každé se svým časem chůze.
- Chůze ze zastávky do cíle: cíl je uzel, spojení končí na kterémkoli jeho stanovišti, bez chůze navíc. (Šipka „🚶 3 min“ do cíle v náhledu tedy odpadá, pokud cíl není poloha – a ta zatím není.)
- **Přestupní čas** mezi stanovišti A → B (stejný uzel i různé uzly) = **max(chůze, minimum)**, kde chůze = vzdálenost × 1,3 / 1,0 m/s a minimum = **2 min**, nebo **4 min**, pokud je A nebo B **stanoviště metra** (stop, na kterém staví některý pattern s `route_type = 1`). Rozhoduje se tedy po stanovištích, ne po uzlech: bus → tram v uzlu Anděl má 2 min, cokoli z/na nástupiště metra 4 min. *(Volba Clauda: příznak „stanoviště metra“ je předpočítatelný a nezávisí na lince, kterou člověk nastupuje.)*
- Přestup na stejném stanovišti (stejný stop): minimum 2 / 4 min, kromě setrvání ve stejném spoji, které přestupem není.
- `footpaths` ukládají **sekundy chůze** (bez minima); minimum se přičítá max-em za běhu z příznaku metra. Změna minim tak nevyžaduje nový formát dat. *(Volba Clauda.)*
- Chůze z polohy na **stanoviště metra**: + 2 min navíc (schody, eskalátory). *(Volba Clauda.)*
- **Rezerva před prvním nástupem:** nastavitelná v Nastavení, **0–5 min, výchozí 2 min** (rozhodnutí uživatele). Přičítá se k chůzi od polohy (resp. při startu ze zvolené zastávky k času odjezdu: spoj odjíždějící dřív než „čas + rezerva“ se nenabídne). Ukládá se lokálně jako ostatní nastavení. „Vyrazte do N min“ ji zahrnuje. Na přestupy se nevztahuje, ty kryje minimum výše.

### 4.4 Algoritmus

- RAPTOR nad stanovišti (stops); „trasa“ v RAPTOR smyslu = `Profile` (všechny spoje profilu mají stejnou posloupnost zastávek a stejné offsety, jsou seřazené podle `start`, takže se nepředjíždějí).
- Nejvýš **4 přestupy** (5 kol).
- Respektují se příznaky `NO_PICKUP` / `NO_DROP_OFF`.
- Kalendář: spoj jede v den D, pokud `service_runs(service, D)`; časy ≥ 24 h patří ke spojům předchozího dne. Hledání pracuje v absolutním čase od půlnoci dne dotazu a zahrnuje služby včerejška, dneška a zítřka (jako `departures`).
- Vyloučené druhy dopravy (podle `RouteKind::from_gtfs(route_type)`) se v kolech přeskakují.
- **Odjezd v T:** dopředný běh; výsledkem je pro každý počet přestupů k nejdřívější příjezd do cíle. Pareto množina (příjezd, přestupy) = kandidáti.
- **Příjezd do T:** zpětný běh (nejpozdější odjezd z výchozích bodů při příjezdu ≤ T).
- **Seznam 3–5 spojení:** opakované hledání s posunutým časem (odjezd = předchozí odjezd + 1 min, resp. příjezd − 1 min), dokud není 3–5 různých spojení. „Další spojení“ pokračuje stejně. Z každého běhu jdou do seznamu všechny Pareto kandidáty (různé počty přestupů). *(Volba Clauda.)*
- **Skrývání horších:** spojení se skryje, pokud jiné ve stejném seznamu odjíždí stejně nebo později, přijíždí stejně nebo dříve a má nejvýš stejně přestupů. Stejné spoje = duplicita, sloučí se. Typicky: autobus 7:40 i 7:50 navazují na tentýž vlak metra → zůstane jen 7:50. *(Volba Clauda.)*
- **Čas odchodu:** u každého nalezeného spojení se odjezd posune na nejpozdější, který ještě stihne stejné navazující spoje (zpětný průchod po nalezených úsecích, bez čekání na přestupech navíc). „Vyrazte do N min“ se počítá z tohoto času. *(Volba Clauda; bezpečnostní rezerva viz 4.3.)*
- **Štítky:** „nejrychlejší“ = nejkratší celková doba (od odchodu z polohy po příjezd) v aktuálně zobrazeném seznamu; „nejméně přestupů“ = nejmenší počet přestupů, při shodě dřívější příjezd. Pokud je to stejné spojení, má oba štítky.
- Rekonstrukce spojení z RAPTOR štítků (zpětné ukazatele) do struktury `Journey { legs: Vec<Leg> }`, kde `Leg` je `Walk { from, to, seconds }` nebo `Ride { trip, board_stop, alight_stop, dep, arr }`. Struktura nese vše potřebné pro budoucí zakládání jízd.
- **Výkon:** cíl < 300 ms na **jedno klepnutí na Hledat** (všechny běhy dohromady) na iPhonu. Od začátku proto: předfiltr profilů, které v dotčené dny nejedou, a hledání v časovém rozsahu (rRAPTOR) místo nezávislých běhů, pokud nezávislé běhy cíl nesplní. Měřit a uvést v PR. *(Volba Clauda.)*

### 4.5 Obrazovky

**Úvodní obrazovka (Spojení)** – náhled `results-list.html`, varianta A:

```
┌──────────────────────────────┐
│ ☰  PID Tracker          ● ok │
├──────────────────────────────┤
│ Odkud  📍 Moje poloha      ▾ │
│ Kam    [ zastávka…        ]  │
│ ◉ Odjezd ○ Příjezd  [ teď ▾] │
│ Vyloučit: □Metro □Tram □Bus… │
│ [        Hledat          ]   │
│ ┌──────────────────────────┐ │
│ │7:44 → 8:21  nejrychlejší │ │
│ │37 min · 1 přestup · 🚶9  │ │
│ │🚶6 [B] › [C] 🚶3          │ │
│ └──────────────────────────┘ │
│ … Další spojení ↓            │
│ [＋ Nová jízda]               │
└──────────────────────────────┘
```

- Odkud: výchozí „📍 Moje poloha“. Rozbalení ukáže blízké uzly s časem chůze; klepnutí omezí hledání na tento uzel. Lze přepsat na libovolnou zastávku (`StopInput`).
- Kam: `StopInput`.
- Odjezd/Příjezd + čas: výchozí „teď“; výběr data a času omezený na platnost dat (`feed_end`).
- Vyloučit: Metro, Tramvaj, Autobus, Trolejbus, Vlak, Přívoz, Lanovka. Ukládá se lokálně (stejně jako ostatní nastavení aplikace).
- Karta spojení: odjezd → příjezd (čas odchodu z polohy, pokud se startuje z polohy), doba, přestupy, chůze celkem, štítky z linek, odjezd první linky se živým zpožděním.
- Bez polohy (zamítnuté oprávnění, nepřesná/žádná fix): Odkud se zobrazí prázdné s výzvou zvolit zastávku.
- Během probíhající jízdy úvodní obrazovka ukazuje probíhající jízdu (beze změny); vyhledávání je dostupné z menu.
- Vyhledávání je vlastní obrazovka (`Screen::Search`). Úvodní obrazovka (`Screen::Home`) ji zobrazí, když žádná jízda neběží; jinak ukáže probíhající jízdu jako dnes.
- Menu: první položka „Spojení“ otevře vždy vyhledávání. Během probíhající jízdy je nad ní navíc zvýrazněná položka „Probíhající jízda“ (otevře Home). Seznam posledních jízd z dnešní úvodní obrazovky odpadá (je v Historii). *(Volba Clauda.)*

**Detail spojení** – náhled `connection-detail.html`, varianta A: svislá časová osa v barvách linek, chůze přerušovanou čarou, mezilehlé zastávky sbalené („▸ 3 zastávky“), nástupiště (`platform`) u nástupu, živé zpoždění u odjezdů, nahoře „Vyrazte do N min“ (čas odchodu z polohy − teď).

### 4.6 Živé zpoždění

Použije se stávající stahování `trip_updates.pb` (jako v Nástupu) pro spoje ve výsledcích; zobrazí se ve formátu ~~plán~~ skutečnost / „+N“. Plánování se zpožděním nepočítá. Bez signálu se zpoždění nezobrazí, hledání funguje dál.

## 5. Omezení

- Rust-only toolchain, WASM v PWA na iPhonu (Safari), Leptos UI jako dosud.
- Data platí ~14 dní; vybraný čas mimo `feed_start..feed_end` se nepovolí.
- Žádný vlastní server.

## 6. Nápady na příště (mimo tento návrh)

- **Cíl jako adresa, offline (preferovaná varianta uživatele).** Přibalit adresní místa z RÚIAN (ČÚZK, otevřená data) pro Prahu a okolí (~100–200 tis. adres, odhad 2–4 MB komprimovaně), sestavovat je podobně jako jízdní řád (nástroj ve stylu `gtfs-prep`), stahovat jednou a cachovat. Našeptávání adresy offline, nic se neodesílá ven. Router: chůze do cíle zrcadlově k chůzi z polohy (zastávky v okruhu cílového bodu s časem chůze, minimalizuje se příjezd + chůze). Zvažované alternativy: online geokodér (Mapy.cz API / Nominatim / Photon – vyžaduje signál, posílá cíl třetí straně) a uložená místa („Domů“, „Škola“ – levné, ale neumí neznámou adresu).
- Založení jízdy z nalezeného spojení (viz 3).

## 7. Co zůstává na implementaci

- Přesné rozmístění a styl prvků formuláře, sbalení formuláře po hledání.
- Struktura modulu (`pid-core::timetable::router` apod.) a testy (malý ručně sestavený `Timetable` + kontrola na reálných datech).
- Zda se blízké uzly pro polohu hledají přes existující `nearby` logiku nebo novou funkci.
