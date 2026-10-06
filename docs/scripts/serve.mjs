// A small static server for the built site (dist/), used by the accessibility and Lighthouse
// checks. It serves the site under its base path (DOCS_BASE, as in the build), like GitHub Pages,
// and a test copy of versions.json next to it so that the version picker (and, for release
// builds, the outdated-version notice) are part of what is checked. Text is gzip-compressed
// when the browser accepts it, as GitHub Pages does.
//
// Usage: node scripts/serve.mjs [--port N]   (prints "listening on <url>" when ready)

import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import { createServer } from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createGzip } from 'node:zlib';

import { docsBase } from '../site.mjs';

const dist = path.resolve(fileURLToPath(new URL('../dist/', import.meta.url)));

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
  '.woff2': 'font/woff2',
  '.xml': 'application/xml; charset=utf-8',
  '.txt': 'text/plain; charset=utf-8',
  '.pf_meta': 'application/octet-stream',
  '.pf_index': 'application/octet-stream',
  '.pf_fragment': 'application/octet-stream',
  '.pagefind': 'application/octet-stream',
  '.wasm': 'application/wasm',
};

const COMPRESSIBLE = /^(text\/|application\/(json|xml|javascript)|image\/svg)/;

/**
 * The version list served next to the site: `latest`, and for a release build (base ending in
 * /vX.Y/) that release plus a newer one, so the outdated-version notice appears; for /latest/ a
 * release, so the unreleased-changes notice appears while the changelog has unreleased changes.
 *
 * @param {string} base
 */
export function versionsFixture(base) {
  const segments = base.split('/').filter(Boolean);
  const parent = `/${segments.slice(0, -1).map((s) => `${s}/`).join('')}`;
  const entries = [{ version: 'latest', path: `${parent}latest/` }];
  const release = /^v(\d+)\.(\d+)$/.exec(segments.at(-1) ?? '');
  if (release) {
    const newer = `${release[1]}.${Number(release[2]) + 1}`;
    entries.push({ version: newer, path: `${parent}v${newer}/` });
    entries.push({ version: `${release[1]}.${release[2]}`, path: base });
  } else {
    entries.push({ version: '1.0', path: `${parent}v1.0/` });
  }
  return { url: `${parent}versions.json`, entries };
}

/**
 * Starts the server; resolves with its origin and the site URL.
 *
 * @param {{ port?: number, base?: string }} [options]
 */
export function serve({ port = 0, base = docsBase() } = {}) {
  const fixture = base === '/' ? null : versionsFixture(base);
  const server = createServer(async (req, res) => {
    const send = (status, type, body) => {
      res.writeHead(status, { 'Content-Type': type, 'Cache-Control': 'no-cache' });
      res.end(req.method === 'HEAD' ? undefined : body);
    };
    if (req.method !== 'GET' && req.method !== 'HEAD') return send(405, 'text/plain', 'method not allowed');
    let pathname;
    try {
      pathname = decodeURIComponent(new URL(req.url ?? '/', 'http://localhost').pathname);
    } catch {
      return send(400, 'text/plain', 'bad request');
    }
    if (fixture && pathname === fixture.url) {
      return send(200, TYPES['.json'], JSON.stringify(fixture.entries));
    }
    if (!pathname.startsWith(base)) return send(404, 'text/plain', 'not found');
    let file = path.resolve(dist, `.${path.posix.sep}${pathname.slice(base.length)}`);
    if (file !== dist && !file.startsWith(dist + path.sep)) return send(404, 'text/plain', 'not found');
    try {
      let info = await stat(file);
      if (info.isDirectory()) {
        if (!pathname.endsWith('/')) {
          res.writeHead(301, { Location: `${pathname}/` });
          return res.end();
        }
        file = path.join(file, 'index.html');
        info = await stat(file);
      }
      const type = TYPES[path.extname(file)] ?? 'application/octet-stream';
      const gzip = COMPRESSIBLE.test(type) && /\bgzip\b/.test(req.headers['accept-encoding'] ?? '');
      res.writeHead(200, {
        'Content-Type': type,
        'Cache-Control': 'no-cache',
        Vary: 'Accept-Encoding',
        ...(gzip ? { 'Content-Encoding': 'gzip' } : { 'Content-Length': info.size }),
      });
      if (req.method === 'HEAD') return res.end();
      const body = createReadStream(file);
      if (gzip) body.pipe(createGzip()).pipe(res);
      else body.pipe(res);
    } catch {
      try {
        const page = path.join(dist, '404.html');
        await stat(page);
        res.writeHead(404, { 'Content-Type': TYPES['.html'] });
        if (req.method === 'HEAD') return res.end();
        createReadStream(page).pipe(res);
      } catch {
        send(404, 'text/plain', 'not found');
      }
    }
  });
  return new Promise((resolve) => {
    server.listen(port, '127.0.0.1', () => {
      const { port: actual } = server.address();
      const origin = `http://127.0.0.1:${actual}`;
      resolve({ server, origin, url: `${origin}${base}` });
    });
  });
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const i = process.argv.indexOf('--port');
  const port = i > 0 ? Number(process.argv[i + 1]) : 8080;
  const { url } = await serve({ port });
  console.log(`listening on ${url}`);
}
