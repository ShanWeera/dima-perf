// Site constants shared by the Astro configuration and the test scripts.
//
// The documentation is published on GitHub Pages under one base path per version (plan D102):
//   /dima-perf/latest/   built from the main branch (the default here)
//   /dima-perf/vX.Y/     built from each release tag with DOCS_BASE=/dima-perf/vX.Y/
//   /                    pull-request previews (DOCS_BASE=/)
// The deploy workflow also publishes /dima-perf/versions.json (the version list), /dima-perf/api/
// (rustdoc) and /dima-perf/schemas/1.0/ (the JSON Schemas).

/** Origin of the published site. */
export const site = 'https://shanweera.github.io';

/** Where the repository's own pages live on `site`, independent of the documentation version. */
export const projectRoot = '/dima-perf/';

/** Rust API documentation (rustdoc), published by the deploy workflow from the main branch. */
export const apiUrl = `${site}${projectRoot}api/`;

/** Published JSON Schemas (their `$id` URLs). */
export const schemaBaseUrl = `${site}${projectRoot}schemas/1.0/`;

/** Source repository. */
export const repository = 'https://github.com/ShanWeera/dima-perf';

/**
 * The base path of this build: `DOCS_BASE` if set, otherwise `/dima-perf/latest/`. Always starts
 * and ends with a slash.
 *
 * @param {Record<string, string | undefined>} [env]
 * @returns {string}
 */
export function docsBase(env = process.env) {
  let base = (env.DOCS_BASE ?? '/dima-perf/latest/').trim();
  if (!base.startsWith('/')) base = `/${base}`;
  if (!base.endsWith('/')) base = `${base}/`;
  return base;
}

/**
 * What a base path builds (plan D102, D113): a release (`/dima-perf/vX.Y/`), the main branch
 * (`/dima-perf/latest/`, also the default) or a preview (any other base, e.g. `/`).
 *
 * @param {string} base
 * @returns {'release' | 'latest' | 'preview'}
 */
export function buildKind(base) {
  if (new RegExp(`^${projectRoot}v\\d+\\.\\d+/$`).test(base)) return 'release';
  if (base === `${projectRoot}latest/`) return 'latest';
  return 'preview';
}
