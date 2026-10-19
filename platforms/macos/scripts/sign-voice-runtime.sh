#!/usr/bin/env bash
# 把 macOS 的 sherpa-onnx 运行库做成可以按需下载的资源包归档：取 resources/voice-runtime.lock.json 固定的上游 universal dylib，剥掉局部符号，用本项目的 Developer ID 加 hardened runtime 和安全时间戳签名，打成只含这一个文件的 ZIP，有公证凭据时再提交公证，最后写出填好归档地址、长度和 SHA-256 的 voice-runtime-macos.lock.json。
#
# 为什么要重签：输入法和它的 msime-voice-local 辅助进程开着 hardened runtime，库验证只加载同一 Team ID 签名的库；上游原样的 dylib 下载下来加载不了。签名带时间戳，每签一次字节都不同，所以每个 sherpa-onnx 版本只签一次、只发布一次，锁文件钉住的就是那一次的字节；同一个标签下的资产发布后永远不替换（publish-macos-voice-runtime.yml 会拒绝）。
#
# Usage: platforms/macos/scripts/sign-voice-runtime.sh OUT_DIR
#   OUT_DIR 收到 <archive>.zip 和 voice-runtime-macos.lock.json（把后者提交到 resources/ 即固定了这次发布）。
# Environment:
#   MACOS_SIGNING_IDENTITY  必需："Developer ID Application: ... (TEAMID)"，其 Team ID 必须等于 resources/voice-runtime-macos.lock.json 的 team_id
#   APPLE_ID, APPLE_TEAM_ID, APPLE_APP_SPECIFIC_PASSWORD  三者都有时提交公证并等待结果。dylib 不能装订公证票据，Gatekeeper 遇到带隔离属性的文件时在线查询；设置应用下载的文件不带隔离属性，公证是额外的一层。
set -Eeuo pipefail

repo_root=$(cd "$(dirname "$0")/../../.." && pwd)
out_dir=${1:?usage: sign-voice-runtime.sh OUT_DIR}
identity=${MACOS_SIGNING_IDENTITY:?set MACOS_SIGNING_IDENTITY to a Developer ID Application identity}
lock="$repo_root/resources/voice-runtime-macos.lock.json"
library=libsherpa-onnx-c-api.dylib

read_lock() {
  python3 -I -c 'import json, sys; print(json.load(open(sys.argv[1]))[sys.argv[2]])' "$lock" "$1"
}
team_id=$(read_lock team_id)
release_tag=$(read_lock release_tag)
version=$(python3 -I -c 'import json, sys; print(json.load(open(sys.argv[1]))["version"])' "$repo_root/resources/voice-runtime.lock.json")
if [ "$release_tag" != "voice-runtime-macos-v$version" ]; then
  echo "resources/voice-runtime-macos.lock.json names $release_tag, but resources/voice-runtime.lock.json pins sherpa-onnx $version; bump release_tag to voice-runtime-macos-v$version and clear archive and artifacts first" >&2
  exit 1
fi
case "$identity" in
  *"($team_id)") ;;
  *) echo "MACOS_SIGNING_IDENTITY is not an identity of Team $team_id, the team resources/voice-runtime-macos.lock.json names; an input method signed by that team could not load this runtime" >&2; exit 1 ;;
esac

work=$(mktemp -d "${TMPDIR:-/tmp}/msime-voice-runtime.XXXXXX")
trap 'rm -rf "$work"' EXIT
python3 "$repo_root/scripts/fetch_voice_runtime.py" --platform macos --out "$work/upstream"
mkdir -p "$work/signed"
cp "$work/upstream/$library" "$work/signed/$library"
signed="$work/signed/$library"
for architecture in arm64 x86_64; do
  lipo "$signed" -verify_arch "$architecture"
done
# 与以前随包的那份相同：`strip -x` 只去局部符号，C API 的导出都还在。
strip -x "$signed"

# 时间戳服务偶尔答 "The timestamp service is not available."，与 package-release.sh 一样重试；别的错误原样失败。
for attempt in 1 2 3 4 5; do
  if output=$(codesign --force --options runtime --timestamp --sign "$identity" "$signed" 2>&1); then
    break
  fi
  printf '%s\n' "$output" >&2
  case "$output" in
    *"timestamp service is not available"*) [ "$attempt" -lt 5 ] && sleep $((attempt * 15)) && continue ;;
  esac
  exit 1
done
codesign --verify --strict --verbose=2 "$signed"
# 先整段取出再匹配：直接接 `grep -q` 时 grep 读到就退出，codesign 收到 SIGPIPE，pipefail 会把匹配成功报成失败。
signature=$(codesign -dv "$signed" 2>&1)
signed_team=$(sed -n 's/^TeamIdentifier=//p' <<< "$signature")
if [ "$signed_team" != "$team_id" ]; then
  echo "the signed runtime carries Team ID '$signed_team', not $team_id" >&2
  exit 1
fi
grep -q 'flags=.*(runtime)' <<< "$signature" || { echo "the signed runtime has no hardened runtime flag" >&2; exit 1; }

archive_name="sherpa-onnx-v$version-macos-universal-signed.zip"
mkdir -p "$out_dir"
out_dir=$(cd "$out_dir" && pwd)
rm -f "$out_dir/$archive_name"
# -X 不带额外的文件属性，归档里只有这一个文件，名字就是安装后的文件名。
(cd "$work/signed" && zip -X -9 -q "$out_dir/$archive_name" "$library")

if [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ] && [ -n "${APPLE_APP_SPECIFIC_PASSWORD:-}" ]; then
  xcrun notarytool submit "$out_dir/$archive_name" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" --wait
else
  echo "warning: not notarized; set APPLE_ID, APPLE_TEAM_ID and APPLE_APP_SPECIFIC_PASSWORD to submit the archive" >&2
fi

python3 -I - "$lock" "$out_dir/$archive_name" "$signed" "$out_dir/voice-runtime-macos.lock.json" <<'PY'
import hashlib, json, sys
from pathlib import Path

lock_path, archive_path, library_path, output = (Path(argument) for argument in sys.argv[1:])
lock = json.loads(lock_path.read_text())


def pinned(path: Path) -> tuple[str, int]:
    data = path.read_bytes()
    return hashlib.sha256(data).hexdigest(), len(data)


archive_sha, archive_size = pinned(archive_path)
library_sha, library_size = pinned(library_path)
lock["archive"] = {
    "name": archive_path.name,
    "url": f"https://github.com/metasequoiaime/msime/releases/download/{lock['release_tag']}/{archive_path.name}",
    "sha256": archive_sha,
    "size": archive_size,
}
lock["artifacts"] = [
    {"name": library_path.name, "member": library_path.name, "sha256": library_sha, "size": library_size}
]
output.write_text(json.dumps(lock, indent=2, ensure_ascii=False) + "\n")
print(f"archive {archive_path.name}: {archive_size} bytes, sha256 {archive_sha}")
print(f"{library_path.name}: {library_size} bytes, sha256 {library_sha}")
PY
echo "signed runtime archive: $out_dir/$archive_name"
echo "lock to commit as resources/voice-runtime-macos.lock.json: $out_dir/voice-runtime-macos.lock.json"
