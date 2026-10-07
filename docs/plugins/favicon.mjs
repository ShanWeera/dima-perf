// Serves the application icon (assets/icon/dima.svg, plan D80) as the site's favicon, so the
// documentation uses the one source file instead of a copy: in development it is served from
// memory, and the build writes it to the output directory.

import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

/**
 * @param {{ source: URL, name?: string }} options `name` must match Starlight's `favicon` option
 * @returns {import('astro').AstroIntegration}
 */
export default function favicon({ source, name = 'favicon.svg' }) {
  const file = fileURLToPath(source);
  return {
    name: 'dima-favicon',
    hooks: {
      'astro:config:setup': ({ config, updateConfig }) => {
        const route = `${config.base.replace(/\/?$/, '/')}${name}`;
        updateConfig({
          vite: {
            plugins: [
              {
                name: 'dima-favicon-dev',
                configureServer(server) {
                  server.middlewares.use(route, async (_req, res) => {
                    res.setHeader('Content-Type', 'image/svg+xml');
                    res.end(await readFile(file));
                  });
                },
              },
            ],
          },
        });
      },
      'astro:build:done': async ({ dir }) => {
        await writeFile(new URL(name, dir), await readFile(file));
      },
    },
  };
}
