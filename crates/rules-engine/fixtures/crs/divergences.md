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

## D-003: TEF Canada score scale depends on the test date

- **Status:** PENDING ruling
- **Found:** 2026-10-18
- **Express Entry pages** (`examen-linguistique.html` and the calculator
  `scg-outil-udit.js`): one TEF table only, e.g. NCLC 7 = speaking 310–348,
  listening 249–279, reading 207–232, writing 310–348. This is the scale for
  tests taken **before 2019-09-30**.
- **IRCC Atlantic Immigration page** (`equivalences-aip-en.html`): three TEF
  tables by test date:
  - after 2023-12-10: e.g. NCLC 7 = reading 434–461, writing 428–471,
    listening 434–461, speaking 456–493 (0–699 scale);
  - 2019-10-01 to 2023-12-10: NCLC 7 = 450–499 for all four skills;
  - before 2019-09-30: same as the Express Entry pages.
- **Impact:** Express Entry results are valid for 2 years, so every valid TEF
  result today is on the post-2023 scale. Converting a real current TEF score
  with the Express Entry table would give a wrong NCLC (or an impossible score).
- **Proposed ruling:** convert TEF by test date using the three tables. The
  CRS points are then looked up by NCLC, which is how the calculator works
  anyway (its options are NCLC rows), so CRS points are unaffected. Record the
  test date on every language result.
- **Gaps on 2019-09-30 → 2019-10-01 and on 2023-12-10 itself:** a test dated
  exactly 2023-12-10 matches two tables. Proposed: treat 2023-12-10 as the
  older scale ("between October 1, 2019 and December 10, 2023"), and refuse
  dates in the one-day gap (2019-09-30 is "before"; nothing for none).
