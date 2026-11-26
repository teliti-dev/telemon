#!/usr/bin/env bash
# Copy docs/wiki/*.md to the GitHub wiki of this repository.
#
#   scripts/sync-wiki.sh           show what would change (dry run)
#   scripts/sync-wiki.sh --push    commit and push the changes to the wiki
#
# The wiki repository (<repo>.wiki.git) only exists after the first page has been created once in the GitHub UI
# (Wiki tab > "Create the first page" > Save). Authentication uses the GitHub CLI (`gh auth login`).
# Pages that exist only on the wiki are left alone; this script never deletes anything.
set -euo pipefail

REPO="${WIKI_REPO:-teliti-dev/telemon}"
PUSH=0
[ "${1:-}" = "--push" ] && PUSH=1

SRC="$(cd "$(dirname "$0")/../docs/wiki" && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

git_gh() { git -c credential.helper= -c credential.helper='!gh auth git-credential' "$@"; }

if ! git_gh clone --quiet "https://github.com/${REPO}.wiki.git" "$TMP/wiki" 2>/dev/null; then
  echo "Cannot clone the wiki of ${REPO}." >&2
  echo "Create the first page once in the GitHub UI (Wiki tab > Create the first page > Save), then run this again." >&2
  exit 1
fi

cp "$SRC"/*.md "$TMP/wiki/"
cd "$TMP/wiki"
git add -A

if git diff --cached --quiet; then
  echo "The wiki is already up to date."
  exit 0
fi

git diff --cached --stat
if [ "$PUSH" -eq 1 ]; then
  git commit --quiet -m "docs(wiki): sync from docs/wiki"
  git_gh push --quiet origin HEAD
  echo "Wiki updated: https://github.com/${REPO}/wiki"
else
  echo "Dry run. Run with --push to publish."
fi
