// Every lane's ui-shots file, in the order shots.mjs runs them. The shape of
// a lane file is documented at the top of shots.mjs.
import { BACKDROPS } from './kit.mjs';
import * as harness from './lanes/harness.mjs';
import * as platform from './lanes/platform.mjs';
import * as windows from './lanes/windows.mjs';
import * as capture from './lanes/capture.mjs';
import * as llm from './lanes/llm.mjs';
import * as agent from './lanes/agent.mjs';
import * as bar from './lanes/bar.mjs';
import * as sheet from './lanes/sheet.mjs';
import * as settings from './lanes/settings.mjs';
import * as voice from './lanes/voice.mjs'; // voice lane
import * as polish from './lanes/polish.mjs'; // polish lane
import * as watch from './lanes/watch.mjs'; // agents lane

export const LANES = { harness, platform, windows, capture, llm, agent, bar, sheet, settings, voice, polish, watch };

// One problem string per way a lane file breaks the shape; empty when fine.
export function shapeProblems(lanes = LANES) {
  const out = [];
  const names = new Map();
  for (const [lane, m] of Object.entries(lanes)) {
    if (!Array.isArray(m.scenes)) out.push(`${lane}: scenes is not an array`);
    if (!m.mocks || typeof m.mocks !== 'object' || Array.isArray(m.mocks)) out.push(`${lane}: mocks is not an object`);
    if (!Array.isArray(m.checks)) out.push(`${lane}: checks is not an array`);
    if (!Array.isArray(m.cssLintEnforce)) out.push(`${lane}: cssLintEnforce is not an array`);
    for (const s of Array.isArray(m.scenes) ? m.scenes : []) {
      const id = s && s.name;
      if (!id || !/^[\w.-]+$/.test(id)) out.push(`${lane}: a scene has no usable name (${JSON.stringify(id)})`);
      else if (names.has(id)) out.push(`${lane}: scene ${id} is also in ${names.get(id)}`);
      else names.set(id, lane);
      if (!s || typeof s.run !== 'function') out.push(`${lane}: scene ${id} has no run(t)`);
      if (!s || !Array.isArray(s.windows) || !s.windows.length) out.push(`${lane}: scene ${id} has no windows`);
      if (s && s.backdrop !== undefined && !BACKDROPS.includes(s.backdrop)) out.push(`${lane}: scene ${id} has backdrop ${JSON.stringify(s.backdrop)}, not one of ${BACKDROPS.join(', ')}`);
      if (s && s.scales !== undefined && !(Array.isArray(s.scales) && s.scales.every((x) => x === 1 || x === 1.5))) out.push(`${lane}: scene ${id} scales must be a list of 1 and 1.5`);
      for (const w of (s && s.windows) || []) {
        if (!w.name || !w.src || ![w.x, w.y, w.w, w.h].every(Number.isFinite)) out.push(`${lane}: scene ${id} has a window without name, src, x, y, w, h`);
      }
    }
    for (const c of Array.isArray(m.checks) ? m.checks : []) {
      if (!c || !c.name || typeof c.run !== 'function') out.push(`${lane}: a check has no name or run(ctx)`);
    }
    for (const f of Array.isArray(m.cssLintEnforce) ? m.cssLintEnforce : []) {
      if (typeof f !== 'string' || !f.endsWith('.css')) out.push(`${lane}: cssLintEnforce entry ${JSON.stringify(f)} is not a .css path`);
    }
  }
  return out;
}
