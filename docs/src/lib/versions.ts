// The documentation versions published next to this one (plan D102).
//
// The deploy workflow writes `versions.json` next to the version directories, e.g. at
// /dima-perf/versions.json:
//   [{"version":"latest","path":"/dima-perf/latest/"},{"version":"1.0","path":"/dima-perf/v1.0/"}]
// "latest" is built from the default branch (`master`); the other entries are releases (newest first). The
// list is optional: when it is missing (local builds, pull-request previews, an unpacked docs
// archive) the version picker and the outdated-version notice simply do not appear.

export interface VersionEntry {
  /** `latest` or a release number such as `1.0`. */
  version: string;
  /** Base path of that version, e.g. `/dima-perf/v1.0/`. */
  path: string;
}

const RELEASE = /^\d+(?:\.\d+){1,2}$/;
const SAFE_PATH = /^\/(?:[A-Za-z0-9._~-]+\/)*$/;

/** Where the version list of a site built for `base` lives: next to the version directories. */
export function versionsUrl(base: string): string | null {
  const segments = base.split('/').filter(Boolean);
  if (segments.length === 0) return null; // Served from a domain root (preview): no siblings.
  return `/${segments.slice(0, -1).map((s) => `${s}/`).join('')}versions.json`;
}

/** Validates the list: only same-origin directory paths are accepted. */
export function parseVersions(data: unknown): VersionEntry[] | null {
  if (!Array.isArray(data)) return null;
  const entries: VersionEntry[] = [];
  for (const item of data) {
    if (typeof item !== 'object' || item === null) return null;
    const { version, path } = item as Record<string, unknown>;
    if (typeof version !== 'string' || typeof path !== 'string') return null;
    if (!(version === 'latest' || RELEASE.test(version)) || !SAFE_PATH.test(path)) return null;
    entries.push({ version, path });
  }
  return entries.length > 0 ? entries : null;
}

/** Numeric comparison of release numbers (`1.10` > `1.9`). */
export function compareReleases(a: string, b: string): number {
  const pa = a.split('.').map(Number);
  const pb = b.split('.').map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d !== 0) return d;
  }
  return 0;
}

/** The newest release in the list, if any. */
export function newestRelease(entries: VersionEntry[]): VersionEntry | undefined {
  return entries
    .filter((e) => RELEASE.test(e.version))
    .reduce<VersionEntry | undefined>(
      (best, e) => (best === undefined || compareReleases(e.version, best.version) > 0 ? e : best),
      undefined,
    );
}

/** How a version is shown. */
export function versionLabel(version: string): string {
  return version === 'latest' ? 'latest' : `v${version}`;
}

/**
 * The release whose documentation the reader should see instead of this one: set when this site is
 * a release that is older than the newest release.
 */
export function outdated(entries: VersionEntry[], base: string): { current: VersionEntry; newest: VersionEntry } | null {
  const current = entries.find((e) => e.path === base);
  const newest = newestRelease(entries);
  if (!current || !newest || !RELEASE.test(current.version)) return null;
  return compareReleases(current.version, newest.version) < 0 ? { current, newest } : null;
}

/**
 * The notice to show on this site, if any: on a release older than the newest release, a link to
 * the newest one; on /latest/ with unreleased changes (`unreleased`), a link to the newest release
 * (D102, D113).
 */
export function notice(
  entries: VersionEntry[],
  base: string,
  unreleased: boolean,
): { kind: 'outdated' | 'unreleased'; current: string; newest: VersionEntry } | null {
  const old = outdated(entries, base);
  if (old) return { kind: 'outdated', current: old.current.version, newest: old.newest };
  const newest = newestRelease(entries);
  const current = entries.find((e) => e.path === base);
  if (unreleased && newest && current?.version === 'latest') {
    return { kind: 'unreleased', current: 'latest', newest };
  }
  return null;
}

let pending: Promise<VersionEntry[] | null> | undefined;

/** Fetches the version list once per page; `null` when there is none or it is invalid. */
export function loadVersions(base: string): Promise<VersionEntry[] | null> {
  pending ??= (async () => {
    const url = versionsUrl(base);
    if (!url) return null;
    try {
      const response = await fetch(url, { headers: { Accept: 'application/json' } });
      if (!response.ok) return null;
      return parseVersions(await response.json());
    } catch {
      return null;
    }
  })();
  return pending;
}

/** The path of the current page inside its version, e.g. `reference/cli/`. */
export function pageInVersion(base: string, pathname: string): string {
  return pathname.startsWith(base) ? pathname.slice(base.length) : '';
}

/**
 * Opens the same page in another version when it exists there, otherwise that version's home
 * page.
 */
export async function goToVersion(target: VersionEntry, base: string): Promise<void> {
  const page = pageInVersion(base, window.location.pathname);
  let destination = target.path;
  if (page) {
    try {
      const response = await fetch(target.path + page, { method: 'HEAD' });
      if (response.ok) destination = target.path + page + window.location.hash;
    } catch {
      // Keep the version's home page.
    }
  }
  window.location.assign(destination);
}

/** Key of the cached notice in sessionStorage (read before first paint to avoid a layout shift). */
export const NOTICE_CACHE_KEY = 'dima-docs-version-notice';
