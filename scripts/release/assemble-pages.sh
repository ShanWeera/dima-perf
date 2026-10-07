#!/usr/bin/env bash
# Assembles the GitHub Pages site around a freshly built /latest/ and /api/ (D102):
#   - the JSON Schemas at /schemas/1.0/ (the URLs in their $id);
#   - each released minor version at /vX.Y/, unpacked from the docs archive attached to its
#     newest patch release (old versions are never rebuilt);
#   - /versions.json for the version switcher (latest first, then newest release first);
#   - a root page that redirects to /latest/, and the site's 404 page at the root (GitHub
#     Pages serves /404.html for every missing path; its links are absolute).
#
# Usage: GH_TOKEN=… REPO=owner/name scripts/release/assemble-pages.sh SITE_DIR
set -euo pipefail

site=$1
root=$(cd "$(dirname "$0")/../.." && pwd)
base=/dima-perf
work=$(mktemp -d)

mkdir -p "$site/schemas/1.0"
cp "$root"/schemas/*.schema.json "$site/schemas/1.0/"

# Newest patch of every released minor version.
declare -A newest
while read -r tag; do
  [[ "$tag" =~ ^v([0-9]+\.[0-9]+)\.([0-9]+)$ ]] || continue
  newest[${BASH_REMATCH[1]}]=${tag#v}
done < <(gh release list --repo "$REPO" --exclude-drafts --exclude-pre-releases --limit 500 \
           --json tagName --jq '.[].tagName' | sort -V)

entries=("{\"version\":\"latest\",\"path\":\"$base/latest/\"}")
for minor in $(printf '%s\n' "${!newest[@]}" | sort -V -r); do
  version=${newest[$minor]}
  archive="dima-$version-docs.tar.gz"
  if gh release download "v$version" --repo "$REPO" --pattern "$archive" --dir "$work" --clobber; then
    mkdir -p "$site/v$minor"
    tar -xzf "$work/$archive" -C "$site/v$minor"
    entries+=("{\"version\":\"$minor\",\"path\":\"$base/v$minor/\"}")
  else
    echo "::warning::release v$version has no $archive; its documentation is not published"
  fi
done
(IFS=,; printf '[%s]\n' "${entries[*]}") > "$site/versions.json"
cat "$site/versions.json"

cat > "$site/index.html" <<HTML
<!doctype html>
<html lang="en">
<meta charset="utf-8">
<title>DiMA documentation</title>
<meta http-equiv="refresh" content="0; url=$base/latest/">
<link rel="canonical" href="$base/latest/">
<p><a href="$base/latest/">DiMA documentation</a></p>
</html>
HTML
if [ -f "$site/latest/404.html" ]; then
  cp "$site/latest/404.html" "$site/404.html"
fi
