# Đại Nam Quấc Âm Tự Vị

A digital edition of Huình Tịnh Paulus Của's dictionary (Saigon, 1895–96), built from the
2026 scanned edition.

**The rule the whole repository is arranged around: nothing may be guessed.** Where the source
does not say, the program stops and writes a dossier row for a person to settle — it never
fills in a value that reads plausibly. Every step below exists to make that checkable rather
than merely promised.

## The pipeline

Each step reads a file and writes a file. Nothing but `tools/fetch-wikisource.mjs` touches the
network, and nothing but `import` touches the database, so any step can be re-run and compared.

```
docs/*.pdf ──1──► data/pages.jsonl ──2──► data/entries.jsonl ──3──► data/gates.json
                                              │                          │
                                              ├──4──► review/witness-*    │
                                              ├──5──► frontend/fonts/     │
                                              └──6──────────────────────► PostgreSQL
```

| # | Command | What it does |
|---|---|---|
| 1 | `extract <in.pdf> <out.jsonl>` | Read the PDF into styled line segments, reporting unmapped glyph codes |
| 2 | `parse <pages.jsonl> [entries.jsonl]` | Classify lines, assemble entries. Gates ①②③ → `gates-parse.json` |
| 3 | `reconcile <entries.jsonl> <Mục Từ.xlsx>` | Cross-check the index, check order. Gates ④⑤ → `gates.json` |
| 4 | `witness <entries.jsonl> <wikisource/pages.jsonl> [review-dir]` | Compare against the 1895 edition. Gate ⑥ |
| 5 | `dnqatv-font <in.pdf> <entries.jsonl> frontend/fonts` | Subset the fonts the print uses |
| 6 | `import <entries.jsonl> [pages.jsonl]` | Load into PostgreSQL — refuses unless `gates.json` passes |

Step 4 needs a one-time snapshot of the other edition:

```
node tools/fetch-wikisource.mjs        # writes data/wikisource/, safe to re-run
```

The PDFs are not in git: they are sources, not code, and one of them is past what GitHub
accepts. Put them under `docs/` yourself before running step 1 — `.gitignore` keeps them out.

Filenames under `docs/` are NFD on disk. A hand-typed NFC string will not open them — use a
glob (`docs/*2026*.pdf`).

## The gates

Steps 1–3 write `data/gates.json`, and `import` **refuses to run** if that report is missing,
if any gate failed, or if it was produced from a different `entries.jsonl` than the one being
loaded. A green but stale report is more dangerous than no report, so the report carries a
fingerprint of the data it checked.

An allowed threshold is never a hand-typed number: it **is** the number of rows in the matching
dossier under `review/`. Turning a gate green for a new deviation means writing a row for it.

| Gate | Invariant |
|---|---|
| ① | No glyph code is decoded by falling back to a lookalike |
| ② | Every part of a headword line lands in a field |
| ③ | Every character sits in exactly one field, once |
| ④ | No glyph the parser produced is absent from the printed index |
| ⑤ | No entry precedes one that should come before it, under the book's own 22-letter order |
| ⑥ | An independent transcription of the 1895 original does not contradict our reading |

Gate ⑥ is deliberately **not** pass/fail. The 1895 and 2026 editions genuinely differ, so
demanding an exact match would either cry wolf or invite someone to soften the threshold. It
sorts findings into kinds and writes them to `review/` — see `review/README.md`.

## Database

```
dnqatv db migrate status | up | down | fresh
dnqatv db health
dnqatv config show | example
```

Configuration comes from `.env`; `.env.example` is generated from the code
(`dnqatv config example`), so edit the code, not the example.

## Checks

CI runs `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, plus three greps that enforce rules a type cannot:

- no raw SQL outside `crates/migration/src/pg_ddl.rs`
- no `Alias::new` — table and column names are `DeriveIden` types
- **no `unwrap_or`/`unwrap_or_default` anywhere under `pipeline/`** — a fallback value in the
  pipeline fabricates data, so absence must be expressed in the type or handled explicitly

Integration tests under `tests/integration/` need a live database and skip without one; the
rest run anywhere.
