# Nastavení Google (přihlášení a tabulka)

Aplikace se přihlašuje Googlem přímo v prohlížeči a zapisuje do vaší Google tabulky.
K tomu potřebuje **OAuth Client ID** z vlastního Google Cloud projektu. Je to jednorázové
nastavení na 10–15 minut.

## 1. Google Cloud projekt

1. Otevřete <https://console.cloud.google.com/> a přihlaste se svým účtem.
2. Nahoře vyberte projekt → **New Project** → název např. `pidtracker` → **Create**.
3. **APIs & Services → Library** → vyhledejte **Google Sheets API** → **Enable**.

## 2. OAuth consent screen (Google Auth Platform)

1. **APIs & Services → OAuth consent screen** (nebo „Google Auth Platform“ → **Get started**).
2. App name `PID Tracker`, support e-mail = váš e-mail.
3. Audience: **External**.
4. Contact information: váš e-mail → **Create**.
5. **Audience → Test users → Add users**: přidejte **svůj e-mail i e-mail syna**.
   Aplikace zůstává v režimu **Testing**, nic se neověřuje ani nepublikuje.
6. **Data Access → Add or remove scopes**: přidejte
   - `.../auth/spreadsheets` (Google Sheets API – See, edit, create, and delete all your Google Sheets spreadsheets)
   - `openid`
   - `.../auth/userinfo.email`
   a uložte.

## 3. OAuth Client ID

1. **APIs & Services → Credentials → Create credentials → OAuth client ID**
   (nebo „Google Auth Platform → Clients → Create client“).
2. Application type: **Web application**, název `PID Tracker web`.
3. **Authorized JavaScript origins** – přidejte:
   - `https://<uživatel>.github.io` (adresa GitHub Pages, **bez** `/pidtracker` na konci)
   - `http://localhost:8080` (lokální vývoj, `trunk serve`)
   - `http://127.0.0.1:8090` (lokální test sestavené složky `dist`)
4. **Create** a zkopírujte **Client ID** (končí `.apps.googleusercontent.com`).
   Client ID není tajný údaj – je vidět v kódu stránky.

## 4. Předání Client ID do buildu

- **GitHub:** Settings → Secrets and variables → Actions → **Variables** →
  `GOOGLE_CLIENT_ID` = zkopírované Client ID. Pak spusťte workflow znovu
  (Actions → Build & deploy → Run workflow).
- **Lokálně (PowerShell):** `$env:PIDTRACKER_GOOGLE_CLIENT_ID = "…apps.googleusercontent.com"`
  před `trunk serve` nebo `cargo xtask dist`.

## 5. Tabulka

1. Na <https://sheets.google.com> založte **prázdnou** tabulku, např. `PID jízdy`.
2. **Sdílet** → přidejte e-mail syna s rolí **Editor**.
3. Při prvním spuštění aplikace vložte odkaz na tabulku. Listy `rides` a `people`
   s hlavičkami si aplikace založí sama.
4. Na listy `rides` a `people` nepište vzorce ani nepřeskupujte sloupce. Vlastní
   analýzy dělejte na jiných listech. Řádky v `rides` lze řadit a filtrovat, aplikace
   si řádky dohledává podle `id`.

## Co uvidíte při přihlášení

Google zobrazí varování **„Google hasn’t verified this app“**. U vlastní aplikace
v režimu Testing je to normální: klikněte na **Continue** (případně **Advanced → Go to
PID Tracker**) a povolte přístup k tabulkám.

Přihlášení platí zhruba hodinu. Potom aplikace dál normálně zaznamenává jízdy
a u stavu synchronizace nabídne **Přihlásit pro synchronizaci** – stačí klepnout.
