// @ts-check
// Documentation site of DiMA (plan D102): Starlight on Astro, built per version under DOCS_BASE.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { unified } from '@astrojs/markdown-remark';
import starlight from '@astrojs/starlight';
import { defineConfig } from 'astro/config';
import rehypeMathjax from 'rehype-mathjax/svg';
import remarkMath from 'remark-math';
import starlightLinksValidator from 'starlight-links-validator';

import contentGuard from './plugins/content-guard.mjs';
import favicon from './plugins/favicon.mjs';
import { rehypeMathFinish, rehypeMathPrepare } from './plugins/rehype-math-a11y.mjs';
import rehypeRegistryCitations from './plugins/rehype-registry-citations.mjs';
import remarkIncludedFiles from './plugins/remark-included-files.mjs';
import remarkSiteLinks from './plugins/remark-site-links.mjs';
import { apiUrl, buildKind, docsBase, repository, site } from './site.mjs';

const base = docsBase();
// D113: only /latest/ is meant for search engines. Release builds stay online for readers of
// that version (version menu, links printed by `dima --help`) but carry noindex; previews are
// served from another domain, so they get no canonical URL, og:url or sitemap at all.
const kind = buildKind(base);
// The newest section of the changelog is "Unreleased" while the default branch has changes not yet released;
// /latest/ then says so and links the newest release (D113).
const unreleased = /^## \[[^\]]+\]\s*-\s*Unreleased\s*$/im.test(
  (readFileSync(fileURLToPath(new URL('../CHANGELOG.md', import.meta.url)), 'utf8').match(/^## .*$/m) ?? [''])[0],
);

/** MathJax TeX input: AMS equation numbering, `\eqref`, and the same settings for both math plugins. */
const tex = { tags: 'ams' };

export default defineConfig({
  site: kind === 'preview' ? undefined : site,
  base,
  trailingSlash: 'always',
  markdown: {
    processor: unified({
      remarkPlugins: [
        remarkMath,
        [remarkSiteLinks, { base, contentDir: new URL('./src/content/docs/', import.meta.url) }],
        [
          remarkIncludedFiles,
          { files: ['../CHANGELOG.md', '../AGENTS.md'].map((f) => fileURLToPath(new URL(f, import.meta.url))) },
        ],
      ],
      rehypePlugins: [
        [rehypeMathPrepare, { tex }],
        [rehypeMathjax, { tex }],
        rehypeMathFinish,
        // Citation keys are those of crates/dima-core/src/citations.rs, e.g. [@chao2013] or
        // [@list2017, Rule 5]; references.json is generated from it by `cargo xtask gen` (D111).
        [rehypeRegistryCitations, { registry: fileURLToPath(new URL('./src/references.json', import.meta.url)) }],
      ],
    }),
  },
  integrations: [
    favicon({ source: new URL('../assets/icon/dima.svg', import.meta.url) }),
    contentGuard({ base, noindex: kind === 'release' }),
    starlight({
      title: 'DiMA',
      description:
        'DiMA quantifies the sequence diversity of viral protein and nucleotide alignments: k-mer entropy, diversity motifs and conserved sequences (Tharanga et al. 2025, Brief Bioinform).',
      logo: { src: '../assets/icon/dima.svg', alt: '' },
      favicon: '/favicon.svg',
      social: [{ icon: 'github', label: 'Source code on GitHub', href: repository }],
      customCss: ['./src/styles/custom.css'],
      head: kind === 'release' ? [{ tag: 'meta', attrs: { name: 'robots', content: 'noindex' } }] : [],
      components: {
        Banner: './src/components/overrides/Banner.astro',
        MarkdownContent: './src/components/overrides/MarkdownContent.astro',
        ThemeSelect: './src/components/overrides/ThemeSelect.astro',
      },
      lastUpdated: false,
      credits: false,
      plugins: [
        starlightLinksValidator({
          // The hero buttons of the home page link relative to it, because frontmatter links are
          // not given the base path; plugins/content-guard.mjs checks them in the built site.
          exclude: ({ file, link }) =>
            file.split(path.sep).join('/').endsWith('content/docs/index.mdx') && !/^[/#]|^[a-z]+:/i.test(link),
        }),
      ],
      sidebar: [
        {
          label: 'Start here',
          items: [
            { slug: 'index', label: 'Overview' },
            'getting-started/install',
            'getting-started/first-analysis',
            'getting-started/faq',
          ],
        },
        {
          label: 'How-to guides',
          items: [
            'how-to/analyse',
            'how-to/metadata',
            'how-to/conserved-sequences',
            'how-to/outputs',
            'how-to/validation',
            'how-to/hpc',
            'how-to/large-alignments',
            'how-to/reproducibility',
            'how-to/desktop-app',
            'how-to/troubleshooting',
          ],
        },
        {
          label: 'Reference',
          items: [
            'reference/cli',
            'reference/json',
            'reference/tsv',
            'reference/exit-codes',
            'reference/environment',
            'reference/diagnostics',
            'reference/symbols',
            'reference/glossary',
            'reference/citing',
            'reference/bibliography',
            'reference/changelog',
            { label: 'Library API (rustdoc)', link: apiUrl, attrs: { rel: 'noopener' } },
          ],
        },
        {
          label: 'Explanation',
          items: [
            'explanation/method',
            'explanation/entropy',
            'explanation/motifs',
            'explanation/conserved-sequences',
            'explanation/input',
            'explanation/paper-comparison',
            'explanation/differences',
            'explanation/numerical-accuracy',
            'explanation/design-rationale',
            'explanation/benchmarks',
          ],
        },
        {
          label: 'Contributing',
          items: [
            'contributing/development',
            'contributing/architecture',
            'contributing/testing',
            'contributing/working-agreements',
          ],
        },
      ],
    }),
  ],
  vite: {
    // Pages include ../CHANGELOG.md and ../AGENTS.md; the logo is ../assets/icon/dima.svg.
    server: { fs: { allow: ['..'] } },
    define: { __DIMA_UNRELEASED__: JSON.stringify(kind === 'latest' && unreleased) },
  },
});
