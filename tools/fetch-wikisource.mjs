// One-time harvest of the vi.wikisource transcription of the 1895 print, for gate 6.
//
// The pipeline itself never touches the network: this writes a local snapshot and every
// later step reads that file, exactly as `extract` reads a PDF from docs/. Each page is
// stored with the revision id it came from, so a comparison can be repeated against the
// same text months later even after the wiki has moved on.
//
// Usage: node tools/fetch-wikisource.mjs [outDir]      (default: data/wikisource)
//
// Safe to re-run: pages already in pages.jsonl are kept and the walk resumes from the
// continuation token in state.json, so an interrupted run never re-downloads what it has.
//
// LICENCE — the 1895 dictionary itself is in the public domain, but this transcription is
// the work of Wikisource contributors and carries CC BY-SA 4.0. It is kept here as a
// CROSS-CHECK WITNESS, not as a source of published text. PROVENANCE.json records that.

import { writeFileSync, appendFileSync, readFileSync, existsSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import https from 'node:https';

const HOST = 'vi.wikisource.org';
const UA = 'dnqatv-gate6/0.1 (offline collation cross-check of a public-domain 1895 dictionary)';
const VOLUMES = [
  { volume: 1, prefix: 'Đại Nam quấc âm tự vị 1.pdf/' },
  { volume: 2, prefix: 'Đại Nam quấc âm tự vị 2.pdf/' },
];
const NS_PAGE = 104;          // Trang: — the scan-backed proofreading namespace
const BATCH = 50;             // the API content limit for an anonymous client
const PAUSE_MS = 2000;        // between successful batches
const MAX_RETRY = 6;

const sleep = ms => new Promise(r => setTimeout(r, ms));

// Resolves to {json} or {retryAfterMs} — a 429 is an instruction to wait, not a failure.
function request(path) {
  return new Promise((resolve, reject) => {
    https
      .get({ host: HOST, path, headers: { 'User-Agent': UA, Accept: 'application/json' } }, res => {
        if (res.statusCode === 429 || res.statusCode === 503) {
          const hdr = Number(res.headers['retry-after']);
          res.resume();
          return resolve({ retryAfterMs: Number.isFinite(hdr) ? hdr * 1000 : null });
        }
        if (res.statusCode !== 200) {
          res.resume();
          return reject(new Error(`HTTP ${res.statusCode} for ${path}`));
        }
        let body = '';
        res.setEncoding('utf8');
        res.on('data', d => (body += d));
        res.on('end', () => {
          try {
            resolve({ json: JSON.parse(body) });
          } catch (e) {
            reject(new Error(`bad JSON: ${e.message}`));
          }
        });
      })
      .on('error', reject);
  });
}

async function get(path) {
  let wait = 10_000;
  for (let attempt = 0; attempt <= MAX_RETRY; attempt++) {
    let out;
    try {
      out = await request(path);
    } catch (e) {
      if (attempt === MAX_RETRY) throw e;
      process.stderr.write(`\n  ${e.message} — retrying in ${wait / 1000}s\n`);
      await sleep(wait);
      wait = Math.min(wait * 2, 120_000);
      continue;
    }
    if (out.json) return out.json;
    const pause = out.retryAfterMs ?? wait;
    if (attempt === MAX_RETRY) throw new Error('still rate-limited after retries');
    process.stderr.write(`\n  rate-limited — waiting ${Math.round(pause / 1000)}s\n`);
    await sleep(pause);
    wait = Math.min(wait * 2, 120_000);
  }
  throw new Error('unreachable');
}

// The proofreading status the transcriber assigned: 0 no text · 1 not proofread ·
// 3 proofread · 4 validated. Recorded because a level-1 page is a weaker witness.
function readQuality(wikitext) {
  const m = wikitext.match(/<pagequality\s+level="(\d)"\s+user="([^"]*)"/);
  return m ? { level: Number(m[1]), user: m[2] } : { level: null, user: null };
}

const outDir = process.argv[2] ?? join('data', 'wikisource');
mkdirSync(outDir, { recursive: true });
const pagesFile = join(outDir, 'pages.jsonl');
const stateFile = join(outDir, 'state.json');

const seen = new Set();
if (existsSync(pagesFile)) {
  for (const line of readFileSync(pagesFile, 'utf8').split('\n')) {
    if (!line.trim()) continue;
    try {
      seen.add(JSON.parse(line).title);
    } catch {
      /* a truncated last line from a killed run — it will simply be refetched */
    }
  }
}
const state = existsSync(stateFile) ? JSON.parse(readFileSync(stateFile, 'utf8')) : {};
if (seen.size) console.error(`resuming: ${seen.size} pages already held`);

async function fetchVolume(volume, prefix) {
  if (state[volume]?.done) {
    console.error(`  vol ${volume}: already complete`);
    return;
  }
  let cont = state[volume]?.cont ?? null;
  let got = 0;
  for (;;) {
    const params = new URLSearchParams({
      action: 'query',
      generator: 'allpages',
      gapnamespace: String(NS_PAGE),
      gapprefix: prefix,
      gaplimit: String(BATCH),
      prop: 'revisions',
      rvprop: 'content|ids|timestamp',
      rvslots: 'main',
      format: 'json',
      formatversion: '2',
    });
    if (cont) for (const [k, v] of Object.entries(cont)) params.set(k, v);

    const data = await get('/w/api.php?' + params.toString());

    const batch = [];
    for (const p of data?.query?.pages ?? []) {
      const rev = p.revisions?.[0];
      const wikitext = rev?.slots?.main?.content;
      if (typeof wikitext !== 'string') continue;   // page exists but holds no text
      if (seen.has(p.title)) continue;
      seen.add(p.title);
      const scanPage = Number(p.title.slice(p.title.lastIndexOf('/') + 1));
      const q = readQuality(wikitext);
      batch.push({
        volume,
        scan_page: Number.isFinite(scanPage) ? scanPage : null,
        title: p.title,
        revid: rev.revid,
        timestamp: rev.timestamp,
        quality: q.level,
        quality_user: q.user,
        wikitext,
      });
    }
    if (batch.length) appendFileSync(pagesFile, batch.map(b => JSON.stringify(b)).join('\n') + '\n');
    got += batch.length;

    cont = data.continue ?? null;
    // Checkpoint AFTER the append, so a crash re-reads at worst one batch.
    state[volume] = cont ? { cont } : { done: true };
    writeFileSync(stateFile, JSON.stringify(state, null, 2));
    process.stderr.write(`  vol ${volume}: +${got} this run (${seen.size} total)\r`);
    if (!cont) break;
    await sleep(PAUSE_MS);
  }
  process.stderr.write('\n');
}

for (const v of VOLUMES) await fetchVolume(v.volume, v.prefix);

// Rewrite the file in reading order and emit provenance.
const pages = readFileSync(pagesFile, 'utf8')
  .split('\n')
  .filter(l => l.trim())
  .map(l => JSON.parse(l))
  .sort((a, b) => a.volume - b.volume || (a.scan_page ?? 0) - (b.scan_page ?? 0));
writeFileSync(pagesFile, pages.map(p => JSON.stringify(p)).join('\n') + '\n');

const byQuality = {};
for (const p of pages) byQuality[p.quality ?? 'none'] = (byQuality[p.quality ?? 'none'] ?? 0) + 1;
const withEntry = pages.filter(p => p.wikitext.includes('DNQATV/mục')).length;
const withSub = pages.filter(p => p.wikitext.includes('DNQATV/nghĩa')).length;

writeFileSync(
  join(outDir, 'PROVENANCE.json'),
  JSON.stringify(
    {
      source: `https://${HOST}/`,
      namespace: 'Trang: (ns 104)',
      indexes: [
        'Mục lục:Đại Nam quấc âm tự vị 1.pdf',
        'Mục lục:Đại Nam quấc âm tự vị 2.pdf',
      ],
      work: 'Đại Nam Quấc âm tự vị, Huình Tịnh Paulus Của, Saigon: Rey, Curiol & Cie, 1895-96',
      underlying_text: 'public domain (published 1895-96, author died 1908)',
      transcription_licence: 'CC BY-SA 4.0 — Wikisource contributors',
      role: 'cross-check witness for gate 6; NOT a source of published text',
      snapshot_taken: new Date().toISOString(),
      pages: pages.length,
      pages_by_quality: byQuality,
      pages_with_entry_template: withEntry,
      pages_with_sub_entry_template: withSub,
      revision_ids: Object.fromEntries(pages.map(p => [p.title, p.revid])),
    },
    null,
    2,
  ) + '\n',
);

console.log(`pages             : ${pages.length}`);
console.log(`by quality level  : ${JSON.stringify(byQuality)}`);
console.log(`with DNQATV/mục   : ${withEntry}`);
console.log(`with DNQATV/nghĩa : ${withSub}`);
console.log(`wrote ${pagesFile} and PROVENANCE.json`);
