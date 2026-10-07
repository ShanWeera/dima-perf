// Checks of the built site, after the build:
//
// * every page has content. Astro's content loader logs an error and carries on when a Markdown
//   page cannot be rendered (for example on an unknown citation key or a TeX error, which the
//   plugins report by failing the file), which would publish an empty page;
// * every internal link of every page leads to a built file. starlight-links-validator checks
//   the links written in Markdown, with their anchors; this check also covers the links that
//   Starlight generates (sidebar, pagination, the hero of the home page), as they are served;
// * search-engine indexing follows D113: every page of a release build carries
//   `<meta name="robots" content="noindex">`, and no page of any other build does.

import { access, readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const EMPTY = /<div class="sl-markdown-content">\s*<\/div>/;
const HREF = /<a\s[^>]*?href="([^"]*)"/g;
const EXTERNAL = /^(?:[a-z][a-z0-9+.-]*:|\/\/|#)/i;
const NOINDEX = /<meta\s+name="robots"\s+content="noindex"\s*\/?>/;

/** The file that serves `pathname` (under `base`), or null when it is outside the site. */
function servedFile(root, base, pathname) {
  if (!pathname.startsWith(base)) return null;
  const rel = decodeURIComponent(pathname.slice(base.length));
  return path.join(root, rel.endsWith('/') || rel === '' ? path.join(rel, 'index.html') : rel);
}

async function exists(file) {
  try {
    await access(file);
    return true;
  } catch {
    return false;
  }
}

/**
 * @param {{ base: string, noindex: boolean }} options `base` is the site's base path, with leading
 *   and trailing slashes; `noindex` whether every page must carry the robots noindex tag.
 * @returns {import('astro').AstroIntegration}
 */
export default function contentGuard({ base, noindex }) {
  return {
    name: 'dima-content-guard',
    hooks: {
      'astro:build:done': async ({ dir, logger }) => {
        const root = fileURLToPath(dir);
        const files = (await readdir(root, { recursive: true }))
          .map((f) => f.split(path.sep).join('/'))
          .filter((f) => f.endsWith('.html') && !f.startsWith('pagefind/'));
        const empty = [];
        const broken = [];
        const indexing = [];
        for (const f of files) {
          const html = await readFile(path.join(root, f), 'utf8');
          if (EMPTY.test(html)) empty.push(f);
          if (NOINDEX.test(html) !== noindex) indexing.push(f);
          const page = new URL(base + f.replace(/(^|\/)index\.html$/, '$1'), 'https://site.invalid');
          for (const [, href] of html.matchAll(HREF)) {
            if (href === '' || EXTERNAL.test(href)) continue;
            const url = new URL(href.replaceAll('&#38;', '&').replaceAll('&amp;', '&'), page);
            const target = servedFile(root, base, url.pathname);
            if (target === null || !(await exists(target))) broken.push(`${f}: ${href}`);
          }
        }
        if (empty.length > 0) {
          throw new Error(
            `${empty.length} page(s) have no content, because rendering failed (see the errors above): ${empty.join(', ')}`,
          );
        }
        if (indexing.length > 0) {
          throw new Error(
            `${indexing.length} page(s) ${noindex ? 'lack' : 'carry'} the robots noindex tag (D113): ${indexing.join(', ')}`,
          );
        }
        if (broken.length > 0) {
          throw new Error(`${broken.length} internal link(s) lead nowhere:\n  ${broken.join('\n  ')}`);
        }
        logger.info(`${files.length} pages have content and working internal links`);
      },
    },
  };
}
