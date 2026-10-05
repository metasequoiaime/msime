#!/usr/bin/env bash
# 生成各发行版源码包离线构建所需的 tarball，随 linux-vVERSION 发布一起上传：Fedora COPR、openSUSE OBS 的 RPM 与 Launchpad PPA 的 Debian 源码包用前两个，AUR 的 msime 与 Gentoo 的版本 ebuild 用源码 tarball 和前端 tarball。
#
# 这些构建农场在构建时都没有网络，而完整版要的东西有一半不在仓库里：Cargo 依赖、由 pnpm 构建并嵌进设置窗口的前端，以及 package-container.sh 构建时下载的语音运行库、手写模型、离线释义和方言词库。这里把它们一次取齐：
#
#   msime-VERSION.tar.xz         当前提交的 `git archive`，顶层目录 msime-VERSION/。
#   msime-VERSION-vendor.tar.xz  顶层目录 msime-VERSION-vendor/，内容：
#     cargo/                     `cargo vendor --locked` 的输出，含 Cargo.lock 里的 git 依赖。
#     cargo-config.toml          `cargo vendor` 打印的源替换配置；构建时把其中的 directory 改写成 cargo/ 的绝对路径。
#     frontend/dist/             `pnpm --filter @msime/desktop build` 的产物，Tauri 编译时嵌进 msime-desktop。
#     frontend/frontend-npm-NOTICES.txt  collect-notices.py npm 的输出，构建时没有 node_modules 可走。
#     voice-runtime/linux-x86_64/.archive/、voice-runtime/linux-aarch64/.archive/  按 resources/voice-runtime.lock.json 下载的原始归档。
#     handwriting-model/、offline-glosses/、language-dictionaries/  各自 fetch 脚本按锁文件取回的文件。
#   msime-VERSION-frontend.tar.xz  顶层目录 msime-VERSION-frontend/，只有上面 frontend/ 的内容（dist/ 与 frontend-npm-NOTICES.txt）。Gentoo 的 crate 逐个列在 SRC_URI 里、资源按锁文件地址下载，不需要整个 vendor 包，但 pnpm 依赖没法逐个列出，前端只能取这份构建好的。
#
# 数据一律经仓库自己的 fetch 脚本取得，构建时再用同一批脚本对着源码树里的锁文件跑一遍：文件已在锁定的摘要上就不联网，对不上就去下载，而构建农场没有网络，于是 vendor 包与源码不一致时构建直接失败。这样哈希只记在锁文件一处，规格文件和 debian/ 里都不另抄一份。语音运行库只留下原始归档，解出的库由构建时的那次 fetch 从归档重新解出。
#
# 用法：platforms/linux/packaging/make-source-tarballs.sh VERSION OUTDIR
#   需要网络，以及 git、cargo、corepack（pnpm）、python3、GNU tar 和 xz。在仓库检出里运行，打包的是 HEAD 已提交的内容。
#   最后打印三个文件的 SHA-256。
set -euo pipefail

if [ $# -ne 2 ]; then
  echo "usage: $0 VERSION OUTDIR" >&2
  exit 2
fi
version=$1
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  echo "version must be MAJOR.MINOR.PATCH: $version" >&2
  exit 2
}
repo_root=$(cd "$(dirname "$0")/../../.." && pwd)
mkdir -p "$2"
out=$(cd "$2" && pwd)
cd "$repo_root"

for tool in git cargo corepack python3 tar xz sha256sum; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "$tool is required" >&2
    exit 2
  }
done
tar --version 2>/dev/null | grep -q 'GNU tar' || {
  echo "GNU tar is required (for --sort=name)" >&2
  exit 2
}

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
name="msime-$version"
vendor="$stage/$name-vendor"
mkdir -p "$vendor/frontend" "$vendor/voice-runtime"
# 两个 tarball 里的文件时间都取 HEAD 的提交时间，同一提交重复生成得到相同内容。
epoch=$(git log -1 --format=%ct HEAD)

git archive --format=tar --prefix="$name/" HEAD | xz -T0 -9 > "$out/$name.tar.xz"

# 在仓库根目录调用，Cargo 才会按 rust-toolchain.toml 选编译器。打印出的配置里 directory 是这台机器上的绝对路径，换成相对的 cargo，构建时再改写成解包后的位置。
cargo vendor --locked --versioned-dirs "$vendor/cargo" | sed 's|^directory = .*|directory = "cargo"|' > "$vendor/cargo-config.toml"
grep -q '^directory = "cargo"$' "$vendor/cargo-config.toml"

corepack enable >/dev/null 2>&1 || true
pnpm install --frozen-lockfile
pnpm --filter @msime/desktop build
cp -a apps/desktop/dist "$vendor/frontend/dist"
python3 platforms/linux/collect-notices.py npm "$vendor/frontend/frontend-npm-NOTICES.txt" apps/desktop

for platform in linux-x86_64 linux-aarch64; do
  python3 scripts/fetch_voice_runtime.py --platform "$platform" --out "$vendor/voice-runtime/$platform" >/dev/null
  # 只留原始归档：构建时的 fetch 从它解出库，解出的副本放在这里只是多一份体积。
  find "$vendor/voice-runtime/$platform" -mindepth 1 -maxdepth 1 ! -name .archive -exec rm -rf {} +
done
python3 scripts/fetch_handwriting_model.py --out "$vendor/handwriting-model" >/dev/null
python3 scripts/fetch_offline_glosses.py --out "$vendor/offline-glosses" >/dev/null
python3 scripts/fetch_language_dictionaries.py --out "$vendor/language-dictionaries" >/dev/null

tar --sort=name --mtime="@$epoch" --owner=0 --group=0 --numeric-owner \
  -C "$stage" -cf - "$name-vendor" | xz -T0 -9 > "$out/$name-vendor.tar.xz"

mkdir -p "$stage/$name-frontend"
cp -a "$vendor/frontend/." "$stage/$name-frontend/"
tar --sort=name --mtime="@$epoch" --owner=0 --group=0 --numeric-owner \
  -C "$stage" -cf - "$name-frontend" | xz -T0 -9 > "$out/$name-frontend.tar.xz"

(cd "$out" && sha256sum -- "$name.tar.xz" "$name-vendor.tar.xz" "$name-frontend.tar.xz")
