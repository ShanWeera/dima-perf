#!/usr/bin/env bash
# The release tag on GitHub, for the release workflow (D67). Needs `gh` with GH_TOKEN set and
# GITHUB_REPOSITORY (both set in GitHub Actions). COMMIT is the full SHA of the commit the
# workflow run builds.
#
#   tag.sh check TAG COMMIT    fails if a published release already exists for TAG, or if TAG
#                              exists on a commit other than COMMIT
#   tag.sh create TAG COMMIT   creates the tag TAG on COMMIT, unless it is already there
#   tag.sh verify TAG COMMIT   fails unless TAG exists on COMMIT
#   tag.sh remove TAG COMMIT   deletes TAG if it is on COMMIT and has no published release
#
# Usage: scripts/release/tag.sh check|create|verify|remove TAG COMMIT
set -euo pipefail

if [ $# -ne 3 ]; then
  echo "usage: $0 check|create|verify|remove TAG COMMIT" >&2
  exit 2
fi
action=$1
tag=$2
commit=$3
repo=$GITHUB_REPOSITORY
err=$(mktemp)
trap 'rm -f -- "$err"' EXIT

# `gh api` with the given arguments: prints the response and returns 0, returns 1 when the
# resource does not exist (HTTP 404) and 2 on any other failure; `fail` then reports the error.
api() {
  if gh api "$@" 2>"$err"; then return 0; fi
  if grep -q '(HTTP 404)' "$err"; then return 1; fi
  return 2
}
fail() {
  cat "$err" >&2
  exit 2
}

# The commit TAG points at (an annotated tag is followed to its commit); empty if there is no TAG.
tag_commit() {
  local object rc=0
  object=$(api "repos/$repo/git/ref/tags/$tag" --jq '.object.type + " " + .object.sha') || rc=$?
  if [ "$rc" -eq 1 ]; then return 0; fi
  if [ "$rc" -ne 0 ]; then fail; fi
  local type=${object%% *} sha=${object#* }
  if [ "$type" = tag ]; then
    sha=$(api "repos/$repo/git/tags/$sha" --jq .object.sha) || fail
  fi
  printf '%s\n' "$sha"
}

# The URL of the published release for TAG; empty if there is none (drafts do not count).
release_url() {
  local url rc=0
  url=$(api "repos/$repo/releases/tags/$tag" --jq .html_url) || rc=$?
  if [ "$rc" -eq 1 ]; then return 0; fi
  if [ "$rc" -ne 0 ]; then fail; fi
  printf '%s\n' "$url"
}

case $action in
  check)
    url=$(release_url)
    if [ -n "$url" ]; then
      echo "::error::$tag is already released ($url). To release changes, set a new version in Cargo.toml and CHANGELOG.md and release that."
      exit 1
    fi
    current=$(tag_commit)
    if [ -n "$current" ] && [ "$current" != "$commit" ]; then
      echo "::error::tag $tag already exists on commit $current, but this run builds $commit. Delete that tag, or release a new version."
      exit 1
    fi
    ;;
  create)
    current=$(tag_commit)
    if [ -z "$current" ]; then
      if ! api "repos/$repo/git/refs" -f "ref=refs/tags/$tag" -f "sha=$commit" > /dev/null; then
        echo "::error::GitHub refused to create the tag $tag on $commit. It can refuse when the default branch has moved on since the run started; start a new run from the Actions page."
        fail
      fi
      echo "created tag $tag on $commit"
    elif [ "$current" = "$commit" ]; then
      echo "tag $tag is already on $commit"
    else
      echo "::error::tag $tag already exists on commit $current, not on $commit"
      exit 1
    fi
    ;;
  verify)
    current=$(tag_commit)
    if [ -z "$current" ]; then
      echo "::error::tag $tag no longer exists, but this run built $commit. Push the tag again to release it."
      exit 1
    elif [ "$current" != "$commit" ]; then
      echo "::error::tag $tag was moved to commit $current, but this run built $commit. The new tag position needs its own run: push the tag again."
      exit 1
    fi
    echo "tag $tag is on $commit"
    ;;
  remove)
    url=$(release_url)
    current=$(tag_commit)
    if [ -n "$url" ]; then
      echo "tag $tag kept: it has a published release ($url)"
    elif [ -z "$current" ]; then
      echo "tag $tag does not exist; nothing to remove"
    elif [ "$current" != "$commit" ]; then
      echo "tag $tag is on another commit ($current); left as it is"
    else
      api "repos/$repo/git/refs/tags/$tag" -X DELETE > /dev/null || fail
      echo "removed tag $tag from $commit"
    fi
    ;;
  *)
    echo "usage: $0 check|create|verify|remove TAG COMMIT" >&2
    exit 2
    ;;
esac
