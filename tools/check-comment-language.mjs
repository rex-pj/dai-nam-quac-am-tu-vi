#!/usr/bin/env node
//! Refuse a comment written in Vietnamese.
//!
//! The repository holds two layers that both speak Vietnamese on the page but must not be
//! confused: the BOOK — headwords, definitions, the words of the 1895 print — and the
//! PROGRAM that serves it. The book's words stay Vietnamese everywhere, including in the
//! strings a reader sees. A comment is never the book: it is one programmer talking to the
//! next, and those are in English so that the code reads in a single language.
//!
//! A comment may still QUOTE the book — `― gươm`, "chữ đầu", *quấc* — and quoting is half
//! the job here, so a bare diacritic cannot be the signal. What this looks for instead is
//! Vietnamese GRAMMAR: the function words (`là`, `của`, `không`, `được` …) that turn up when
//! a sentence is being spoken rather than a word cited. Text inside backticks, double
//! quotes, «» or *emphasis* is dropped before the search, so citing is always allowed.
//!
//! That makes this a smoke alarm, not a proof: a short Vietnamese comment carrying no
//! function word gets through. It catches the paragraph, which is what actually accumulates.
//!
//! Usage: node tools/check-comment-language.mjs [paths…]   (default: every tracked file)

import { readFileSync } from 'node:fs';
import { execSync } from 'node:child_process';

// ── What the rule covers ────────────────────────────────────────────────────

const EXTENSIONS = ['rs', 'mjs', 'js', 'css', 'html', 'toml', 'yml', 'yaml'];

// `review/` is not code. Those files are dossiers a PERSON fills in — the header explains to
// a Vietnamese reader what they are being asked to settle, and the `reason` rows are their
// own words. Translating them would move the cost onto the only human in the loop.
const EXEMPT = [/^review\//];

// Vietnamese grammar words, every one carrying a diacritic so that none of them is also an
// English word. Content words are deliberately absent: they are what a comment quotes.
const FUNCTION_WORDS = [
  'là', 'của', 'và', 'được', 'nếu', 'thì', 'không', 'những', 'với', 'này', 'đó', 'sẽ',
  'phải', 'một', 'đã', 'đang', 'rồi', 'vì', 'nhưng', 'hoặc', 'tại', 'đây', 'đến', 'mỗi',
  'nào', 'cũng', 'vẫn', 'còn', 'chưa', 'mà', 'nhiều', 'hơn', 'nhất', 'rất', 'quá', 'lại',
  'nữa', 'đều', 'ở', 'ấy', 'nó', 'chúng', 'mình', 'cần', 'nên', 'bởi', 'để', 'dùng',
  'làm', 'viết', 'đọc', 'người', 'việc', 'cách', 'kết quả', 'dữ liệu', 'lỗi', 'trước',
  'giữa', 'thêm', 'bảng', 'dòng', 'chữ', 'số', 'tên', 'khác', 'theo', 'sau',
];
const GRAMMAR = new RegExp(`(?<!\\p{L})(${FUNCTION_WORDS.join('|')})(?!\\p{L})`, 'giu');

// One function word can be a quotation that slipped past the strippers below; two of them in
// a line is somebody speaking Vietnamese.
const THRESHOLD = 2;

// ── Pulling the comments out ────────────────────────────────────────────────

/** Comment text in a C-like file: a line comment, and a block comment across lines. */
function commentsOfCLike(source) {
  const found = [];
  let line = 1;
  let i = 0;
  let block = null; // The line the block was opened on, or null outside one.
  let start = 0;
  while (i < source.length) {
    const c = source[i];
    if (c === '\n') line++;
    if (block !== null) {
      if (c === '*' && source[i + 1] === '/') {
        found.push([block, source.slice(start, i)]);
        block = null;
        i += 2;
        continue;
      }
      i++;
      continue;
    }
    if (c === '"' || c === "'") {
      // A string literal may hold anything, comment markers included. Skip to its end.
      const quote = c;
      i++;
      while (i < source.length && source[i] !== quote) {
        if (source[i] === '\\') i++;
        if (source[i] === '\n') line++;
        i++;
      }
      i++;
      continue;
    }
    if (c === '/' && source[i + 1] === '/') {
      const end = source.indexOf('\n', i);
      const stop = end === -1 ? source.length : end;
      found.push([line, source.slice(i + 2, stop)]);
      i = stop;
      continue;
    }
    if (c === '/' && source[i + 1] === '*') {
      block = line;
      i += 2;
      start = i;
      continue;
    }
    i++;
  }
  return found;
}

/** Comment text between a pair of delimiters, e.g. HTML's or a template engine's. */
function commentsBetween(source, open, close) {
  const found = [];
  let i = 0;
  for (;;) {
    const from = source.indexOf(open, i);
    if (from === -1) return found;
    const to = source.indexOf(close, from + open.length);
    const stop = to === -1 ? source.length : to;
    const line = source.slice(0, from).split('\n').length;
    found.push([line, source.slice(from + open.length, stop)]);
    i = stop + close.length;
  }
}

/** `#` comments, skipping a `#` that sits inside a quoted value. */
function commentsOfHash(source) {
  return source.split(/\r?\n/).flatMap((text, index) => {
    let quote = null;
    for (let i = 0; i < text.length; i++) {
      const c = text[i];
      if (quote) {
        if (c === quote) quote = null;
      } else if (c === '"' || c === "'") {
        quote = c;
      } else if (c === '#') {
        return [[index + 1, text.slice(i + 1)]];
      }
    }
    return [];
  });
}

function commentsOf(path, source) {
  const extension = path.split('.').pop();
  if (['rs', 'mjs', 'js', 'css'].includes(extension)) return commentsOfCLike(source);
  if (extension === 'html') {
    return [...commentsBetween(source, '<!--', '-->'), ...commentsBetween(source, '{#', '#}')];
  }
  return commentsOfHash(source);
}

// ── The check ───────────────────────────────────────────────────────────────

/** Everything the comment is CITING rather than saying, removed. */
function spokenPart(text) {
  return text
    .replace(/`[^`]*`/g, ' ')
    .replace(/"[^"]*"/g, ' ')
    .replace(/“[^”]*”/g, ' ')
    .replace(/«[^»]*»/g, ' ')
    .replace(/\*[^*\n]*\*/g, ' ');
}

const argued = process.argv.slice(2);
const listed = argued.length
  ? argued
  : execSync('git ls-files', { encoding: 'utf8' }).trim().split('\n');
const files = listed
  .map((path) => path.replace(/\\/g, '/'))
  .filter((path) => EXTENSIONS.includes(path.split('.').pop()))
  .filter((path) => !EXEMPT.some((pattern) => pattern.test(path)));

const violations = [];
for (const path of files) {
  let source;
  try {
    source = readFileSync(path, 'utf8');
  } catch {
    continue; // Listed in git but not on disk — a deletion being staged.
  }
  // A fenced block inside a doc comment is a specimen, not a sentence: `subentry.rs` shows
  // measured sub-entry lines there, book text and all. Fences run across consecutive `//!`
  // lines, so the state is carried over the whole file rather than reset per comment.
  let fenced = false;
  for (const [line, text] of commentsOf(path, source)) {
    text.split('\n').forEach((one, offset) => {
      const bare = one.replace(/^[\s!/*]*/, '');
      if (bare.startsWith('```')) {
        fenced = !fenced;
        return;
      }
      if (fenced) return;
      const spoken = spokenPart(one).match(GRAMMAR) || [];
      const words = [...new Set(spoken.map((word) => word.toLowerCase()))];
      if (words.length >= THRESHOLD) {
        violations.push({ path, line: line + offset, words, text: one.trim() });
      }
    });
  }
}

for (const { path, line, words, text } of violations) {
  console.log(`${path}:${line}: ${text}`);
  console.log(`   └─ Vietnamese: ${words.join(', ')}`);
}

if (violations.length > 0) {
  console.error(`\n::error::${violations.length} comment line(s) in Vietnamese. Comments are English.`);
  process.exit(1);
}
console.log(`No Vietnamese comment in ${files.length} files.`);
