#!/usr/bin/env node
// Check `tools/scan-page.mjs` — the CCITT Group 4 decoder and the PNG writer.
//
// The decoder is hand-written, so it needs a test that bites. Two halves:
//
//   1. Round-trip: bitmaps encoded here by hand in G4, decoded back, compared pixel by
//      pixel. No PDF needed, so this half runs in CI where `docs/` is absent.
//   2. Against the real scans, when they are present: a page of the print must come out
//      about 5-12% ink. A polarity mistake reads as 93% — that is exactly the bug this
//      catches, and nothing in the round-trip half would have noticed it.
//
// Run: node tools/check-scan-page.mjs

import fs from 'node:fs';
import zlib from 'node:zlib';
import { decodeGroup4, encodePng, loadPdf, pageOrder, pageImage, scanFile } from './scan-page.mjs';

let failures = 0;
function check(name, ok, detail = '') {
  if (ok) {
    process.stdout.write(`  ok   ${name}\n`);
  } else {
    failures++;
    process.stdout.write(`  FAIL ${name}${detail ? ' — ' + detail : ''}\n`);
  }
}

// ── A tiny G4 encoder, written only so the decoder has something to decode ───
// It emits vertical and horizontal modes exactly as T.6 defines them. Writing the encoder
// separately from the decoder is the point: a shared mistake in one table would not cancel out.

const WHITE_CODE = { 0: '00110101', 1: '000111', 2: '0111', 3: '1000', 4: '1011', 5: '1100', 6: '1110', 7: '1111', 8: '10011', 9: '10100', 10: '00111', 11: '01000', 12: '001000', 13: '000011', 14: '110100', 15: '110101', 16: '101010' };
const BLACK_CODE = { 0: '0000110111', 1: '010', 2: '11', 3: '10', 4: '011', 5: '0011', 6: '0010', 7: '00011', 8: '000101', 9: '000100', 10: '0000100', 11: '0000101', 12: '0000111', 13: '00000100', 14: '00000111', 15: '000011000', 16: '0000010111' };

function changingElements(row, columns) {
  const out = [];
  let colour = 0;
  for (let x = 0; x < columns; x++) {
    if (row[x] !== colour) {
      out.push(x);
      colour = row[x];
    }
  }
  return out;
}

function encodeGroup4(rows, columns) {
  let bits = '';
  let reference = new Uint8Array(columns); // an imaginary all-white line above the first

  for (const row of rows) {
    const cur = changingElements(row, columns);
    const ref = changingElements(reference, columns);
    let a0 = -1;
    let colour = 0;

    while (a0 < columns) {
      const a1 = cur.find((x) => x > a0) ?? columns;
      let i = 0;
      while (i < ref.length && (ref[i] <= a0 || (i & 1) !== colour)) i++;
      const b1 = i < ref.length ? ref[i] : columns;
      const b2 = i + 1 < ref.length ? ref[i + 1] : columns;

      if (b2 < a1) {
        bits += '0001'; // pass
        a0 = b2;
        continue;
      }
      const delta = a1 - b1;
      if (delta >= -3 && delta <= 3) {
        bits += { '-3': '0000010', '-2': '000010', '-1': '010', 0: '1', 1: '011', 2: '000011', 3: '0000011' }[String(delta)];
        a0 = a1;
        colour ^= 1;
        continue;
      }
      // horizontal: two runs from a0 (or 0 on the first element)
      const start = a0 < 0 ? 0 : a0;
      const a2 = cur.find((x) => x > a1) ?? columns;
      const run1 = a1 - start;
      const run2 = a2 - a1;
      const table1 = colour ? BLACK_CODE : WHITE_CODE;
      const table2 = colour ? WHITE_CODE : BLACK_CODE;
      if (table1[run1] === undefined || table2[run2] === undefined) {
        throw new Error(`the toy encoder only handles runs 0-16, got ${run1}/${run2}`);
      }
      bits += '001' + table1[run1] + table2[run2];
      a0 = a2;
    }
    reference = row;
  }

  while (bits.length % 8) bits += '0';
  const bytes = Buffer.alloc(bits.length / 8);
  for (let i = 0; i < bytes.length; i++) bytes[i] = parseInt(bits.slice(i * 8, i * 8 + 8), 2);
  return bytes;
}

function roundTrip(name, pattern) {
  const columns = pattern[0].length;
  const rows = pattern.map((r) => Uint8Array.from([...r].map((c) => (c === '#' ? 1 : 0))));
  const encoded = encodeGroup4(rows, columns);
  const decoded = decodeGroup4(encoded, columns, rows.length);
  let same = true;
  for (let y = 0; y < rows.length; y++) {
    for (let x = 0; x < columns; x++) {
      if (decoded[y * columns + x] !== rows[y][x]) same = false;
    }
  }
  const shown = [];
  for (let y = 0; y < rows.length; y++) {
    let line = '';
    for (let x = 0; x < columns; x++) line += decoded[y * columns + x] ? '#' : '.';
    shown.push(line);
  }
  check(`G4 round trip: ${name}`, same, same ? '' : `got\n${shown.join('\n')}\nwant\n${pattern.join('\n')}`);
}

process.stdout.write('CCITT G4 decoder\n');
roundTrip('all white', ['................', '................']);
roundTrip('a solid bar', ['................', '....########....', '....########....', '................']);
roundTrip('vertical modes', ['....####........', '...####.........', '..####..........', '...####.........']);
roundTrip('two runs on a line', ['..##....##......', '..##....##......']);
roundTrip('ink at both edges', ['##............##', '##............##']);
roundTrip('a single pixel', ['................', '.......#........', '................']);

process.stdout.write('\nPNG writer\n');
{
  const image = { width: 4, height: 2, grey: Buffer.from([0, 255, 0, 255, 255, 0, 255, 0]) };
  const png = encodePng(image);
  check('PNG signature', png.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])));
  const ihdr = png.subarray(12, 16).toString('latin1');
  check('IHDR first', ihdr === 'IHDR', ihdr);
  check('width and height', png.readUInt32BE(16) === 4 && png.readUInt32BE(20) === 2);
  check('8-bit greyscale', png[24] === 8 && png[25] === 0);
  // Inflate the IDAT back and compare, filter byte included.
  const idatAt = png.indexOf('IDAT', 0, 'latin1');
  const idatLen = png.readUInt32BE(idatAt - 4);
  const raw = zlib.inflateSync(png.subarray(idatAt + 4, idatAt + 4 + idatLen));
  check('pixels survive the round trip', raw.equals(Buffer.from([0, 0, 255, 0, 255, 0, 255, 0, 255, 0])), raw.join(','));
}

process.stdout.write('\nAgainst the scans in docs/\n');
try {
  const file = scanFile(1);
  const pdf = loadPdf(file);
  const order = pageOrder(pdf);
  check('volume 1 page tree', order.length === 623, `${order.length} pages`);

  // Page 15 is the SAI SÓT errata, the page review/errata-ban-in.toml was read from.
  const image = pageImage(pdf, order[14]);
  check('page size', image.width === 1765 && image.height === 2467, `${image.width}x${image.height}`);

  let dark = 0;
  for (let i = 0; i < image.grey.length; i++) if (image.grey[i] === 0) dark++;
  const ink = (dark / image.grey.length) * 100;
  // Printed text covers a few per cent of a page. Inverted polarity gives 93%, which is the
  // mistake this number exists to catch.
  check(`ink coverage ${ink.toFixed(1)}% is in the 2-20% a printed page has`, ink > 2 && ink < 20);

  // The page must not be blank, and must not be one colour.
  const light = image.grey.length - dark;
  check('the page has both ink and paper', dark > 0 && light > 0);
} catch (e) {
  if (String(e.message).includes('no scan of volume')) {
    process.stdout.write('  skip  the source PDFs are not in git; put them under docs/ to run this half\n');
  } else {
    failures++;
    process.stdout.write(`  FAIL  ${e.message}\n`);
  }
}

process.stdout.write(`\n${failures === 0 ? 'all checks passed' : failures + ' check(s) failed'}\n`);
process.exit(failures === 0 ? 0 : 1);
