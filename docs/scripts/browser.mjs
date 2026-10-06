// Finds a Chromium-based browser for the checks, without downloading one:
//   1. CHROME_PATH, if set (e.g. a Playwright-managed Chromium);
//   2. the installed Google Chrome (preinstalled on GitHub's ubuntu-24.04 runners);
//   3. the Chromium managed by Playwright (PLAYWRIGHT_BROWSERS_PATH).

import { chromium } from 'playwright-core';

/** Flags for headless runs in containers and CI (the pages are our own static files). */
export const CHROME_FLAGS = ['--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu'];

export async function launchBrowser() {
  const attempts = [];
  if (process.env.CHROME_PATH) attempts.push({ executablePath: process.env.CHROME_PATH });
  attempts.push({ channel: 'chrome' }, {});
  const errors = [];
  for (const options of attempts) {
    try {
      return await chromium.launch({ headless: true, args: CHROME_FLAGS, ...options });
    } catch (error) {
      errors.push(`${JSON.stringify(options)}: ${String(error).split('\n')[0]}`);
    }
  }
  throw new Error(`no usable Chromium or Chrome found; set CHROME_PATH.\n${errors.join('\n')}`);
}
