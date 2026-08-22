'use strict';

// Small markdown renderer for model answers. HTML is escaped before any
// transform, and every tag in the output is produced here, so model text can
// never inject markup. Covers the constructs chat models actually emit:
// headings, fenced code, inline code, bold, italic, links, lists,
// blockquotes, and horizontal rules.

(function attachMarkdown() {
  function escapeHtml(s) {
    return s
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  function inline(text) {
    let out = text;
    out = out.replace(/`([^`\n]+)`/g, (_m, code) => `<code>${code}</code>`);
    out = out.replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>');
    out = out.replace(/__([^_\n]+)__/g, '<strong>$1</strong>');
    out = out.replace(/(^|[\s(])\*([^*\n]+)\*/g, '$1<em>$2</em>');
    out = out.replace(
      /\[([^\]\n]+)\]\((https?:[^)\s]+)\)/g,
      (_m, label, url) => `<a href="#" data-href="${url}">${label}</a>`
    );
    return out;
  }

  function render(raw) {
    const lines = escapeHtml(String(raw || '')).split('\n');
    const html = [];
    let inCode = false;
    let codeBuf = [];
    let listType = null; // 'ul' | 'ol' | null

    const closeList = () => {
      if (listType) {
        html.push(`</${listType}>`);
        listType = null;
      }
    };

    for (const line of lines) {
      const fence = line.match(/^\s*```/);
      if (fence) {
        if (inCode) {
          html.push(`<pre>${codeBuf.join('\n')}</pre>`);
          codeBuf = [];
          inCode = false;
        } else {
          closeList();
          inCode = true;
        }
        continue;
      }
      if (inCode) {
        codeBuf.push(line);
        continue;
      }

      const heading = line.match(/^(#{1,6})\s+(.*)$/);
      if (heading) {
        closeList();
        const level = Math.min(heading[1].length, 3);
        html.push(`<div class="md-h md-h${level}">${inline(heading[2])}</div>`);
        continue;
      }
      if (/^\s*(---+|\*\*\*+)\s*$/.test(line)) {
        closeList();
        html.push('<hr>');
        continue;
      }
      const quote = line.match(/^\s*&gt;\s?(.*)$/);
      if (quote) {
        closeList();
        html.push(`<blockquote>${inline(quote[1])}</blockquote>`);
        continue;
      }
      const bullet = line.match(/^\s*[-*+]\s+(.*)$/);
      const ordered = line.match(/^\s*\d+[.)]\s+(.*)$/);
      if (bullet || ordered) {
        const want = bullet ? 'ul' : 'ol';
        if (listType !== want) {
          closeList();
          html.push(`<${want}>`);
          listType = want;
        }
        html.push(`<li>${inline((bullet || ordered)[1])}</li>`);
        continue;
      }
      closeList();
      if (line.trim() === '') {
        html.push('<div class="md-gap"></div>');
      } else {
        html.push(`<div class="md-p">${inline(line)}</div>`);
      }
    }
    if (inCode) {
      // Stream is mid-fence; show what has arrived so far.
      html.push(`<pre>${codeBuf.join('\n')}</pre>`);
    }
    closeList();
    return html.join('');
  }

  window.renderMarkdownInto = function renderMarkdownInto(el, raw) {
    el.innerHTML = render(raw);
  };
})();
