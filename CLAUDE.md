# PID Tracker – pokyny pro Claude

- Vývoj se řídí procesem v [`docs/proces.md`](docs/proces.md): pracuje se jen na story
  v aktuálním sprintu, stav se eviduje na GitHubu (issues + GitHub Project), Claude drží
  konzistenci a na začátku sezení shrne stav sprintu a nová hlášení `k probrání`.
- Komunikace s uživatelem česky; dokumentace a texty aplikace česky, commit zprávy anglicky
  (conventional commits, odkaz na issue `(#n)`, uzavření `(closes #n)`).
- Práce přímo na `main`, bez větví.
- Git Bash: `MSYS_NO_PATHCONV=1 cargo xtask dist --public-url / …`.
