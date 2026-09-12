#!/usr/bin/env node
// Render one page of the 1895-96 scan as a PNG, so a reviewer can look at the ink.
//
// Every dossier row under `review/` asks a person to "open the page image and look". Until
// now there was no way to do that: the two scan volumes are CCITT Group-4 bitonal images
// inside a PDF, and this machine has no poppler, no ImageMagick and no Python. So the
// decoder is here, in full, with no dependency beyond `node:zlib`.
//
//   node tools/scan-page.mjs <volume> <scanPage> <out.png> [x0 y0 x1 y1]
//
// `volume` is 1 or 2; `scanPage` is the page as the PDF numbers it. The optional box crops
// the result, given as fractions of the page (0..1), which keeps a call readable when the
// interesting thing is one entry in the right-hand column.
//
// Why the resolution matters: volume 1 is 1765x2480 and volume 2 is 1758x2467, about 270 dpi,
// and at that size an individual Han glyph is legible. The third scan in `docs/`
// ("... HTC.pdf") is 700x1050 RGB and cannot settle a glyph, so it is deliberately not
// supported here.

import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';

// ---------------------------------------------------------------------------
// CCITT Group 4 (ITU-T T.6)
// ---------------------------------------------------------------------------
// The run-length code tables of T.4, which T.6 reuses for horizontal mode. Index = run
// length for the terminating codes; the make-up tables are keyed by their bit string.

const WHITE_TERMINATING = ['00110101', '000111', '0111', '1000', '1011', '1100', '1110', '1111', '10011', '10100', '00111', '01000', '001000', '000011', '110100', '110101', '101010', '101011', '0100111', '0001100', '0001000', '0010111', '0000011', '0000100', '0101000', '0101011', '0010011', '0100100', '0011000', '00000010', '00000011', '00011010', '00011011', '00010010', '00010011', '00010100', '00010101', '00010110', '00010111', '00101000', '00101001', '00101010', '00101011', '00101100', '00101101', '00000100', '00000101', '00001010', '00001011', '01010010', '01010011', '01010100', '01010101', '00100100', '00100101', '01011000', '01011001', '01011010', '01011011', '01001010', '01001011', '00110010', '00110011', '00110100'];

const WHITE_MAKEUP = { '11011': 64, '10010': 128, '010111': 192, '0110111': 256, '00110110': 320, '00110111': 384, '01100100': 448, '01100101': 512, '01101000': 576, '01100111': 640, '011001100': 704, '011001101': 768, '011010010': 832, '011010011': 896, '011010100': 960, '011010101': 1024, '011010110': 1088, '011010111': 1152, '011011000': 1216, '011011001': 1280, '011011010': 1344, '011011011': 1408, '010011000': 1472, '010011001': 1536, '010011010': 1600, '011000': 1664, '010011011': 1728 };

const BLACK_TERMINATING = ['0000110111', '010', '11', '10', '011', '0011', '0010', '00011', '000101', '000100', '0000100', '0000101', '0000111', '00000100', '00000111', '000011000', '0000010111', '0000011000', '0000001000', '00001100111', '00001101000', '00001101100', '00000110111', '00000101000', '00000010111', '00000011000', '000011001010', '000011001011', '000011001100', '000011001101', '000001101000', '000001101001', '000001101010', '000001101011', '000011010010', '000011010011', '000011010100', '000011010101', '000011010110', '000011010111', '000001101100', '000001101101', '000011011010', '000011011011', '000001010100', '000001010101', '000001010110', '000001010111', '000001100100', '000001100101', '000001010010', '000001010011', '000000100100', '000000110111', '000000111000', '000000100111', '000000101000', '000001011000', '000001011001', '000000101011', '000000101100', '000001011010', '000001100110', '000001100111'];

const BLACK_MAKEUP = { '0000001111': 64, '000011001000': 128, '000011001001': 192, '000001011011': 256, '000000110011': 320, '000000110100': 384, '000000110101': 448, '0000001101100': 512, '0000001101101': 576, '0000001001010': 640, '0000001001011': 704, '0000001001100': 768, '0000001001101': 832, '0000001110010': 896, '0000001110011': 960, '0000001110100': 1024, '0000001110101': 1088, '0000001110110': 1152, '0000001110111': 1216, '0000001010010': 1280, '0000001010011': 1344, '0000001010100': 1408, '0000001010101': 1472, '0000001011010': 1536, '0000001011011': 1600, '0000001100100': 1664, '0000001100101': 1728 };

// Shared by both colours, for runs above 1728.
const EXTENDED_MAKEUP = { '00000001000': 1792, '00000001100': 1856, '00000001101': 1920, '000000010010': 1984, '000000010011': 2048, '000000010100': 2112, '000000010101': 2176, '000000010110': 2240, '000000010111': 2304, '000000011100': 2368, '000000011101': 2432, '000000011110': 2496, '000000011111': 2560 };

function buildTree(pairs) {
  const root = {};
  for (const [bits, run] of pairs) {
    let node = root;
    for (const bit of bits) node = (node[bit] ||= {});
    node.run = run;
  }
  return root;
}

const WHITE_TREE = buildTree([
  ...WHITE_TERMINATING.map((bits, run) => [bits, run]),
  ...Object.entries(WHITE_MAKEUP),
  ...Object.entries(EXTENDED_MAKEUP),
]);
const BLACK_TREE = buildTree([
  ...BLACK_TERMINATING.map((bits, run) => [bits, run]),
  ...Object.entries(BLACK_MAKEUP),
  ...Object.entries(EXTENDED_MAKEUP),
]);

// The longest legal code in either table is 13 bits; the EOL pattern is 12.
const MAX_CODE_BITS = 14;

class BitReader {
  constructor(buffer) {
    this.buffer = buffer;
    this.position = 0;
  }

  /** Next bit, or -1 past the end. */
  next() {
    const byte = this.buffer[this.position >> 3];
    if (byte === undefined) return -1;
    const bit = (byte >> (7 - (this.position & 7))) & 1;
    this.position++;
    return bit;
  }
}

/** One run length: make-up codes accumulate until a terminating code (< 64) closes the run. */
function readRun(bits, tree) {
  let total = 0;
  for (;;) {
    let node = tree;
    let depth = 0;
    for (;;) {
      const bit = bits.next();
      if (bit < 0) return null;
      node = node[bit];
      depth++;
      if (!node) return null;
      if (node.run !== undefined) break;
      if (depth > MAX_CODE_BITS) return null;
    }
    total += node.run;
    if (node.run < 64) return total;
  }
}

const VERTICAL_OFFSET = { V0: 0, VR1: 1, VR2: 2, VR3: 3, VL1: -1, VL2: -2, VL3: -3 };

/** Read one mode code. Returns a mode name, or null at end of data. */
function readMode(bits) {
  let code = '';
  for (let i = 0; i < MAX_CODE_BITS; i++) {
    const bit = bits.next();
    if (bit < 0) return null;
    code += bit;
    if (code === '1') return 'V0';
    if (code === '011') return 'VR1';
    if (code === '010') return 'VL1';
    if (code === '001') return 'H';
    if (code === '0001') return 'P';
    if (code === '000011') return 'VR2';
    if (code === '000010') return 'VL2';
    if (code === '0000011') return 'VR3';
    if (code === '0000010') return 'VL3';
    if (code === '000000000001') return null; // EOL / EOFB
  }
  return null;
}

/**
 * Decode a G4 stream into one byte per pixel: 1 where the decoder ran a black run, 0 white.
 *
 * This is the decoder's own notion of black, not the PDF's — `/BlackIs1` decides which of
 * the two the page actually wants, and that is applied by the caller.
 */
export function decodeGroup4(data, columns, rows) {
  const bits = new BitReader(data);
  const out = Buffer.alloc(columns * rows, 0);
  // Changing elements of the reference line. An all-white line has none before `columns`.
  let reference = [columns, columns];

  for (let y = 0; y < rows; y++) {
    const current = [];
    let a0 = -1;
    let colourIsBlack = 0;

    while (a0 < columns) {
      // b1 is the first changing element on the reference line right of a0 whose colour is
      // opposite to the colour of a0; b2 is the one after it.
      let i = 0;
      while (i < reference.length && (reference[i] <= a0 || (i & 1) !== colourIsBlack)) i++;
      const b1 = i < reference.length ? reference[i] : columns;
      const b2 = i + 1 < reference.length ? reference[i + 1] : columns;

      const mode = readMode(bits);
      if (mode === null) {
        a0 = columns;
        break;
      }

      if (mode === 'P') {
        // Pass: the colour runs on past b2, and b2 does not become a changing element.
        if (colourIsBlack) for (let x = Math.max(a0, 0); x < b2; x++) out[y * columns + x] = 1;
        a0 = b2;
        continue;
      }

      if (mode === 'H') {
        const first = readRun(bits, colourIsBlack ? BLACK_TREE : WHITE_TREE);
        const second = readRun(bits, colourIsBlack ? WHITE_TREE : BLACK_TREE);
        if (first === null || second === null) {
          a0 = columns;
          break;
        }
        const start = a0 < 0 ? 0 : a0;
        const a1 = Math.min(start + first, columns);
        const a2 = Math.min(a1 + second, columns);
        if (colourIsBlack) for (let x = start; x < a1; x++) out[y * columns + x] = 1;
        else for (let x = a1; x < a2; x++) out[y * columns + x] = 1;
        current.push(a1, a2);
        a0 = a2;
        continue;
      }

      const a1 = Math.max(0, Math.min(b1 + VERTICAL_OFFSET[mode], columns));
      if (colourIsBlack) for (let x = Math.max(a0, 0); x < a1; x++) out[y * columns + x] = 1;
      current.push(a1);
      a0 = a1;
      colourIsBlack ^= 1;
    }

    reference = current.length ? current : [columns, columns];
  }

  return out;
}

// ---------------------------------------------------------------------------
// Just enough PDF to reach a page image
// ---------------------------------------------------------------------------
// Both scan volumes are PDF 1.5 with no object streams, so indexing `N 0 obj` by hand is
// sound. This is a reader for these two files, not a PDF library.

export function loadPdf(file) {
  const buffer = fs.readFileSync(file);
  const text = buffer.toString('latin1');
  const offsets = new Map();
  const pattern = /(\d+)\s+0\s+obj\b/g;
  let match;
  while ((match = pattern.exec(text))) offsets.set(Number(match[1]), match.index + match[0].length);
  return { buffer, text, offsets };
}

function objectBody(pdf, number) {
  const start = pdf.offsets.get(number);
  if (start === undefined) return null;
  return pdf.text.slice(start, pdf.text.indexOf('endobj', start));
}

/** Page object numbers in reading order, by walking /Kids depth-first. */
export function pageOrder(pdf) {
  const catalogAt = pdf.text.search(/\/Type\s*\/Catalog/);
  if (catalogAt < 0) throw new Error('no /Catalog in this PDF');
  // The catalog may write /Pages either side of /Type, so search the whole object.
  const body = pdf.text.slice(pdf.text.lastIndexOf(' obj', catalogAt), pdf.text.indexOf('endobj', catalogAt));
  const rootRef = /\/Pages\s+(\d+)\s+0\s+R/.exec(body);
  if (!rootRef) throw new Error('the /Catalog names no /Pages tree');

  const order = [];
  const seen = new Set();
  const visit = (number) => {
    if (seen.has(number)) return;
    seen.add(number);
    const node = objectBody(pdf, number);
    if (!node) return;
    if (/\/Type\s*\/Page[^s]/.test(node)) {
      order.push(number);
      return;
    }
    const kids = /\/Kids\s*\[([\s\S]*?)\]/.exec(node);
    if (!kids) return;
    for (const kid of kids[1].matchAll(/(\d+)\s+0\s+R/g)) visit(Number(kid[1]));
  };
  visit(Number(rootRef[1]));
  return order;
}

/** The scanned image of one page, as 8-bit grey with ink dark. */
export function pageImage(pdf, pageObject) {
  const page = objectBody(pdf, pageObject);
  let imageRef = null;

  const inlineResources = /\/XObject\s*<<([\s\S]*?)>>/.exec(page);
  if (inlineResources) {
    const ref = /(\d+)\s+0\s+R/.exec(inlineResources[1]);
    if (ref) imageRef = Number(ref[1]);
  } else {
    const indirect = /\/Resources\s+(\d+)\s+0\s+R/.exec(page);
    if (indirect) {
      const resources = objectBody(pdf, Number(indirect[1]));
      const xobject = /\/XObject\s*<<([\s\S]*?)>>/.exec(resources ?? '');
      if (xobject) {
        const ref = /(\d+)\s+0\s+R/.exec(xobject[1]);
        if (ref) imageRef = Number(ref[1]);
      }
    }
  }
  if (imageRef === null) throw new Error(`page object ${pageObject} carries no image`);

  const start = pdf.offsets.get(imageRef);
  const streamAt = pdf.text.indexOf('stream', start);
  const dict = pdf.text.slice(start, streamAt);
  if (!/CCITTFaxDecode/.test(dict)) {
    throw new Error(`page object ${pageObject} is not a CCITT scan: ${dict.slice(0, 120)}`);
  }

  const width = Number(/\/Width\s+(\d+)/.exec(dict)[1]);
  const height = Number(/\/Height\s+(\d+)/.exec(dict)[1]);
  const length = Number(/\/Length\s+(\d+)/.exec(dict)[1]);
  const columnsParm = /\/Columns\s+(\d+)/.exec(dict);
  // `BlackIs1` says the encoded data already uses 1 for black, so the decoder's own 1-runs
  // are the white ones and have to be flipped back. Measured: without this a page comes out
  // 93% dark instead of 7%.
  const blackIs1 = /\/BlackIs1\s+true/.test(dict);

  let dataAt = streamAt + 'stream'.length;
  if (pdf.text[dataAt] === '\r') dataAt++;
  if (pdf.text[dataAt] === '\n') dataAt++;

  const columns = columnsParm ? Number(columnsParm[1]) : width;
  const pixels = decodeGroup4(pdf.buffer.subarray(dataAt, dataAt + length), columns, height);

  const grey = Buffer.alloc(width * height);
  for (let i = 0; i < width * height; i++) {
    grey[i] = (blackIs1 ? 1 - pixels[i] : pixels[i]) ? 0 : 255;
  }
  return { width, height, grey };
}

/** Cut a box out of an image, given as fractions of its width and height. */
export function crop(image, x0, y0, x1, y1) {
  const left = Math.round(x0 * image.width);
  const right = Math.round(x1 * image.width);
  const top = Math.round(y0 * image.height);
  const bottom = Math.round(y1 * image.height);
  const width = right - left;
  const height = bottom - top;
  if (width <= 0 || height <= 0) throw new Error('the crop box is empty');
  const grey = Buffer.alloc(width * height);
  for (let y = 0; y < height; y++) {
    image.grey.copy(grey, y * width, (top + y) * image.width + left, (top + y) * image.width + right);
  }
  return { width, height, grey };
}

// ---------------------------------------------------------------------------
// PNG
// ---------------------------------------------------------------------------

let CRC_TABLE = null;

function crc32(bytes) {
  if (!CRC_TABLE) {
    CRC_TABLE = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      CRC_TABLE[n] = c;
    }
  }
  let c = 0xffffffff;
  for (let i = 0; i < bytes.length; i++) c = CRC_TABLE[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/** An 8-bit greyscale PNG. Filter 0 on every row: the source is bitonal, so filtering buys nothing. */
export function encodePng(image) {
  const { width, height, grey } = image;
  const raw = Buffer.alloc((width + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (width + 1)] = 0;
    grey.copy(raw, y * (width + 1) + 1, y * width, (y + 1) * width);
  }

  const parts = [];
  const chunk = (type, data) => {
    const length = Buffer.alloc(4);
    length.writeUInt32BE(data.length);
    const typed = Buffer.concat([Buffer.from(type, 'latin1'), data]);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(typed));
    parts.push(length, typed, crc);
  };

  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header[8] = 8; // bit depth
  header[9] = 0; // colour type: greyscale
  chunk('IHDR', header);
  chunk('IDAT', zlib.deflateSync(raw, { level: 6 }));
  chunk('IEND', Buffer.alloc(0));

  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), ...parts]);
}

// ---------------------------------------------------------------------------
// Finding the scans
// ---------------------------------------------------------------------------
// The filenames under `docs/` are NFD on disk and the PDFs are not in git, so the file is
// located by pattern rather than by a hand-typed name.

const VOLUME_PATTERN = { 1: /Dai Nam Quoc Am Tu Vi - 1\.pdf$/, 2: /Dai Nam Quoc Am Tu Vi - 2\.pdf$/ };

export function scanFile(volume, docsDir = 'docs') {
  const pattern = VOLUME_PATTERN[volume];
  if (!pattern) throw new Error(`volume must be 1 or 2, got ${volume}`);
  const found = fs.readdirSync(docsDir).find((name) => pattern.test(name));
  if (!found) {
    throw new Error(
      `no scan of volume ${volume} under ${docsDir}/. The source PDFs are not in git — see README.`,
    );
  }
  return path.join(docsDir, found);
}

/** Render one scan page, optionally cropped. */
export function renderScanPage(volume, scanPage, box = null, docsDir = 'docs') {
  const pdf = loadPdf(scanFile(volume, docsDir));
  const order = pageOrder(pdf);
  if (scanPage < 1 || scanPage > order.length) {
    throw new Error(`volume ${volume} has ${order.length} pages; asked for ${scanPage}`);
  }
  const image = pageImage(pdf, order[scanPage - 1]);
  return box ? crop(image, ...box) : image;
}

function main(argv) {
  const [volume, scanPage, out, ...box] = argv;
  if (!volume || !scanPage || !out) {
    process.stderr.write('usage: node tools/scan-page.mjs <volume> <scanPage> <out.png> [x0 y0 x1 y1]\n');
    return 2;
  }
  if (box.length !== 0 && box.length !== 4) {
    process.stderr.write('the crop box needs exactly four fractions: x0 y0 x1 y1\n');
    return 2;
  }
  const image = renderScanPage(Number(volume), Number(scanPage), box.length ? box.map(Number) : null);
  fs.writeFileSync(out, encodePng(image));
  process.stderr.write(`${out}  ${image.width}x${image.height}\n`);
  return 0;
}

// Exact basename, not a suffix: check-scan-page.mjs ends with 'scan-page.mjs' too, and
// importing this module must not fire the command line.
if (process.argv[1] && path.basename(process.argv[1]) === 'scan-page.mjs') {
  process.exit(main(process.argv.slice(2)));
}
