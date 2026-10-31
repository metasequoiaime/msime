#!/usr/bin/env bash
# 从本次 Linux 发布的 x86_64 RPM 重新打出可直接安装的 Arch Linux 二进制包。
# 用法：build-bin-release.sh VERSION RPM OUTDIR
set -euo pipefail
if [[ $# -ne 3 ]]; then echo "usage: $0 VERSION RPM OUTDIR" >&2; exit 2; fi
version=$1; rpm=$2; out=$3
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "invalid version: $version" >&2; exit 2; }
[[ -f "$rpm" ]] || { echo "RPM does not exist: $rpm" >&2; exit 1; }
mkdir -p "$out"; out=$(cd "$out" && pwd); rpm=$(cd "$(dirname "$rpm")" && pwd)/$(basename "$rpm")
here=$(cd "$(dirname "$0")" && pwd); work="$out/.build"; mkdir -p "$work"
cp "$here/msime-bin/PKGBUILD" "$work/PKGBUILD"
cp "$here/msime-bin/msime.install" "$work/msime.install"
rpm_name="msime-linux-${version}-1.x86_64.rpm"; cp "$rpm" "$work/$rpm_name"
rpm_hash=$(sha256sum "$rpm" | awk '{print $1}')
python3 - "$work/PKGBUILD" "$version" "$rpm_name" "$rpm_hash" <<'PY'
from pathlib import Path
import re, sys
path, version, rpm_name, digest = sys.argv[1:]
text = Path(path).read_text(encoding="utf-8")
for pattern, replacement, label in [
    (r"^pkgver=.*$", f"pkgver={version}", "pkgver"),
    (r"^source_x86_64=.*$", f"source_x86_64=(\"file:///input/{rpm_name}\")", "source_x86_64"),
    (r"^sha256sums_x86_64=.*$", f"sha256sums_x86_64=('{digest}')", "sha256sums_x86_64"),
]:
    text, count = re.subn(pattern, replacement, text, flags=re.MULTILINE)
    if count != 1: raise SystemExit(f"PKGBUILD {label} line is not unique")
Path(path).write_text(text, encoding="utf-8")
PY
docker run --rm --platform linux/amd64 \
  -v "$work":/work -v "$work/$rpm_name":/input/$rpm_name:ro -v "$out":/out \
  archlinux:latest bash -euo pipefail -c '
    sed -i "/^\[options\]/a DisableSandbox" /etc/pacman.conf
    pacman -Syu --noconfirm --needed base-devel patchelf >/dev/null
    useradd -m builder; chown -R builder: /work
    # PKGBUILD 在打包时核对动态库，必须装齐依赖；makepkg 经 sudo 调用 pacman。
    echo "builder ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/builder
    su builder -c "cd /work && SRCDEST=/input makepkg --syncdeps --noconfirm"
    cp /work/*.pkg.tar.zst /out/
  '
python3 - "$out" <<'PY'
from pathlib import Path
import sys
files = list(Path(sys.argv[1]).glob("msime-bin-*.pkg.tar.zst"))
if len(files) != 1: raise SystemExit(f"expected one Arch package, found {len(files)}")
print(files[0])
PY
python3 - "$work" <<'PY'
from pathlib import Path
import shutil, sys
shutil.rmtree(Path(sys.argv[1]), ignore_errors=True)
PY
