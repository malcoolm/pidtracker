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
