#!/usr/bin/env bash
# 核对一个 .deb（或解开的目录）里每个 ELF 文件要求的符号版本不超过给定的上限，用来确认 legacy 包（#6311）能在 glibc 2.28 基线的系统上加载。
#
# Usage: check-elf-symbol-versions.sh <包.deb 或目录> PREFIX=VERSION...
#   例如 check-elf-symbol-versions.sh msime-linux_1.0.0_arm64.deb GLIBC=2.28 GLIBCXX=3.4.25 CXXABI=1.3.11 GCC=7.0.0
#   PREFIX 是 readelf -V 里版本名的前缀（GLIBC、GLIBCXX、CXXABI、GCC），VERSION 是目标系统的库定义的最高版本。没列出的前缀不检查。
#
# 读的是每个 ELF 文件的版本需求（.gnu.version_r），不是它链接的库：上游预编译的 sherpa-onnx 与 ONNX Runtime 库也在检查范围里，它们才是最可能超出的。
# 需要 readelf（binutils）；参数是 .deb 时还需要 dpkg-deb。全部不超出时退出 0，有超出时逐条打印「文件 needs 版本」并退出 1，用法错误退出 2。
set -euo pipefail

usage() {
  sed -n 's/^# \{0,1\}//; 4,6p' "$0" >&2
  exit 2
}
[ "$#" -ge 2 ] || usage
input=$1
shift

declare -A ceiling=()
for pair in "$@"; do
  [[ "$pair" =~ ^([A-Z]+)=([0-9]+(\.[0-9]+)+)$ ]] || { echo "not PREFIX=VERSION: $pair" >&2; exit 2; }
  ceiling[${BASH_REMATCH[1]}]=${BASH_REMATCH[2]}
done
command -v readelf >/dev/null || { echo "readelf (binutils) is required" >&2; exit 2; }

if [ -d "$input" ]; then
  root=$input
elif [ -f "$input" ]; then
  command -v dpkg-deb >/dev/null || { echo "dpkg-deb is required to unpack $input" >&2; exit 2; }
  root=$(mktemp -d)
  trap 'rm -rf "$root"' EXIT
  dpkg-deb -x "$input" "$root"
else
  echo "no such package or directory: $input" >&2
  exit 2
fi

checked=0
too_new=0
while IFS= read -r -d '' path; do
  # 只看 ELF 文件；用文件头判断，不依赖 file 命令（最小镜像里没有它）。
  [ "$(head -c 4 "$path" | od -An -c | tr -d ' \n')" = '177ELF' ] || continue
  checked=$((checked + 1))
  needs=$(readelf -V "$path" 2>/dev/null || true)
  for prefix in "${!ceiling[@]}"; do
    limit=${ceiling[$prefix]}
    while IFS= read -r needed; do
      [ -n "$needed" ] || continue
      if [ "$(printf '%s\n%s\n' "$needed" "$limit" | sort -V | tail -1)" != "$limit" ]; then
        echo "${path#"$root"} needs ${prefix}_$needed, the target provides up to ${prefix}_$limit" >&2
        too_new=1
      fi
    done < <(grep -oE "\b${prefix}_[0-9]+(\.[0-9]+)+" <<<"$needs" | sed "s/^${prefix}_//" | sort -uV || true)
  done
done < <(find "$root" -type f -print0)

[ "$checked" -gt 0 ] || { echo "no ELF file in $input" >&2; exit 1; }
summary=$(for prefix in "${!ceiling[@]}"; do printf '%s_%s\n' "$prefix" "${ceiling[$prefix]}"; done | sort | tr '\n' ' ')
if [ "$too_new" = 1 ]; then
  echo "$input needs a library newer than ${summary% }" >&2
  exit 1
fi
echo "$checked ELF files in $input need no symbol version newer than ${summary% }"
