'use strict';

// Generates build/icon.ico, the application icon: the .exe, the installer, the
// Start Menu shortcut and the Add/Remove Programs entry all read it. Without
// it electron-builder says so on every run -- "default Electron icon is used,
// reason=application icon is not set" -- and the app ships wearing Electron's
// logo.
//
// It is generated rather than committed for the same reason the tray icon is
// carried as base64: the repository stays text-only. And it is generated from
// renderer/buddy.html rather than from a copy of the path, so the icon is the
// pointer. Move the arrow and the icon moves with it; there is no second
// drawing to keep in sync.
//
// No dependencies, because the app has none: the path is flattened here, the
// rasteriser is a supersampled distance test, and PNG and ICO are both short
// enough to write out by hand.

const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

const root = path.join(__dirname, '..');
const outDir = path.join(root, 'build');
const outFile = path.join(outDir, 'icon.ico');

// Windows asks for these; 256 is the one Explorer shows at large sizes.
const SIZES = [16, 24, 32, 48, 64, 128, 256];
const SS = 4; // supersampling per axis

// ---------------------------------------------------------------- the source

/** Pull the arrow's geometry and colours out of the files that draw it. */
function readArrow() {
  const html = fs.readFileSync(path.join(root, 'renderer', 'buddy.html'), 'utf8');
  const css = fs.readFileSync(path.join(root, 'renderer', 'cursor-buddy.css'), 'utf8');

  const viewBox = /class="arrow"[^>]*viewBox="([^"]+)"/.exec(html);
  const d = /class="arrow-body"[\s\S]*?d="([^"]+)"/.exec(html);
  if (!viewBox || !d) throw new Error('renderer/buddy.html: arrow path not found');

  const body = /\.arrow-body\s*\{([\s\S]*?)\}/.exec(css);
  if (!body) throw new Error('renderer/cursor-buddy.css: .arrow-body not found');
  const pick = (prop, fallback) => {
    const m = new RegExp(prop + '\\s*:\\s*([^;]+);').exec(body[1]);
    return m ? m[1].trim() : fallback;
  };

  const [, , vw, vh] = viewBox[1].trim().split(/[\s,]+/).map(Number);
  return {
    d: d[1],
    vw,
    vh,
    fill: hex(pick('fill', '#823A80')),
    stroke: hex(pick('stroke', '#C9CFDD')),
    strokeWidth: parseFloat(pick('stroke-width', '1.6')),
  };
}

function hex(s) {
  const m = /#([0-9a-f]{6})/i.exec(s);
  if (!m) throw new Error('not a hex colour: ' + s);
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

// ------------------------------------------------------------------ the path

/**
 * Flattens the subset of SVG path syntax the arrow uses (absolute M, L, Q, Z)
 * into a closed polygon. Quadratics are sampled rather than subdivided
 * adaptively: at icon sizes the difference is well under a pixel.
 */
function flatten(d) {
  const tokens = d.match(/[MLQZ]|-?\d*\.?\d+/gi);
  const pts = [];
  let i = 0;
  let cx = 0;
  let cy = 0;
  let cmd = null;
  const num = () => parseFloat(tokens[i++]);

  while (i < tokens.length) {
    if (/[MLQZ]/i.test(tokens[i])) cmd = tokens[i++].toUpperCase();
    if (cmd === 'Z') break;
    if (cmd === 'M' || cmd === 'L') {
      cx = num();
      cy = num();
      pts.push([cx, cy]);
    } else if (cmd === 'Q') {
      const qx = num();
      const qy = num();
      const x = num();
      const y = num();
      const STEPS = 24;
      for (let s = 1; s <= STEPS; s++) {
        const t = s / STEPS;
        const u = 1 - t;
        pts.push([
          u * u * cx + 2 * u * t * qx + t * t * x,
          u * u * cy + 2 * u * t * qy + t * t * y,
        ]);
      }
      cx = x;
      cy = y;
    } else {
      throw new Error('unsupported path command: ' + cmd);
    }
  }
  return pts;
}

function segments(pts) {
  const segs = [];
  for (let i = 0; i < pts.length; i++) {
    const a = pts[i];
    const b = pts[(i + 1) % pts.length];
    segs.push([a[0], a[1], b[0], b[1]]);
  }
  return segs;
}

function bounds(pts) {
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -Infinity;
  let y1 = -Infinity;
  for (const [x, y] of pts) {
    if (x < x0) x0 = x;
    if (y < y0) y0 = y;
    if (x > x1) x1 = x;
    if (y > y1) y1 = y;
  }
  return { x0, y0, x1, y1 };
}

function inside(segs, px, py) {
  let hit = false;
  for (const [ax, ay, bx, by] of segs) {
    if (ay > py !== by > py) {
      const t = (py - ay) / (by - ay);
      if (px < ax + t * (bx - ax)) hit = !hit;
    }
  }
  return hit;
}

function distSq(segs, px, py) {
  let best = Infinity;
  for (const [ax, ay, bx, by] of segs) {
    const dx = bx - ax;
    const dy = by - ay;
    const len = dx * dx + dy * dy;
    let t = len ? ((px - ax) * dx + (py - ay) * dy) / len : 0;
    t = t < 0 ? 0 : t > 1 ? 1 : t;
    const ex = ax + t * dx - px;
    const ey = ay + t * dy - py;
    const dsq = ex * ex + ey * ey;
    if (dsq < best) best = dsq;
  }
  return best;
}

// ------------------------------------------------------------------- surface

/* The surface the arrow sits on is the material DESIGN.md settles on for every
   other window: a top-lit gradient between the two greys, one crisp edge, and
   nothing that needs the compositor. Full bleed with a Windows-shaped corner
   radius, because an icon is not a floating window and padding inside a tile
   only makes the mark smaller. */
const SURFACE_TOP = [62, 64, 74]; // #3E404A
const SURFACE_BOTTOM = [38, 41, 48]; // #262930
const EDGE = [201, 207, 221]; // pale slate, at low alpha along the top

function insideRounded(x, y, size, radius) {
  // Signed coverage of a rounded square, in the same units as the sample grid.
  const r = radius;
  const cx = Math.min(Math.max(x, r), size - r);
  const cy = Math.min(Math.max(y, r), size - r);
  const dx = x - cx;
  const dy = y - cy;
  if (dx === 0 && dy === 0) return true;
  return dx * dx + dy * dy <= r * r;
}

function render(size, arrow, segs, bb) {
  const rgba = Buffer.alloc(size * size * 4);
  const radius = size * 0.2;

  // Fit the arrow into the tile with room to breathe, keeping its aspect.
  const scale = (size * 0.56) / Math.max(bb.x1 - bb.x0, bb.y1 - bb.y0);
  const ox = size / 2 - ((bb.x0 + bb.x1) / 2) * scale;
  const oy = size / 2 - ((bb.y0 + bb.y1) / 2) * scale;

  const half = arrow.strokeWidth / 2;
  const halfSq = half * half;
  // The arrow's own box in device pixels, so the background does no path work.
  const px0 = bb.x0 * scale + ox - half * scale - 2;
  const px1 = bb.x1 * scale + ox + half * scale + 2;
  const py0 = bb.y0 * scale + oy - half * scale - 2;
  const py1 = bb.y1 * scale + oy + half * scale + 2;

  const step = 1 / SS;
  const start = step / 2;

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      let cov = 0; // rounded-square coverage
      let fillHits = 0;
      let strokeHits = 0;

      for (let sy = 0; sy < SS; sy++) {
        const py = y + start + sy * step;
        for (let sx = 0; sx < SS; sx++) {
          const px = x + start + sx * step;
          if (insideRounded(px, py, size, radius)) cov++;
          if (px < px0 || px > px1 || py < py0 || py > py1) continue;
          // Back into path space, where the stroke width is defined.
          const ux = (px - ox) / scale;
          const uy = (py - oy) / scale;
          const on = distSq(segs, ux, uy) <= halfSq;
          if (on) strokeHits++;
          else if (inside(segs, ux, uy)) fillHits++;
        }
      }

      const total = SS * SS;
      const i = (y * size + x) * 4;
      if (!cov) continue;

      // Background: the top-lit gradient, with a slate bevel on the top edge.
      const t = y / (size - 1);
      let r = SURFACE_TOP[0] + (SURFACE_BOTTOM[0] - SURFACE_TOP[0]) * t;
      let g = SURFACE_TOP[1] + (SURFACE_BOTTOM[1] - SURFACE_TOP[1]) * t;
      let b = SURFACE_TOP[2] + (SURFACE_BOTTOM[2] - SURFACE_TOP[2]) * t;
      const bevel = Math.max(0, 1 - y / Math.max(1, size * 0.04)) * 0.09;
      r += (EDGE[0] - r) * bevel;
      g += (EDGE[1] - g) * bevel;
      b += (EDGE[2] - b) * bevel;

      // The arrow over it: stroke first, then fill inside the stroke.
      const fa = fillHits / total;
      const sa = strokeHits / total;
      r = r * (1 - fa) + arrow.fill[0] * fa;
      g = g * (1 - fa) + arrow.fill[1] * fa;
      b = b * (1 - fa) + arrow.fill[2] * fa;
      r = r * (1 - sa) + arrow.stroke[0] * sa;
      g = g * (1 - sa) + arrow.stroke[1] * sa;
      b = b * (1 - sa) + arrow.stroke[2] * sa;

      rgba[i] = Math.round(r);
      rgba[i + 1] = Math.round(g);
      rgba[i + 2] = Math.round(b);
      rgba[i + 3] = Math.round((cov / total) * 255);
    }
  }
  return rgba;
}

// ----------------------------------------------------------------- encoding

let CRC_TABLE = null;
function crc32(buf) {
  if (!CRC_TABLE) {
    CRC_TABLE = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      CRC_TABLE[n] = c;
    }
  }
  let c = -1;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 255] ^ (c >>> 8);
  return (c ^ -1) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

function png(size, rgba) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // truecolour with alpha
  const stride = size * 4;
  const raw = Buffer.alloc((stride + 1) * size);
  for (let y = 0; y < size; y++) {
    raw[y * (stride + 1)] = 0; // filter: none
    rgba.copy(raw, y * (stride + 1) + 1, y * stride, (y + 1) * stride);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

/** ICONDIR + one ICONDIRENTRY per size, then the PNG payloads. */
function ico(images) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0); // reserved
  header.writeUInt16LE(1, 2); // type: icon
  header.writeUInt16LE(images.length, 4);

  let offset = 6 + images.length * 16;
  const entries = [];
  for (const { size, data } of images) {
    const e = Buffer.alloc(16);
    e[0] = size >= 256 ? 0 : size; // 0 means 256
    e[1] = size >= 256 ? 0 : size;
    e[2] = 0; // palette size
    e[3] = 0; // reserved
    e.writeUInt16LE(1, 4); // colour planes
    e.writeUInt16LE(32, 6); // bits per pixel
    e.writeUInt32LE(data.length, 8);
    e.writeUInt32LE(offset, 12);
    entries.push(e);
    offset += data.length;
  }
  return Buffer.concat([header, ...entries, ...images.map((i) => i.data)]);
}

// --------------------------------------------------------------------- main

function main() {
  const arrow = readArrow();
  const pts = flatten(arrow.d);
  const segs = segments(pts);
  const bb = bounds(pts);

  const images = SIZES.map((size) => ({
    size,
    data: png(size, render(size, arrow, segs, bb)),
  }));

  fs.mkdirSync(outDir, { recursive: true });
  fs.writeFileSync(outFile, ico(images));
  const kb = (fs.statSync(outFile).size / 1024).toFixed(1);
  console.log(
    `[icon] ${path.relative(root, outFile)} ${kb}kB  ` +
      `${SIZES.join('/')}  from ${pts.length} flattened points`
  );
}

main();
