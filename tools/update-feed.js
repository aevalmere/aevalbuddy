'use strict';

// A stand-in for GitHub releases, for tools/update-check.ps1 on a runner.
// Serves a folder over http on 127.0.0.1 with single-range support, which is
// what electron-updater's differential download asks for when the feed is
// configured with useMultipleRangeRequest: false. Every request is written to
// a JSON-lines log, so the test can tell a differential download (ranges of
// the exe) from a full one (one plain GET of it).
//
//   node tools/update-feed.js <root> <port> <log>
//
// Layout mirrors GitHub's: <root>/v1.2.3/latest.yml, AevalBuddy-Setup.exe and
// its .blockmap, so the old-blockmap URL electron-updater derives by swapping
// the version in the path exists here as it does there.

const fs = require('fs');
const http = require('http');
const path = require('path');

const [root, port, logFile] = process.argv.slice(2);
if (!root || !port || !logFile) {
  console.error('usage: node tools/update-feed.js <root> <port> <log>');
  process.exit(2);
}
const base = path.resolve(root);

function record(entry) {
  fs.appendFileSync(logFile, `${JSON.stringify({ t: new Date().toISOString(), ...entry })}\n`);
}

http
  .createServer((req, res) => {
    const urlPath = decodeURIComponent(new URL(req.url, 'http://x').pathname);
    const file = path.join(base, path.normalize(urlPath));
    const range = req.headers.range || '';
    if (!file.startsWith(base) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
      res.writeHead(404).end();
      record({ method: req.method, path: urlPath, range, status: 404 });
      return;
    }
    const size = fs.statSync(file).size;
    const m = /^bytes=(\d*)-(\d*)$/.exec(range);
    if (m) {
      const start = m[1] === '' ? size - Number(m[2]) : Number(m[1]);
      const end = m[1] === '' || m[2] === '' ? size - 1 : Math.min(Number(m[2]), size - 1);
      if (start > end || start >= size) {
        res.writeHead(416, { 'Content-Range': `bytes */${size}` }).end();
        record({ method: req.method, path: urlPath, range, status: 416 });
        return;
      }
      res.writeHead(206, {
        'Content-Range': `bytes ${start}-${end}/${size}`,
        'Content-Length': end - start + 1,
        'Accept-Ranges': 'bytes',
      });
      if (req.method === 'HEAD') res.end();
      else fs.createReadStream(file, { start, end }).pipe(res);
      record({ method: req.method, path: urlPath, range, status: 206, bytes: end - start + 1 });
      return;
    }
    res.writeHead(200, { 'Content-Length': size, 'Accept-Ranges': 'bytes' });
    if (req.method === 'HEAD') res.end();
    else fs.createReadStream(file).pipe(res);
    record({ method: req.method, path: urlPath, range, status: 200, bytes: size });
  })
  .listen(Number(port), '127.0.0.1', () => console.log(`feed on http://127.0.0.1:${port}/ from ${base}`));
