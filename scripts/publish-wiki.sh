#!/usr/bin/env bash
# Copy docs/wiki/*.md into the repository wiki and push it.
#
# GitHub has no API for wiki pages: the wiki is a separate git repository, and it
# only springs into existence once somebody has created the first page in the web
# UI. After that, this script is the whole publishing step.
#
#   scripts/publish-wiki.sh [owner/repo]
set -euo pipefail

REPO="${1:-RustChrome/RustChrome}"
SRC="$(cd "$(dirname "$0")/.." && pwd)/docs/wiki"

[ -d "$SRC" ] || { echo "no docs/wiki directory"; exit 1; }

# git here is a native Windows build and cannot resolve MSYS /tmp paths, so the
# working copy lives next to the repository instead of in a system temp dir.
WORK="$(cd "$(dirname "$0")/.." && pwd)/.wiki-work"
rm -rf "$WORK"
mkdir -p "$WORK"
# Native git needs a Windows path; bash needs the MSYS one.
WORK_WIN="$(cygpath -w "$WORK" 2>/dev/null || echo "$WORK")"
trap 'rm -rf "$WORK"' EXIT

if ! git clone --quiet "https://github.com/${REPO}.wiki.git" "$WORK_WIN/wiki" 2>/dev/null; then
  cat <<EOF
The wiki repository does not exist yet.

GitHub creates it when the first page is made through the web UI — there is no
API for that one step:

  1. Open https://github.com/${REPO}/wiki
  2. Choose "Create the first page"
  3. Title: Home    Body: anything (it is about to be overwritten)
  4. Save, then run this script again.
EOF
  exit 2
fi

cp "$SRC"/*.md "$WORK/wiki/"
cd "$WORK/wiki"
git add -A
if git diff --cached --quiet; then
  echo "wiki already up to date"
  exit 0
fi
git -c user.name="RustChrome docs" -c user.email="noreply@github.com" \
  commit --quiet -m "docs: sync wiki from docs/wiki"
git push --quiet
echo "wiki updated: https://github.com/${REPO}/wiki"
