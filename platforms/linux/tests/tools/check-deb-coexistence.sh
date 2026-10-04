#!/usr/bin/env bash
# 几个版本的 .deb（msime-linux 和 msime-linux-<id>）能不能同时装、卸载一个会不会带走另一个。它们装进同一个干净的 Debian 容器（与 Dockerfile.build-gate 同一个基础镜像），确认 dpkg 不报文件冲突、各自的首次配置命令都在 /usr/bin 下；然后逐个卸载，每卸一个都用 dpkg --verify 核对其余的包，一个文件都没少。maintainer 脚本在容器里没有 systemd 和 loginctl，按设计什么也不做（tests/core/deb_maintainer_scripts.py 另测它们的行为）。
#
# Usage: platforms/linux/tests/tools/check-deb-coexistence.sh <package.deb>...
# Needs docker and network access for the packages' dependencies.
set -euo pipefail

[ $# -gt 0 ] || { echo "usage: check-deb-coexistence.sh <package.deb>..." >&2; exit 2; }
dir=""
names=()
for deb_path in "$@"; do
  [ -f "$deb_path" ] || { echo "no such file: $deb_path" >&2; exit 2; }
  deb_dir=$(cd "$(dirname "$deb_path")" && pwd)
  [ -z "$dir" ] || [ "$dir" = "$deb_dir" ] || { echo "every package must be in the same directory" >&2; exit 2; }
  dir=$deb_dir
  names+=("$(basename "$deb_path")")
done

image=$(sed -n 's/^FROM \(rust:[^ ]*\).*/\1/p' "$(dirname "$0")/Dockerfile.build-gate")
[ -n "$image" ] || { echo "could not read the Debian image from Dockerfile.build-gate" >&2; exit 2; }

docker run --rm -v "$dir":/dist:ro "$image" bash -euo pipefail -c '
  export DEBIAN_FRONTEND=noninteractive
  packages=()
  files=()
  for file in "$@"; do
    packages+=("$(dpkg-deb -f "/dist/$file" Package)")
    files+=("/dist/$file")
  done
  apt-get update -qq
  apt-get install -y -qq --no-install-recommends "${files[@]}"
  for package in "${packages[@]}"; do
    dpkg -s "$package" >/dev/null
    test -x "/usr/bin/$package-setup"
  done
  for index in "${!packages[@]}"; do
    apt-get remove -y -qq "${packages[$index]}"
    if dpkg -s "${packages[$index]}" 2>/dev/null | grep -q "^Status: install ok installed"; then
      echo "${packages[$index]} is still installed after apt-get remove" >&2
      exit 1
    fi
    for remaining in "${packages[@]:$((index + 1))}"; do
      if ! dpkg --verify "$remaining"; then
        echo "removing ${packages[$index]} changed or took files of $remaining" >&2
        exit 1
      fi
      test -x "/usr/bin/$remaining-setup"
    done
  done
  echo "deb coexistence check passed: ${packages[*]}"
' _ "${names[@]}"
