import { defineCollection } from 'astro:content';
import { docsLoader } from '@astrojs/starlight/loaders';
import { docsSchema } from '@astrojs/starlight/schema';

export const collections = {
  docs: defineCollection({
    loader: docsLoader({
      // Strips /index suffix so directory index files produce clean slugs
      // (e.g., cli/index.mdx → 'cli' instead of 'cli/index').
      // Required because sidebar { slug: 'cli' } must match the generated ID.
      generateId: ({ entry }) => {
        const withoutExt = entry.replace(/\.mdx?$/, '');
        return withoutExt.endsWith('/index') ? withoutExt.slice(0, -6) : withoutExt;
      },
    }),
    schema: docsSchema(),
  }),
};
