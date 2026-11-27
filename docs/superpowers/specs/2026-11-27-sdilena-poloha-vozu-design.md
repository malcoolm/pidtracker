# Čerstvá poloha vozu hned po otevření detailu (#14)

Story #14 epiku #8 *Poloha vozu v detailu spoje zaostává za skutečností* (bod 2 návrhu řešení).

## Proč

Poloha vozu v detailu je často stará. Feed PID je sám o sobě starý desítky sekund, každá
obrazovka si ho stahuje sama jednou za 60 s a po otevření detailu se čeká na nové stažení.
Cílem je, aby poloha byla čerstvější a po otevření detailu byla vidět hned.

## Cíle

- Jedno sdílené úložiště polohy vozů pro celou aplikaci, nezávislé na obrazovce.
- Stahování **každých 20 s**, ale jen když je aplikace vidět a nějaká obrazovka polohu
  potřebuje: **detail spoje** (`trip.rs`) a **detail spojení** (`journey.rs`).
- Po otevření detailu se hned ukáže poloha mladší než 20 s; jinak se hned stáhne.
- Po návratu aplikace do popředí se hned stáhne, pokud polohu někdo potřebuje.
- Doplnění čísla vozu k zaznamenané jízdě (`fill_vehicle`) použije čerstvou polohu ze
  sdíleného úložiště, jinak si stáhne sama jako dosud.

## Mimo rozsah / nechceme

- Obrazovka probíhající jízdy polohu vozu nezobrazuje a v této story ji nedostane (patří
  k #13). Kritérium přijetí v #14 se podle toho opravilo.
- Odhad polohy mezi staženími (#15), zobrazení stáří polohy (#12).
- Stahování na pozadí: PWA na iOS na pozadí neběží.
- **Nesmí** se stahovat, když polohu nic nepotřebuje (feed má ~500 kB a server ho
  nekomprimuje; 20 s ≈ 90 MB/h při trvale otevřeném detailu).

## Přístupy

1. **Sdílené úložiště s přihlášením obrazovek** (zvoleno). Obrazovka se při otevření
   přihlásí a při zavření odhlásí. Stahuje se jen, když je přihlášená aspoň jedna obrazovka.
   Jedno místo pro interval, hlídání viditelnosti a návrat do popředí.
2. Sdílená mezipaměť stahování s časovači v obrazovkách. Menší změna, ale časovače zůstanou
   rozházené a pravidlo „nikdo nepotřebuje → nestahuj“ se špatně hlídá.
3. Golemio JSON API pro jeden spoj. Malé odpovědi, ale potřebuje API klíč, který by byl
   v PWA veřejný, nebo proxy. Zahozeno.

## Návrh

- **`pid-core::realtime`**: `parse_vehicle_fixes(feed) -> HashMap<trip_id, VehicleFix>`,
  jedno dekódování feedu. Dosavadní `vehicle_fix`, `vehicle_for_trip` a
  `vehicles_for_trips` budou nad ní, aby se chování nezměnilo (stávající testy zůstanou).
  Pravidlo „kdy stáhnout“ jako čistá funkce s testy: stahuje se, když je někdo přihlášený,
  stránka je vidět, žádné stahování neběží a poslední data jsou starší než 20 s (nebo žádná).
- **Aplikace, nový modul `positions`**: v `AppState` signál s daty (tabulka spoj → poloha
  a čas stažení), počet přihlášených obrazovek a příznak běžícího stahování. `install(state)`
  spustí smyčku, která každých ~5 s zkontroluje pravidlo a případně stáhne. Přihlášení
  (`subscribe`) zvýší počet, hned zkusí stáhnout a odhlášení zaregistruje přes `on_cleanup`.
  Návrat do popředí zkusí stáhnout hned. Při chybě (offline) zůstanou poslední data.
- **`trip.rs` a `journey.rs`**: místo vlastních smyček se přihlásí a čtou polohu, resp.
  čísla vozů ze sdíleného signálu.

## Na implementaci

Pojmenování, přesná struktura signálů a kontrolní bod na iPhonu (O) v
`docs/testing-iphone.md`.
