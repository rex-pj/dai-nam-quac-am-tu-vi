/* The standalone static edition of Đại Nam Quấc Âm Tự Vị.
 *
 * No server, no API, no build step at read time: this file fetches plain JSON written by
 * `tools/build-static-site.mjs` and renders it. It shares no code with the Rust server —
 * every rule reproduced here is marked with the file it was copied from, so the two can be
 * compared by reading rather than by trust.
 *
 * Routing is hash-based (`#/muc-tu/…`). A static host cannot rewrite paths, and the usual
 * workaround — a 404.html that swallows every unknown path — makes the server answer 404
 * for pages that exist. A fragment is honest: one real document, addressed from inside.
 *
 * The rule this whole repository is arranged around applies to the screen as well:
 * **nothing may be guessed.** Where the book's own words end and a derivation begins, the
 * page says so — the `―` placeholder keeps its verbatim form, and the expanded form is
 * marked as derived.
 */

const DATA = 'du-lieu';
/** Results per page. `core::search::Pagination::DEFAULT_LIMIT`. */
const PER_PAGE = 30;
/** pg_trgm's default similarity threshold, used for the "Gần giống" tier. */
const FUZZY_THRESHOLD = 0.3;
/** `app::service::MAX_SUGGESTIONS`. */
const MAX_SUGGESTIONS = 8;

// ── State ───────────────────────────────────────────────────────────────────

let manifest = null;
/** Every entry summary, in book order. */
let index = [];
/** The accent-folded reading of each entry, computed once at boot. */
let foldedReading = [];
/** The lowercased reading of each entry, for tone-sensitive comparison. */
let lowerReading = [];
let bySlug = new Map();
let byGlyph = new Map();
let byLetter = new Map();
let byPrintedPage = new Map();
/** Detail shards, fetched on demand and kept. */
const shardCache = new Map();
/** The full-text table — a separate fetch, only when the reader asks for Toàn văn. */
let fullText = null;
let frontMatter = null;

const main = document.querySelector('[data-main]');

// ── Escaping ────────────────────────────────────────────────────────────────
//
// Tera auto-escapes; this file must do the same by hand. `h` escapes every interpolation,
// and the only way to insert markup is to pass it through `raw` — so an unescaped string
// is always visible at the call site.

const RAW = Symbol('raw');

function esc(value) {
  return String(value).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
}

function raw(markup) {
  return { [RAW]: markup };
}

function h(strings, ...values) {
  let out = strings[0];
  for (const [i, value] of values.entries()) {
    if (value === null || value === undefined || value === false) {
      // Nothing — lets `cond && h\`…\`` read naturally.
    } else if (Array.isArray(value)) {
      out += value.map((v) => (v !== null && typeof v === 'object' && RAW in v ? v[RAW] : esc(v))).join('');
    } else if (typeof value === 'object' && RAW in value) {
      out += value[RAW];
    } else {
      out += esc(value);
    }
    out += strings[i + 1];
  }
  return raw(out);
}

// ── Text rules, copied from crates/core/src/text/normalize.rs ───────────────

/** `fold`: strip diacritics and lowercase. `đ` is U+0111; NFD does not decompose it. */
function fold(s) {
  return [...s.normalize('NFD')]
    .filter((c) => !(c >= '̀' && c <= 'ͯ'))
    .map((c) => (c === 'đ' ? 'd' : c === 'Đ' ? 'D' : c))
    .join('')
    .toLowerCase();
}

/** `core::search::is_han_nom` — the ranges this book uses. */
function isHanNom(c) {
  const cp = c.codePointAt(0);
  return (
    (cp >= 0x3400 && cp <= 0x4dbf) ||
    (cp >= 0x4e00 && cp <= 0x9fff) ||
    (cp >= 0xf900 && cp <= 0xfaff) ||
    (cp >= 0x20000 && cp <= 0x3ffff) ||
    (cp >= 0xe000 && cp <= 0xf8ff) ||
    (cp >= 0xf0000 && cp <= 0x10fffd)
  );
}

// ── Placeholders, copied from crates/adapter-web/src/view.rs ────────────────
//
// Four code points, not one. Measured across the book body: `―` U+2015 58,706 times,
// `|` 2,197, `—` U+2014 1,065, `–` U+2013 129. The DẤU RIÊNG page writes `—` while the
// typesetter used `―`; hard-coding one code point loses 540 lines.

const PLACEHOLDERS = ['―', '—', '–', '|'];
/** The two-dash form, checked BEFORE the single dash or it becomes two placeholders. */
const PLACEHOLDER_DIGRAPH = '--';

/** `split_form`: the pieces of a form, marking which ones are placeholder marks. */
function splitForm(form) {
  const parts = [];
  let buffer = '';
  let rest = form;
  const flush = () => {
    if (buffer.length > 0) {
      parts.push({ text: buffer, placeholder: false });
      buffer = '';
    }
  };
  while (rest.length > 0) {
    if (rest.startsWith(PLACEHOLDER_DIGRAPH)) {
      flush();
      parts.push({ text: PLACEHOLDER_DIGRAPH, placeholder: true });
      rest = rest.slice(PLACEHOLDER_DIGRAPH.length);
      continue;
    }
    const c = [...rest][0];
    if (PLACEHOLDERS.includes(c)) {
      flush();
      parts.push({ text: c, placeholder: true });
    } else {
      buffer += c;
    }
    rest = rest.slice(c.length);
  }
  flush();
  return parts;
}

// ── Search tiers, from crates/core/src/search.rs ────────────────────────────
//
// Declaration order IS ranking order. Tone marks are meaning-bearing — Ả, Á and À are three
// different entries — so an accented match must always beat an unaccented one.
//
// WHICH tier a mode consults is this edition's own rule, not a copy of the Postgres query
// plan. The data-quality page states the tier order and what "Toàn văn" actually matches, so
// a reader is told how results are ranked rather than left to assume.

const TIERS = [
  { key: 'trung-khit', label: 'Trùng khít' },
  { key: 'trung-tu-dang', label: 'Trùng tự dạng' },
  { key: 'bat-dau-bang', label: 'Bắt đầu bằng' },
  { key: 'bo-dau', label: 'Bỏ dấu' },
  { key: 'trong-loi-giai-nghia', label: 'Trong lời giải nghĩa' },
  { key: 'gan-giong', label: 'Gần giống' },
];
const TIER_RANK = Object.fromEntries(TIERS.map((t, i) => [t.key, i]));

const MODES = [
  { value: 'auto', label: 'Tự nhận' },
  { value: 'han-nom', label: 'Chữ Hán-Nôm' },
  { value: 'quoc-ngu', label: 'Âm Quốc ngữ' },
  { value: 'toan-van', label: 'Toàn văn' },
];

/** `SearchMode::resolve` — decided by character shape alone, never by the dictionary. */
function resolveMode(mode, query) {
  if (mode !== 'auto') return mode;
  return [...query].some(isHanNom) ? 'han-nom' : 'quoc-ngu';
}

/** pg_trgm-style trigrams: each word padded with two leading spaces and one trailing. */
function trigrams(s) {
  const out = new Set();
  for (const word of fold(s).split(/[^\p{Letter}\p{Number}]+/u).filter((w) => w.length > 0)) {
    const padded = `  ${word} `;
    for (let i = 0; i + 3 <= padded.length; i++) out.add(padded.slice(i, i + 3));
  }
  return out;
}

function similarity(a, b) {
  if (a.size === 0 || b.size === 0) return 0;
  let shared = 0;
  for (const t of a) if (b.has(t)) shared += 1;
  return shared / (a.size + b.size - shared);
}

/**
 * Search. Returns `{ groups, total, mode }`, each group `{ tier, entries }` in rank order.
 *
 * Every entry is placed in the BEST tier it matches, never in two.
 */
function search(rawQuery, requestedMode) {
  const query = rawQuery.trim().normalize('NFC');
  const mode = resolveMode(requestedMode, query);
  const lower = query.toLowerCase();
  const folded = fold(query);
  const hits = new Map();

  const place = (i, tierKey) => {
    const current = hits.get(i);
    if (current === undefined || TIER_RANK[tierKey] < TIER_RANK[current]) hits.set(i, tierKey);
  };

  if (mode === 'han-nom') {
    for (const [i, e] of index.entries()) {
      if (e.glyph !== null && e.glyph === query) place(i, 'trung-tu-dang');
    }
  } else if (mode === 'toan-van') {
    if (folded.length > 0) {
      for (const [i, text] of fullText.entries()) {
        if (text.includes(folded)) place(i, 'trong-loi-giai-nghia');
      }
    }
  } else {
    for (const [i, e] of index.entries()) {
      if (lowerReading[i] === lower) place(i, 'trung-khit');
      else if (lowerReading[i].startsWith(lower)) place(i, 'bat-dau-bang');
      else if (foldedReading[i] === folded) place(i, 'bo-dau');
      if (e.glyph !== null && e.glyph === query) place(i, 'trung-tu-dang');
    }
    if (hits.size === 0 && folded.length >= 2) {
      const qt = trigrams(query);
      const scored = index
        .map((_, i) => [i, similarity(qt, trigrams(index[i].reading))])
        .filter(([, s]) => s >= FUZZY_THRESHOLD)
        .sort((a, b) => b[1] - a[1])
        .slice(0, MAX_SUGGESTIONS);
      for (const [i] of scored) place(i, 'gan-giong');
    }
  }

  const groups = TIERS.map((t) => ({
    tier: t,
    entries: [...hits.entries()].filter(([, key]) => key === t.key).map(([i]) => index[i]),
  })).filter((g) => g.entries.length > 0);

  return { groups, total: hits.size, mode };
}

/**
 * `app::bridge::Bridge::suggest` — it SUGGESTS, it never rewrites.
 *
 * Most of these pairs are two separate entries in the book, each with its own definition.
 * Merging them would blend the meanings of two different entries.
 */
function bridgeSuggest(query) {
  const key = fold(query.trim());
  if (key.length === 0) return [];
  const out = [];
  for (const p of manifest.bridge) {
    if (fold(p.old) === key) out.push({ alternative: p.new, reason: 'sách in năm 1895 theo chính tả Nam Kỳ đương thời', verified: p.verified });
    else if (fold(p.new) === key) out.push({ alternative: p.old, reason: 'dạng chính tả 1895 của cùng âm', verified: p.verified });
  }
  return out;
}

// ── Fetching ────────────────────────────────────────────────────────────────

async function getJson(path) {
  const response = await fetch(path);
  if (!response.ok) throw new Error(`${path}: ${response.status}`);
  return response.json();
}

/** The detail shard holding entry number `i` of the index. */
async function detail(i) {
  const shard = Math.floor(i / manifest.shardSize);
  if (!shardCache.has(shard)) {
    shardCache.set(shard, getJson(`${DATA}/muc/${String(shard).padStart(4, '0')}.json`));
  }
  return (await shardCache.get(shard))[i % manifest.shardSize];
}

async function ensureFullText() {
  if (fullText === null) fullText = await getJson(`${DATA}/toan-van.json`);
}

async function ensureFrontMatter() {
  if (frontMatter === null) frontMatter = await getJson(`${DATA}/gioi-thieu.json`);
}

// ── Views ───────────────────────────────────────────────────────────────────

function number(n) {
  // The book's own thousands separator, and the one used across the Rust site.
  return String(n).replace(/\B(?=(\d{3})+(?!\d))/g, '.');
}

function glyphAria(e) {
  return e.glyph === null ? `chữ Nôm đọc là ${e.reading}, chưa có mã Unicode` : `chữ Hán-Nôm đọc là ${e.reading}`;
}

/** `partial/entry_row.html`. Every display decision is already made before this point. */
function entryRow(e) {
  const needsFontWarning = e.kind === 'pua' && !manifest.hasNomFont;
  return h`<a class="entry-row" href="#/muc-tu/${e.slug}">
  <span class="glyph glyph--row glyph--${e.kind}" lang="vi-Hani"
        ${needsFontWarning ? raw('title="Tự dạng này dùng mã riêng của font bản in; máy chưa cài font Nôm Na Tống sẽ thấy ô vuông"') : ''}
        aria-label="${glyphAria(e)}">${e.glyph === null ? '' : e.glyph}</span>
  <span class="entry-row__reading">${e.reading}${e.alt === null ? '' : h` <span class="muted">${e.alt}</span>`}</span>
  <span class="pos-badge" title="${e.posName}">${e.posLabel}</span>
  <span class="entry-row__gloss">${e.rv ? raw('<span class="chip-review" title="chờ người đối chứng bản in">?</span> ') : ''}${e.gloss}</span>
  <span class="entry-row__page">tr.${e.pp}</span>
</a>`;
}

function navPair(previous, next) {
  return h`<nav class="nav-pair" aria-label="Điều hướng">
  ${previous === null ? raw('<span></span>') : h`<a href="${previous.href}">‹ ${previous.label}</a>`}
  ${next === null ? raw('<span></span>') : h`<a href="${next.href}">${next.label} ›</a>`}
</nav>`;
}

async function viewHome() {
  // Entry of the day: deterministic on the day count since the Unix epoch, so everyone sees
  // the same entry on a given day and a shared link means something. No random number.
  const day = Math.floor(Date.now() / 86400000);
  const featured = index[day % index.length];
  const s = manifest.stats;
  return h`
<h1 class="visually-hidden">Đại Nam Quấc Âm Tự Vị — tra tìm</h1>

<div class="section-head"><h2>Chữ đầu hôm nay</h2></div>
<article class="featured">
  <a class="glyph glyph--display glyph--${featured.kind}" href="#/muc-tu/${featured.slug}"
     lang="vi-Hani" aria-label="${glyphAria(featured)}">${featured.glyph === null ? '' : featured.glyph}</a>
  <div class="featured__body">
    <h3 class="featured__reading"><a href="#/muc-tu/${featured.slug}">${featured.reading}</a></h3>
    <p>${featured.gloss}</p>
  </div>
  <div class="featured__meta">
    <p><span class="pos-badge" title="${featured.posName}">${featured.posLabel}</span></p>
    <p><a href="#/trang/${featured.pp}">Trang in ${featured.pp} →</a></p>
  </div>
</article>

<section class="section--prose">
  <div class="section-head"><h2>Về cuốn sách này</h2></div>
  <div class="prose">
    <p>
      <em>Đại Nam Quấc Âm Tự Vị</em> của Huình-Tịnh Paulus Của (Sài Gòn, 1895–1896) là bộ tự vị
      Quốc ngữ đầu tiên do người Việt biên soạn. Mỗi chữ đầu gồm một tự dạng Hán hoặc Nôm, âm đọc
      Quốc ngữ, dấu riêng (<code>c.</code> <code>n.</code> <code>cn.</code>), lời giải nghĩa,
      và các mục con dẫn ra cách dùng.
    </p>
    <p>
      Sách xếp theo <strong>22 chữ</strong>, và thứ tự ấy không phải bảng chữ Latin: chữ
      <strong>Y</strong> đứng ở vị trí của I, và không có I, F, J, W, Z. Cột bên trái giữ đúng
      thứ tự bản in.
    </p>
    <p>
      Bản điện tử này giữ <strong>nguyên văn</strong>. Chỗ nào chương trình suy diễn — chẳng hạn
      thay dấu <span class="placeholder">―</span> bằng âm chữ đầu — đều được ghi dấu cho rõ, và
      người đọc bật tắt được.
    </p>
  </div>
</section>

<div class="section-head">
  <h2>Đã số hoá</h2>
  <span class="muted"><a href="#/pham-chat-du-lieu">Phẩm chất dữ liệu →</a></span>
</div>
<dl class="figures">
  <div><dt>Chữ đầu</dt><dd>${number(s.entries)}</dd></div>
  <div><dt>Mục con</dt><dd>${number(s.subEntries)}</dd></div>
  <div><dt>Tự dạng</dt><dd>${number(s.glyphs)}</dd></div>
  <div><dt>Trang</dt><dd>${number(s.pages)}</dd></div>
</dl>
<p class="note">
  Trong đó <strong>${number(s.entriesNeedingReview)}</strong> dòng chữ đầu và
  <strong>${number(s.subEntriesNeedingReview)}</strong> mục con còn chờ người đối chứng bản in,
  và <strong>${number(s.imageOnlyGlyphs)}</strong> chữ đầu có tự dạng chưa được gán mã Unicode.
  Với một cuốn tự vị, nói ra chỗ còn yếu mới là chỗ đáng tin.
</p>`;
}

async function viewEntry(slug) {
  const i = bySlug.get(slug);
  if (i === undefined) return notFound(`Không có chữ đầu nào mang địa chỉ “${slug}”.`);
  const e = index[i];
  const d = await detail(i);
  const heads = await headsFor(e.pdf);

  const subList = d.subs.map((s) => {
    const hasPlaceholder = s.form !== s.formEx;
    return h`<li class="sub-item">
  <span class="sub-form">
    ${s.han === null ? '' : h`<span class="sub-han" lang="vi-Hani">${s.hanEx === null ? s.han : s.hanEx}</span>`}
    ${
      hasPlaceholder
        ? h`<span class="form-verbatim">${splitForm(s.form).map((p) =>
            p.placeholder ? h`<abbr class="placeholder" title="${e.reading}">${p.text}</abbr>` : h`${p.text}`,
          )}</span><span class="form-expanded derived" title="dạng suy diễn, không phải nguyên văn bản in">${s.formEx}</span>`
        : h`<span>${s.form}</span>`
    }
    ${s.rv ? raw('<span class="chip-review" title="chờ người đối chứng bản in">?</span>') : ''}
  </span>
  <span class="sub-def">${s.def}</span>
</li>`;
  });

  const previous = i > 0 ? { href: `#/muc-tu/${index[i - 1].slug}`, label: index[i - 1].reading } : null;
  const next = i + 1 < index.length ? { href: `#/muc-tu/${index[i + 1].slug}`, label: index[i + 1].reading } : null;

  return h`<article>
  <div class="entry-head">
    ${
      e.glyph === null
        ? h`<span class="glyph glyph--display glyph--image_only" lang="vi-Hani" role="img" aria-label="${glyphAria(e)}"></span>`
        : h`<a class="glyph glyph--display glyph--${e.kind}" href="#/chu/${encodeURIComponent(e.glyph)}"
             lang="vi-Hani" aria-label="${glyphAria(e)}">${e.glyph}</a>`
    }
    <div class="entry-head__text">
      <h1 class="entry-reading">${e.reading}</h1>
      ${e.alt === null ? '' : h`<p class="entry-alternate">âm khác trong bản in: <strong>${e.alt}</strong></p>`}
      <p>
        <span class="pos-badge" title="${e.posName}">${e.posLabel}</span>
        <span class="muted">${e.posName}</span>
        ${e.glyph === null ? '' : h`<button class="copy-btn" type="button" data-copy="${e.glyph}" hidden>chép chữ</button>`}
      </p>
      ${
        e.kind === 'pua' && !manifest.hasNomFont
          ? h`<p>
        <span class="chip-no-unicode">Cần font Nôm Na Tống</span>
        <span class="muted">— tự dạng này dùng mã riêng của font bản in
          (U+${e.glyph.codePointAt(0).toString(16).toUpperCase().padStart(4, '0')}); máy chưa cài font sẽ thấy một ô vuông.</span>
      </p>`
          : ''
      }
      ${
        e.glyph === null
          ? h`<p>
        <span class="chip-no-unicode">Chữ chưa có mã Unicode</span>
        <span class="muted">— bản in 2026 dùng ảnh cho tự dạng này, và ban biên tập cố ý không
          gán một mã “gần giống”. Trang này chưa có ảnh chụp trang, nên chỗ này để trống.</span>
      </p>${
        d.shapeNote === null
          ? ''
          : h`<p class="muted"><strong>Hình chữ:</strong> <span class="derived">${d.shapeNote}</span>
        — lời tả lấy từ bản chép Wikisource của bản in 1895, đã có người đối chiếu ảnh trang rồi ký nhận.
        Đây là <em>tả hình</em>, không phải mã chữ.</p>`
      }`
          : ''
      }
      ${d.inherits ? raw('<p class="muted">Chữ đầu này dùng lại tự dạng của mục liền trước trong bản in.</p>') : ''}
    </div>
  </div>

  ${
    e.rv
      ? h`<div class="notice">
    <strong>Dòng chữ đầu này có chỗ khả nghi trong bản in</strong> và đang chờ người đối chứng.
    Chương trình cố ý <em>không đoán</em>: chữ hiện đúng như lớp văn bản của bản điện tử.
    <a href="#/trang/${e.pp}">Xem trang in ${e.pp}</a>.
  </div>`
      : ''
  }

  <p class="entry-gloss">${e.gloss}</p>

  ${
    d.subs.length === 0
      ? ''
      : h`<div class="section-head">
    <h2>Mục con · ${d.subs.length}</h2>
    <label class="mode-chip" for="expand"><input type="checkbox" id="expand" data-expand-toggle> Hiện dạng đầy đủ</label>
  </div>
  ${
    d.subs.some((s) => s.rv)
      ? raw(
          '<div class="notice">Một số mục con bên dưới đang chờ người xét lại; chúng đều có ghi chú ngay tại chỗ.</div>',
        )
      : ''
  }
  <ul class="sub-list" data-sub-list>${subList}</ul>`
  }

  <div class="provenance">
    <span>
      Trang in <strong>${e.pp}</strong> · PDF ${e.pdf}
      ${heads[0] === null ? '' : h`<span class="muted"> · đầu trang: ${heads[0]}</span>`}
    </span>
    <a href="#/trang/${e.pp}">Xem nguyên bản →</a>
  </div>

  ${navPair(previous, next)}
</article>`;
}

/** The running heads the print sets at the top of each page. Fetched once, then kept. */
let runningHeads = null;

async function headsFor(pdfPage) {
  if (runningHeads === null) runningHeads = await getJson(`${DATA}/trang.json`);
  return runningHeads[pdfPage] ?? [null, null];
}

function viewLetter(letterSlug, pageNumber) {
  const letter = manifest.letters.find((l) => l.slug === letterSlug);
  if (letter === undefined) return notFound(`Bản in không có vần nào mang địa chỉ “${letterSlug}”.`);
  const all = byLetter.get(letter.label) ?? [];
  const from = (pageNumber - 1) * PER_PAGE;
  const rows = all.slice(from, from + PER_PAGE);
  const hasMore = from + rows.length < all.length;

  return h`
<div class="section-head">
  <h1>Vần ${letter.label}</h1>
  <span class="muted">${
    rows.length < all.length ? `${number(rows.length)} / ${number(all.length)} chữ đầu` : `${number(all.length)} chữ đầu`
  }</span>
</div>
${rows.map(entryRow)}
${navPair(
  pageNumber > 1 ? { href: `#/van/${letter.slug}?trang=${pageNumber - 1}`, label: 'Trang trước' } : null,
  hasMore ? { href: `#/van/${letter.slug}?trang=${pageNumber + 1}`, label: 'Trang sau' } : null,
)}`;
}

function viewGlyph(glyph) {
  const rows = (byGlyph.get(glyph) ?? []).map((i) => index[i]);
  if (rows.length === 0) return notFound(`Không có chữ đầu nào dùng tự dạng “${glyph}”.`);
  return h`
<div class="entry-head">
  <span class="glyph glyph--display glyph--${rows[0].kind}" role="img" lang="vi-Hani"
        aria-label="tự dạng Hán-Nôm">${glyph}</span>
  <div class="entry-head__text">
    <h1 class="entry-reading" lang="vi-Hani">${glyph}</h1>
    <p class="muted">Một tự dạng thường mang nhiều âm đọc; bên dưới là mọi chữ đầu dùng chữ này.</p>
    <p><button class="copy-btn" type="button" data-copy="${glyph}" hidden>chép chữ</button></p>
  </div>
</div>
<div class="section-head"><h2>Các âm đọc</h2><span class="muted">${rows.length} chữ đầu</span></div>
${rows.map(entryRow)}`;
}

async function viewPage(printed) {
  const rows = (byPrintedPage.get(printed) ?? []).map((i) => index[i]);
  if (rows.length === 0) return notFound(`Trang in ${printed} không có chữ đầu nào trong dữ liệu.`);
  const heads = await headsFor(rows[0].pdf);
  const pages = [...byPrintedPage.keys()].sort((a, b) => a - b);
  const at = pages.indexOf(printed);

  return h`
<div class="section-head">
  <h1>Trang in ${printed}</h1>
  <span class="muted">PDF ${rows[0].pdf}</span>
</div>
${
  heads[0] === null && heads[1] === null
    ? ''
    : h`<p class="hint">Dòng chạy đầu trang, nguyên văn bản in:
  ${heads[0] === null ? '' : h`<strong>${heads[0]}</strong>`}${heads[1] === null ? '' : h` — <strong>${heads[1]}</strong>`}</p>`
}
<div class="page-image-missing">
  <p>Chưa có ảnh chụp trang này.</p>
  <p>
    Bản điện tử 2026 là bản tái sắp chữ chớ không phải ảnh bản in; ảnh trang phải lấy riêng từ
    bản in 1895–1896, và bước ấy chưa làm. Trang này <strong>không hứa</strong> điều chưa
    làm được.
  </p>
</div>
<div class="section-head"><h2>Chữ đầu của trang này</h2><span class="muted">${number(rows.length)} chữ đầu</span></div>
${rows.map(entryRow)}
${navPair(
  at > 0 ? { href: `#/trang/${pages[at - 1]}`, label: 'Trang trước' } : null,
  at >= 0 && at + 1 < pages.length ? { href: `#/trang/${pages[at + 1]}`, label: 'Trang sau' } : null,
)}`;
}

async function viewSearch(query, mode, pageNumber) {
  if (query.trim().length === 0) {
    return h`<p class="muted">Gõ một âm Quốc ngữ, dán một chữ Hán-Nôm, hoặc tìm một cụm trong lời giải nghĩa.</p>`;
  }
  if (resolveMode(mode, query) === 'toan-van') await ensureFullText();

  const { groups, total } = search(query, mode);
  const bridge = bridgeSuggest(query);

  // Paginate across the flattened, rank-ordered result list so the tier headings stay
  // meaningful on every page rather than only on the first.
  const flat = groups.flatMap((g) => g.entries.map((e) => [g.tier, e]));
  const from = (pageNumber - 1) * PER_PAGE;
  const shown = flat.slice(from, from + PER_PAGE);
  const pageGroups = [];
  for (const [tier, e] of shown) {
    const last = pageGroups[pageGroups.length - 1];
    if (last !== undefined && last.tier === tier) last.entries.push(e);
    else pageGroups.push({ tier, entries: [e] });
  }

  return h`
<div class="section-head">
  <h1>Kết quả cho “${query}”</h1>
  <span class="muted">${number(total)} chữ đầu</span>
</div>

${
  bridge.length === 0
    ? ''
    : h`<div class="bridge">
  <p class="bridge__title">Chính tả 1895 ↔ nay</p>
  <p>${bridge.map((s) => h`<a class="bridge__link" href="#/tra-tim?q=${encodeURIComponent(s.alternative)}">${s.alternative}</a>`)}</p>
  <p class="bridge__note">
    ${bridge[0].reason}. Đây là <strong>gợi ý</strong>: hai dạng chính tả thường là
    <em>hai chữ đầu khác nhau</em> trong sách, nên chữ bạn gõ không bị sửa.
    ${bridge[0].verified ? '' : raw('<br><span class="muted">Cặp này chưa có người đối chứng bản in.</span>')}
  </p>
</div>`
}

${
  total === 0
    ? h`<p>Không tìm thấy chữ đầu nào khớp “${query}”.</p>
<div class="section-head"><h2>Hoặc thử</h2></div>
<ul>
  <li><a href="#/tra-tim?q=${encodeURIComponent(query)}&che_do=toan-van">Tìm “${query}” trong phần giải nghĩa</a></li>
  <li><a href="#/van/a">Duyệt theo vần</a></li>
</ul>`
    : h`${pageGroups.map(
        (g) => h`<section class="result-group">
  <h3 class="result-group__label">${g.tier.label} <span class="result-group__count">· ${g.entries.length}</span></h3>
  ${g.entries.map(entryRow)}
</section>`,
      )}
${navPair(
  pageNumber > 1 ? { href: searchHref(query, mode, pageNumber - 1), label: 'Trang trước' } : null,
  from + shown.length < flat.length ? { href: searchHref(query, mode, pageNumber + 1), label: 'Trang sau' } : null,
)}`
}`;
}

function searchHref(query, mode, pageNumber) {
  const parts = [`q=${encodeURIComponent(query)}`, `che_do=${mode}`];
  if (pageNumber > 1) parts.push(`trang=${pageNumber}`);
  return `#/tra-tim?${parts.join('&')}`;
}

async function viewAbout(slug) {
  await ensureFrontMatter();
  if (slug === null) {
    return h`
<div class="section-head"><h1>Giới thiệu</h1><span class="muted">${frontMatter.length} trang đầu sách</span></div>
<div class="prose">
  <p>
    Trang này chép trọn <em>Đại Nam Quấc Âm Tự Vị</em> của Huình-Tịnh Paulus Của
    (Sài Gòn, 1895–1896) theo bản điện tử 2026, để tra chữ đầu, mục con và tự dạng.
  </p>
  <p>
    Phần dưới là <strong>nguyên văn mấy trang đầu sách</strong> — TIỂU TỰ, DẤU RIÊNG, PRÉFACE,
    LỜI DẶN — chép từ lớp văn bản của bản in, không sửa một chữ.
  </p>
</div>
<div class="section-head"><h2>Mấy trang đầu sách</h2></div>
<ul>${frontMatter.map((f) => h`<li><a href="#/gioi-thieu/${f.slug}">${f.title}</a> <span class="muted">· PDF ${f.pdf}</span></li>`)}</ul>`;
  }
  const f = frontMatter.find((x) => x.slug === slug);
  if (f === undefined) return notFound(`Không có trang đầu sách nào mang địa chỉ “${slug}”.`);
  const at = frontMatter.indexOf(f);
  return h`
<div class="section-head"><h1>${f.title}</h1><span class="muted">PDF ${f.pdf}</span></div>
<div class="prose">${f.body.map((line) => h`<p>${line}</p>`)}</div>
${navPair(
  at > 0 ? { href: `#/gioi-thieu/${frontMatter[at - 1].slug}`, label: frontMatter[at - 1].title } : null,
  at + 1 < frontMatter.length ? { href: `#/gioi-thieu/${frontMatter[at + 1].slug}`, label: frontMatter[at + 1].title } : null,
)}`;
}

/* What each gate settles, in the reader's language.
 *
 * The report itself is written for whoever runs the pipeline: its notes are English and count
 * things only that person can act on, so they are not put on the page. What a reader needs is
 * the question the gate asks, and that is written here.
 *
 * A gate absent from this map is shown by its own name and nothing else — inventing a
 * description for a check nobody has described is the guess this edition forbids.
 * `tools/check-static-site.mjs` fails when that happens, so it is noticed at build time. */
const GATE_TEXT = {
  UnmappedGlyphs: {
    title: 'Không đoán tự dạng',
    about:
      'Mỗi chữ Hán-Nôm trong bản điện tử được ghi bằng một mã. Gặp mã tra không ra, chương trình dừng lại và báo lỗi, chớ không thay bằng một chữ trông gần giống.',
  },
  HeadwordCoverage: {
    title: 'Dòng chữ đầu không sót phần nào',
    about:
      'Mỗi dòng chữ đầu được cắt thành tự dạng, âm, dấu riêng, lời giải nghĩa. Ghép các phần ấy lại phải ra đúng dòng ban đầu, không mẩu nào bị bỏ ra ngoài.',
  },
  CharacterConservation: {
    title: 'Không mất, không nhân đôi con chữ',
    about: 'Mọi con chữ của thân sách phải nằm trong đúng một phần, đúng một lần.',
  },
  IndexReconciliation: {
    title: 'Đối chứng với bảng tra đi kèm',
    about:
      'Mọi tự dạng rút được đều đem so với bảng “Mục Từ” do người lập, để bắt những chữ chương trình có mà bảng không có — tức những chữ chương trình có thể đã đặt ra.',
  },
  CollationOrder: {
    title: 'Thứ lớp chữ đầu đúng như sách',
    about:
      'Sách xếp mục theo bảng 22 chữ riêng của nó, không theo bảng chữ Latin. Đọc suốt từ đầu đến cuối, không mục nào được đứng trật lên trước mục lẽ ra đi trước nó.',
  },
  PrintedErrata: {
    title: 'Bảng đính chính của chính bản in',
    about:
      'Bản in tự chỉ chỗ sai của mình ở hai bảng do chính tác giả lập — SAI SÓT và ĐÍNH NGOA. Mỗi chỗ trong đó hoặc được theo, hoặc phải nói rõ vì sao không theo.',
  },
  ScanVerified: {
    title: 'Chỗ đã đối chiếu ảnh trang',
    about:
      'Mấy chỗ đã mở ảnh trang bản in ra coi rồi ký nhận: mỗi chỗ vẫn phải tìm được đúng lời ấy trên trang mà nó dẫn.',
  },
};

function gateStatus(status) {
  if (status === 'Green') return 'đạt';
  if (status === 'Red') return 'chưa đạt';
  return status;
}

/** The two numbers of a gate, said as a sentence. The allowed number *is* the number of
 *  deviations a person has written a reason for — there is no threshold to loosen. */
function gateCount(r) {
  if (r.measured === 0 && r.allowed === 0) return 'Không có chỗ lệch nào.';
  if (r.measured === r.allowed) {
    return r.measured === 1
      ? '1 chỗ lệch, và chỗ ấy đã có hồ sơ người xét.'
      : `${number(r.measured)} chỗ lệch, chỗ nào cũng đã có hồ sơ người xét.`;
  }
  return `${number(r.measured)} chỗ lệch, ${number(r.allowed)} chỗ đã có hồ sơ.`;
}

function gateEntry(r) {
  const text = GATE_TEXT[r.gate];
  return h`<dt>${text === undefined ? r.gate : text.title} <span class="muted">— ${gateStatus(r.status)}</span></dt>
<dd>${text === undefined ? '' : text.about} <span class="muted">${gateCount(r)}</span></dd>`;
}

function viewQuality() {
  const s = manifest.stats;
  const gates = manifest.gates;
  return h`
<div class="section-head"><h1>Phẩm chất dữ liệu</h1><span class="muted">dựng ngày ${manifest.builtAt}</span></div>
<div class="prose">
  <p>
    Với một cuốn tự vị, <strong>nói ra chỗ còn yếu mới là chỗ đáng tin</strong>. Trang này
    không quảng cáo; nó liệt kê chỗ dữ liệu chưa chắc và chỗ trang này còn làm chưa tới.
  </p>
</div>

<div class="section-head"><h2>Đã số hoá</h2></div>
<dl class="figures">
  <div><dt>Chữ đầu</dt><dd>${number(s.entries)}</dd></div>
  <div><dt>Mục con</dt><dd>${number(s.subEntries)}</dd></div>
  <div><dt>Tự dạng</dt><dd>${number(s.glyphs)}</dd></div>
  <div><dt>Trang</dt><dd>${number(s.pages)}</dd></div>
</dl>
<p class="note">
  <strong>${number(s.entriesNeedingReview)}</strong> dòng chữ đầu và
  <strong>${number(s.subEntriesNeedingReview)}</strong> mục con còn chờ người đối chứng bản in.
  <strong>${number(s.imageOnlyGlyphs)}</strong> chữ đầu có tự dạng bản in 2026 dùng ảnh, chưa gán mã Unicode —
  ban biên tập cố ý không gán một mã “gần giống”, và bản này giữ đúng quyết định ấy.
</p>

<section class="section--prose">
<div class="section-head"><h2>Các phép kiểm phải đạt trước khi nạp</h2></div>
<div class="prose">
  <p>
    Dữ liệu chỉ được nạp khi qua hết các phép kiểm dưới đây; còn một phép chưa đạt thì không
    nạp. Kết quả dưới đây là của chính lần dựng ra trang này.
  </p>
</div>
<dl class="glossary">${gates.results.map(gateEntry)}</dl>
<p class="note">
  Bản phúc trình này đi liền với tập dữ liệu mà nó đã kiểm. Một bản phúc trình toàn “đạt”
  nhưng của lần chạy trước còn nguy hơn là không có bản nào, nên đem nạp một tập dữ liệu khác
  với tập đã kiểm thì cũng không nạp được.
</p>
</section>

<section class="section--prose">
<div class="section-head"><h2>Chỗ trang này còn làm chưa tới</h2></div>
<div class="prose">
  <p>
    Tra tìm ở đây xếp kết quả theo một <strong>thứ bậc cố định</strong>: trùng khít → trùng tự
    dạng → bắt đầu bằng → bỏ dấu → gần giống. Còn “toàn văn” là phép tìm đúng cụm chữ trong
    lời giải nghĩa, tìm trên chữ đã bỏ dấu: nó đo mặt chữ chớ không đo nghĩa, nên một tiếng
    đồng nghĩa mà viết khác thì nó không đem về.
  </p>
  <p>
    Trang in ở đây chỉ liệt kê chữ đầu, <strong>chưa có ảnh chụp trang</strong>: bản điện tử
    2026 là bản tái sắp chữ chớ không phải ảnh bản in, nên ảnh trang phải lấy riêng từ bản in
    1895–1896, và bước ấy chưa làm.
  </p>
</div>
</section>`;
}

function notFound(message) {
  return h`<div class="section-head"><h1>Không tìm thấy</h1></div>
<p>${message}</p>
<p><a href="#/">Về trang đầu</a></p>`;
}

// ── Routing ─────────────────────────────────────────────────────────────────

function parseRoute() {
  const hash = location.hash.replace(/^#/, '');
  const [path, queryString] = hash.split('?');
  const segments = path.split('/').filter((s) => s.length > 0);
  return { segments, params: new URLSearchParams(queryString ?? '') };
}

function pageParam(params) {
  const n = Number.parseInt(params.get('trang') ?? '1', 10);
  return Number.isNaN(n) || n < 1 ? 1 : n;
}

async function render() {
  const { segments, params } = parseRoute();
  const [head, ...rest] = segments;

  let body;
  let title = 'Đại Nam Quấc Âm Tự Vị';
  try {
    if (head === undefined) {
      body = await viewHome();
    } else if (head === 'muc-tu' && rest.length === 1) {
      body = await viewEntry(rest[0]);
      const i = bySlug.get(rest[0]);
      if (i !== undefined) title = `${index[i].reading} — Đại Nam Quấc Âm Tự Vị`;
    } else if (head === 'van' && rest.length === 1) {
      body = viewLetter(rest[0], pageParam(params));
      title = `Vần ${rest[0].toUpperCase()} — Đại Nam Quấc Âm Tự Vị`;
    } else if (head === 'chu' && rest.length === 1) {
      body = viewGlyph(decodeURIComponent(rest[0]));
      title = `${decodeURIComponent(rest[0])} — Đại Nam Quấc Âm Tự Vị`;
    } else if (head === 'trang' && rest.length === 1) {
      body = await viewPage(Number.parseInt(rest[0], 10));
      title = `Trang in ${rest[0]} — Đại Nam Quấc Âm Tự Vị`;
    } else if (head === 'tra-tim' && rest.length === 0) {
      const q = params.get('q') ?? '';
      body = await viewSearch(q, params.get('che_do') ?? 'auto', pageParam(params));
      title = q.length === 0 ? 'Tra tìm — Đại Nam Quấc Âm Tự Vị' : `“${q}” — tra tìm`;
    } else if (head === 'gioi-thieu') {
      body = await viewAbout(rest.length === 1 ? rest[0] : null);
      title = 'Giới thiệu — Đại Nam Quấc Âm Tự Vị';
    } else if (head === 'pham-chat-du-lieu' && rest.length === 0) {
      body = viewQuality();
      title = 'Phẩm chất dữ liệu — Đại Nam Quấc Âm Tự Vị';
    } else {
      body = notFound(`Địa chỉ “${location.hash}” không dẫn tới đâu cả.`);
    }
  } catch (error) {
    // A failed fetch must say what failed. A blank screen teaches the reader nothing.
    body = h`<div class="section-head"><h1>Không mở được</h1></div>
<p>Không tải được dữ liệu cho địa chỉ này.</p>
<p class="muted">${String(error && error.message ? error.message : error)}</p>`;
  }

  document.title = title;
  main.innerHTML = body[RAW];
  syncChrome(params);
  wireContent();
  main.focus({ preventScroll: true });
  window.scrollTo(0, 0);
}

/** Keep the rail in step with the view: the active letter chip and the search box. */
function syncChrome(params) {
  const { segments } = parseRoute();
  let active = null;
  if (segments[0] === 'van') active = segments[1];
  else if (segments[0] === 'muc-tu') {
    const i = bySlug.get(segments[1]);
    if (i !== undefined) active = manifest.letters.find((l) => l.label === index[i].letter).slug;
  }
  for (const chip of document.querySelectorAll('[data-letter-chip]')) {
    const on = chip.dataset.letterChip === active;
    chip.classList.toggle('is-active', on);
    if (on) chip.setAttribute('aria-current', 'page');
    else chip.removeAttribute('aria-current');
  }

  const input = document.querySelector('[data-search-input]');
  if (segments[0] === 'tra-tim') input.value = params.get('q') ?? '';
  const mode = segments[0] === 'tra-tim' ? (params.get('che_do') ?? 'auto') : 'auto';
  for (const chip of document.querySelectorAll('[data-mode-chip]')) {
    chip.setAttribute('aria-pressed', chip.dataset.modeChip === mode ? 'true' : 'false');
  }
}

// ── Progressive touches, from frontend/styles/enhance.js ────────────────────

const EXPAND_KEY = 'dnqatv:hien-dang-day-du';

function wireContent() {
  // The reader cannot type 𨰲, so copying is the primary action, not a nicety. The button
  // appears only when the browser can really copy — a button that does nothing is worse
  // than no button.
  if (navigator.clipboard && window.isSecureContext) {
    for (const btn of main.querySelectorAll('[data-copy]')) {
      btn.hidden = false;
      btn.addEventListener('click', () => {
        navigator.clipboard.writeText(btn.dataset.copy).then(
          () => {
            const before = btn.textContent;
            btn.textContent = 'đã chép';
            setTimeout(() => {
              btn.textContent = before;
            }, 1400);
          },
          () => {
            btn.textContent = 'không chép được';
          },
        );
      });
    }
  }

  const toggle = main.querySelector('[data-expand-toggle]');
  const list = main.querySelector('[data-sub-list]');
  if (toggle !== null && list !== null) {
    let stored = null;
    try {
      stored = sessionStorage.getItem(EXPAND_KEY);
    } catch {
      /* private mode */
    }
    if (stored === '1') {
      toggle.checked = true;
      list.classList.add('show-expanded');
    }
    toggle.addEventListener('change', () => {
      list.classList.toggle('show-expanded', toggle.checked);
      try {
        sessionStorage.setItem(EXPAND_KEY, toggle.checked ? '1' : '0');
      } catch {
        /* ignore */
      }
    });
  }
}

function wireChrome() {
  const form = document.querySelector('[data-search-form]');
  const input = document.querySelector('[data-search-input]');
  let pendingMode = null;

  // The fragment is the router, so `href="#main"` would be read as a route and land on a
  // not-found page. Focus is moved directly instead.
  const skip = document.querySelector('[data-skip-link]');
  if (skip !== null) {
    skip.addEventListener('click', (event) => {
      event.preventDefault();
      main.focus();
    });
  }

  for (const chip of document.querySelectorAll('[data-mode-chip]')) {
    chip.addEventListener('click', () => {
      pendingMode = chip.dataset.modeChip;
    });
  }
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    const mode = pendingMode ?? 'auto';
    pendingMode = null;
    location.hash = searchHref(input.value, mode, 1).slice(1);
    // A repeat search on the same query changes no hash, so render explicitly.
    render();
  });

  // The mode chip lights up to match what is being typed. A hint only — the value still
  // comes from the chip the reader presses.
  const update = () => {
    const v = input.value;
    const guess = v.length === 0 ? 'auto' : [...v].some(isHanNom) ? 'han-nom' : 'quoc-ngu';
    for (const chip of document.querySelectorAll('[data-mode-chip]')) {
      chip.classList.toggle('is-guess', chip.dataset.modeChip === guess && v.length > 0);
    }
  };
  input.addEventListener('input', update);
  update();

  // "/" jumps to the search box, Esc clears it — skipped while already typing.
  document.addEventListener('keydown', (e) => {
    const tag = e.target.tagName ?? '';
    const typing = tag === 'INPUT' || tag === 'TEXTAREA' || e.target.isContentEditable;
    if (e.key === '/' && !typing) {
      e.preventDefault();
      input.focus();
      input.select();
    }
    if (e.key === 'Escape' && typing && e.target.value) e.target.value = '';
  });
}

// ── Boot ────────────────────────────────────────────────────────────────────

async function boot() {
  [manifest, index] = await Promise.all([getJson(`${DATA}/manifest.json`), getJson(`${DATA}/tra.json`)]);

  foldedReading = index.map((e) => fold(e.reading));
  lowerReading = index.map((e) => e.reading.toLowerCase());
  for (const [i, e] of index.entries()) {
    bySlug.set(e.slug, i);
    if (e.glyph !== null) {
      if (!byGlyph.has(e.glyph)) byGlyph.set(e.glyph, []);
      byGlyph.get(e.glyph).push(i);
    }
    if (!byLetter.has(e.letter)) byLetter.set(e.letter, []);
    byLetter.get(e.letter).push(e);
    if (!byPrintedPage.has(e.pp)) byPrintedPage.set(e.pp, []);
    byPrintedPage.get(e.pp).push(i);
  }

  document.querySelector('[data-letters]').innerHTML = manifest.letters
    .map((l) => `<li><a class="letter-chip" data-letter-chip="${l.slug}" href="#/van/${l.slug}">${l.label}</a></li>`)
    .join('');

  wireChrome();
  window.addEventListener('hashchange', render);
  await render();
}

boot().catch((error) => {
  main.innerHTML = `<div class="section-head"><h1>Không mở được tự vị</h1></div><p class="muted">${esc(String(error && error.message ? error.message : error))}</p>`;
});
