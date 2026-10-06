#!/usr/bin/env bash
# Packages a built `dima` binary for release (D67): the binary, shell completions, man pages,
# README, licences, changelog and citation metadata, as dima-<version>-<target>.tar.gz (.zip on
# Windows). Runs the binary to generate completions, so it must run on (or emulate) the target.
#
# Usage: scripts/release/package-cli.sh TARGET VERSION BINARY OUT_DIR [MAN_DIR]
set -euo pipefail

target=$1
version=$2
binary=$3
out=$4
man=${5:-}
root=$(cd "$(dirname "$0")/../.." && pwd)
name="dima-${version}-${target}"
stage="${out}/${name}"

rm -rf "$stage"
mkdir -p "$stage/completions"
exe=dima
case "$target" in *windows*) exe=dima.exe ;; esac
cp "$binary" "$stage/$exe"

"$stage/$exe" completions bash > "$stage/completions/dima.bash"
"$stage/$exe" completions zsh > "$stage/completions/_dima"
"$stage/$exe" completions fish > "$stage/completions/dima.fish"
"$stage/$exe" completions elvish > "$stage/completions/dima.elv"
"$stage/$exe" completions powershell > "$stage/completions/_dima.ps1"

for f in README.md LICENSE THIRD-PARTY-NOTICES.md CHANGELOG.md CITATION.cff; do
  cp "$root/$f" "$stage/"
done
if [ -n "$man" ]; then
  mkdir -p "$stage/man/man1"
  cp "$man"/*.1 "$stage/man/man1/"
fi

case "$target" in
  *windows*) (cd "$out" && 7z a -tzip -bso0 "${name}.zip" "$name") ; echo "${out}/${name}.zip" ;;
  *) tar -C "$out" -czf "${out}/${name}.tar.gz" "$name" ; echo "${out}/${name}.tar.gz" ;;
esac
rm -rf "$stage"
