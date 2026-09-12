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
| 2 | `parse <pages.jsonl> [entries.jsonl]` | Classify lines, assemble entries, apply signed scan readings. Gates ①②③⑧ → `gates-parse.json` |
| 3 | `reconcile <entries.jsonl> <Mục Từ.xlsx>` | Cross-check the index, check order, check the print's own errata. Gates ④⑤⑦ → `gates.json` |
| 4 | `witness <entries.jsonl> <wikisource/pages.jsonl> [review-dir]` | Compare against the 1895 edition. Gate ⑥ |
| 5 | `dnqatv-font <in.pdf> <entries.jsonl> frontend/fonts` | Subset the fonts the print uses |
| 6 | `import <entries.jsonl> [pages.jsonl]` | Load into PostgreSQL — refuses unless `gates.json` passes |

Step 4 needs a one-time snapshot of the other edition:

```
node tools/fetch-wikisource.mjs        # writes data/wikisource/, safe to re-run
```

One more thing is not a step but a pair of eyes. Every dossier row under `review/` ends by
asking a person to open the page image and look, so `tools/scan-page.mjs` renders a page of
the 1895-96 scan as a PNG — CCITT Group-4 decoder included, no dependencies:

```
node tools/scan-page.mjs 1 15 saisot.png                    # a whole page
node tools/scan-page.mjs 1 190 co.png 0.5 0.36 0.9 0.47     # one entry; the box is fractions
node tools/check-scan-page.mjs                              # its own checks
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
| ⑥ | A second transcription of the 1895 original does not contradict our reading |
| ⑦ | The corrections the print prints about itself are honoured, or a person has said why not |
| ⑧ | Every reading settled against the scan still finds the wording on the page it names |

Gate ⑥ is deliberately **not** pass/fail. The 1895 and 2026 editions genuinely differ, so
demanding an exact match would either cry wolf or invite someone to soften the threshold. It
sorts findings into kinds and writes them to `review/` — see `review/README.md`.

## How independent the witnesses really are

Gate ⑥ was built calling the Wikisource transcription an *independent* witness. It is not,
and the measurement is not close:

| Measured over the whole book | |
|---|---|
| distinct Han-Nôm glyphs used as a headword | ours 4,673 · theirs 4,673 · **shared 4,662** |
| rare glyphs (Ext-B/C-F, PUA) occurring **exactly once** on both sides | **716 of 721 identical** |
| Private-Use code points (U+F0036, U+F0320, U+F13A3 …) | **31 of 32 identical**, same reading, same page |

Two people transcribing a blurred 1895 print do not agree on 721 rare Nôm characters, still
less land on the same private-use numbers, which carry no standard meaning at all. The 2026
PDF says as much in its own metadata: *"Original 1038-page layout … preserved; dictionary
searchable text **rebuilt**"*. The two share an ancestor.

So gate ⑥ can still see where the two editions **drifted apart**, and that is worth having.
It cannot see anything they got wrong **together** — and they do. `故 Cô` glossed *"chị em bên
cha"* and `姑 Cố` glossed *"sự cớ; cũ càng"* are the same two sorts swapped, in both, exactly
as the 1895 typesetter set them.

That leaves two witnesses that really are independent of the 2026 text:

1. **the ink** — `docs/VN - … - 1.pdf` and `- 2.pdf`, 1765×2480 bitonal at about 270 dpi, in
   which an individual Han glyph is legible. (The third scan, `… HTC.pdf`, is 700×1050 RGB;
   it is 145 MB of nothing this project can use.) `tools/scan-page.mjs` opens it, and a reading
   settled that way goes into `review/scan-verified.toml`, the one dossier that changes the
   published text — and only for rows somebody has signed.
2. **the author** — the print corrects itself in `SAI SÓT` (volume 1) and `ĐÍNH NGOA 訂訛`
   (volume 2). Those 68 corrections are transcribed in `review/errata-ban-in.toml` and gate ⑦
   checks the data against them. The spreadsheet is not a third witness: `reconcile` records
   that it was generated from the PDF's own text layer.

### What that looks like in practice

Page 861 read `thế giới`, twice, in a sub-entry of `千 Thiên`. Both the 2026 text layer and the
1895 transcription say so, and the Wikisource page carrying it is unproofread — so gate ⑥ was
green and had nothing to say. The print reads `thế giái` both times.

What raised the suspicion was **the book disagreeing with itself**: `thế giái` appears 13 other
times, the headword 界 is read *Giái*, and no entry anywhere reads 界 *Giới*. What settled it was
opening volume 2, image 400.

That kind of check does not generalise into a gate, and the measurement says why: the book
genuinely mixes `sanh`/`sinh` (68% modern), `chánh`/`chính` (75%) and `thiệt`/`thật` (45%)
throughout. Flagging every compound spelled two ways yields 51 candidates of which nearly all
are the author's own orthography. One hit and fifty cries of wolf is not a gate worth having —
so this stays a reader's judgement, with a tool for looking and a dossier for recording.

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

and `node tools/check-comment-language.mjs`, which enforces the one rule a grep cannot: **every
comment is English**. The book stays Vietnamese wherever a reader meets it, and a comment may
quote it freely, but the sentence around the quote is English — see `CLAUDE.md`. `review/` is
exempt: those dossiers are written for the person filling them in.

Integration tests under `tests/integration/` need a live database and skip without one; the
rest run anywhere.
