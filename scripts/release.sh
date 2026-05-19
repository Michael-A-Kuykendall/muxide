#!/usr/bin/env bash
# Release script for Muxide
# Usage: ./scripts/release.sh <version>
#
# Bumps Cargo.toml version, prompts for CHANGELOG entry, then commits,
# tags, and pushes to origin/main.

set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.2.4"
    exit 1
fi

VERSION=$1

echo "Preparing release $VERSION"

# Update version in Cargo.toml
# sed -i '' works on both macOS (BSD sed) and Linux (GNU sed).
if sed --version 2>&1 | grep -q GNU; then
    sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
else
    sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
fi

# Prompt for CHANGELOG entry
echo "Update CHANGELOG.md with the v$VERSION release notes, then press Enter to continue..."
read -r

# Commit version bump
git add Cargo.toml CHANGELOG.md
git commit -m "Release $VERSION"

# Create and push tag
git tag "v$VERSION"
git push origin main
git push origin "v$VERSION"

echo "Release $VERSION tagged and pushed."
echo "Check the Actions tab for build and publish progress."