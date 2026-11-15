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
