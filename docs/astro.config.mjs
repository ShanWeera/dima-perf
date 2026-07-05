import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { starlightKatex } from 'starlight-katex';

export default defineConfig({
  site: 'https://ShanWeera.github.io',
  base: '/dima-perf/',
  // Workaround: Astro 6.4.x regression (withastro/astro#16971) broke GFM in .mdx files
  // for projects using @astrojs/mdx@5.x (Starlight 0.39.x). Remove when upgrading to
  // Starlight >=0.40.0 which ships @astrojs/mdx@6.x with a proper fallback.
  markdown: { gfm: true },
  integrations: [
    starlight({
      title: 'DiMA',
      plugins: [starlightKatex()],
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/ShanWeera/dima-perf' },
      ],
      sidebar: [
        {
          label: 'Getting Started',
          items: [{ autogenerate: { directory: 'getting-started' } }],
        },
        {
          label: 'CLI Reference',
          items: [
            { slug: 'cli' },
            {
              label: 'Commands',
              items: [
                { slug: 'cli/commands/analyze' },
                { slug: 'cli/commands/view' },
                { slug: 'cli/commands/completions' },
              ],
            },
            {
              label: 'Input & Output',
              items: [
                { slug: 'cli/io/input-formats' },
                { slug: 'cli/io/output-formats' },
              ],
            },
            {
              label: 'Features',
              items: [
                { slug: 'cli/features/metadata' },
                { slug: 'cli/features/hcs' },
                { slug: 'cli/features/validation' },
              ],
            },
            {
              label: 'Advanced',
              items: [
                { slug: 'cli/advanced/performance-tuning' },
                { slug: 'cli/advanced/environment-variables' },
                { slug: 'cli/advanced/exit-codes' },
              ],
            },
          ],
        },
        { label: 'Methodology', items: [{ autogenerate: { directory: 'methodology' } }] },
        { label: 'Desktop App', items: [{ autogenerate: { directory: 'desktop' } }] },
        { label: 'Rust Library', items: [{ autogenerate: { directory: 'library' } }] },
        { slug: 'benchmarks', label: 'Benchmarks' },
        { label: 'Contributing', items: [{ autogenerate: { directory: 'contributing' } }] },
        { label: 'Reference', items: [{ autogenerate: { directory: 'reference' } }] },
      ],
      customCss: ['./src/styles/custom.css'],
      expressiveCode: { themes: ['github-dark', 'github-light'] },
      lastUpdated: true,
    }),
  ],
});
