# Vůz na svislé lince trasy a za kolik přijede (#13)

Story #13 epiku #9 *Srozumitelnější zobrazení polohy vozu na trase*.

## Proč

Když čekám na zastávce, chci z detailu spoje na první pohled poznat, kde vůz je, jak daleko
je od mé zastávky a za jak dlouho přijede. Dnešní řádek „🚋 vůz 1234 · před 20 s“ vložený
mezi dvě zastávky to neříká.

## Cíle

- **Souhrn nahoře** (varianta B, vybraná na náhledu): velký text „za 3 min · 2 zastávky“,
  pod ním menším „autobus 1956 za Pod Královkou · před 20 s“.
  - Minuty = čas příjezdu na mou zastávku podle jízdního řádu a zpoždění (stejný údaj jako
    „za N min“ u zastávky), zastávky = kolik zastávek zbývá od vozu k mé zastávce.
  - Vůz do 50 m před mou zastávkou → „přijíždí“; vůz na mé zastávce → „ve stanici“.
  - Vůz už mou zastávku projel → „vůz už odjel“.
- **Značka vozu na čáře trasy**: kolečko v barvě linky s bílým obrysem a **vlastním
  piktogramem podle druhu dopravy linky** (autobus, tramvaj se sběračem, trolejbus se dvěma
  tyčemi, metro, vlak; ostatní obecný vůz), vedle žlutý štítek s **evidenčním číslem vozu**.
  Značka stojí mezi zastávkami podle ujeté části úseku (0–100 %), ne jen „u zastávky / mezi“.
- Projetá část čáry je bledá, projeté zastávky zůstávají zešedlé jako dnes.
- Druh vozu i v souhrnu („tramvaj 9322 …“), aby šel poznat na první pohled.
- Stáří polohy z #12 platí dál: po 2 min je značka i souhrn zašedlé s „poloha před N min“,
  po 10 min se vůz nezobrazí.

## Mimo rozsah / nechceme

- Detail spojení z vyhledávání (#1 tam ukazuje jen číslo vozu). Případně samostatná story.
- Směr jízdy z `bearing` (na svislé lince je směr vždy dolů po trase).
- Mapa (neuděláme, viz #9), skutečný tvar trasy (#16), plynulý pohyb mezi staženími (#15).
- Emoji jako ikony (vzhled závisí na systému; zvoleny vlastní piktogramy).

## Přístupy

Jediná rozumná cesta je upravit stávající detail spoje (`trip.rs`), který už svislou čáru
trasy kreslí v CSS, a doplnit do výpočtu polohy podíl úseku. Alternativy (kreslit celou
trasu jako SVG, nebo samostatná nová obrazovka) by duplikovaly hotové věci (sbalování
zastávek, časy se zpožděním, „tady stojíš“) bez přínosu pro uživatele.

## Návrh

- **`pid-core::realtime`**: `place_vehicle` vrátí i podíl úseku – `VehiclePlace::Between(i)`
  dostane `progress: f32` 0–1 (z projekce GPS na úsek, která `t` už počítá). Nová čistá
  funkce pro souhrn: z místa vozu, indexu mé zastávky a počtu zastávek vrátí stav
  `Arriving` / `AtStop` / `Departed` / `StopsAway(n)` – s testy.
- **Piktogramy**: inline SVG (bílé, 18 px) v kruhu 30 px v barvě linky, podle
  `RouteKind` linky; komponenta v `components.rs`.
- **`trip.rs`**: souhrn nad trasou; značka vozu absolutně umístěná na čáře v rámci řádku
  úseku podle `progress`; štítek s číslem vozu vedle. Bez polohy (nebo starší 10 min)
  souhrn ukáže jen „za N min“ podle jízdního řádu a „Poloha vozu není k dispozici.“
- Kontrolní bod na iPhonu (T) v `docs/testing-iphone.md`.

## Na implementaci

Přesné rozměry a odsazení podle náhledu, piktogramy metra a vlaku ve stejném stylu,
chování značky u sbalených zastávek (značka v sbalené části → rozbalit okolí vozu jako dnes).
