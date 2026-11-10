'use strict';

// Markdown for model answers, set as a document: real headings, paragraphs
// that join their soft-wrapped lines, lists that survive blank lines and
// nest, tables, code blocks with a Copy button, and links that show where
// they go.
//
// Model output is untrusted. The whole text is HTML-escaped before any rule
// runs, and every tag in the result is written here, so an answer can never
// inject markup, script or an attribute. Links are only http and https, and
// a click on one is intercepted by the page (output.js) and sent to Rust's
// open-external, which validates it again; nothing here navigates.
//
// blocks(raw) returns the top-level blocks as HTML strings, which is what
// lets a streaming answer append instead of re-rendering (syncInto, below).

(function attachMarkdown() {
  const ESC = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };
  const UNESC = { '&amp;': '&', '&lt;': '<', '&gt;': '>', '&quot;': '"', '&#39;': "'" };
  const escapeHtml = (s) => s.replace(/[&<>"']/g, (c) => ESC[c]);
  const unescapeHtml = (s) => s.replace(/&(amp|lt|gt|quot|#39);/g, (m) => UNESC[m]);

  // Placeholders for code spans and links while the emphasis rules run. NUL
  // is stripped from the input first, so the model cannot forge one.
  const PH = '\u0000';

  // The host a link goes to, shown beside its label when the label is not
  // already the address.
  function hostOf(url) {
    try {
      return new URL(unescapeHtml(url)).host;
    } catch (err) {
      return '';
    }
  }

  const bare = (s) =>
    unescapeHtml(s.replace(/<[^>]+>/g, ''))
      .replace(/[*_`~]/g, '')
      .trim()
      .toLowerCase()
      .replace(/^https?:\/\//, '')
      .replace(/^www\./, '')
      .replace(/\/+$/, '');

  function emphasis(text) {
    let out = text;
    out = out.replace(/\*\*\*([^*\n]+)\*\*\*/g, '<strong><em>$1</em></strong>');
    out = out.replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>');
    out = out.replace(/__([^_\n]+)__/g, '<strong>$1</strong>');
    out = out.replace(/(^|[\s(])\*([^*\n]+)\*/g, '$1<em>$2</em>');
    out = out.replace(/(^|[\s(])_([^_\n]+)_(?=$|[\s).,;:!?])/g, '$1<em>$2</em>');
    out = out.replace(/~~([^~\n]+)~~/g, '<del>$1</del>');
    return out;
  }

  // Inline rules on already-escaped text: code spans first, so nothing inside
  // one is touched, then links (their URL is kept verbatim), then emphasis.
  function inline(text) {
    const codes = [];
    const links = [];
    let out = text.replace(/`([^`\n]+)`/g, (_m, code) => {
      // A short span stays on one line (a flag or a file name never splits
      // at its hyphen); a long one may break, or it would overflow the sheet.
      const cls = code.length > 28 ? ' class="md-long"' : '';
      codes.push(`<code${cls}>${code}</code>`);
      return `${PH}c${codes.length - 1}${PH}`;
    });
    out = out.replace(/\[([^\]\n]+)\]\((https?:\/\/[^)\s]+)\)/g, (_m, label, url) => {
      const host = hostOf(url);
      const plain = bare(label.replace(new RegExp(`${PH}c(\\d+)${PH}`, 'g'), (m, i) => codes[Number(i)]));
      const h = host.replace(/^www\./, '').toLowerCase();
      const showHost = host && plain !== h && plain !== bare(url);
      links.push(
        `<a href="${url}" data-href="${url}" title="${url}">${emphasis(label)}</a>` +
          (showHost ? ` <span class="md-host">(${escapeHtml(host)})</span>` : '')
      );
      return `${PH}l${links.length - 1}${PH}`;
    });
    out = emphasis(out);
    out = out.replace(new RegExp(`${PH}l(\\d+)${PH}`, 'g'), (_m, i) => links[Number(i)]);
    out = out.replace(new RegExp(`${PH}c(\\d+)${PH}`, 'g'), (_m, i) => codes[Number(i)]);
    return out;
  }

  // ------------------------------------------------------------ code blocks

  // The copy and check glyphs of renderer/icons.svg (glass.css masks them).
  const COPY_SVG =
    '<span class="ico i-copy md-copy-idle" aria-hidden="true"></span>' +
    '<span class="ico i-check md-copy-done" aria-hidden="true"></span>';

  function codeBlock(lang, lines) {
    const head = lang ? `<div class="md-code-head"><span class="md-lang">${lang}</span></div>` : '';
    return (
      `<div class="md-code${lang ? ' has-lang' : ''}">${head}` +
      '<button type="button" class="g-icon small md-copy" aria-label="Copy code" title="Copy code">' +
      `${COPY_SVG}</button><pre><code>${lines.join('\n')}</code></pre></div>`
    );
  }

  // ----------------------------------------------------------------- tables

  const TABLE_ROW = /^\|.*\|$/;
  const TABLE_SEP = /^\|?\s*:?-{3,}[\s|:-]*$/;

  function cells(line) {
    let s = line.trim();
    if (s.startsWith('|')) s = s.slice(1);
    if (s.endsWith('|') && !s.endsWith('\\|')) s = s.slice(0, -1);
    return s.split(/(?<!\\)\|/).map((c) => c.replace(/\\\|/g, '|').trim());
  }

  function alignOf(sep) {
    const c = sep.replace(/\s/g, '');
    if (/^:-+:$/.test(c)) return 'center';
    if (/^-+:$/.test(c)) return 'right';
    if (/^:-+$/.test(c)) return 'left';
    return '';
  }

  function table(header, sep, rows) {
    const head = cells(header);
    const aligns = cells(sep).map(alignOf);
    const n = head.length;
    const attr = (i) => (aligns[i] ? ` data-align="${aligns[i]}"` : '');
    const th = head.map((c, i) => `<th scope="col"${attr(i)}>${inline(c)}</th>`).join('');
    const body = rows
      .map((r) => {
        const cs = cells(r);
        const tds = [];
        for (let i = 0; i < n; i += 1) tds.push(`<td${attr(i)}>${inline(cs[i] || '')}</td>`);
        return `<tr>${tds.join('')}</tr>`;
      })
      .join('');
    return `<div class="md-table"><table><thead><tr>${th}</tr></thead><tbody>${body}</tbody></table></div>`;
  }

  // ------------------------------------------------------------------ lists

  // A list is built as a tree and written out when it closes, because a
  // blank line, a nested list or an indented code block can still belong to
  // the item above it.
  function listHtml(node) {
    const start = node.type === 'ol' && node.start !== 1 ? ` start="${node.start}"` : '';
    const items = node.items
      .map((it) => {
        let firstText = true;
        const parts = it.parts
          .map((p) => {
            if (p.kind === 'list') return listHtml(p.node);
            if (p.kind === 'code') return p.html;
            const html = inline(p.lines.join(' '));
            if (firstText) {
              firstText = false;
              return html;
            }
            return `<p class="md-lp">${html}</p>`;
          })
          .join('');
        return `<li>${parts}</li>`;
      })
      .join('');
    return `<${node.type}${start}>${items}</${node.type}>`;
  }

  const ITEM = /^(\s*)([-*+]|(\d{1,9})[.)])\s+(.*)$/;
  const FENCE = /^(\s*)(`{3,}|~{3,})\s*([\w+#.-]*)/;
  const indentOf = (s) => s.replace(/\t/g, '    ').length;

  // ----------------------------------------------------------------- blocks

  function blocks(raw) {
    const src = String(raw || '')
      .replace(/\u0000/g, '')
      .replace(/\r\n?/g, '\n');
    const lines = escapeHtml(src).split('\n');
    const out = [];

    let para = [];
    const flushPara = () => {
      if (!para.length) return;
      out.push(`<p class="md-p">${inline(para.join(' '))}</p>`);
      para = [];
    };

    // Open lists, outermost first: { type, start, level, items: [{ parts }] }.
    let stack = [];
    let pendingBreak = false;
    const item = () => {
      const top = stack[stack.length - 1];
      return top && top.items[top.items.length - 1];
    };
    const closeLists = () => {
      if (stack.length) out.push(listHtml(stack[0]));
      stack = [];
      pendingBreak = false;
    };
    const addText = (text, fresh) => {
      const it = item();
      const last = it.parts[it.parts.length - 1];
      if (!fresh && last && last.kind === 'text') last.lines.push(text);
      else it.parts.push({ kind: 'text', lines: [text] });
    };

    // A blank line keeps the list open only when what follows still belongs
    // to it: an item of the same type, a nested item, or indented text.
    const continuesList = (line) => {
      const m = ITEM.exec(line);
      if (m) {
        const level = Math.floor(indentOf(m[1]) / 2);
        if (level > stack[0].level) return true;
        return (m[3] ? 'ol' : 'ul') === stack[0].type;
      }
      return indentOf(/^\s*/.exec(line)[0]) >= 2;
    };

    let code = null; // { lang, lines, indent, fence, inList }
    const endCode = () => {
      const html = codeBlock(code.lang, code.lines);
      if (code.inList && item()) item().parts.push({ kind: 'code', html });
      else out.push(html);
      code = null;
    };

    for (let i = 0; i < lines.length; i += 1) {
      const line = lines[i];

      if (code) {
        const close = FENCE.exec(line);
        if (close && close[2][0] === code.fence[0] && close[2].length >= code.fence.length && !line.trim().slice(close[2].length)) {
          endCode();
          continue;
        }
        // Drop the fence's own indent from each line, never more.
        let k = 0;
        while (k < code.indent && line[k] === ' ') k += 1;
        code.lines.push(line.slice(k));
        continue;
      }

      const fence = FENCE.exec(line);
      if (fence) {
        flushPara();
        const indent = indentOf(fence[1]);
        const inList = stack.length > 0 && indent >= 2;
        if (!inList) closeLists();
        pendingBreak = false;
        code = { lang: fence[3] || '', lines: [], indent, fence: fence[2], inList };
        continue;
      }

      if (line.trim() === '') {
        flushPara();
        if (stack.length) {
          let j = i + 1;
          while (j < lines.length && lines[j].trim() === '') j += 1;
          if (j < lines.length && continuesList(lines[j])) pendingBreak = true;
          else closeLists();
        }
        continue;
      }

      const heading = /^\s{0,3}(#{1,6})\s+(.*?)(?:\s+#+)?\s*$/.exec(line);
      if (heading) {
        flushPara();
        closeLists();
        // The sheet title is the page's h1, so the answer's levels start at 2.
        const level = Math.min(heading[1].length, 3);
        out.push(`<h${level + 1} class="md-h md-h${level}">${inline(heading[2])}</h${level + 1}>`);
        continue;
      }

      if (/^\s{0,3}([-*_])(\s*\1){2,}\s*$/.test(line)) {
        flushPara();
        closeLists();
        out.push('<hr>');
        continue;
      }

      if (TABLE_ROW.test(line.trim()) && i + 1 < lines.length && TABLE_SEP.test(lines[i + 1].trim())) {
        flushPara();
        closeLists();
        const rows = [];
        let j = i + 2;
        while (j < lines.length && lines[j].trim().startsWith('|')) {
          rows.push(lines[j]);
          j += 1;
        }
        out.push(table(line, lines[i + 1], rows));
        i = j - 1;
        continue;
      }

      const quote = /^\s{0,3}&gt;\s?(.*)$/.exec(line);
      if (quote) {
        flushPara();
        closeLists();
        const paras = [[]];
        let j = i;
        let m = quote;
        while (m) {
          if (m[1].trim() === '') {
            if (paras[paras.length - 1].length) paras.push([]);
          } else {
            paras[paras.length - 1].push(m[1]);
          }
          j += 1;
          m = j < lines.length ? /^\s{0,3}&gt;\s?(.*)$/.exec(lines[j]) : null;
        }
        const ps = paras.filter((p) => p.length).map((p) => `<p>${inline(p.join(' '))}</p>`);
        out.push(`<blockquote>${ps.join('')}</blockquote>`);
        i = j - 1;
        continue;
      }

      const m = ITEM.exec(line);
      if (m) {
        flushPara();
        const level = Math.floor(indentOf(m[1]) / 2);
        const type = m[3] ? 'ol' : 'ul';
        const start = m[3] ? Number(m[3]) : 1;
        // Back out of lists nested deeper than this item; a root list
        // indented deeper than it ends.
        while (stack.length > 1 && stack[stack.length - 1].level > level) stack.pop();
        if (stack.length === 1 && stack[0].level > level) closeLists();
        let top = stack[stack.length - 1];
        // Same level, other type: that list ends and a new one starts.
        if (top && top.level === level && top.type !== type) {
          if (stack.length === 1) closeLists();
          else stack.pop();
          top = stack[stack.length - 1];
        }
        if (!top) {
          stack = [{ type, start, level, items: [] }];
        } else if (top.level < level) {
          const node = { type, start, level, items: [] };
          top.items[top.items.length - 1].parts.push({ kind: 'list', node });
          stack.push(node);
        }
        stack[stack.length - 1].items.push({ parts: [{ kind: 'text', lines: [m[4]] }] });
        pendingBreak = false;
        continue;
      }

      if (stack.length) {
        // Text under an item: a lazy continuation of its line, or after a
        // blank line an indented paragraph of its own.
        if (!pendingBreak || indentOf(/^\s*/.exec(line)[0]) >= 2) {
          addText(line.trim(), pendingBreak);
          pendingBreak = false;
          continue;
        }
        closeLists();
      }
      para.push(line.trim());
    }

    // Mid-stream: whatever has arrived so far is shown, open fence included.
    if (code) endCode();
    flushPara();
    closeLists();
    return out;
  }

  function render(raw) {
    return blocks(raw).join('');
  }

  // ------------------------------------------------------------- streaming

  // A streamed answer keeps the nodes of every block that has not changed,
  // so a selection in an earlier paragraph survives, earlier blocks never
  // re-layout, and only the block still being written is touched per frame.
  // A new block fades in (md-in); the block being written carries a still
  // trailing dot until the answer is done.
  const tpl = typeof document !== 'undefined' ? document.createElement('template') : null;
  const openTag = (html) => html.slice(0, html.indexOf('>') + 1);
  const innerOf = (html) => html.slice(openTag(html).length, html.lastIndexOf('</'));

  function nodeOf(html) {
    tpl.innerHTML = html;
    return tpl.content.firstElementChild;
  }

  // The innermost element the last words of a block sit in.
  function tailOf(node) {
    let el = node;
    while (el) {
      if (el.matches('div.md-code')) return el.querySelector('code');
      if (el.matches('hr')) return null;
      const last = el.lastElementChild;
      if (!last) return el;
      if (el.matches('ol, ul, blockquote, div.md-table, table, thead, tbody, tr')) {
        el = last;
      } else if (el.matches('li') && last.matches('ol, ul, div.md-code, p.md-lp')) {
        el = last;
      } else {
        return el;
      }
    }
    return null;
  }

  // Horizontal scroll in a code block or table is shown by a fade at the
  // edge, only while there is more to the right.
  function markScrollOf(box) {
    const host = box.closest('.md-code') || box;
    host.classList.toggle('scrolls-x', box.scrollWidth - box.clientWidth - box.scrollLeft > 1);
  }

  function markScroll(root) {
    if (root.matches && root.matches('.md-code pre, .md-table')) markScrollOf(root);
    for (const box of root.querySelectorAll('.md-code pre, .md-table')) markScrollOf(box);
  }

  function syncInto(el, raw, opts) {
    const o = opts || {};
    const next = blocks(raw);
    let st = el.__md;
    if (!st || st.list.length !== el.childElementCount || (st.list[0] && st.list[0].node.parentNode !== el)) {
      el.textContent = '';
      st = { list: [] };
      el.__md = st;
    }
    const cur = st.list;
    const now = performance.now();
    const changed = [];
    for (let j = 0; j < next.length; j += 1) {
      const html = next[j];
      const old = cur[j];
      if (old && old.html === html) continue;
      if (old && openTag(old.html) === openTag(html) && html !== openTag(html)) {
        old.node.innerHTML = innerOf(html);
        old.html = html;
        changed.push(old.node);
        continue;
      }
      const node = nodeOf(html);
      if (old) {
        // A block that is still fading in keeps fading in.
        if (now - old.born < 160) node.classList.add('md-in');
        el.replaceChild(node, old.node);
        cur[j] = { html, node, born: old.born };
      } else {
        if (o.animate) node.classList.add('md-in');
        el.appendChild(node);
        cur.push({ html, node, born: now });
      }
      changed.push(node);
    }
    while (cur.length > next.length) cur.pop().node.remove();
    for (const t of el.querySelectorAll('.md-tail')) t.classList.remove('md-tail');
    if (o.busy && cur.length) {
      const t = tailOf(cur[cur.length - 1].node);
      if (t) t.classList.add('md-tail');
    }
    for (const n of changed) markScroll(n);
  }

  window.AevalMarkdown = { render, blocks, inline, markScroll, markScrollOf };

  window.renderMarkdownInto = function renderMarkdownInto(el, raw) {
    el.__md = null;
    el.innerHTML = render(raw);
    markScroll(el);
  };

  window.streamMarkdownInto = syncInto;
})();
