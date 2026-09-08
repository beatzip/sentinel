# TODO Audit

Audit date: 2026-09-07
Repository revision: `878c162`

## Scope

The audit compared both task ledgers with the current source tree, committed
tests, documentation, and Git history:

- `todo.md`: 182 checked and 50 unchecked entries.
- `dashboard/todo.md`: 14 checked entries.

A checked item was classified as repository-verifiable only when its current
implementation or a durable report could be found outside the checkbox itself.
Local demos, ignored evidence bundles, browser approvals, delivered
attachments, and previous-session command output were not treated as
independently reproducible repository evidence.

## Result

| Ledger | Checked | Repository-verifiable | Historical or external-only |
|---|---:|---:|---:|
| Main | 182 | 104 | 78 |
| Dashboard | 14 | 14 | 0 |

The 78 historical or external-only entries are not automatically false. Most
record work performed against local demos, VPK bytes, authenticated sessions,
browser pages, or ignored corpus artifacts that are intentionally absent from
Git. They cannot be revalidated from a clean clone and should not be used as
current implementation evidence without their referenced artifacts.

## Confirmed reconciliation

- The unchecked Encounter Ledger item was superseded by the observed
  `weapon_fire` / `player_hurt` stream, deterministic shot-to-damage links,
  direct-damage sequences, and observed damage-to-death intervals now present
  in the report and replay contracts.
- The dashboard ledger is represented by the strict
  `approximate_spatial` validator, replay navigation helpers, viewer
  implementation, focused Vitest coverage, and the committed functional
  validation notes.
- The two retained performance optimizations are present in source at
  `878c162`. Their task-log measurements are now preserved in
  `docs/performance-optimization-ledger.md`; they remain reported prior-session
  results until the raw benchmark bundle is reproduced.
- The segment benchmark still used a `MapData` struct literal after the cache
  field became private, so all-target compilation and the benchmark CI job
  failed. The benchmark now starts from a public constructor and replaces only
  its synthetic fixture fields.
- The checked final-format claim did not hold under the repository's declared
  Rust 1.88 toolchain. The affected source files have been normalized with
  `cargo +1.88.0 fmt --all`.
- The repository still contains no verified supervised corpus:
  `datasets/manifest.json` is empty. Corpus-backed calibration, regression,
  A/B metrics, and feature importance therefore remain pending regardless of
  historical local-corpus checkboxes.

## Ledger rules going forward

1. Use `[x]` for code or documentation that exists in the repository, or for a
   bounded external task whose durable result is linked from the repository.
2. Record a blocked attempt as a result, not as successful acquisition or
   validation.
3. Keep raw demos, credentials, and restricted source bytes outside Git, but
   commit a redacted manifest containing stable hashes, provenance, scope, and
   outcome.
4. Do not use a checkbox or an untracked local file as the only evidence for
   numerical performance, corpus composition, browser verification, or
   human-review decisions.
5. Keep unknown demos out of supervised metrics until the ground-truth gates in
   `docs/ground-truth-review-process.md` pass.
