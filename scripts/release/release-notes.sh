#!/usr/bin/env bash
# Release notes (D67, D68): the version's CHANGELOG section, followed by install instructions
# with the exact asset names. Fails if the section is missing or undated.
#
# Usage: scripts/release/release-notes.sh VERSION [EXTRA_MARKDOWN_FILE]
set -euo pipefail

version=$1
extra=${2:-}
root=$(cd "$(dirname "$0")/../.." && pwd)
changelog="$root/CHANGELOG.md"
repo="https://github.com/ShanWeera/dima-perf"
# The documentation of this release (major.minor), as `dima --help` and CITATION.cff link it.
docs="https://shanweera.github.io/dima-perf/v${version%.*}/"

if ! grep -Eq "^## \[${version//./\\.}\] - [0-9]{4}-[0-9]{2}-[0-9]{2}$" "$changelog"; then
  echo "error: CHANGELOG.md has no dated section '## [${version}] - YYYY-MM-DD'" >&2
  exit 1
fi

awk -v v="$version" '
  index($0, "## [" v "]") == 1 { on = 1; next }
  on && /^## \[/ { exit }
  on { print }
' "$changelog"

cat <<NOTES

## Install

Command-line tool (pick the archive for your system, unpack it and put \`dima\` on your PATH):

| System | Archive |
|---|---|
| Linux x86_64 (glibc 2.17 or newer: CentOS 7, Ubuntu, Debian, Rocky, most HPC clusters) | \`dima-${version}-x86_64-unknown-linux-gnu.tar.gz\` |
| Linux ARM64 (glibc 2.28 or newer) | \`dima-${version}-aarch64-unknown-linux-gnu.tar.gz\` |
| Linux, any distribution (static musl build; Alpine) | \`dima-${version}-x86_64-unknown-linux-musl.tar.gz\`, \`dima-${version}-aarch64-unknown-linux-musl.tar.gz\` |
| macOS (Apple silicon / Intel) | \`dima-${version}-aarch64-apple-darwin.tar.gz\` / \`dima-${version}-x86_64-apple-darwin.tar.gz\` |
| Windows (x86_64 / ARM64) | \`dima-${version}-x86_64-pc-windows-msvc.zip\` / \`dima-${version}-aarch64-pc-windows-msvc.zip\` |

The glibc builds are recommended on Linux; the musl builds need no system libraries.

\`\`\`sh
tar xzf dima-${version}-x86_64-unknown-linux-gnu.tar.gz
./dima-${version}-x86_64-unknown-linux-gnu/dima --version
\`\`\`

From source (Rust 1.85 or newer): \`cargo install --locked --git ${repo} --tag v${version} dima\`.

Desktop app: \`DiMA-${version}-universal.dmg\` (macOS), \`DiMA-${version}-x86_64.msi\` / \`DiMA-${version}-aarch64.msi\` (Windows), and for Linux (glibc 2.28 or newer) \`DiMA-${version}-x86_64.AppImage\`, \`dima-gui_${version}_x86_64.deb\` or \`dima-gui-${version}-x86_64-linux.tar.gz\`, with the same names for ARM64 (\`aarch64\` in place of \`x86_64\`). The packages are not code-signed; see the installation guide for first-run steps.

Verify a download: \`sha256sum -c SHA256SUMS --ignore-missing\`, and its build provenance: \`gh attestation verify <file> --repo ShanWeera/dima-perf\`. Each archive has a CycloneDX SBOM (\`.cdx.json\`), and the binaries embed their dependency list (\`cargo auditable\`).

Documentation for this version: \`dima-${version}-docs.tar.gz\` and ${docs}

Cite: Tharanga S, et al. DiMA: sequence diversity dynamics analyser for viruses. Briefings in Bioinformatics. 2025;26(1):bbae607. doi:10.1093/bib/bbae607 (run \`dima cite\` for the software citation).
NOTES

if [ -n "$extra" ] && [ -s "$extra" ]; then
  printf '\n'
  cat "$extra"
fi
