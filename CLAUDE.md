# Conventions

`README.md` explains what the pipeline does. This file is how to work here — the rules that were
paid for with mistakes, so they need not be paid for twice.

Full plan (approved, outside the repo, so it never loads on its own):
`C:\Users\trung\.claude\plans\t-i-c-c-c-t-i-mighty-ember.md`

## Comments are English. Always.

Two layers speak Vietnamese in this repository and must not be confused:

- **The book** — headwords, definitions, the words of the 1895 print, and everything written
  for the person looking a word up. This layer is Vietnamese, in the author's own Nam Kỳ
  spelling, and stays Vietnamese even inside `.rs` (`SearchMode::label()`,
  `WebError::public_message()`, `GlyphView::aria_label`, the `openapi.rs` descriptions, the
  text in `frontend/templates/`).
- **The program** — everything one programmer writes for the next. English, without
  exception.

**A comment is never the book.** Every `//`, `///`, `//!`, `/* */`, `<!-- -->`, `{# #}` and
`#` comment is English — in `.rs`, `.js`, `.mjs`, `.css`, `.html`, `.toml` and the workflow
files alike. So are the operator-facing strings: `#[error(...)]`, `bail!`, `tracing::*`,
`.expect()`/`.context()`, CLI help, gate labels.

A comment may **quote** the book freely, and often must — `― gươm`, "chữ đầu", *quấc*, a
fenced specimen of sub-entry lines. Quoting is citing a word; the sentence around it is
English.

The one exemption is `review/`. Those `.toml` files are dossiers a **person** fills in: the
header explains what they are being asked to settle and `reason` rows are their own words.
Translating them would move the cost onto the only human in the loop.

### Enforced by

```
node tools/check-comment-language.mjs [paths…]
```

It reads the comments out of every tracked file and looks for Vietnamese **grammar** — the
function words (`là`, `của`, `không`, `được` …) that appear when a sentence is spoken rather
than a word cited — after dropping backticks, quotes, «» , *emphasis* and fenced blocks. A
smoke alarm, not a proof: a two-word Vietnamese comment carrying no function word gets
through. It catches the paragraph, which is what actually accumulates. CI runs it.

## Nothing may be guessed

The rule the repository is arranged around, and the reason for the gates. For a dictionary,
invented data is worse than missing data: a silent error propagates into every view and nobody
ever sees it.

- **No fallback when decoding.** An unmapped glyph code is a hard error, never
  `String.fromCharCode(code)` or `unwrap_or_default()`.
- **A quality number must be a conservation law, not a coverage ratio.** "92% of entries have a
  definition" hid a bug that lost **355,833 characters (16.65%)**; `Σ(fields) == source` said
  everything that ratio could not.
  > A conservation law catches text that is **lost**, not text that is **misattributed**. It is
  > necessary, not sufficient — gate ③ reported 2 bad lines while 548 were in fact mis-split.
- **Every deviation is either fixed or gets a dossier row in `review/`, with a reason a person
  wrote.** The allowed threshold **is** the dossier size (`pipeline/gate-report`). There is no
  number to edit, and deleting a dossier row turns the gate red too.
- **Fail closed.** `import` refuses to load unless `gates.json` is green *and* was produced from
  the file being loaded — a stale green report is more dangerous than none.
- Never render a character the print did not set. All 43 alternate readings already carry their
  own parentheses; wrapping them again produced `A ((Nha.))` — the display layer inventing text.

## Measure before asserting

When Trung pushes back on a claim, **assume he is right and go and verify**. Every time this has
happened, measuring produced the opposite answer and a better design.

Claims that stood until measured, then fell: "tests cannot live in a separate folder", "private
means untestable", "printed page = PDF page − 1, continuously", "no font supports that
character" — that last one while the font sat embedded in our own PDF, 15.9 MB, complete.

Before stating anything about an API, a tool's behaviour, or a number about the book: run it,
read the source, measure it. Numbers about the dictionary come from `data/*.jsonl`, never from
memory.

## The reader's Vietnamese is the book's Vietnamese

Strings a reader sees prefer, in order: (1) the book's own word, (2) pre-1975 southern
Vietnamese, (3) anything but a post-1975 technical or administrative coinage.

**Look the word up before writing it.** `data/entries.jsonl` (7,687 entries) and
`data/pages.jsonl` (all 1,038 pages, front matter included) are both searchable.

The author names his own machinery, so use his words: **chữ đầu** (entry), **dấu riêng** (part
of speech), **chữ ta** (the Quốc ngữ reading), **thứ lớp** (order), **tra tìm** (lookup),
**chú giải** (gloss). He spends the whole TIỂU TỰ arguing against the word *tự điển*: this book
is a **tự vị**, never a *từ điển*.

Three counting traps, each hit at least once:

1. Do not join the book into one string and `split` — it matches across entry boundaries and
   reports words the book does not contain. Count field by field.
2. `pdf_page` 1–10 is not all the author: **page 10 is the 2026 editors** and pages 6–7 are
   C. Cotel's French. Only pages 3, 4 (TIỂU TỰ) and 5 (DẤU RIÊNG) are Huình-Tịnh Của.
3. A word being present does not mean the sense you need is present: `mặc định` (p.201) means
   "mặc lỏng", `sự cố` (p.161) means "duyên cớ".

Settled with evidence, do not re-litigate: keep `này` (the author's own prose uses it), `vào`,
`mục con`, `tự dạng`, `dữ liệu`, `khả nghi`, and `bản in` — the book has that last one, so
"bổn in" would itself be a guess.

## Architecture

Hexagonal, because **Cargo's dependency graph enforces it and a convention does not**: `core`
does not declare `sea-orm`, so persistence leaking into the domain fails to compile.

- Ports speak the domain's language, never SQL.
- **Five type families, kept apart**: `core::model` · `entity::Model` · `dto::v1` ·
  `adapter-web::view` · pipeline records. `TryFrom` for DB → domain, because a row that breaks
  an invariant must fail loudly; `From` for domain → DTO and view.
- **No magic strings.** Table and column names are `DeriveIden` enums; Postgres enum types are
  generated by iterating `Pos::ALL` / `Letter::ALL` / `GlyphKind::ALL`, so a new variant reaches
  the migration on its own and `match` exhaustiveness finds every other site.
- `crates/migration/src/pg_ddl.rs` is the **only** module allowed to execute raw SQL: three
  statements no builder can express, each snapshot-tested, plus a test asserting there are
  exactly three.
- Grep gates in CI enforce the above, and they have caught real bugs in code written here —
  `partial_cmp(..).unwrap_or(Equal)` in the line sorter, where a NaN coordinate would have
  silently merged two printed lines, and eight `try_from(..).unwrap_or(0)` in the font subsetter.
- `unsafe_code = "forbid"`, `unwrap/expect/panic = "deny"`, relaxed for tests only via
  `clippy.toml` — in a test, panicking is the correct behaviour.

## Tests

- **Every test file lives under `tests/`**, attached with `[[test]] path = …`. Nothing uses
  `#[cfg(test)] #[path]`: whatever is worth testing was extracted into a pure `*-core` library
  where being `pub` is legitimate. The crate boundary is the encapsulation boundary.
- **Write the test first where the answer is already known** — value objects, the two derivation
  rules, ranking policy, the gates. **Characterize instead where the work is discovery**: PDF
  extraction is exploration, and a test written first there only freezes an assumption.
- Green unit tests are not enough; run the gates. The 355,833-character bug passed every unit
  test that existed.
- **Prove a test bites.** Break what it guards, watch it go red, restore. This is not optional:
  two tests written in one sitting were vacuous until deliberately broken — one of them matched
  the string `section--prose` inside a *comment* that explained the rule.
- **Strip comments before asserting on CSS or source text.** The same trap produced a false
  green and later a false red; `without_comments()` in `tests/unit/web/templates.rs` exists for
  it. A plain `grep` for a symbol will happily match the comment that forbids it.

## This machine

- **`python` / `python3` is a Microsoft Store stub that HANGS.** Never call it. Use `node`.
- No `jq`, `rg`, `bc`. Sum with `awk`.
- `grep -r` from the repo root is far too slow (`data/` is 25 MB, plus `target/`) — name a
  subdirectory, or use the harness Grep tool.
- For data munging, write a `.mjs` into the scratchpad and run it with `node`.

## Editing discipline

A careless slice-based string edit once **deleted 11 handlers** from a file that was not open,
at a time when the repository had no git history to recover from.

- Prefer Edit/Write over string surgery on source files. When a script must patch, **assert the
  anchor matched** and check the result — a silent no-op replacement has wasted time repeatedly.
- Never slice a file using an index computed from a pattern that may occur more than once.

## Frontend rules that came from real bugs

- **Never point at a file that is not there.** `@font-face` fails silently, but
  `<link rel="preload">` 404s loudly on every page load. `FontSet::scan` looks at the disk
  before anything is declared.
- Every `grid-area: <name>` needs a matching `grid-template-areas` **in the same scope**. A
  dangling name raises no error — the browser quietly puts the element in an implicit row.
- A grid cell holding `nowrap` text needs `min-width: 0`; `1fr` has a `min-content` minimum and
  will push the whole page into horizontal scroll.
- One `<h1>` per page, describing *that* page. The masthead repeats everywhere, so it is a `<p>`.
- A sticky element covers whatever shares its column below it. Spacing cannot fix overlap: the
  sticky element moves and the gap does not.
- For layout, alignment and whitespace, **a screenshot from Trung outranks any reasoning about
  the CSS**. Three consecutive fixes derived from reading the code were wrong.

## Before calling it done

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                      # 378 green as of 2026-09-11
node tools/check-comment-language.mjs
node tools/check-static-site.mjs            # when static-site/ or tools/ changed
```

Plus the grep gates in `.github/workflows/ci.yml`.

If a server was started to check something, **stop it**; do not leave it running.

Report plainly: what was measured, what is still unverified, and what was left out.
