#!/usr/bin/env bash
# 仅由明确要求发布的 workflow 调用；符号独立于用户下载页，按构建运行号长期保留。
set -euo pipefail
platform=${1:?platform required}
shift
case "$platform" in android|linux|harmony|windows7) ;; *) echo "unsupported symbol platform" >&2; exit 1 ;; esac
[[ $# -gt 0 ]] || { echo "no symbol archives" >&2; exit 1; }
for archive in "$@"; do [[ -f "$archive" && "$archive" == *.zip ]] || { echo "missing symbol archive" >&2; exit 1; }; done
tag="$platform-symbols"
if ! gh release view "$tag" >/dev/null 2>&1; then
  gh release create "$tag" --target "${MSIME_BUILD_REVISION:-$(git rev-parse HEAD)}" \
    --title "$platform 调试符号" --notes '维护者调试归档。每份 zip 包含构建身份、模块 build-id、SHA-256 和原生行号，不属于用户安装包。' \
    --prerelease --latest=false || gh release view "$tag" >/dev/null
fi
gh release upload "$tag" "$@" --clobber
