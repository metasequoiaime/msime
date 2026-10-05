#!/usr/bin/env bash
# 把一个已发布的 linux-vVERSION 提交到 openSUSE OBS，由 OBS 按发行版构建、签名并托管软件源，用户添加源之后就能 apt/dnf/zypper install msime。publish-linux-obs.yml 在每次发布后调用它，也可以手动运行。
#
# 用法：platforms/linux/packaging/obs/publish.sh VERSION DEFS RELEASE
#   DEFS 是 render-definitions.sh 的输出目录（MSIME_DEFINITIONS=rpm,debian 即可），里面要有 rpm/ 与 debian/。
#   RELEASE 是放发布资产的目录，里面要有 msime-VERSION.tar.xz 与 msime-VERSION-vendor.tar.xz；把它作为 MSIME_RELEASE_DIR 交给 render-definitions.sh，它会先对着同目录的 SHA256SUMS 核对，这里不再重复核对。
#   需要已经登录的 osc（~/.config/osc/oscrc，或 OSC_CONFIG 指向的配置）。
#
# 环境变量：
#   OBS_PROJECT  默认 home:<osc 配置里的用户名>
#   OBS_PACKAGE  默认 msime
#
# 每次运行都会：按 repositories.txt 和 OBS 的发行版列表重写项目配置（仓库、架构），写入 prjconf，确保包存在，再把包里的文件整体换成这次发布的（RPM 用 spec、rpmlintrc 与两个 tarball；Debian/Ubuntu 用 .dsc、.debian.tar.xz 与按 .dsc 要求改名的两个 orig tarball；加上 _constraints），提交后 OBS 自动开始构建。重复运行同一版本不会产生新的构建。
set -euo pipefail

if [ $# -ne 3 ]; then
  echo "usage: $0 VERSION DEFS RELEASE" >&2
  exit 2
fi
version=$1
defs=$(cd "$2" && pwd)
release=$(cd "$3" && pwd)
here=$(cd "$(dirname "$0")" && pwd)
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "version must be MAJOR.MINOR.PATCH: $version" >&2; exit 2; }

osc=(osc)
[ -z "${OSC_CONFIG:-}" ] || osc+=(--config "$OSC_CONFIG")
user=$("${osc[@]}" whois | awk '{print $1; exit}')
user=${user%:}
[ -n "$user" ] || { echo "osc is not logged in" >&2; exit 1; }
project=${OBS_PROJECT:-home:$user}
package=${OBS_PACKAGE:-msime}

need() { [ -f "$1" ] || { echo "missing $1 (run render-definitions.sh $version with MSIME_DEFINITIONS=rpm,debian and MSIME_RELEASE_DIR=RELEASE first)" >&2; exit 1; }; }
spec=$defs/rpm/msime.spec
rpmlintrc=$defs/rpm/msime-rpmlintrc
dsc=$defs/debian/msime_$version-1.dsc
debian_tar=$defs/debian/msime_$version-1.debian.tar.xz
source_tar=$release/msime-$version.tar.xz
vendor_tar=$release/msime-$version-vendor.tar.xz
for f in "$spec" "$rpmlintrc" "$dsc" "$debian_tar" "$source_tar" "$vendor_tar"; do need "$f"; done

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "== project $project"
# 发行版名字到 OBS 项目、仓库与架构的映射以 OBS 自己的发行版列表为准；只构建 x86_64 与 aarch64，与 spec 的 ExclusiveArch 和 control 的 Architecture 一致。
"${osc[@]}" api /distributions > "$work/distributions.xml"
python3 - "$work/distributions.xml" "$here/repositories.txt" "$project" "$user" > "$work/project.xml" <<'PY'
import sys
import xml.etree.ElementTree as ET
from xml.sax.saxutils import quoteattr

dists_path, wanted_path, project, user = sys.argv[1:5]
wanted = [line.split() for line in open(wanted_path, encoding="utf-8") if line.strip() and not line.startswith("#")]
by_name = {}
for d in ET.parse(dists_path).getroot().findall("distribution"):
    by_name[d.findtext("reponame")] = d
lines = [
    f"<project name={quoteattr(project)}>",
    "  <title>水杉输入法 MSIME</title>",
    "  <description>水杉输入法（MSIME）的 Linux 包，含 Fcitx5 与 IBus 输入法。由 https://github.com/metasequoiaime/msime 的发布流程在每次发布后自动提交。</description>",
    f"  <person userid={quoteattr(user)} role=\"maintainer\"/>",
    "  <build><enable/></build>",
    "  <publish><enable/></publish>",
]
for name, *override in wanted:
    d = by_name.get(name)
    if d is None:
        print(f"skip {name}: not in the OBS distribution list", file=sys.stderr)
        continue
    archs = [a.text for a in d.findall("architecture") if a.text in ("x86_64", "aarch64")]
    if not archs:
        print(f"skip {name}: no x86_64 or aarch64", file=sys.stderr)
        continue
    lines.append(f"  <repository name={quoteattr(name)}>")
    paths = [p.split("/", 1) for p in override[0].split(",")] if override else [(d.findtext("project"), d.findtext("repository"))]
    lines += [f"    <path project={quoteattr(p)} repository={quoteattr(r)}/>" for p, r in paths]
    lines += [f"    <arch>{a}</arch>" for a in archs]
    lines.append("  </repository>")
lines.append("</project>")
print("\n".join(lines))
PY
"${osc[@]}" meta prj "$project" -F "$work/project.xml"
"${osc[@]}" meta prjconf "$project" -F "$here/prjconf"

echo "== package $project/$package"
cat > "$work/package.xml" <<EOF
<package name="$package" project="$project">
  <title>水杉输入法 MSIME</title>
  <description>开源中文输入法，Fcitx5 与 IBus 输入法引擎，支持全拼、双拼与五笔。装好后每个用户运行一次 msime-linux-setup --download 完成首次配置。</description>
  <url>https://msime.app</url>
</package>
EOF
"${osc[@]}" meta pkg "$project" "$package" -F "$work/package.xml"

echo "== upload $version"
(
  cd "$work"
  "${osc[@]}" checkout "$project" "$package" >/dev/null
  cd "$project/$package"
  # 包里的文件整体换成这次发布的，上一版的 tarball 不留。
  find . -maxdepth 1 -type f ! -name '.*' -delete
  cp "$spec" "$rpmlintrc" "$dsc" "$debian_tar" "$source_tar" "$vendor_tar" "$here/_constraints" .
  # .dsc 里写的是 Debian 的 orig 名字，按它改名的副本供 Debian/Ubuntu 仓库使用；RPM 仓库用原名的那两个。
  cp "$source_tar" "msime_$version.orig.tar.xz"
  cp "$vendor_tar" "msime_$version.orig-vendor.tar.xz"
  "${osc[@]}" addremove >/dev/null
  if "${osc[@]}" status | grep -q .; then
    "${osc[@]}" commit -m "Update to $version (linux-v$version)"
  else
    echo "already at $version, nothing to commit"
  fi
)
echo "== $project/$package: https://build.opensuse.org/package/show/$project/$package"
