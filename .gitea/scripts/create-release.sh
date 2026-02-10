#!/bin/sh
set -eu

TAG="$GITHUB_REF_NAME"
VERSION=$(echo "$TAG" | sed 's/^v//')
DATE=$(date +%Y-%m-%d)

echo "🚀 Creating release for $TAG"

# -------------------------------------------------------------------
# 1. Extract release notes from annotated tag
# -------------------------------------------------------------------

# Ensure the local git knows this is an annotated tag with metadata
git fetch origin "refs/tags/$TAG:refs/tags/$TAG" --force

# %(contents) gets the whole message.
# If you want ONLY what you typed after the first line, use %(contents:body)
NOTES=$(git tag -l "$TAG" --format='%(contents)' | sed '/-----BEGIN PGP SIGNATURE-----/,/-----END PGP SIGNATURE-----/d')

if [ -z "$(echo "$NOTES" | tr -d '[:space:]')" ]; then
  echo "❌ Tag message is empty or tag is not annotated"
  exit 1
fi

git checkout main

# -------------------------------------------------------------------
# 2. Update all package.json versions
# -------------------------------------------------------------------
echo "🔧 Updating package.json versions to $VERSION"
find . -name package.json ! -path "*/node_modules/*" | while read -r file; do
  tmp_file="$file.tmp"
  jq --arg v "$VERSION" '.version = $v' "$file" >"$tmp_file"
  mv "$tmp_file" "$file"
done

# -------------------------------------------------------------------
# 3. Generate commit list since last release
# -------------------------------------------------------------------
LAST_TAG=$(git tag --sort=-creatordate | grep -v "^$TAG$" | head -n 1 || echo "")

if [ -n "$LAST_TAG" ]; then
  # Filter out previous 'chore(release)' commits so the list stays clean
  COMMITS=$(git log "$LAST_TAG"..HEAD --pretty=format:'* [%h](https://git.max-richter.dev/max/nodarium/commit/%H) %s' | grep -v "chore(release)")
else
  COMMITS=$(git log HEAD --pretty=format:'* [%h](https://git.max-richter.dev/max/nodarium/commit/%H) %s' | grep -v "chore(release)")
fi

# -------------------------------------------------------------------
# 4. Update CHANGELOG.md (prepend)
# -------------------------------------------------------------------
tmp_changelog="CHANGELOG.tmp"
{
  echo "# $TAG ($DATE)"
  echo ""
  echo "$NOTES"
  echo ""
  if [ -n "$COMMITS" ]; then
    echo "---"
    echo "$COMMITS"
    echo ""
  fi
  echo ""
  if [ -f CHANGELOG.md ]; then
    cat CHANGELOG.md
  fi
} >"$tmp_changelog"

mv "$tmp_changelog" CHANGELOG.md

pnpm exec dprint fmt CHANGELOG.md

# -------------------------------------------------------------------
# 5. Create release commit
# -------------------------------------------------------------------
git config user.name "release-bot"
git config user.email "release-bot@ci"

git add CHANGELOG.md $(find . -name package.json ! -path "*/node_modules/*")

if git diff --cached --quiet; then
  echo "No changes to commit for release $TAG"
else
  git commit -m "chore(release): $TAG"
  git push origin main
fi

echo "✅ Release process for $TAG complete"
