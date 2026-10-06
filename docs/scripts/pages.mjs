// Lists the pages of the built site (dist/), as paths relative to the base, e.g. `reference/cli/`.

import { readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const dist = fileURLToPath(new URL('../dist/', import.meta.url));

/** The built HTML file of a page listed by `builtPages`. */
export function pageFile(page) {
  return path.join(dist, page === '' || page.endsWith('/') ? `${page}index.html` : page);
}

export async function builtPages() {
  const entries = await readdir(dist, { recursive: true, withFileTypes: true });
  const pages = [];
  for (const e of entries) {
    if (!e.isFile() || !e.name.endsWith('.html')) continue;
    const rel = path.relative(dist, path.join(e.parentPath, e.name)).split(path.sep).join('/');
    if (rel.startsWith('pagefind/')) continue;
    pages.push(rel === 'index.html' ? '' : rel.endsWith('/index.html') ? rel.slice(0, -'index.html'.length) : rel);
  }
  if (pages.length === 0) throw new Error('dist/ has no pages; run `pnpm run build` first');
  return pages.sort();
}
