#!/usr/bin/env node
//! Build the standalone static edition under `static-site/`.
//!
//! This is a SEPARATE site from the Rust server. It shares no code with it — no Tera, no
//! API, no database. It reads exactly the files `pipeline/import` reads and writes plain
//! JSON that a browser can fetch from any static host.
//!
//! The repository rule applies here too: **nothing may be guessed**. Every derivation below
//! is either a rule copied verbatim from the Rust source (each marked with its origin) or an
//! explicit failure. There is no fallback value anywhere in this file: an input the program
//! does not understand stops the build instead of producing a plausible-looking site.
//!
//! Usage: node tools/build-static-site.mjs

import { readFileSync, writeFileSync, mkdirSync, copyFileSync, rmSync, existsSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const OUT = join(ROOT, 'static-site');

/** Entries per detail shard. Small enough that opening one entry never pulls the book. */
const SHARD_SIZE = 100;

/** `pipeline/import`: front matter is everything up to and including this PDF page. */
const FRONT_MATTER_LAST_PAGE = 10;
/** `pipeline/import`: the y-coordinate of the running head line. */
const RUNNING_HEAD_Y = 797.8898;
const Y_EPSILON = 0.01;
/** `pipeline/import::is_heading`: a title is at most this long. */
const HEADING_MAX_CHARS = 40;

function die(message) {
  console.error(`error: ${message}`);
  process.exit(1);
}

function readLines(path) {
  if (!existsSync(path)) {
    die(`${path} is missing. Run the extract/parse pipeline first — data/ is not in git.`);
  }
  return readFileSync(path, 'utf8').split('\n').filter((l) => l.length > 0);
}

// ── Text rules, copied from crates/core/src/text/normalize.rs ────────────────

/** `fold`: strip diacritics and lowercase. `đ` is U+0111 and NFD does not decompose it. */
function fold(s) {
  return [...s.normalize('NFD')]
    .filter((c) => !(c >= '̀' && c <= 'ͯ'))
    .map((c) => (c === 'đ' ? 'd' : c === 'Đ' ? 'D' : c))
    .join('')
    .toLowerCase();
}

// ── The 22 CHỮ sections, from crates/core/src/model/letter.rs ────────────────

const LETTERS = ['A', 'B', 'C', 'D', 'Đ', 'E', 'G', 'H', 'Y', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'X'];
/** The URL segment for each letter. Đ is `dd` — a letter's label and its slug differ. */
const LETTER_SLUG = { A: 'a', B: 'b', C: 'c', D: 'd', 'Đ': 'dd', E: 'e', G: 'g', H: 'h', Y: 'y', K: 'k', L: 'l', M: 'm', N: 'n', O: 'o', P: 'p', Q: 'q', R: 'r', S: 's', T: 't', U: 'u', V: 'v', X: 'x' };

/** `Letter::from_initial`. Đ is checked BEFORE folding, since fold turns đ into d. */
function letterOf(reading) {
  const c = [...reading][0];
  if (c === undefined) die('an entry has an empty reading');
  if (c === 'Đ' || c === 'đ') return 'Đ';
  const base = fold(c)[0];
  // The book files I under CHỮ Y — measured across every entry, no exception.
  if (base === 'i' || base === 'y') return 'Y';
  const up = base === undefined ? '' : base.toUpperCase();
  if (!LETTERS.includes(up)) die(`reading "${reading}" starts with an initial the book has no section for`);
  return up;
}

// ── Slugs, from crates/core/src/model/slug.rs ────────────────────────────────

/** `Slug::stem`: the accent-folded reading, non-alphanumerics collapsed to one `-`. */
function slugStem(reading) {
  let out = '';
  let lastWasSep = true;
  for (const c of fold(reading)) {
    if ((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9')) {
      out += c;
      lastWasSep = false;
    } else if (!lastWasSep) {
      out += '-';
      lastWasSep = true;
    }
  }
  while (out.endsWith('-')) out = out.slice(0, -1);
  if (out.length === 0) die(`reading "${reading}" yields an empty slug`);
  return out;
}

/** `SlugMinter`: first entry with a reading keeps the stem, later ones get -2, -3 … in book order. */
function mintSlugs(records) {
  const seen = new Map();
  return records.map((r) => {
    const stem = slugStem(r.reading);
    const n = (seen.get(stem) ?? 0) + 1;
    seen.set(stem, n);
    return n === 1 ? stem : `${stem}-${n}`;
  });
}

// ── Part-of-speech labels, from crates/core/src/model/pos.rs ─────────────────

const POS_BOOK_LABEL = { chu_nho: 'c.', chu_nom: 'n.', chu_nho_dung_nom: 'cn.' };
const POS_DISPLAY_NAME = { chu_nho: 'chữ nho', chu_nom: 'chữ nôm', chu_nho_dung_nom: 'chữ nho dùng nôm' };

function posLabel(pos) {
  return pos.map((p) => POS_BOOK_LABEL[p] ?? die(`unknown part-of-speech label "${p}"`)).join(' ');
}
function posName(pos) {
  return pos.map((p) => POS_DISPLAY_NAME[p] ?? die(`unknown part-of-speech label "${p}"`)).join(' · ');
}

// ── Glyph kinds, from crates/core/src/model/glyph.rs ─────────────────────────

/** `GlyphKind::from_codepoint`, returning the CSS class the stylesheet already uses. */
function glyphKind(ch) {
  const cp = ch.codePointAt(0);
  if ((cp >= 0xe000 && cp <= 0xf8ff) || (cp >= 0xf0000 && cp <= 0x10fffd)) return 'pua';
  if (cp >= 0x20000 && cp <= 0x3ffff) return 'ext_b';
  return 'bmp';
}

// ── A minimal TOML reader for the review dossiers ────────────────────────────
//
// The dossiers use exactly one shape: `[[table]]` blocks of `key = value` lines, where a
// value is a basic string or an integer. Anything else stops the build rather than being
// skipped — a silently dropped dossier row means a warning that never reaches a reader.

function parseDossier(path) {
  const out = [];
  let current = null;
  const lines = readFileSync(path, 'utf8').split('\n');
  for (const [i, raw] of lines.entries()) {
    const line = raw.trim();
    if (line.length === 0 || line.startsWith('#')) continue;
    if (line.startsWith('[[')) {
      current = {};
      out.push(current);
      continue;
    }
    const eq = line.indexOf('=');
    if (eq < 0) die(`${path}:${i + 1}: expected \`key = value\``);
    if (current === null) die(`${path}:${i + 1}: a key outside any [[table]] block`);
    const key = line.slice(0, eq).trim();
    const value = line.slice(eq + 1).trim();
    if (value.startsWith('"')) {
      if (!value.endsWith('"') || value.length < 2) die(`${path}:${i + 1}: unterminated string`);
      current[key] = unescapeToml(value.slice(1, -1), path, i + 1);
    } else if (/^-?\d+$/.test(value)) {
      current[key] = Number(value);
    } else {
      die(`${path}:${i + 1}: value "${value}" is neither a basic string nor an integer`);
    }
  }
  return out;
}

function unescapeToml(s, path, lineNo) {
  let out = '';
  for (let i = 0; i < s.length; i++) {
    if (s[i] !== '\\') {
      out += s[i];
      continue;
    }
    i++;
    const c = s[i];
    if (c === '\\') out += '\\';
    else if (c === '"') out += '"';
    else if (c === 'n') out += '\n';
    else if (c === 't') out += '\t';
    else die(`${path}:${lineNo}: unsupported escape \\${c}`);
  }
  return out;
}

// ── Front matter and running heads, from pipeline/import/src/main.rs ─────────

function isHeading(text) {
  const chars = [...text];
  return (
    chars.length <= HEADING_MAX_CHARS &&
    chars.some((c) => /\p{Alphabetic}/u.test(c)) &&
    !chars.some((c) => /\p{Lowercase}/u.test(c))
  );
}

function lineText(line) {
  return line.segments.map((s) => s.text).join('').trim();
}

function readPages(path) {
  const frontMatter = [];
  const runningHeads = new Map();
  for (const raw of readLines(path)) {
    const p = JSON.parse(raw);
    let left = null;
    let right = null;
    const body = [];
    for (const l of p.lines) {
      if (Math.abs(l.y - RUNNING_HEAD_Y) < Y_EPSILON) {
        const t = lineText(l);
        if (t.length === 0) continue;
        if (l.column === 'left') left = t;
        else right = t;
      } else {
        const t = lineText(l);
        if (t.length > 0) body.push(t);
      }
    }
    runningHeads.set(p.pdf_page, [left, right]);
    if (p.pdf_page <= FRONT_MATTER_LAST_PAGE && body.length > 0) {
      // The title is the first short all-caps line. Without one, the page number —
      // a dull title beats an invented one.
      const heading = body.find(isHeading);
      frontMatter.push({
        slug: `trang-${p.pdf_page}`,
        title: heading === undefined ? `Trang ${p.pdf_page}` : heading,
        body,
        pdf_page: p.pdf_page,
      });
    }
  }
  return { frontMatter, runningHeads };
}

// ── The gap font's unicode-range, read from its own cmap ─────────────────────
//
// `pipeline/font` writes cmap format 12 only. A file whose range cannot be read is left
// UNDECLARED rather than given a guessed range: an @font-face claiming characters the file
// does not hold makes the browser show nothing instead of falling through to a font that has them.

function cmapCodepoints(path) {
  const data = readFileSync(path);
  const numTables = data.readUInt16BE(4);
  let cmapOffset = -1;
  for (let i = 0; i < numTables; i++) {
    const rec = 12 + i * 16;
    if (data.toString('latin1', rec, rec + 4) === 'cmap') cmapOffset = data.readUInt32BE(rec + 8);
  }
  if (cmapOffset < 0) return null;
  const subTables = data.readUInt16BE(cmapOffset + 2);
  for (let i = 0; i < subTables; i++) {
    const rec = cmapOffset + 4 + i * 8;
    const sub = cmapOffset + data.readUInt32BE(rec + 4);
    if (data.readUInt16BE(sub) !== 12) continue;
    const groups = data.readUInt32BE(sub + 12);
    const out = [];
    for (let g = 0; g < groups; g++) {
      const rec2 = sub + 16 + g * 12;
      const lo = data.readUInt32BE(rec2);
      const hi = data.readUInt32BE(rec2 + 4);
      for (let cp = lo; cp <= hi; cp++) out.push(cp);
    }
    return out.length > 0 ? out : null;
  }
  return null;
}

/** The font files, mirroring crates/adapter-web/src/assets.rs. */
const FONT_RANGES = [
  { stem: 'NomNaTong-bmp', family: 'Nom Na Tong', fixed: 'U+3400-4DBF, U+4E00-9FFF, U+F900-FAFF' },
  { stem: 'NomNaTong-extb', family: 'Nom Na Tong', fixed: 'U+20000-2FFFF, U+30000-3FFFF' },
  { stem: 'NomNaTong-pua', family: 'Nom Na Tong', fixed: 'U+E000-F8FF, U+F0000-FFFFD' },
  { stem: 'BabelStoneHan-gap', family: 'BabelStone Han', fixed: null },
];
const FONT_FORMATS = [['woff2', 'woff2'], ['ttf', 'truetype']];

function buildFonts(fontDir, outDir) {
  mkdirSync(outDir, { recursive: true });
  const faces = [];
  let preload = null;
  for (const range of FONT_RANGES) {
    for (const [ext, cssFormat] of FONT_FORMATS) {
      const name = `${range.stem}.${ext}`;
      const src = join(fontDir, name);
      if (!existsSync(src)) continue;
      let unicodeRange = range.fixed;
      if (unicodeRange === null) {
        const cps = cmapCodepoints(src);
        if (cps === null) {
          console.warn(`  ${name}: cmap unreadable, left undeclared`);
          break;
        }
        unicodeRange = cps.map((cp) => `U+${cp.toString(16).toUpperCase()}`).join(', ');
      }
      copyFileSync(src, join(outDir, name));
      // font-display: block — with Han characters a reader would believe the ▯ is the glyph.
      faces.push(
        `@font-face {\n  font-family: "${range.family}";\n  src: url("fonts/${name}") format("${cssFormat}");\n  unicode-range: ${unicodeRange};\n  font-display: block;\n}`,
      );
      // The @font-face url is relative to the stylesheet (tinh/main.css); the preload tag
      // is relative to the shell (index.html). Two different bases, two different strings.
      if (range.stem === 'NomNaTong-bmp') preload = { href: `tinh/fonts/${name}`, mime: ext === 'woff2' ? 'font/woff2' : 'font/ttf' };
      break;
    }
  }
  return { faces, preload };
}

// ── Build ────────────────────────────────────────────────────────────────────

function main() {
  const records = readLines(join(ROOT, 'data/entries.jsonl')).map((l) => JSON.parse(l));
  console.log(`entries        ${records.length}`);

  for (const r of records) {
    if (r.schema_version !== 1) die(`entry ${r.seq} has schema_version ${r.schema_version}, expected 1`);
  }

  const { frontMatter, runningHeads } = readPages(join(ROOT, 'data/pages.jsonl'));
  console.log(`front matter   ${frontMatter.length} pages`);

  // ── The review dossiers, matched by exactly the rules pipeline/import uses ──
  const typoLines = parseDossier(join(ROOT, 'review/label-typos.toml')).map((t) => [t.pdf_page, t.line]);
  const ambiguous = parseDossier(join(ROOT, 'review/ambiguous-sub-entries.toml'))
    .map((a) => [a.pdf_page, a.text.trim()])
    .filter(([, t]) => t.length > 0);
  // Only a SIGNED row becomes this edition's statement; an unsigned one reaches no reader.
  const shapeNotes = new Map(
    parseDossier(join(ROOT, 'review/witness-shape-note-available.toml'))
      .filter((s) => s.verified_by.length > 0)
      .map((s) => [`${s.printed_page}|${s.reading}`, s.theirs]),
  );
  console.log(`dossiers       ${typoLines.length} label typos · ${ambiguous.length} ambiguous lines · ${shapeNotes.size} signed shape notes`);

  // The 1895 ↔ modern orthography bridge. It SUGGESTS and never rewrites: most of these
  // pairs are two separate entries in the book, each with its own definition.
  const bridge = parseDossier(join(ROOT, 'review/orthography-bridge.toml')).map((p) => ({
    old: p.old,
    new: p.new,
    note: p.note,
    verified: p.verified_by.trim().length > 0,
  }));
  console.log(`bridge         ${bridge.length} pairs · ${bridge.filter((p) => !p.verified).length} unverified`);

  const slugs = mintSlugs(records);

  const index = [];
  const details = [];
  let needingReview = 0;
  let subCount = 0;
  let subNeedingReview = 0;
  let imageOnly = 0;

  for (const [i, r] of records.entries()) {
    const letter = letterOf(r.reading);

    // A sub-entry holding an ambiguous line carries the mark itself, so it sits next to the
    // suspect passage. The page window is ±1: an entry begun on one page runs on to the next.
    const subAmbiguous = r.sub_entries.map((s) =>
      ambiguous.some(([page, text]) => Math.abs(page - r.pdf_page) <= 1 && (s.definition.includes(text) || s.form.includes(text))),
    );
    // The headword flag is about the HEADWORD LINE only. A sub-entry needing review carries
    // its own mark; merging the two would warn on hundreds of perfectly sound heads.
    const hasTypo = typoLines.some(([page, line]) => page === r.pdf_page && line.includes(r.reading));
    const hasAmbiguous = ambiguous.some(([page, text]) => Math.abs(page - r.pdf_page) <= 1 && r.gloss.includes(text));
    const needsReview = hasTypo || hasAmbiguous;
    if (needsReview) needingReview += 1;
    if (r.glyph === null) imageOnly += 1;

    index.push({
      slug: slugs[i],
      seq: r.seq,
      reading: r.reading,
      alt: r.alternate,
      glyph: r.glyph,
      kind: r.glyph === null ? 'image_only' : glyphKind(r.glyph),
      posLabel: posLabel(r.pos),
      posName: posName(r.pos),
      gloss: r.gloss,
      letter,
      pdf: r.pdf_page,
      pp: r.printed_page,
      rv: needsReview,
      subs: r.sub_entries.length,
    });

    const shapeNote = r.glyph === null ? (shapeNotes.get(`${r.printed_page}|${r.reading}`) ?? null) : null;
    details.push({
      inherits: r.inherits_glyph,
      shapeNote,
      subs: r.sub_entries.map((s, k) => {
        subCount += 1;
        const rv = s.needs_review || subAmbiguous[k];
        if (rv) subNeedingReview += 1;
        return {
          han: s.han_form,
          hanEx: s.han_expanded,
          form: s.form,
          formEx: s.form_expanded,
          def: s.definition,
          rv,
        };
      }),
    });
  }

  // ── Write ─────────────────────────────────────────────────────────────────
  const dataDir = join(OUT, 'du-lieu');
  rmSync(join(dataDir, 'muc'), { recursive: true, force: true });
  mkdirSync(join(dataDir, 'muc'), { recursive: true });

  const written = [];
  const write = (rel, text) => {
    const path = join(OUT, rel);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, text);
    written.push([rel, Buffer.byteLength(text), gzipSync(Buffer.from(text), { level: 9 }).length]);
  };

  write('du-lieu/tra.json', JSON.stringify(index));

  const shardCount = Math.ceil(details.length / SHARD_SIZE);
  for (let s = 0; s < shardCount; s++) {
    write(`du-lieu/muc/${String(s).padStart(4, '0')}.json`, JSON.stringify(details.slice(s * SHARD_SIZE, (s + 1) * SHARD_SIZE)));
  }

  // The full-text table, fetched only when the reader picks "Toàn văn". It holds the
  // FOLDED text: a reader types "nam guom" and must still reach "nạm gươm".
  const fullText = records.map((r) =>
    fold([r.gloss, ...r.sub_entries.flatMap((s) => [s.form_expanded, s.definition])].join(' ')),
  );
  write('du-lieu/toan-van.json', JSON.stringify(fullText));

  write(
    'du-lieu/gioi-thieu.json',
    JSON.stringify(frontMatter.map((f) => ({ slug: f.slug, title: f.title, body: f.body, pdf: f.pdf_page }))),
  );

  // Running heads, for the printed-page view. Only pages that actually carry entries.
  const heads = {};
  for (const e of index) {
    if (heads[e.pdf] === undefined) {
      const h = runningHeads.get(e.pdf);
      heads[e.pdf] = h === undefined ? [null, null] : h;
    }
  }
  write('du-lieu/trang.json', JSON.stringify(heads));

  // ── Stylesheet and fonts ──────────────────────────────────────────────────
  // Done before the manifest, because the manifest must state whether a Nôm font was
  // actually shipped: without one the 31 PUA glyphs are squares on every machine, and the
  // page has to say so rather than let the reader think the data is broken.
  const { faces, preload } = buildFonts(join(ROOT, 'frontend/fonts'), join(OUT, 'tinh/fonts'));
  const baseCss = readFileSync(join(ROOT, 'frontend/styles/main.css'), 'utf8');
  const fontCss =
    faces.length === 0
      ? '/* No font file shipped under tinh/fonts/, so no @font-face is declared. */\n'
      : `/* Generated from the font files present on disk — see tools/build-static-site.mjs. */\n${faces.join('\n')}\n`;
  write('tinh/main.css', `${fontCss}\n${baseCss}`);
  write('tinh/favicon.svg', readFaviconSvg());
  writeFileSync(join(OUT, '.nojekyll'), '');

  const gates = JSON.parse(readFileSync(join(ROOT, 'data/gates.json'), 'utf8'));
  write(
    'du-lieu/manifest.json',
    JSON.stringify({
      builtAt: new Date().toISOString().slice(0, 10),
      shardSize: SHARD_SIZE,
      shardCount,
      letters: LETTERS.map((l) => ({ label: l, slug: LETTER_SLUG[l] })),
      bridge,
      hasNomFont: faces.length > 0,
      stats: {
        entries: index.length,
        subEntries: subCount,
        glyphs: new Set(records.filter((r) => r.glyph !== null).map((r) => r.glyph)).size,
        pages: Object.keys(heads).length,
        entriesNeedingReview: needingReview,
        subEntriesNeedingReview: subNeedingReview,
        imageOnlyGlyphs: imageOnly,
      },
      gates,
    }),
  );

  // The preload tag is written into the shell, and only when the file really exists.
  const shellPath = join(OUT, 'index.html');
  if (existsSync(shellPath)) {
    const shell = readFileSync(shellPath, 'utf8');
    const tag =
      preload === null
        ? '<!-- no font shipped, so nothing is preloaded -->'
        : `<link rel="preload" href="${preload.href}" as="font" type="${preload.mime}" crossorigin>`;
    const marked = shell.replace(/<!--FONT_PRELOAD-->[\s\S]*?<!--\/FONT_PRELOAD-->/, `<!--FONT_PRELOAD-->${tag}<!--/FONT_PRELOAD-->`);
    if (marked !== shell) writeFileSync(shellPath, marked);
  }

  // ── Report ────────────────────────────────────────────────────────────────
  let raw = 0;
  let gz = 0;
  for (const [, r, g] of written) {
    raw += r;
    gz += g;
  }
  const shards = written.filter(([rel]) => rel.startsWith('du-lieu/muc/'));
  const shardRaw = shards.reduce((a, [, r]) => a + r, 0);
  const shardGz = shards.reduce((a, [, , g]) => a + g, 0);
  const kb = (n) => `${(n / 1024).toFixed(0)} KB`;

  console.log('');
  for (const [rel, r, g] of written) {
    if (rel.startsWith('du-lieu/muc/')) continue;
    console.log(`  ${rel.padEnd(24)} ${kb(r).padStart(9)}  ${kb(g).padStart(8)} gz`);
  }
  console.log(`  ${`du-lieu/muc/ (${shards.length})`.padEnd(24)} ${kb(shardRaw).padStart(9)}  ${kb(shardGz).padStart(8)} gz`);
  const fontBytes = FONT_RANGES.map((r) => join(OUT, `tinh/fonts/${r.stem}.ttf`))
    .filter(existsSync)
    .reduce((a, p) => a + statSync(p).size, 0);
  console.log(`  ${'tinh/fonts/'.padEnd(24)} ${kb(fontBytes).padStart(9)}`);
  console.log('');
  console.log(`total ${written.length + 1} files · ${kb(raw + fontBytes)} · ${kb(gz + fontBytes)} over the wire`);
  console.log(`largest single fetch: ${kb(Math.max(...written.map(([, , g]) => g)))} gz`);
}

/** The site icon, copied from crates/adapter-web/src/assets.rs. */
function readFaviconSvg() {
  const src = readFileSync(join(ROOT, 'crates/adapter-web/src/assets.rs'), 'utf8');
  const m = src.match(/FAVICON_SVG: &str = r##"([\s\S]*?)"##/);
  if (m === null) die('FAVICON_SVG not found in crates/adapter-web/src/assets.rs');
  return m[1];
}

main();
