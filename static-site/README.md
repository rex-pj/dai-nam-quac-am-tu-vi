# The static edition

A standalone dictionary site. **No server, no API, no database, no Tera** — it shares no code
with the Rust application in `crates/`. Open `index.html` from any static host, or from
`file://`, and it works.

```
static-site/
  index.html          the shell: header, search box, 22 letter chips, footer
  tinh/app.js         routing, rendering and search — the whole application
  tinh/main.css       copied from frontend/styles/main.css, with @font-face prepended
  tinh/fonts/         the subsetted Nôm fonts of the print
  du-lieu/            the data, as plain JSON
```

## Regenerating

```
node tools/build-static-site.mjs     # writes du-lieu/, tinh/main.css, tinh/fonts/
node tools/check-static-site.mjs     # renders every route and every entry, outside a browser
```

The build reads `data/entries.jsonl`, `data/pages.jsonl`, `data/gates.json` and `review/*.toml`
— the same files `pipeline/import` reads. Those are **not in git**, so the build runs on a
machine that has them and the output is committed. That is why `.github/workflows/pages.yml`
publishes this directory rather than building it.

## Two decisions worth knowing

**Routing lives in the fragment** (`#/muc-tu/a`). A static host cannot rewrite paths. The usual
workaround — a `404.html` that swallows every unknown path — makes the server answer 404 for
pages that exist. A fragment is honest: one real document, addressed from inside. It also means
every path in here is relative, so the site works at a domain root and under `/<repo>/` alike;
the workflow fails the build if a root-absolute path appears.

**Search is this edition's own rule, not a copy of the Postgres one.** The six tiers come from
`core::search::RankTier`, but how they are computed does not: matching happens in the browser
over a folded table, and "toàn văn" is a substring scan rather than a text-search index. The
data-quality page says so on screen instead of implying the two rank alike.

## What is reproduced from the Rust source

Each of these is marked at its definition with the file it came from, so the two can be
compared by reading:

| Rule | Origin |
|---|---|
| `fold` — strip diacritics, `đ` → `d` | `core::text::normalize` |
| The 22 CHỮ sections and `from_initial` | `core::model::letter` |
| Slug minting, including the `-2`, `-3` suffixes | `core::model::slug` |
| Part-of-speech labels and their spelled-out names | `core::model::pos` |
| Glyph kinds by code point | `core::model::glyph` |
| The four placeholder characters and `split_form` | `adapter-web::view` |
| The orthography bridge — suggests, never rewrites | `app::bridge` |
| Which dossier rows flag an entry for review | `pipeline/import` |
| `@font-face` declared only for files that exist | `adapter-web::assets` |

## What it does not have

- **No page scans.** The 2026 edition is reset type, not a scan; rendering page images is a
  step that has not run. The printed-page view says so rather than showing an empty frame.
- **No shape notes.** Those come from signed rows in `review/witness-shape-note-available.toml`,
  and none is signed yet. When one is, it appears here with no code change.
- **No sitemap.** Fragment routes are one document to a crawler. A dictionary that wants to be
  found needs real per-entry URLs, which means a server or thousands of files.
