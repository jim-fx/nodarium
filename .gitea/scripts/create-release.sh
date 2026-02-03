#!/usr/bin/env bash
set -euo pipefail

TAG="$GITHUB_REF_NAME"
VERSION="${TAG#v}"
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

find . -name package.json -not -path "*/node_modules/*" | while read -r file; do
  jq --arg v "$VERSION" '.version = $v' "$file" >"$file.tmp"
  mv "$file.tmp" "$file"
done

# -------------------------------------------------------------------
# 3. Update CHANGELOG.md (prepend)
# -------------------------------------------------------------------

{
  echo "## $TAG ($DATE)"
  echo ""
  echo "$NOTES"
  echo ""
  echo "---"
  echo ""
  cat CHANGELOG.md 2>/dev/null || true
} >CHANGELOG.tmp

mv CHANGELOG.tmp CHANGELOG.md

# -------------------------------------------------------------------
# 4. Create release commit
# -------------------------------------------------------------------

git config user.name "release-bot"
git config user.email "release-bot@ci"

git add CHANGELOG.md $(find . -name package.json -not -path "*/node_modules/*")

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
