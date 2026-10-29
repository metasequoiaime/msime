#!/usr/bin/env bash
# 把 `cargo cinstall` 装好的前缀打成 msime-engine-capi 的预编译归档，`.github/workflows/release-engine-capi.yml` 在每个平台上调用它。
#
#   bash crates/engine-capi/packaging/package.sh <装好的前缀> <归档名> <输出目录>
#
# 归档的顶层目录就是 <归档名>，里面是：
#
# - `include/msime_engine.h`、`lib/`（共享库和静态库；Windows 上还有 `bin/msime_engine.dll`，导入库是 `lib/msime_engine.dll.lib`）、`lib/pkgconfig/msime-engine.pc`。`.pc` 的 prefix 改成 `${pcfiledir}/../..`，归档解到哪里都能直接用 `PKG_CONFIG_PATH` 指过去；macOS 上共享库的 install name 改成 `@rpath/libmsime_engine.1.dylib`（cargo-c 写的是构建机上的绝对路径），改完重新做 ad-hoc 签名。
# - `examples/c/`：README「快速上手」编译的那个 C 示例和它的 CMakeLists.txt。
# - `README.md`（crate 的 README）、`LICENSE`（MPL-2.0），`licenses/` 里是引擎自己的 `LICENSE`（同一份 MPL-2.0 加 Exhibit A）、编进引擎的数据表与改编代码的许可证全文（`crates/engine/licenses/`，与 `resources/licenses/` 里的同名文件逐字节相同）和 `ewts-MIT.txt`，这是 README「二进制归档与第三方声明」要求的那几份。
# - `THIRD_PARTY_NOTICES.txt`：在本平台上按 msime-engine-capi 的依赖图生成（`platforms/linux/collect-notices.py`）。
# - `SOURCE.txt`：对应的源码在哪里（MPL-2.0 §3.2），以及 rink-core 的源码位置。
#
# Linux 和 macOS 打 `.tar.gz`（保留 `.so` 的版本符号链接），Windows 打 `.zip`。打完之后的验收不在这里：workflow 把归档解到别处，用 `crates/engine-capi/tests/c-example.sh --prefix` 编译并运行示例。
set -euo pipefail

[[ $# -eq 3 ]] || { echo "usage: $0 <installed prefix> <archive name> <output directory>" >&2; exit 2; }
root=$(cd "$(dirname "$0")/../../.." && pwd)
installed=$(cd "$1" && pwd)
name=$2
mkdir -p "$3"
out=$(cd "$3" && pwd)
work="$root/target/engine-capi/package"
dir="$work/$name"

# Windows 的 Git Bash 里路径是 /d/a/...，交给 Python 之前换成 D:/a/... 的写法；cargo-c 写进 .pc 的也是这种写法。
native() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -m "$1"
  else
    printf '%s' "$1"
  fi
}

windows=false
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) windows=true ;;
esac
# Windows 上的 python3 可能只是应用商店的占位程序，那里先用 python。
if [[ "$windows" == true ]]; then
  python=${PYTHON:-$(command -v python || command -v python3)}
else
  python=${PYTHON:-$(command -v python3 || command -v python)}
fi

rm -rf "$dir"
mkdir -p "$dir"
for part in include lib bin; do
  if [[ -d "$installed/$part" ]]; then
    cp -RP "$installed/$part" "$dir/"
  fi
done
[[ -f "$dir/include/msime_engine.h" ]] || { echo "$installed has no include/msime_engine.h" >&2; exit 1; }

pc="$dir/lib/pkgconfig/msime-engine.pc"
[[ -f "$pc" ]] || { echo "$installed has no lib/pkgconfig/msime-engine.pc" >&2; exit 1; }
# shellcheck disable=SC2016 # `${pcfiledir}` 是给 pkg-config 展开的，不是 shell 变量。
sed -e 's|^prefix=.*$|prefix=${pcfiledir}/../..|' "$pc" > "$pc.relocated"
mv "$pc.relocated" "$pc"
for path in "$installed" "$(native "$installed")"; do
  if grep -qF "$path" "$pc"; then
    echo "msime-engine.pc still names the build directory $path:" >&2
    cat "$pc" >&2
    exit 1
  fi
done

if [[ "$(uname -s)" == Darwin ]]; then
  dylib="$dir/lib/libmsime_engine.1.1.0.dylib"
  install_name_tool -id @rpath/libmsime_engine.1.dylib "$dylib"
  # 改了 install name，原来的签名就失效了；arm64 上没有有效签名的 dylib 加载不了。
  codesign --force --sign - "$dylib"
fi

mkdir -p "$dir/examples"
cp -R "$root/crates/engine-capi/examples/c" "$dir/examples/c"
cp "$root/crates/engine-capi/README.md" "$dir/README.md"
cp "$root/crates/engine-capi/LICENSE" "$dir/LICENSE"
mkdir -p "$dir/licenses"
cp "$root/crates/engine/LICENSE" "$dir/licenses/msime-engine-LICENSE.txt"
cp "$root/crates/engine/licenses/"* "$dir/licenses/"
cp "$root/resources/licenses/ewts-MIT.txt" "$dir/licenses/ewts-MIT.txt"

# `-X utf8`：Windows 上 Python 默认按 cp1252 读 cargo 的输出，crate 元数据里的非 ASCII 字符会让它失败，与 c-example.sh 相同。
(cd "$root" && "$python" -X utf8 platforms/linux/collect-notices.py cargo "$(native "$dir/THIRD_PARTY_NOTICES.txt")" msime-engine-capi)

# 版本取 Cargo.lock 解析出的那一版，与链接进库的一致。
metadata="$work/metadata.json"
(cd "$root" && cargo metadata --locked --format-version 1 > "$(native "$metadata")")
versions=$("$python" -X utf8 - "$(native "$metadata")" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    packages = json.load(handle)["packages"]
wanted = ("msime-engine-capi", "msime-engine", "msime-path-trust", "rink-core")
found = {}
for package in packages:
    if package["name"] in wanted:
        found.setdefault(package["name"], set()).add(package["version"])
for crate in wanted:
    versions = found.get(crate, set())
    if len(versions) != 1:
        sys.exit(f"expected one version of {crate} in the dependency graph, found {sorted(versions)}")
print(" ".join(found[crate].pop() for crate in wanted))
PY
)
read -r capi engine path_trust rink <<< "$versions"
commit=$(git -C "$root" rev-parse HEAD)
# 变量一律加花括号：macOS 自带的 bash 3.2 会把紧跟在变量名后的中文字节当成名字的一部分。
cat > "$dir/SOURCE.txt" <<EOF
msime-engine-capi ${capi}（C ABI 版本见 include/msime_engine.h 的 MSIME_ENGINE_ABI_MAJOR、MSIME_ENGINE_ABI_MINOR）

这个库按 MPL-2.0 分发，对应的源码（MPL-2.0 第 3.2 条所说的 Source Code Form）是 metasequoiaime/msime 提交 ${commit} 的 crates/engine-capi、crates/engine 和 crates/path-trust：
https://github.com/metasequoiaime/msime/tree/${commit}

同样的源码也在 crates.io 上：
https://crates.io/crates/msime-engine-capi/${capi}
https://crates.io/crates/msime-engine/${engine}
https://crates.io/crates/msime-path-trust/${path_trust}

库里链接了未经修改的 rink-core ${rink}（MPL-2.0），它的源码在 https://crates.io/crates/rink-core/${rink} ，上游仓库是 https://codeberg.org/tiffany/rink 。其余第三方 crate 的许可证与声明见 THIRD_PARTY_NOTICES.txt，编进引擎的数据表与改编代码的许可证见 licenses/ 和 README.md 的「许可证」一节。

词库不在这个归档里，见 README.md 的「要带的数据文件」。
EOF

archive=
if [[ "$windows" == true ]]; then
  archive="$out/$name.zip"
  rm -f "$archive"
  (cd "$work" && "$python" -m zipfile -c "$(native "$archive")" "$name")
else
  archive="$out/$name.tar.gz"
  # COPYFILE_DISABLE：macOS 的 tar 不要把扩展属性写成 ._ 文件放进归档。
  (cd "$work" && COPYFILE_DISABLE=1 tar -czf "$archive" "$name")
fi
echo "$archive"
