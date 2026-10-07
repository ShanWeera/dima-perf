// Makes internal links independent of the base path the site is built for (DOCS_BASE), and
// checkable by starlight-links-validator.
//
// Pages link to each other by their path from the site root *without* the base, e.g.
// `/reference/cli/#dima-analyze`. This plugin prefixes the base, so the same source works for
// /dima-perf/latest/, /dima-perf/vX.Y/ and previews served from /. Relative links (such as
// `../citing/` in the generated bibliography) are resolved against the page's own URL first, so
// they are validated too. External links, in-page anchors and links in files outside the docs
// collection that cannot be resolved are left unchanged.

import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { visit } from 'unist-util-visit';

const EXTERNAL = /^[a-z][a-z\d+.-]*:/i;
/** Components whose `href` attribute is a link (Starlight's own link components). */
const LINK_COMPONENTS = new Set(['a', 'LinkCard', 'LinkButton']);

/**
 * The URL path of the page built from `file`, e.g. `/reference/bibliography/`, or `null` for a
 * file outside the docs collection.
 *
 * @param {string | undefined} file
 * @param {string} contentDir absolute path of `src/content/docs`
 */
function pageUrl(file, contentDir) {
  if (!file) return null;
  const rel = path.relative(contentDir, file);
  if (rel.startsWith('..') || path.isAbsolute(rel)) return null;
  const parts = rel.replace(/\.(md|mdx)$/i, '').split(path.sep);
  if (parts.at(-1) === 'index') parts.pop();
  return parts.length ? `/${parts.join('/')}/` : '/';
}

/**
 * The base-prefixed form of an internal link, or `null` to leave it unchanged.
 *
 * @param {string} url
 * @param {string} base e.g. `/dima-perf/latest/`
 * @param {string | null} page URL path of the page that contains the link
 */
export function rewrite(url, base, page) {
  if (!url || url.startsWith('#') || url.startsWith('?') || url.startsWith('//')) return null;
  if (EXTERNAL.test(url)) return null;
  let target;
  if (url.startsWith('/')) {
    target = url;
  } else if (page) {
    const resolved = new URL(url, `https://site.invalid${page}`);
    target = `${resolved.pathname}${resolved.search}${resolved.hash}`;
  } else {
    return null;
  }
  if (base !== '/' && target.startsWith(base)) return target;
  return `${base}${target.slice(1)}`;
}

/**
 * @param {{ base: string, contentDir: string | URL }} options
 */
export default function remarkSiteLinks({ base, contentDir }) {
  const dir = contentDir instanceof URL ? fileURLToPath(contentDir) : contentDir;
  return (tree, file) => {
    const page = pageUrl(file.history[0], dir);
    visit(tree, (node) => {
      if (node.type === 'link' || node.type === 'definition') {
        const url = rewrite(node.url, base, page);
        if (url !== null) node.url = url;
      } else if (
        (node.type === 'mdxJsxFlowElement' || node.type === 'mdxJsxTextElement') &&
        LINK_COMPONENTS.has(node.name)
      ) {
        for (const attr of node.attributes) {
          if (attr.type === 'mdxJsxAttribute' && attr.name === 'href' && typeof attr.value === 'string') {
            const url = rewrite(attr.value, base, page);
            if (url !== null) attr.value = url;
          }
        }
      }
    });
  };
}
