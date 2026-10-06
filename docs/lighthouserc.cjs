// Lighthouse CI budget for the documentation site (plan D102). Run with `pnpm run test:lighthouse`
// after `pnpm run build`: the built site is served locally (scripts/serve.mjs) and nothing is
// uploaded anywhere; the reports stay in .lighthouseci/.
//
// Pages: the home page, the install guide (tabs, long code), the entropy explanation (most
// formulas and references), the JSON reference (longest tables) and the generated CLI reference
// (longest page). Scores are the median of three runs with Lighthouse's default mobile emulation
// and simulated throttling.

const { docsBase } = require('./site.mjs');

const port = Number(process.env.LHCI_PORT ?? 9123);
const site = `http://127.0.0.1:${port}${docsBase()}`;

module.exports = {
  ci: {
    collect: {
      startServerCommand: `node scripts/serve.mjs --port ${port}`,
      startServerReadyPattern: 'listening on',
      url: ['', 'getting-started/install/', 'explanation/entropy/', 'reference/json/', 'reference/cli/'].map(
        (page) => site + page,
      ),
      numberOfRuns: 3,
      settings: {
        chromeFlags: '--headless=new --no-sandbox --disable-dev-shm-usage --disable-gpu',
      },
    },
    assert: {
      assertions: {
        'categories:performance': ['error', { minScore: 0.9, aggregationMethod: 'median' }],
        'categories:accessibility': ['error', { minScore: 1, aggregationMethod: 'median' }],
        'categories:best-practices': ['error', { minScore: 0.9, aggregationMethod: 'median' }],
        'categories:seo': ['error', { minScore: 0.9, aggregationMethod: 'median' }],
      },
    },
  },
};
