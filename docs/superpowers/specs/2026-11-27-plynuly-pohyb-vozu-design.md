# Plynulý pohyb vozu mezi staženími polohy (#15)

Story #15 epiku #8 *Poloha vozu v detailu spoje zaostává za skutečností* (bod 3 návrhu řešení).
Navazuje na #13 (značka vozu na čáře trasy) a #14 (sdílené stahování po 20 s).

## Proč

Značka vozu v detailu spoje se posune jen při novém stažení (~20 s) a poloha ve feedu je
sama stará desítky sekund. Vůz proto poskakuje a je pozadu. Mezi staženími se má značka
plynule posouvat podle jízdního řádu a zpoždění, aby odpovídala tomu, kde vůz asi je teď.

## Cíle

- Značka vozu se posouvá **každou sekundu** podle odhadu, s krátkou animací (plynulý pohyb).
- **Odhad je poznat** (varianta B z náhledu): čárkovaný bílý obrys značky a v souhrnu
  „odhad, GPS před N s“ místo „před N s“; po stažení nové polohy je obrys zase plný.
  Odhad = od časové značky GPS polohy uplynulo aspoň 10 s.
- Výpočet odhadu:
  - Výchozí bod je GPS poloha (úsek a podíl úseku z #13) v čase `timestamp` polohy.
  - Vůz mezi zastávkami `i → i+1`: podíl úseku roste o uplynulý čas / jízdní doba úseku
    podle jízdního řádu (`čas(i+1) − čas(i)`, nejméně 30 s).
  - Vůz na zastávce `i`: stojí do odjezdu podle jízdního řádu a zpoždění, pak se rozjede
    do úseku `i → i+1` stejným tempem.
  - **Nikdy za další zastávku**: podíl úseku nejvýš 1 (vůz u další zastávky), dokud ji
    nepotvrdí feed.
  - Nová poloha z feedu odhad nahradí a počítá se od ní.
- Projetá (vybledlá) část čáry jde se značkou.
- Přepočet běží jen s otevřeným detailem spoje a viditelnou aplikací.

## Mimo rozsah / nechceme

- Souhrn nahoře (počet zastávek, „přijíždí“, „ve stanici“) zůstává podle GPS polohy;
  odhad mění jen značku na čáře a text o stáří polohy. Odhad vůz nepouští za další
  zastávku, takže počet zastávek se tím nerozchází.
- Stopa skutečné polohy (varianta C), skutečný tvar trasy (#16), detail spojení.
- Zastaralá poloha (#12) má přednost: po 2 min zašedlé, po 10 min skryté, odhad se nepočítá
  dál než pro čerstvou polohu (viz builder).

## Přístupy

1. **Přepočet každou sekundu** (zvoleno): odhad v `pid-core` jako čistá funkce s testy,
   v detailu spoje hodiny po 1 s a CSS přechod `top` 1 s lineárně.
2. Jedna dlouhá CSS animace do konce úseku po každém stažení: méně výpočtů, ale složité
   zastavení (nová poloha, stáří, zavření) a špatně testovatelné. Zahozeno.

## Návrh

- **`pid-core::realtime`**: `estimate_progress(place, progress, seg_times, fix_ts, now_ts,
  depart_ts) -> (VehiclePlace, f32)` – vrací místo a podíl úseku pro „teď“; konstanty
  `MIN_SEGMENT_S = 30`, `ESTIMATE_AFTER_S = 10`. Testy: pohyb uprostřed úseku, strop na
  další zastávce, stání na zastávce do odjezdu a rozjezd, nulová jízdní doba, čas polohy
  v budoucnosti (žádný pohyb).
- **`trip.rs`**: hodiny `at` po 1 s (místo 15 s) jen při viditelné stránce; značka a
  vyblednutí z odhadu; třída `est` na značce, text v souhrnu.
- **CSS**: `.veh-mark { transition: top 1s linear }`, `.trip-gap .rail-fade` stejně,
  `.veh-mark.est .veh-icon { border-style: dashed }`.
- Kontrolní bod na iPhonu (U).

## Na implementaci

Přesná signatura funkce odhadu, jak se do ní dostanou časy zastávek se zpožděním, a chování
značky při přechodu z mezery mezi zastávkami na zastávku (u stropu zůstane značka na konci
mezery, dokud feed nepotvrdí zastávku).
