// Runs Lighthouse CI against the built site with the budget in lighthouserc.cjs: `lhci collect`
// then `lhci assert` (never `upload`: results stay local, in .lighthouseci/). Chrome is found by
// Lighthouse; set CHROME_PATH to use another Chromium (Playwright's is used when no Chrome is
// installed).
//
// Usage: pnpm run build && pnpm run test:lighthouse   (DOCS_BASE as for the build)

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright-core';

const docs = fileURLToPath(new URL('..', import.meta.url));
const require = createRequire(import.meta.url);
const manifest = require.resolve('@lhci/cli/package.json');
const bin = path.join(path.dirname(manifest), JSON.parse(readFileSync(manifest, 'utf8')).bin.lhci);
const config = path.join(docs, 'lighthouserc.cjs');

const env = { ...process.env };
if (!env.CHROME_PATH) {
  const installed = [
    '/usr/bin/google-chrome-stable',
    '/usr/bin/google-chrome',
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
  ].find((p) => existsSync(p));
  const managed = chromium.executablePath();
  if (!installed && managed && existsSync(managed)) env.CHROME_PATH = managed;
}

for (const step of ['collect', 'assert']) {
  const run = spawnSync(process.execPath, [bin, step, `--config=${config}`], { cwd: docs, env, stdio: 'inherit' });
  if (run.status !== 0) process.exit(run.status ?? 1);
}
