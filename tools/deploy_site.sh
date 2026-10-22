#!/usr/bin/env bash
# Deploy the Product A site to GitHub Pages.
#
# The site is static HTML with relative links, so it is served from the
# repository's gh-pages branch and appears at devansh0703.github.io/girc.
#
# Usage: tools/deploy_site.sh [source-dir]
set -euo pipefail

REPO=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
SOURCE=${1:-$REPO/web/girc-site}
BRANCH=gh-pages

if [ ! -f "$SOURCE/index.html" ]; then
  echo "no index.html in $SOURCE" >&2
  exit 1
fi

cd "$REPO"

# The site's own history lives on the gh-pages branch; the working tree
# stays on master.
git fetch origin "$BRANCH" 2>/dev/null || true
if git rev-parse --verify "origin/$BRANCH" >/dev/null 2>&1; then
  git checkout -B "$BRANCH" "origin/$BRANCH"
else
  git checkout --orphan "$BRANCH"
fi

git rm -rq --cached . 2>/dev/null || true
rm -rf ./* 2>/dev/null || true

cp -R "$SOURCE"/. .
touch .nojekyll

git add -A
git -c user.name="${GIT_AUTHOR_NAME:-devansh0703}" \
    -c user.email="${GIT_AUTHOR_EMAIL:-151062727+devansh0703@users.noreply.github.com}" \
    commit -q -m "build: the Product A site" || echo "nothing to commit"

git push -q origin "$BRANCH"

# Back to the working branch.
git checkout -q master

gh api "repos/{owner}/{repo}/pages" \
  -X PUT \
  -f source[branch]="$BRANCH" \
  -f source[path]=/ \
  --silent 2>/dev/null || true

echo "deployed $SOURCE to origin/$BRANCH"
