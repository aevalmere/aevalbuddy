// The dictation pill (src-tauri/src/voice/pill.rs): shows the state Rust
// sends and draws the level as a waveform that scrolls right to left.
(() => {
  const wave = [...document.querySelectorAll('#wave > i')];
  const say = document.getElementById('say');
  const WORDS = {
    listening: 'Listening',
    handsfree: 'Hands-free · chord to finish',
    working: 'Writing',
    cleaning: 'Writing',
    done: 'Done',
  };
  const levels = wave.map(() => 0);

  function draw() {
    wave.forEach((bar, i) => bar.style.setProperty('--l', levels[i].toFixed(2)));
  }

  window.aeval.on('voice:level', (p) => {
    if (!p || p.origin !== 'dictation') return;
    levels.shift();
    levels.push(Math.max(0, Math.min(1, Number(p.level) || 0)));
    draw();
  });

  window.aeval.on('voice:progress', (p) => {
    if (p && p.origin === 'dictation' && document.body.dataset.state === 'preparing') {
      say.textContent = `Getting ready ${p.progress}%`;
    }
  });

  window.aeval.on('voice:state', (p) => {
    if (!p || p.origin !== 'dictation') return;
    document.body.dataset.state = p.state;
    if (p.state === 'listening') {
      levels.fill(0);
      draw();
    }
    if (p.state === 'preparing') say.textContent = `Getting ready ${p.progress || 0}%`;
    else if (p.state === 'error') say.textContent = p.message || 'That did not work.';
    else say.textContent = WORDS[p.state] || '';
  });
})();
