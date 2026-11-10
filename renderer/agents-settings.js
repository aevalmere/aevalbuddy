// Settings' Agents page (settings.html #pg-watch). Loads after settings.js
// and uses its helpers: call, listen, failed, $, el, checkRadios. Every
// change is saved at once through agents:set, which answers with the whole
// page's state; a change that could not be saved goes back to what is
// stored.
(() => {
  if (!document.getElementById('pg-watch')) return;
  let snap = null;

  const w = {
    enabled: $('watch-enabled'),
    list: $('watch-agent-list'),
    status: $('watch-status'),
    awakeMore: $('watch-awake-more'),
    lidRow: $('watch-lid-row'),
    awakeNote: $('watch-awake-note'),
    test: $('watch-test'),
  };
  const switches = [...document.querySelectorAll('#pg-watch input[data-watch]')];
  const speak = (t) => window.AevalA11y && window.AevalA11y.say(t);

  function status(text, error) {
    w.status.hidden = !text;
    w.status.textContent = text || '';
    w.status.classList.toggle('error', !!error);
  }

  // One switch per agent found on this PC, with what it still needs from
  // the person (restart open sessions, trust the hooks).
  function renderAgents() {
    const found = snap.agents.filter((a) => a.found);
    w.list.hidden = !snap.prefs.enabled || !found.length;
    w.list.textContent = '';
    for (const a of found) {
      const row = el('label', { class: 'switch' });
      const text = el('span', null, a.name);
      const says = a.error || (a.on && a.connected ? a.note : '');
      if (says) text.appendChild(el('span', { class: a.error ? 'watch-says error' : 'watch-says' }, says));
      const box = el('input', { type: 'checkbox', role: 'switch' });
      box.checked = a.on;
      box.addEventListener('change', () => {
        const off = new Set(snap.prefs.agentsOff);
        if (box.checked) off.delete(a.id);
        else off.add(a.id);
        save({ agentsOff: [...off] });
      });
      row.append(text, box);
      w.list.appendChild(row);
    }
  }

  function render() {
    if (!snap) return;
    const p = snap.prefs;
    w.enabled.checked = p.enabled;
    for (const s of switches) s.checked = !!p[s.dataset.watch];
    for (const under of document.querySelectorAll('#pg-watch [data-watch-under]')) {
      under.hidden = !p[under.dataset.watchUnder];
    }
    w.awakeMore.hidden = !p.keepAwake;
    w.lidRow.hidden = !snap.hasLid;
    checkRadios('[data-watch-time]', (r) => Number(r.dataset.watchTime) === p.cardTime);
    renderAgents();

    const found = snap.agents.filter((a) => a.found);
    const connected = found.filter((a) => a.on && a.connected).length;
    const failedOnes = found.filter((a) => a.error);
    if (!p.enabled) $('watch-agents-summary').textContent = 'Off';
    else if (!found.length) $('watch-agents-summary').textContent = 'On, none found yet';
    else $('watch-agents-summary').textContent = `${connected} of ${found.length} connected`;
    if (p.enabled && !found.length) status('No coding agent was found on this PC. One installed later is picked up within 10 minutes.');
    else if (failedOnes.length) status(`Could not connect ${failedOnes.map((a) => a.name).join(', ')}. The reason is under its name.`, true);
    else status('');

    const awake = !p.keepAwake ? 'Off' : p.lidAwake && snap.hasLid ? 'On, lid closed too' : p.displayOn ? 'On, screen too' : 'On';
    $('watch-awake-summary').textContent = snap.working && snap.awake ? `${awake} · holding now` : awake;
    w.awakeNote.textContent = p.lidAwake && snap.hasLid
      ? 'While agents work, closing the lid does nothing; your own lid setting comes back when the last one stops.'
      : 'Windows will not sleep until the last agent stops. The screen can still turn off unless you keep it on.';

    const cards = [p.notifyDone && 'Finished', p.notifyNeedsYou && 'needs you'].filter(Boolean);
    $('watch-cards-summary').textContent = cards.length ? cards.join(' and ').replace(/^n/, 'N') : 'Off';
    w.test.disabled = false;
  }

  function take(r) {
    if (failed(r) || !r || !r.prefs) return false;
    snap = r;
    render();
    return true;
  }

  function load() {
    return call('agents:get').then(take);
  }

  function save(patch) {
    return call('agents:set', patch, 15000).then((r) => {
      if (take(r)) return true;
      status("Couldn't save that change.", true);
      speak("Couldn't save that change.");
      render();
      return false;
    });
  }

  w.enabled.addEventListener('change', () => {
    const on = w.enabled.checked;
    save({ enabled: on }).then((ok) => {
      if (ok) speak(on ? 'Watching your coding agents.' : 'Agent watching is off.');
    });
  });
  for (const s of switches) {
    s.addEventListener('change', () => save({ [s.dataset.watch]: s.checked }));
  }
  for (const r of document.querySelectorAll('[data-watch-time]')) {
    r.addEventListener('click', () => save({ cardTime: Number(r.dataset.watchTime) }));
  }
  w.test.addEventListener('click', () => {
    w.test.disabled = true;
    call('agents:test').then(() => {
      w.test.disabled = false;
    });
  });

  listen('agents:state', take);
  window.addEventListener('focus', load);
  load();
})();
