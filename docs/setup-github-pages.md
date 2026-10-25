# Nastavení GitHub Pages

Aplikace se nasazuje automaticky přes GitHub Actions (`.github/workflows/deploy.yml`)
při každém push do `main`, jednou týdně (pondělí 03:30 UTC, kvůli aktualizaci
jízdních řádů) a ručně přes **Actions → Build & deploy → Run workflow**.

## Jednorázové nastavení

1. Repozitář musí být **veřejný** (GitHub Pages na bezplatném účtu). Zdrojový kód
   je veřejný, osobní data jsou jen ve vaší Google tabulce.
2. **Settings → Pages → Build and deployment → Source: GitHub Actions.**
3. **Settings → Secrets and variables → Actions → Variables → New repository variable**
   - Name: `GOOGLE_CLIENT_ID`
   - Value: Client ID z Google Cloud Console (viz `setup-google.md`).
   Client ID není tajný údaj, proto je to „variable“, ne „secret“.
4. Po prvním úspěšném běhu workflow je aplikace na
   `https://<uživatel>.github.io/pidtracker/`.

## Ověření

- `https://<uživatel>.github.io/pidtracker/version.json` vrací JSON s `build_id`.
- Patička aplikace ukazuje stejné `build_id`.

## Pozor na týdenní aktualizaci jízdních řádů

GitHub automaticky **vypne plánované spouštění workflow po 60 dnech bez aktivity**
v repozitáři. Jízdní řády PID platí jen 14 dní, takže by aplikace začala hlásit prošlá
data. Pokud se to stane, v **Actions → Build & deploy** klikněte na **Enable workflow**
(a případně **Run workflow**). Aplikace na blížící se konec platnosti upozorní 2 dny předem.
