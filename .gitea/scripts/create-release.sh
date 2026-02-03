#!/bin/sh
set -eu

TAG="$GITHUB_REF_NAME"
VERSION=$(echo "$TAG" | sed 's/^v//')
DATE=$(date +%Y-%m-%d)

echo "🚀 Creating release for $TAG (safe mode)"

# -------------------------------------------------------------------
# 1. Extract release notes from annotated tag
# -------------------------------------------------------------------

NOTES=$(git tag -l "$TAG" --format='%(contents)')

if [ -z "$NOTES" ]; then
  echo "❌ Tag message is empty"
  exit 1
fi

git checkout main

# -------------------------------------------------------------------
# 2. Update all package.json versions
# -------------------------------------------------------------------

echo "🔧 Updating package.json versions to $VERSION"

find . -name package.json ! -path "*/node_modules/*" | while read file; do
  tmp_file="$file.tmp"
  jq --arg v "$VERSION" '.version = $v' "$file" >"$tmp_file"
  mv "$tmp_file" "$file"
done

# -------------------------------------------------------------------
# 3. Update CHANGELOG.md (prepend)
# -------------------------------------------------------------------

tmp_changelog="CHANGELOG.tmp"
{
  echo "## $TAG ($DATE)"
  echo ""
  echo "$NOTES"
  echo ""
  echo "---"
  echo ""
  if [ -f CHANGELOG.md ]; then
    cat CHANGELOG.md
  fi
} >"$tmp_changelog"

mv "$tmp_changelog" CHANGELOG.md

# -------------------------------------------------------------------
# 4. Create release commit
# -------------------------------------------------------------------

git config user.name "release-bot"
git config user.email "release-bot@ci"

git add CHANGELOG.md $(find . -name package.json ! -path "*/node_modules/*")

# Skip commit if nothing changed
if git diff --cached --quiet; then
  echo "No changes to commit for release $TAG"
  exit 0
fi

git commit -m "chore(release): $TAG"

# -------------------------------------------------------------------
# 5. Push changes
# -------------------------------------------------------------------

git push origin main

echo "✅ Release commit for $TAG created successfully (tag untouched)"
