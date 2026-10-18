# Known divergences between official sources

Each entry: what differs, sources, status. The engine follows the lawyer's
ruling; until a ruling exists the entry is **PENDING** and any client hitting
the case is flagged for manual review.

## D-001: TCF Canada at NCLC 4, first official language

- **Status:** RULED 2026-10-18: follow the criteria page (6 points)
- **Found:** 2026-10-18, reading the calculator source
- **Criteria page** (`criteries-scg.html`, language table, first official
  language): "NCLC 4 ou 5 → 6 points" per skill, with or without spouse.
- **Calculator** (`scg-outil-udit.js`, `tcf` array, row `B` = NCLC 4): first
  official language points `0` / `0`. Every other test (CELPIP, IELTS, PTE,
  TEF) gives `6` / `6` at NCLC 4.
- **Impact:** a TCF-first applicant with any skill at NCLC 4 gets up to
  24 points less in the calculator than the published grid gives.
- **Ruling:** follow the criteria page (6 points). It is the published grid,
  and the calculator is inconsistent with itself across tests. Approved by the
  project owner on 2026-10-18; to re-confirm against the Ministerial
  Instructions during the pilot (M4).
- **Engine:** scores the TCF row at NCLC 4 as 6 / 6. The oracle harness
  skips only this pattern (TCF first language, any skill at NCLC 4).
- **Live check:** to confirm in Chrome (golden profile to be added).

## D-002: Second official language at NCLC 4

- **Status:** RULED 2026-10-18: 0 points at NCLC 4
- **Found:** 2026-10-18, by `Grid::issues()` on the parsed v0 grid
- **Criteria page** (second official language table): rows are "Moins que le
  niveau 4 des NCLC → 0", then "NCLC 5 ou 6 → 1". NCLC 4 is in no row.
- **Calculator** (`scg-outil-udit.js`, column "SOL", row `B` = NCLC 4): `0`
  for every test.
- **Impact:** an applicant with a second-language skill at exactly NCLC 4.
  0 vs 1 point per skill, at most 4 points.
- **Ruling:** 0 points at NCLC 4. This matches the calculator, and the
  table's first row is most likely meant to read "NCLC 4 ou moins" (as the
  spouse table does). Approved by the project owner on 2026-10-18; to
  re-confirm against the Ministerial Instructions during the pilot (M4).
- **Engine:** `rulings::apply` adds the row `NCLC 4 → 0 / 0` to the second
  language table, cited as `ruling:D-002`.
