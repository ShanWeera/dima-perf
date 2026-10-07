#!/usr/bin/env bash
# The release tag on GitHub, for the release workflow (D67). Needs `gh` with GH_TOKEN set and
# GITHUB_REPOSITORY (both set in GitHub Actions).
#
#   tag.sh check TAG [COMMIT]   fails if a published release already exists for TAG (the workflow
#                               creates the release itself), or, when COMMIT is given, if TAG
#                               already exists on another commit
#   tag.sh create TAG COMMIT    creates the tag TAG on COMMIT, unless it is already there
#
# Usage: scripts/release/tag.sh check|create TAG [COMMIT]
set -euo pipefail

action=$1
tag=$2
commit=${3:-}
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

case $action in
  check)
    rc=0
    url=$(api "repos/$repo/releases/tags/$tag" --jq .html_url) || rc=$?
    if [ "$rc" -eq 0 ]; then
      echo "::error::a release for $tag already exists ($url). This workflow creates the release itself: delete that release and start the workflow again."
      exit 1
    fi
    if [ "$rc" -ne 1 ]; then fail; fi
    if [ -n "$commit" ]; then
      current=$(tag_commit)
      if [ -n "$current" ] && [ "$current" != "$commit" ]; then
        echo "::error::tag $tag already exists on commit $current, not on $commit. Delete the tag, or release a new version."
        exit 1
      fi
    fi
    ;;
  create)
    current=$(tag_commit)
    if [ -z "$current" ]; then
      api "repos/$repo/git/refs" -f "ref=refs/tags/$tag" -f "sha=$commit" > /dev/null || fail
      echo "created tag $tag on $commit"
    elif [ "$current" = "$commit" ]; then
      echo "tag $tag is already on $commit"
    else
      echo "::error::tag $tag already exists on commit $current, not on $commit"
      exit 1
    fi
    ;;
  *)
    echo "usage: $0 check|create TAG [COMMIT]" >&2
    exit 2
    ;;
esac
