# Testování na iPhonu

Aplikace se testuje jako PWA přidaná na plochu (Safari).

## Instalace

1. V Safari otevřete `https://<uživatel>.github.io/pidtracker/`.
2. Sdílet (čtvereček se šipkou) → **Přidat na plochu** → Přidat.
3. Spouštějte aplikaci vždy z ikony na ploše (běží pak bez lišty Safari).

Verze aplikace je vidět v patičce a v **Nastavení → O aplikaci**.

## Kontrolní bod A – instalace a offline

- [ ] Aplikace se otevře z plochy a ukazuje verzi.
- [ ] Zapněte režim Letadlo, zavřete aplikaci (přetažením nahoru z přepínače aplikací)
      a znovu ji otevřete z plochy – musí se načíst.

## Kontrolní bod B – přihlášení Googlem a tabulka (nejdůležitější)

Předpoklad: hotové kroky v `setup-google.md`.

- [ ] V aplikaci spuštěné **z plochy** klepněte na **Přihlásit se Googlem**.
      Otevře se okno Googlu → vyberte účet → potvrďte varování o neověřené aplikaci → povolte.
- [ ] Aplikace se vrátí a ukáže „Přihlášen(a): váš e-mail“.
- [ ] Vložte odkaz na tabulku → **Ověřit a pokračovat**. V tabulce vzniknou listy
      `rides` a `people` s hlavičkami a v `people` se objeví váš e-mail.
- [ ] Totéž na synově iPhonu s jeho účtem.
- [ ] Zavřete a znovu otevřete aplikaci do hodiny – zůstává přihlášená.

**Pokud se po přihlášení okno nezavře nebo aplikace nic neukáže**, napište, co přesně
se stalo (případně screenshot). Je připravená náhradní varianta přihlášení
přesměrováním.

## Kontrolní bod C – záznam jízdy

- [ ] Na zastávce: **Nová jízda** → předvyplní se nejbližší zastávka a seznam odjezdů.
      Nejbližší odjezd je předvybraný.
- [ ] **Start** → běží čas jízdy. Zavřete aplikaci a otevřete znovu – jízda pořád běží.
- [ ] Při výstupu **Výstup** → nabídnou se zastávky na trase, nejbližší je předvybraná.
- [ ] **Přestoupit** → nová jízda s předvybranou zastávkou výstupu → zvolte linku → **Start**.
- [ ] Jednu jízdu zaznamenejte v režimu Letadlo – musí jít normálně.

## Kontrolní bod D – synchronizace

- [ ] Po připojení k síti se jízdy objeví v tabulce (list `rides`), stav ukazuje
      „✓ Synchronizováno HH:MM“.
- [ ] Po více než hodině se ukáže **Přihlásit pro synchronizaci** – po klepnutí se
      jízdy odešlou.
- [ ] Synovy jízdy jsou vidět v Historii (filtr podle osoby) a nejdou upravit.
- [ ] Úprava a smazání jízdy se propíše do stejného řádku tabulky
      (smazaná jízda má ve sloupci `deleted` hodnotu `TRUE`).

## Kontrolní bod E – statistiky a aktualizace

- [ ] Statistiky ukazují počty a čas po osobách za týden / měsíc / rok.
- [ ] V **Nastavení** nastavte přezdívku – objeví se v historii a statistikách u druhého.
- [ ] Po novém nasazení (push do `main`) se aplikace sama aktualizuje při návratu
      na úvodní obrazovku a krátce ukáže „Aktualizováno na verzi …“.
      Rozpracovaná jízda aktualizaci přežije.

## Kontrolní bod F – verze 0.4.0

- [ ] Do 4. 11. je aplikace v halloweenském vzhledu (tmavě šedá, oranžová, pavouk, kostlivci,
      čarodějnice), od 5. 11. se sama vrátí k původnímu. Vynucení pro test: `?halloween=1`.
- [ ] V metru na Můstku: **Nová jízda** → první nabídnutá zastávka je **Můstek**
      (stanice metra jdou při nepřesné poloze dopředu), ne Václavské náměstí.
- [ ] V seznamu odjezdů je nahoře nejbližší vlak, který ještě neodjel. Starší spoje jsou
      pod **Právě odjelo**. Seznam se sám posouvá.
- [ ] U tramvají a autobusů na trase je vidět zpoždění („+3 min“). Offline seznam funguje bez něj.
- [ ] V **Úpravě jízdy** napište „mus“ → nabídne se Můstek (i bez diakritiky).
- [ ] U probíhající jízdy vyplňte **SPZ** → v Historii je „vůz 8758 · 1AB 2345“, ve Statistikách
      u vozu také. Další jízda stejným vozem má SPZ předvyplněnou.
- [ ] V tabulce přibyl sloupec `license_plate` (AA).

## Kontrolní bod G – verze 0.5.0

**Vzhled a horní lišta**

- [ ] Nahoře je pevný červený proužek s nápisem „PID Tracker“, nezajíždí pod výřez (notch)
      a při posouvání obsahu zůstává na místě. Písmo je Fira Sans.
- [ ] Zapněte režim Letadlo a aplikaci zavřete a znovu otevřete: písmo vypadá stejně.
- [ ] Tmavý režim se řídí systémem (Nastavení iOS → Zobrazení a jas): tmavé pozadí,
      proužek zůstává s červenou linkou dole.
- [ ] Ikonka stavu je **v proužku** (✓ s časem / ⟳ / „čeká N“ / ⚠). Klepnutí na ni otevře menu ☰.
- [ ] Menu obsahuje řádek „Přihlášen: …“, zavře se přes ✕, klepnutím na ztmavenou plochu
      i výběrem položky. Aktivní položka má červený proužek.
- [ ] Tlačítko **Nová jízda** (fab) je na úvodní obrazovce vlevo dole. Během probíhající jízdy
      a na ostatních obrazovkách není vidět. Poslední jízdu lze vysunout nad něj.

**Zpoždění a detail spoje**

Zpoždění se ukazuje jen u vytíženého spoje v provozu: vyberte tramvaj nebo autobus ve špičce.
Když žádný zpožděný spoj není, zapište „neověřeno“ a zkuste později.

- [ ] Zpožděný spoj: původní čas přeškrtnutý, vedle červený nový čas a „+N min · za M min“.
      Dřívější odjezd má „−N min“. Spoj s živými daty bez zpoždění: „včas · za N min“.
      V Letadlo režimu jen šedé „za N min“.
- [ ] „›“ na konci řádku odjezdu otevře trasu spoje: řádek „tady stojíš“ je zvýrazněný,
      projeté zastávky jsou šedé, dlouhé úseky jsou sbalené jako „⋮ dalších N zastávek“.
- [ ] V detailu je štítek vozu „🚋 vůz … · před …“. Offline místo něj
      „Poloha vozu není k dispozici“.
- [ ] Klepnutí na řádek odjezdu spoj stále vybere, klepnutí na „›“ ho jen otevře.
      **Zpět** vrátí na stejnou zastávku i s výběrem.
- [ ] „›“ funguje i u spojů pod **Právě odjelo**. U **Jiná linka…** žádné „›“ není.

**Mimořádnosti**

- [ ] V menu je **Mimořádnosti** s počtem, na ikoně ☰ je tečka, když něco platí.
      U odjezdů na dotčené lince je ⚠.
- [ ] Položky seznamu se po klepnutí rozbalí a mají odkaz „Více na pid.cz“. Plánované
      mají „plánováno od …“.
- [ ] Při nové mimořádnosti na lince z historie se nahoře objeví oranžový pruh
      „⚠ Nová mimořádnost“ (nebo „⚠ N nové mimořádnosti“). Klepnutí otevře seznam rozbalený,
      ✕ pruh zavře. Nikdy nepřijde systémová notifikace. Závisí na skutečné nové
      mimořádnosti; když se při testu žádná neobjeví, zapište „neověřeno, čeká na skutečnou
      mimořádnost“.

**Poznámka a statistiky**

- [ ] V **Úpravě jízdy** vyplňte poznámku → v Historii je u jízdy 📝. V Nástupu
      a u probíhající jízdy se poznámka nezobrazuje. V tabulce je ve sloupci `note` (AB).
- [ ] Ve **Statistikách** ukazuje volba **Oba** jeden společný součet (jeden blok).

**Tiché obnovení přihlášení**

- [ ] Zůstaňte přihlášeni, aplikaci nechte nečinnou nebo zavřenou aspoň 55 minut. Pak online
      klepněte na tlačítko (ne do textového pole): okno Googlu krátce probliklo a samo se zavře,
      proběhne synchronizace. Klepnutí do textového pole nic nespustí. Ruční
      **Přihlásit pro synchronizaci** funguje dál.
- [ ] Pokud okno zůstane prázdné nebo nejde zavřít (aplikace na ploše), zapište sem, co se
      stalo, a verzi iOS. Obnova se pak pro iOS standalone vypne (viz spec 4.9).

## Kontrolní bod H – verze 0.6.0

Vyhledávání spojení běží celé v telefonu z jízdních řádů, proto vše zkoušejte i bez sítě.
Co nejde ověřit (např. nevyšlo se ven), zapište „neověřeno“.

**Hledání z polohy (venku)**

- [ ] Menu ☰ → **Spojení** otevře vyhledávání. Pole „Odkud“ je předvyplněné **📍 Moje poloha**
      (během zjišťování vedle něj „zjišťuji…“). Do pole „Kam“ napište cíl (např. „Letňany“)
      a vyberte ho z nabídky.
- [ ] Klepněte na **Hledat** a změřte čas do zobrazení výsledků: _____ ms (cíl pod 300 ms,
      přijatelné do ~1 s). Každá karta ukazuje čas odchodu → příjezd, dobu, přestupy,
      chůzi („🚶N“), linky a řádek „<zastávka> odj. <čas>“.
- [ ] V detailu spojení je pod časy řádek „Vyrazte do N min“ (nebo „Vyrazte hned“, u spojení
      za víc než 90 min „Odchod v HH:MM“), první řádek osy „📍 Moje poloha“ a pod ním
      „🚶 N min na zastávku <název>“.
- [ ] Klepněte na **📍 Moje poloha**: rozbalí se blízké zastávky („<název> · 🚶N min“).
      Vyberte jednu (ne nejbližší): pole ukáže „📍 Moje poloha → <název>“. Hledejte: karta
      i detail začínají chůzí z polohy (detail: „📍 Moje poloha“, „🚶 N min na zastávku …“)
      a všechna spojení odjíždějí z vybrané zastávky. „Všechny zastávky v okolí“ výběr zruší.
- [ ] Nechte aplikaci na obrazovce hledání v pozadí, přejděte jinam (aspoň 2 min, ideálně
      pár set metrů) a vraťte se: po chvíli „zjišťuji…“ se poloha obnoví. Staré výsledky
      zůstanou, dokud znovu neklepnete na **Hledat**; pak souhrn nad výsledky i chůze
      odpovídá novému místu.
- [ ] Zakažte aplikaci polohu (Nastavení → Soukromí → Polohové služby) a otevřete hledání:
      pole „Odkud“ je výběr zastávky s hláškou a odkazem „📍 Zkusit znovu zjistit polohu“.
      Polohu znovu povolte a klepněte na odkaz: pole se vrátí na „📍 Moje poloha“.

**Hledání ze zvolené zastávky a příjezd**

- [ ] Klepněte na **📍 Moje poloha** → „Jiná zastávka…“, do „Odkud“ napište zastávku (např.
      „Anděl“) a vyberte ji. Najde se spojení do cíle. Karta začíná odjezdem prvního spoje:
      „08:00 → 08:21“ a pod tím „Anděl odj. 08:00“ (čas vlevo = čas odjezdu, ne o rezervu dřív).
- [ ] Přepněte na **Příjezd** (vedle „Odjezd“), v poli data a času zvolte čas (tlačítko „teď“ ho smaže) a hledejte: všechna spojení přijedou do zvoleného času
      nebo dřív.
- [ ] Tlačítka „Dřívější spojení ↑“ (nad seznamem) a „Další spojení ↓“ (pod ním) doplní seznam a nic se neduplikuje.

**Nastavení hledání**

- [ ] Na obrazovce hledání zaškrtněte u „Vyloučit:“ druh dopravy (např. autobus), hledejte: výsledky ho neobsahují.
      Zavřete a znovu otevřete aplikaci: vyloučení se pamatuje.
- [ ] V Nastavení → „Vyhledávání spojení“ změňte „Rezerva před prvním nástupem“ (např. z 2 na 5 min): „Vyrazte do N min“ v detailu
      spojení se zkrátí o rozdíl.

**Detail spojení**

- [ ] Klepnutí na spojení otevře detail: časová osa s jízdami, zastávkami a nástupišti.
      Přestupní chůze je „přestup 🚶 N min“, poslední chůze k cíli „🚶 N min do cíle“
      (bez „přestup“). Přestup na stejném nástupišti (bez chůze) je „přestup N min“ a osa
      mezi jízdami nemá mezeru. Cílový řádek je „🏁 …“.
- [ ] Ve špičce vyberte spoj se zpožděním: původní čas přeškrtnutý, vedle nový čas. Záhlaví
      ukazuje „Vyrazte hned“ nebo po odjezdu červené „Ujelo“. Zpoždění se mění za provozu.
      Není-li žádný spoj zpožděný, zapište „neověřeno“.

**Pozdní večer**

- [ ] Po 22:00 vyhledejte spojení (čas změřte jako výše): _____ ms (na desktopu v Chrome naměřeno ~115 ms, cíl na iPhonu pod 300 ms, přijatelné do ~1 s). Zobrazí se noční
      linky, nebo srozumitelná zpráva, že nic nejede; aplikace nespadne.

**Offline**

- [ ] Zapněte režim Letadlo, aplikaci zavřete a znovu otevřete. Hledání spojení funguje
      stejně (včetně času) a detail se otevře. Poloha může být bez sítě nedostupná,
      zastávka zadaná ručně funguje.

**Tmavý režim**

- [ ] Systém v tmavém režimu: obrazovka hledání (pole, seznam spojení, nabídka) i detail
      spojení (osa, barevné linky, „Ujelo“) jsou čitelné, nic není bílé na bílém.

**Probíhající jízda**

- [ ] Spusťte jízdu (Nová jízda → Nástup). Úvodní obrazovka ukazuje probíhající jízdu
      s tlačítkem **Výstup**, ne hledání.
- [ ] V menu ☰ je nad „Spojení“ položka **Probíhající jízda**; klepnutí na ni vrátí na jízdu.
      Po Výstupu je úvodní obrazovkou zase hledání spojení.

**Aktualizace z 0.5.0 během jízdy**

- [ ] Ve verzi 0.5.0 spusťte jízdu a nechte ji běžet, zatímco se nasadí 0.6.0 (aplikaci
      jen přepínejte do pozadí a zpět). Dokud se aplikace neobnoví, může se objevit pruh
      „Jízdní řád je poškozený …“ (stará verze nezná nový formát) nebo „Jízdní řád je ve
      starém formátu, stáhne se po připojení.“ – to je v pořádku. Výstup jde i tak zapsat
      ručně: na obrazovce Výstup napište název zastávky do pole „Hledat zastávku nebo napsat
      název…“ a klepněte na „Hotovo“.
- [ ] Po Výstupu se aktualizace použije (hláška „Aktualizováno na verzi 0.6.0 …“), pruh
      zmizí nejpozději po návratu do popředí nebo připojení k síti a hledání spojení funguje.

## Kontrolní bod I – vánoční vzhled (#4)

Vánoční vzhled se sám zapne v sobotu 28. 11. 2026. Do té doby ho vynutíte adresou
`https://malcoolm.github.io/pidtracker/?vanoce=1` v Safari (aplikace z plochy parametr
neumí; vzhled v ní uvidíte až od 28. 11.).

- [ ] **Světlý režim** (Nastavení iPhonu → Zobrazení a jas → Světlý): krémové pozadí,
      tmavě zelená horní lišta se zlatou linkou, červená tlačítka, zelené nadpisy s 🎄.
- [ ] **Tmavý režim**: tmavě zelené pozadí, zlaté nadpisy, červená tlačítka; text dobře
      čitelný. Přepněte režim za běhu – barvy se přepnou samy.
- [ ] Vánoční písmo je na nadpisech, názvu v liště, položkách menu, tlačítkách a štítcích
      („nejrychlejší“); háčky a kroužky (Ř, ř, ů, ď, ť, Č) vypadají jako zbytek písma, žádné
      písmeno není v jiném písmu.
- [ ] Časy, zastávky, nástupiště, zpoždění a čísla linek zůstávají v běžném písmu
      a jsou dobře čitelné, i na slunci.
- [ ] Zpoždění (oranžová) a „Vyrazte …“ (zelená) jsou dobře odlišené v obou režimech.
- [ ] Offline: zapněte režim letadlo, obnovte stránku s `?vanoce=1` – písmo i barvy zůstávají.
- [ ] `?halloween=1` vypadá jako o Halloweenu (tmavý vzhled, pavouk, 🎃).
- [ ] Bez parametru (před 28. 11.) běžný vzhled; barva systémové lišty nahoře je červená
      (ve tmavém režimu tmavě šedá), ne modrá.

## Kontrolní bod J – padající sníh (#6)

Stejně jako u bodu I: do 28. 11. 2026 vynutíte adresou `?vanoce=1` v Safari.

- [ ] Za obsahem padá sníh (ve světlém režimu šedomodré vločky, v tmavém bílé); přes karty,
      tlačítka ani horní lištu nepadá.
- [ ] Klepání na tlačítka, karty a pole funguje jako bez sněhu; posouvání stránky je plynulé
      i na starším iPhonu a telefon se nezahřívá víc než obvykle.
- [ ] Zapněte Nastavení → Zpřístupnění → Pohyb → **Omezit pohyb**: vločky zůstanou stát
      (žádný pohyb). Po vypnutí zase padají.
- [ ] `?halloween=1` a běžný vzhled sníh nemají.

## Kontrolní bod K – vánoční ikona (#7)

Ikonu si iOS uloží při přidání na plochu a později ji už nemění. Vánoční ikona se nasadí
buildem v sobotu před 1. adventem (28. 11. 2026, 4:30) a běžná se vrátí buildem 29. 12.

- [ ] Od 28. 11.: nejdřív v aplikaci počkejte, až je vše odesláno (v proužku ✓, ne „čeká N“) –
      odstraněním z plochy iOS smaže i lokální data aplikace.
- [ ] Dlouze podržte starou ikonu na ploše → Odstranit aplikaci → Odstranit z plochy. V Safari
      otevřete `https://malcoolm.github.io/pidtracker/` a Sdílet → Přidat na plochu.
      Na ploše je zelená ikona s P v Santově čepici.
- [ ] Z nové ikony se aplikace spustí bez lišty Safari; po přihlášení Googlem se jízdy načtou
      z tabulky.
- [ ] Od 29. 12.: stejným postupem vraťte běžnou modrou ikonu.

## Kontrolní bod L – vánoční ilustrace (#5)

Do 28. 11. 2026 vynutíte adresou `?vanoce=1` v Safari.

- [ ] Úvodní obrazovka (Spojení): pod tlačítkem Hledat je obrázek – rodina na pohovce,
      stromek s dárky, za oknem sněží, kočka. Při posunu na konec ho nezakrývá tlačítko
      „＋ Nová jízda“.
- [ ] Probíhající jízda: obrázek je pod tlačítky Výstup / Zrušit jízdu, nic nepřekrývá.
- [ ] Historie (dárky), Statistiky (stromek), Mimořádnosti (svíčka): malý obrázek vpravo vedle
      nadpisu, nedotýká se přepínačů pod nadpisem.
- [ ] Ve světlém i tmavém režimu jsou obrázky dobře vidět a text kolem zůstává čitelný.
- [ ] `?halloween=1` a běžný vzhled obrázky nemají.

## Kontrolní bod M – číslo linky uprostřed štítku (#2)

- [ ] Detail spojení (Spojení → Hledat → klepnout na spojení): číslo linky ve štítku vedle
      názvu zastávky je svisle i vodorovně uprostřed.
- [ ] Totéž na kartě spojení, v odjezdech, v historii a v pruhu Mimořádností.
- [ ] Jednoznakové (`C`, `A`) i delší (`S49`, `X-A`, `119`) linky, ve světlém i tmavém režimu
      a s `?vanoce=1`.

## Kontrolní bod N – číslo vozu v detailu spojení (#1)

- [ ] Vyhledat spojení s tramvají nebo autobusem, který už jede (odjezd do pár minut),
      a klepnout na něj. U nástupu je pod názvem zastávky „🚋 vůz 1234“, případně
      „nástupiště B · 🚋 vůz 1234“.
- [ ] U jízdy, jejíž spoj ještě nevyjel, a u metra nebo vlaku se číslo vozu nezobrazí
      a detail vypadá jako dřív.
- [ ] V režimu letadlo: detail spojení se otevře a funguje, jen bez čísla vozu.

## Kontrolní bod O – čerstvá poloha vozu (#14)

- [ ] Odjezdy → `›` u jedoucího spoje: poloha vozu je vidět hned po otevření a „před N s“
      se obnovuje zhruba každých 20 s (ne jednou za minutu).
- [ ] Detail spojení s jedoucí tramvají nebo autobusem: číslo vozu je vidět hned. Zavřít
      a do 20 s znovu otevřít: číslo je tam okamžitě, bez čekání.
- [ ] Přepnout do jiné aplikace, po minutě se vrátit do otevřeného detailu: poloha se hned
      obnoví.
- [ ] Režim letadlo s otevřeným detailem: zůstane poslední poloha, aplikace nespadne.
- [ ] Nastavení → Mobilní data → Safari/PID Tracker: po pár minutách na úvodní obrazovce
      (bez otevřeného detailu) spotřeba dat neroste.

## Kontrolní bod P – stáří polohy vozu (#12)

- [ ] Odjezdy → `›` u jedoucího spoje: čerstvá poloha je v oranžovém štítku „🚋 vůz 1234 ·
      před 20 s“.
- [ ] Otevřít detail a zapnout režim letadlo: po víc než 2 minutách je štítek zašedlý
      s textem „poloha před N min“ (čitelný ve světlém i tmavém režimu i o Vánocích).
- [ ] Po víc než 10 minutách v režimu letadlo vůz z trasy zmizí, zobrazí se „Poloha vozu
      není k dispozici.“ a jízdní řád funguje dál.

## Kontrolní bod Q – čitelná data a počet mimořádností o Vánocích (#19)

- [ ] Ve vánočním období (nebo s `?vanoce=1`): Historie → nadpisy dnů (např. „pátek 27. 11.“)
      jsou v běžném písmu, ostatní nadpisy zůstávají vánoční.
- [ ] Menu: počet mimořádností v oranžovém kolečku je v běžném písmu a dobře čitelný,
      názvy položek menu zůstávají vánoční.
- [ ] Platí ve světlém i tmavém režimu.
