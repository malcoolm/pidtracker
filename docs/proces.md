# Jak řídíme vývoj

Lehký agilní proces nad GitHubem: přání a chyby od uživatelů → backlog → sprint → vydání.
Platí pro všechny, kdo na aplikaci pracují, lidi i Claude.

## Přehled

```
Uživatel                  Analýza                   Plánování            Vývoj                 Vydání
chyba / nápad    ──►   story / epic       ──►    sprint (2 týdny) ──►  jen story     ──►   tag vX.Y.Z
„k probrání“           kritéria, odhad            priorita, kapacita    ze sprintu            Release, iPhone
```

Všechno žije na GitHubu: issues v repu a stav na nástěnce **[GitHub Project „PID Tracker“](https://github.com/users/malcoolm/projects/4)**
(pole Status, Priorita, Velikost, Sprint; nástěnka je i v záložce *Projects* repozitáře).
Mimo GitHub se stav neeviduje.

Stavy na nástěnce: **Backlog → Připraveno → Ve sprintu → Rozpracováno → Hotovo**.

## 1. Vstup: hlášení uživatelů

- Uživatel založí issue formulářem **Něco nefunguje** nebo **Nápad / přání**
  (z menu aplikace, nebo na GitHubu → Issues → New issue). Formulář přidá štítek `chyba`
  nebo `nápad` a `k probrání`.
- Hlášení je veřejné. Pokud v něm jsou osobní údaje (e-mail, poloha, odkaz na tabulku),
  správce je z textu odstraní.

## 2. Analýza: z hlášení story nebo epic

Každé hlášení s `k probrání` se při analýze rozhodne jedním ze způsobů:

| Rozhodnutí | Co se stane |
|---|---|
| **Story** (většina) | Hlášení samo se stane story: přidá se štítek `story`, doplní se kritéria přijetí a velikost. Nezakládá se duplicitní issue. |
| **Epic** (velké přání) | Hlášení dostane štítek `epic`; jednotlivé story se založí jako jeho **sub-issues**. |
| **Sloučit** | Okomentuje se odkazem na existující issue a zavře jako duplicitní. |
| **Neuděláme** | Štítek `neuděláme`, komentář s důvodem, zavřít. |
| **Doptat se** | Komentář s otázkou pro autora; `k probrání` zůstává. |

Po rozhodnutí se `k probrání` odebere.

### Story je připravená (Definition of Ready), když má

- název z pohledu uživatele („Stanoviště u každého nástupu v detailu spojení“),
- **kritéria přijetí** v issue, ve tvaru *Když … pak …*, ověřitelná na telefonu,
- **velikost** S / M / L (S = do jednoho sezení, M = 1–2 sezení, L = rozdělit na menší story,
  do sprintu se L nebere),
- u větších změn poznámku, zda je potřeba návrh (spec v `docs/superpowers/specs/`).

Pak se na nástěnce přesune do **Připraveno**.

## 3. Plánování sprintu

- **Sprint = 2 týdny** (stejný rytmus jako platnost jízdních řádů PID). Na nástěnce je to
  pole **Sprint** (iterace).
- Na začátku sprintu: správce produktu určí **prioritu** (Vysoká / Střední / Nízká), vývojáři
  řeknou, co se reálně vejde. Vybrané story dostanou iteraci a stav **Ve sprintu**.
- Story s termínem (sezónní vzhled, změna jízdních řádů) se plánují podle termínu, ne jen
  podle priority.
- **Cíl sprintu**: jedna věta v README nástěnky (iterace popis nemají) („Vánoční vzhled od 1. adventu, stanoviště
  v detailu spojení“).

## 4. Vývoj

- Pracuje se **jen na story ve sprintu**. Nová práce, která se objeví během sezení, se
  zapíše jako issue do backlogu; do běžícího sprintu ji lze přidat jen po dohodě (a za to
  z něj něco vypadne).
- **Výjimka – kritická chyba**: aplikace nejde spustit, neukládá nebo nesynchronizuje jízdy.
  Ta jde do sprintu hned, bez čekání na plánování.
- Rozpracovaná story má stav **Rozpracováno**; najednou nejvýš jedna až dvě.
- Commity odkazují na issue: `feat(app): stanoviště u nástupu (#1)`. Poslední commit story
  ji uzavírá: `… (closes #1)`.
- Větší story jdou přes návrh → plán → implementaci; odkazy na spec a plán se dají do issue.

### Story je hotová (Definition of Done), když

- jsou splněná kritéria přijetí,
- prošly testy a clippy, změna je na `main` a nasazená,
- je ověřená na iPhonu (kontrolní bod v `docs/testing-iphone.md`, pokud se týká UI),
- issue je zavřená commitem a na nástěnce je **Hotovo**.

## 5. Konec sprintu

1. Zvýšit `version` v `Cargo.toml` (nové funkce → minor, jen opravy → patch).
2. Kontrolní bod na iPhonu pro story sprintu.
3. Anotovaný tag s poznámkami k vydání sestavenými z uzavřených story (postup v `README.md`,
   sekce *Vydání verze*). Workflow vytvoří GitHub Release se zipem buildu.
4. Krátké ohlédnutí: co nestihneme se vrací do backlogu (stav **Připraveno**), co se naučilo
   se zapíše sem do procesu.
5. Komentář v uzavřených uživatelských hlášeních: ve které verzi je změna.

## Štítky

| Štítek | Význam |
|---|---|
| `chyba` | hlášení: něco nefunguje |
| `nápad` | hlášení: přání nebo nová funkce |
| `k probrání` | čeká na analýzu |
| `story` | připravená jednotka práce |
| `epic` | velké přání rozdělené na story (sub-issues) |
| `uděláme` | domluveno, ještě nenaplánováno do sprintu |
| `neuděláme` | rozhodnuto nedělat (s důvodem v komentáři) |

## Role

| Role | Kdo | Co dělá |
|---|---|---|
| Uživatelé | kdokoli s účtem GitHub | zakládají hlášení, odpovídají na doplňující otázky |
| Správce produktu | Miroslav | priorita, výběr do sprintu, přijetí hotových story |
| Vývoj | Miroslav + Claude (a další, kdo se přidají) | analýza, odhady, implementace, vydání |
| Strážce procesu | Claude | drží konzistenci (viz níže) |

### Kontroly, které dělá Claude

- **Začátek sezení:** načte sprint z nástěnky a řekne stav (co je hotovo, rozpracováno,
  zbývá, kolik dní do konce sprintu) a nová hlášení `k probrání`.
- **Během sezení:** upozorní, když se pracuje na něčem mimo sprint, a nabídne zapsat to do
  backlogu.
- **Konec sezení:** aktualizuje stavy na nástěnce, okomentuje rozpracované story.
- **Před vydáním:** každá story ve sprintu má kritéria a velikost; každá zavřená má commit;
  štítky a stavy na nástěnce odpovídají issues; nic hotového nezůstalo otevřené.
- Stav nikdy neeviduje mimo GitHub; mezi sezeními si ho nepamatuje.

## Příkazy

```
gh issue list --label "k probrání"                     # nová hlášení
gh issue list --label story --state open               # backlog story
gh project item-list 4 --owner malcoolm                # nástěnka
gh auth refresh -s project                             # jednorázově: přístup k Projects z gh
```
