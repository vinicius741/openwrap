#!/bin/sh
# Starts a new OpenWrap release. GitHub Actions (.github/workflows/release.yml)
# then builds it on a free macOS runner and publishes a GitHub Release with
# OpenWrap-Share.zip and the in-app update files. Installed apps pick it up the
# next time they start.
#
# Usage: scripts/release.sh <version> ["release notes"]
#   scripts/release.sh 0.3.0 "Fixes reconnect after sleep"
#
# Bumps the workspace version in Cargo.toml, commits, creates the annotated
# tag v<version> (its message becomes the release notes) and pushes both.
set -eu

if [ $# -lt 1 ]; then
  echo "usage: $0 <version> [\"release notes\"]" >&2
  exit 64
fi

VERSION=${1#v}
NOTES=${2:-"OpenWrap $VERSION"}
TAG="v$VERSION"
ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT_DIR"

case "$VERSION" in
  *[!0-9.]* | "" | .* | *. ) echo "Version must look like 1.2.3, got: $VERSION" >&2; exit 64 ;;
esac

if [ "$(git rev-parse --abbrev-ref HEAD)" != "main" ]; then
  echo "Releases are cut from main." >&2
  exit 1
fi
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "Commit or stash your changes first." >&2
  exit 1
fi
if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  echo "Tag $TAG already exists." >&2
  exit 1
fi

CURRENT=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
echo "Current version: $CURRENT"
echo "New version:     $VERSION"
echo "Release notes:   $NOTES"
printf "Commit, tag %s and push to GitHub? [y/N] " "$TAG"
read -r answer
case "$answer" in
  y|Y) ;;
  *) echo "Aborted."; exit 1 ;;
esac

sed -i '' "s/^version = \"$CURRENT\"$/version = \"$VERSION\"/" Cargo.toml
# Refresh the workspace crates' versions in Cargo.lock without upgrading deps.
cargo update --workspace --offline >/dev/null 2>&1 || cargo update --workspace >/dev/null

git add Cargo.toml Cargo.lock
git commit -m "release: $TAG"
git tag -a "$TAG" -m "$NOTES"
git push origin main
git push origin "$TAG"

echo
echo "Pushed $TAG. Follow the build with:"
echo "  gh run watch \$(gh run list --workflow release.yml --limit 1 --json databaseId -q '.[0].databaseId')"
