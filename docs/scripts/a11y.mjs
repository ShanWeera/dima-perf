// Accessibility check of the built site (plan D102): every page is scanned with axe-core against
// WCAG 2.0, 2.1 and 2.2 levels A and AA, in the light and dark themes on a desktop screen and in
// the light theme on a phone screen, plus the open version picker, the open mobile menu and an
// open reference preview. The reference previews are also checked against WCAG 2.2 SC 1.4.13
// (Content on Hover or Focus) and for keyboard and touch use (plan D111). Any violation or failed
// check fails the run (exit 1).
//
// Usage: pnpm run build && pnpm run test:a11y   (DOCS_BASE as for the build)

import { readFile } from 'node:fs/promises';

import { AxeBuilder } from '@axe-core/playwright';

import { launchBrowser } from './browser.mjs';
import { builtPages, pageFile } from './pages.mjs';
import { serve } from './serve.mjs';

const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];
const VIEWS = [
  { name: 'desktop, light', theme: 'light', viewport: { width: 1280, height: 900 } },
  { name: 'desktop, dark', theme: 'dark', viewport: { width: 1280, height: 900 } },
  { name: 'phone, light', theme: 'light', viewport: { width: 375, height: 740 }, isMobile: true },
];

const { server, url } = await serve();
const browser = await launchBrowser();
const pages = await builtPages();
const failures = [];
let scans = 0;

const CITE = '.sl-markdown-content a[role="doc-biblioref"]';
const PREVIEW = '.dima-ref-preview';
// The page with the most citations exercises the reference previews.
let citing = null;
let most = 0;
for (const p of pages) {
  const count = (await readFile(pageFile(p), 'utf8')).split('role="doc-biblioref"').length - 1;
  if (count > most) [citing, most] = [p, count];
}

function fail(label, check, detail = '') {
  failures.push({ label, rule: 'reference-preview', impact: 'serious', help: check + (detail ? `: ${detail}` : ''), url: '', targets: [] });
}

const isOpen = (page) => page.locator(PREVIEW).evaluate((el) => el.matches(':popover-open'));

/** Hover, pointer, Escape, focus, Enter and back-link behaviour of the previews on a wide screen. */
async function checkPreviewsDesktop(page, view) {
  const label = `${citing || '/'} (${view.name})`;
  const cite = page.locator(CITE).first();
  await cite.hover();
  await page.waitForTimeout(500);
  if (!(await isOpen(page))) return fail(label, 'hovering a citation shows its reference');
  const text = await page.locator(PREVIEW).innerText();
  const entry = await page.locator(`${await cite.getAttribute('href')} > span`).first().innerText();
  if (!text.startsWith(entry)) fail(label, 'the preview repeats the reference-list entry', text.slice(0, 80));
  await scan(page, `${label}, reference preview open`);
  const box = await page.locator(PREVIEW).boundingBox();
  await page.mouse.move(box.x + 10, box.y + 10, { steps: 5 });
  await page.waitForTimeout(600);
  if (!(await isOpen(page))) fail(label, 'the pointer can move onto the preview (SC 1.4.13 hoverable)');
  await page.keyboard.press('Escape');
  if (await isOpen(page)) fail(label, 'Escape dismisses the preview (SC 1.4.13 dismissible)');
  await page.mouse.move(1, 1);
  await page.waitForTimeout(600);
  await cite.focus();
  if (!(await isOpen(page))) fail(label, 'focusing a citation shows its reference');
  await page.keyboard.press('Enter');
  await page.waitForTimeout(300);
  const hash = await page.evaluate(() => location.hash);
  if (!hash.startsWith('#bib-')) return fail(label, 'Enter follows the citation to its entry', hash);
  if (await isOpen(page)) fail(label, 'following a citation closes its preview');
  const visible = await page.evaluate(() => {
    const r = document.querySelector(':target')?.getBoundingClientRect();
    return !!r && r.top >= 0 && r.bottom <= innerHeight;
  });
  if (!visible) fail(label, 'the entry a citation leads to is visible below the page header');
  await page.locator(`${hash} a[role="doc-backlink"]`).first().click();
  await page.waitForTimeout(300);
  const back = await page.evaluate(() => location.hash);
  if (!back.startsWith('#cite-')) fail(label, 'the back-link returns to the citation', back);
}

/** On a phone: a tap follows the citation without a preview; a focused preview fits the screen. */
async function checkPreviewsPhone(page, view) {
  const label = `${citing || '/'} (${view.name})`;
  const cite = page.locator(CITE).first();
  await cite.tap();
  await page.waitForTimeout(400);
  if (!(await page.evaluate(() => location.hash)).startsWith('#bib-')) fail(label, 'a tap follows the citation to its entry');
  if (await isOpen(page)) fail(label, 'a tap shows no preview');
  await page.goto(url + citing, { waitUntil: 'networkidle' });
  await page.locator(CITE).last().focus();
  const box = await page.locator(PREVIEW).boundingBox();
  const width = view.viewport.width;
  if (!box || box.x < 0 || box.x + box.width > width) fail(label, 'a focused preview fits the screen', JSON.stringify(box));
}

async function scan(page, label) {
  const result = await new AxeBuilder({ page }).withTags(TAGS).analyze();
  scans += 1;
  for (const v of result.violations) {
    failures.push({ label, rule: v.id, impact: v.impact, help: v.help, url: v.helpUrl, targets: v.nodes.map((n) => n.target.join(' ')) });
  }
}

try {
  for (const view of VIEWS) {
    const context = await browser.newContext({
      viewport: view.viewport,
      isMobile: view.isMobile ?? false,
      hasTouch: view.isMobile ?? false,
      colorScheme: view.theme,
    });
    await context.addInitScript((theme) => {
      try {
        localStorage.setItem('starlight-theme', theme);
      } catch {}
    }, view.theme);
    const page = await context.newPage();
    for (const p of pages) {
      const response = await page.goto(url + p, { waitUntil: 'networkidle' });
      if (!response || (response.status() >= 400 && !p.startsWith('404'))) {
        failures.push({ label: `${p} (${view.name})`, rule: 'http', impact: 'critical', help: `HTTP ${response?.status()}`, url: '', targets: [] });
        continue;
      }
      await scan(page, `${p || '/'} (${view.name})`);
    }
    if (citing !== null) {
      await page.goto(url + citing, { waitUntil: 'networkidle' });
      if (view.isMobile) await checkPreviewsPhone(page, view);
      else await checkPreviewsDesktop(page, view);
    }
    // Interactive states on the home page.
    await page.goto(url, { waitUntil: 'networkidle' });
    if (view.isMobile) {
      const menu = page.locator('starlight-menu-button button');
      if (await menu.isVisible()) {
        await menu.click();
        const picker = page.locator('.sidebar-pane dima-version-picker summary');
        if (await picker.isVisible()) await picker.click();
        await scan(page, `/ (${view.name}, menu open)`);
      }
    } else {
      const picker = page.locator('header dima-version-picker summary');
      if (await picker.isVisible()) {
        await picker.click();
        await scan(page, `/ (${view.name}, version picker open)`);
      }
    }
    await context.close();
  }
} finally {
  await browser.close();
  server.close();
}

if (failures.length > 0) {
  for (const f of failures) {
    console.error(`✗ ${f.label}: ${f.rule} [${f.impact}] ${f.help}${f.url ? ` (${f.url})` : ''}`);
    for (const t of f.targets.slice(0, 5)) console.error(`    ${t}`);
  }
  console.error(`\n${failures.length} accessibility violation(s) in ${scans} scans of ${pages.length} pages.`);
  process.exit(1);
}
console.log(`No WCAG 2.2 A/AA violations in ${scans} scans of ${pages.length} pages${citing === null ? '' : '; reference previews pass'}.`);
