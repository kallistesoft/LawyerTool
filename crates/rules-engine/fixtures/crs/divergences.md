# Known divergences between official sources

Each entry: what differs, sources, status. The engine follows the lawyer's
ruling; until a ruling exists the entry is **PENDING** and any client hitting
the case is flagged for manual review.

## D-001: TCF Canada at NCLC 4, first official language

- **Status:** PENDING lawyer ruling
- **Found:** 2026-10-18, reading the calculator source
- **Criteria page** (`criteries-scg.html`, language table, first official
  language): "NCLC 4 ou 5 → 6 points" per skill, with or without spouse.
- **Calculator** (`scg-outil-udit.js`, `tcf` array, row `B` = NCLC 4): first
  official language points `0` / `0`. Every other test (CELPIP, IELTS, PTE,
  TEF) gives `6` / `6` at NCLC 4.
- **Impact:** a TCF-first applicant with any skill at NCLC 4 gets up to
  24 points less in the calculator than the published grid gives.
- **Proposed ruling:** follow the criteria page (6 points), since it is the
  published grid and the calculator is inconsistent with itself across tests.
  Confirm against the Ministerial Instructions.
- **Live check:** to confirm in Chrome (golden profile to be added).
