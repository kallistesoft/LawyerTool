# Official sources (snapshots)

Fetched 2026-10-18. Saved verbatim; the rules engine and grid parser are tested
against these exact files. Re-fetch only through the watcher flow (M2b), never
by hand-editing.

| File | URL | SHA-256 |
|---|---|---|
| `criteries-scg.html` | https://www.canada.ca/fr/immigration-refugies-citoyennete/services/immigrer-canada/entree-express/verifier-note/criteries-scg.html | `c4a492adfa2cad679c2e797cc4391d7d2231198ad8b6e08c8483f37ad2080961` |
| `verifier-note.html` | https://www.canada.ca/fr/immigration-refugies-citoyennete/services/immigrer-canada/entree-express/verifier-note.html | `76036f8d718b7a0469943a78b74237ad30ff3abb77e01e2c50946fc61b67c348` |
| `scg-outil-udit.js` | https://ircc.canada.ca/francais/immigrer/qualifie/scg-outil-udit.js | `61392e6266be6d976735305982eeb72ce30296f086ebf1b5dc802a305639a65a` |
| `modifications.html` | https://www.canada.ca/fr/immigration-refugies-citoyennete/services/immigrer-canada/entree-express/modifications.html | `9729a26925f1ceed605766636b212c0ae1a6785ed0b638667dc1ad63e155a718` |
| `ee_rounds_123_fr.json` | https://www.canada.ca/content/dam/ircc/documents/json/ee_rounds_123_fr.json | `6c6468ad045d0bd7e0ef2fc308b02efeed7510b25dddbec5ddb7f31d26c47b57` |

`criteries-scg.html` is the reference grid. `scg-outil-udit.js` is the
calculator logic, used only as a test oracle (never executed by the app).

Still to add (M0 step 2): FSW / CEC / FST criteria pages and the language test
equivalency charts.
