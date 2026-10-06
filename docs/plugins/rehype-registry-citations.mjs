// Citations of the documentation pages, rendered from the citation registry
// (crates/dima-core/src/citations.rs) through src/references.json, which `cargo xtask gen` writes:
// for every key, its in-text label (e.g. "List et al. 2017", as in the help texts and the desktop
// app), its full text as the Bibliography page lists it (the text `dima cite` prints, led by a
// standard's designation and with any year letter) without the DOI or URL it ends with, that
// link, and its rank in author-year order (plan D111).
//
// `[@key]`, `[@key, Rule 5]` and `[@a; @b, Fig. 2]` become "(List et al. 2017, Rule 5)": each
// work is a link to its entry in the page's reference list (role doc-biblioref, DPUB-ARIA 1.1),
// described by that entry's text for assistive technologies (aria-describedby). The paragraph
// `[^ref]` becomes the reference list (role doc-bibliography, labelled by its heading), in
// author-year order; each entry ends with its DOI or URL as a link and one back-link (role
// doc-backlink) per citation of the work on the page. Entries have the ids `bib-<key>`, citations
// `cite-<key>-<n>`. Hover and focus previews are added by src/components/overrides/
// MarkdownContent.astro; without JavaScript every citation is a plain link to its entry.
//
// The build fails on an unknown key, an empty locator, a citation inside a link, a citation on a
// page without `[^ref]`, a `[^ref]` on a page without citations, and a second `[^ref]`.

import { readFileSync } from 'node:fs';
import { h } from 'hastscript';
import { visit } from 'unist-util-visit';
import { visitParents } from 'unist-util-visit-parents';

const CITATION = /\[(@[^\][]+)\]/g;
const ITEM = /^@([a-z0-9_]+)(?:\s*,\s*([^;]*))?$/i;
const LITERAL = new Set(['code', 'pre', 'kbd', 'samp', 'script', 'style']);
const BACK = '↩︎';

/** The `[^ref]` placeholder paragraph. */
function isPlaceholder(node) {
  return (
    node.type === 'element' &&
    node.tagName === 'p' &&
    node.children.length === 1 &&
    node.children[0].type === 'text' &&
    node.children[0].value.trim() === '[^ref]'
  );
}

/** @param {{ registry: string }} options `registry`: the path of references.json. */
export default function rehypeRegistryCitations({ registry: path }) {
  const registry = JSON.parse(readFileSync(path, 'utf8'));
  return (tree, file) => {
    /** The ids of the citations of each key, in page order. */
    const uses = new Map();

    const found = [];
    visitParents(tree, 'text', (node, parents) => {
      if (parents.some((p) => p.type === 'element' && LITERAL.has(p.tagName))) return;
      const matches = [...node.value.matchAll(CITATION)];
      if (matches.length === 0) return;
      if (parents.some((p) => p.type === 'element' && p.tagName === 'a')) {
        file.fail(`citation [${matches[0][1]}] inside a link`, node);
      }
      found.push({ node, parent: parents.at(-1), matches });
    });

    for (const { node, parent, matches } of found) {
      const parts = [];
      let last = 0;
      for (const m of matches) {
        if (m.index > last) parts.push({ type: 'text', value: node.value.slice(last, m.index) });
        parts.push({ type: 'text', value: '(' });
        m[1].split(';').forEach((raw, i) => {
          const item = ITEM.exec(raw.trim());
          if (!item || !Object.hasOwn(registry, item[1])) {
            file.fail(`unknown citation [${m[1]}] (is the key in crates/dima-core/src/citations.rs?)`, node);
          }
          const [, key, locator] = item;
          if (locator !== undefined && locator.trim() === '') file.fail(`empty locator in [${m[1]}]`, node);
          const ids = uses.get(key) ?? [];
          const id = `cite-${key}-${ids.length + 1}`;
          uses.set(key, [...ids, id]);
          const label = registry[key].label + (locator ? `, ${locator.trim().replace(/\s+/g, ' ')}` : '');
          if (i > 0) parts.push({ type: 'text', value: '; ' });
          parts.push(
            h('a', { href: `#bib-${key}`, id, role: 'doc-biblioref', ariaDescribedBy: `bib-${key}-text` }, label),
          );
        });
        parts.push({ type: 'text', value: ')' });
        last = m.index + m[0].length;
      }
      if (last < node.value.length) parts.push({ type: 'text', value: node.value.slice(last) });
      parent.children.splice(parent.children.indexOf(node), 1, ...parts);
    }

    let placed = false;
    visit(tree, isPlaceholder, (p, index, parent) => {
      if (placed) file.fail('a second [^ref] reference list', p);
      if (uses.size === 0) file.fail('[^ref] on a page without citations', p);
      placed = true;
      const keys = [...uses.keys()].sort((a, b) => registry[a].order - registry[b].order);
      const items = keys.map((key) => {
        const { label, text, link } = registry[key];
        const content = [h('span', { id: `bib-${key}-text` }, text)];
        if (link) content.push({ type: 'text', value: ' ' }, h('a', { href: link.href }, link.label));
        const ids = uses.get(key);
        const back = ids.map((id, i) =>
          h(
            'a',
            {
              href: `#${id}`,
              role: 'doc-backlink',
              ariaLabel: ids.length === 1 ? `Back to the citation of ${label}` : `Back to citation ${i + 1} of ${label}`,
            },
            ids.length === 1 ? BACK : String(i + 1),
          ),
        );
        const backlinks = ids.length === 1 ? back : [`${BACK} `, ...back.flatMap((a, i) => (i > 0 ? [' ', a] : [a]))];
        content.push({ type: 'text', value: ' ' }, h('span', { className: ['dima-backlinks'] }, backlinks));
        return h('li', { id: `bib-${key}` }, content);
      });
      const attrs = { className: ['dima-references'], role: 'doc-bibliography' };
      // Astro gives headings their ids after the Markdown plugins run, and keeps ids set here.
      const heading = parent.children
        .slice(0, index)
        .findLast((n) => n.type === 'element' && /^h[1-6]$/.test(n.tagName));
      if (heading) attrs.ariaLabelledBy = heading.properties.id ??= 'references';
      parent.children[index] = h('div', attrs, [h('ul', items)]);
      return 'skip';
    });
    if (uses.size > 0 && !placed) file.fail('citations on a page without a [^ref] reference list');
  };
}
