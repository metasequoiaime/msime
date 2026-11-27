#!/usr/bin/env bash
# Publish the GitHub release <tag> at <target> with the given assets. A release of the same tag is overwritten in place rather than refused, so a release workflow re-run for the same version replaces it: its assets are uploaded over the old ones, assets the new build no longer has are removed, and the title, notes and prerelease flag are replaced. Overwriting in place keeps the release page and every asset URL that is not renamed answering throughout, which deleting the release and creating it again would not while the new assets upload.
#
# The tag is never moved. Releases are triggered by pushing the tag (AGENTS.md「发版」), so <target> is the commit the tag already points at; a release whose tag points elsewhere is refused before anything is uploaded. Moving the ref used to rewrite an annotated tag into a lightweight one, and because the workflows run this with RELEASE_PUBLISH_TOKEN, a personal access token, the ref update raised a new tag push event that started a second full release run.
#
# Usage: publish-release.sh <tag> <target-commit> <title> <notes-file> <asset>...
# Environment: GH_TOKEN; PRERELEASE=true marks a prerelease; RELEASE_LATEST=false keeps the release from becoming the repository's latest. GH_REPO names the repository when the working directory is not a checkout of it.
# Written for bash 3.2 as well, the bash a macOS runner may start: no associative arrays, and an empty array is expanded guarded under `set -u`.
set -euo pipefail

[[ $# -ge 5 ]] || { echo "usage: publish-release.sh <tag> <target-commit> <title> <notes-file> <asset>..." >&2; exit 2; }
tag=$1 target=$2 title=$3 notes=$4
shift 4

flags=()
[[ "${PRERELEASE:-false}" == true ]] && flags+=(--prerelease)
[[ "${RELEASE_LATEST:-}" == false ]] && flags+=(--latest=false)

# The commit the tag names, through an annotated tag object if there is one; empty when the tag is not on the remote yet.
tagged=$(gh api "repos/{owner}/{repo}/commits/$tag" --jq .sha 2>/dev/null || true)
if [[ -n "$tagged" && "$tagged" != "$target" ]]; then
  echo "Tag $tag points at $tagged, not at $target; a release is published on the commit its tag names and the tag is never moved" >&2
  exit 1
fi

if ! gh release view "$tag" >/dev/null 2>&1; then
  gh release create "$tag" "$@" --target "$target" --title "$title" --notes-file "$notes" ${flags[@]+"${flags[@]}"}
  exit 0
fi

echo "Release $tag exists; overwriting it with this build" >&2
gh release upload "$tag" "$@" --clobber
uploaded=$(for asset in "$@"; do basename -- "$asset"; done)
while IFS= read -r name; do
  if [[ -n "$name" ]] && ! grep -qxF -- "$name" <<<"$uploaded"; then
    gh release delete-asset "$tag" "$name" --yes
  fi
done < <(gh release view "$tag" --json assets --jq '.assets[].name')
gh release edit "$tag" --title "$title" --notes-file "$notes" --prerelease="$([[ "${PRERELEASE:-false}" == true ]] && echo true || echo false)" \
  ${RELEASE_LATEST:+--latest="$RELEASE_LATEST"}
