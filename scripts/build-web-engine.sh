#!/usr/bin/env bash
# 构建网页内置输入法用的引擎包：msime-engine-wasm 编译成 wasm，经 wasm-bindgen 和 wasm-opt 处理，再加上裁剪后的词库、整句模型、NOTICE、清单和校验和，全部写到 target/web-engine/dist/；再把 packages/web-engine 的 SDK、这些文件和 gzip 过的辅助码表组装成 npm 包 target/web-engine/npm/msime-web-engine-<版本>.tgz。
#
# TapTapGo 按 web-engine-manifest.json 里每个文件的 sha256 和大小钉住 release（data/msime/web-engine.lock.json），所以同一提交、同一输入构建出来的文件必须逐字节相同：gzip 用 -n 去掉文件名和时间戳，词库由 `msime-dict-build web` 确定性地生成。
#
# 用法：
#   scripts/build-web-engine.sh --pinyin <msime-pinyin.db> --wubi <msime-wubi.db> --japanese <msime-japanese.dat> --model <sentence-model.safetensors> [--keep-multi N] [--keep-japanese N] [--version X.Y.Z]
#   scripts/build-web-engine.sh --no-data [--version X.Y.Z]     只构建 wasm、加载代码和 NOTICE（CI 用）
#
# --pinyin、--wubi 和 --japanese 是词库 release 的 msime-pinyin.db、msime-wubi.db 和 msime-japanese.dat，与 --keep-multi、--keep-japanese 一起透传给 `msime-dict-build web`；--keep-multi 是拼音库保留的多字词条数，默认 200000，--keep-japanese 是日语模型保留的词条数，默认 250000。--version 写进清单，默认取 msime-engine-wasm 的 crate 版本；release-web-engine.yml 传入要发布的版本号。
#
# 需要：Rust 的 wasm32-unknown-unknown 目标、能编译 wasm 的 LLVM clang 和 llvm-ar（Apple 的 ar 会产出空的 libwsqlite3.a）、wasm-bindgen 0.2.128（必须与 crates/engine-wasm 钉住的 wasm-bindgen crate 同版本）、binaryen 133 的 wasm-opt、jq、gzip，以及打 npm 包用的 Node.js 和 npm（生成内置皮肤表也用 Node.js）。并行度由 cargo 自己的 CARGO_BUILD_JOBS 控制。
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

WASM_BINDGEN_VERSION="0.2.128"
WASM_OPT_VERSION="133"
# 体积门槛：wasm 原始大小（spike 实测 4,577,187 B）和拼音库 gzip 后的大小。超出说明有东西意外进了包，先查清楚再放宽。
MAX_WASM_BYTES=5500000
MAX_PINYIN_GZ_BYTES=9000000
# 日语模型 gzip 后的大小：默认保留 250000 条时约 5.3 MB。
MAX_JAPANESE_GZ_BYTES=8000000

usage() {
  sed -n '6,8p' "$0" | sed 's/^# \{0,1\}//' >&2
  exit 2
}

no_data=0
pinyin=""
wubi=""
japanese=""
model=""
keep_multi=200000
keep_japanese=250000
version=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --no-data) no_data=1; shift ;;
    --pinyin) [ "$#" -ge 2 ] || usage; pinyin="$2"; shift 2 ;;
    --wubi) [ "$#" -ge 2 ] || usage; wubi="$2"; shift 2 ;;
    --japanese) [ "$#" -ge 2 ] || usage; japanese="$2"; shift 2 ;;
    --model) [ "$#" -ge 2 ] || usage; model="$2"; shift 2 ;;
    --keep-multi) [ "$#" -ge 2 ] || usage; keep_multi="$2"; shift 2 ;;
    --keep-japanese) [ "$#" -ge 2 ] || usage; keep_japanese="$2"; shift 2 ;;
    --version) [ "$#" -ge 2 ] || usage; version="$2"; shift 2 ;;
    -h|--help) usage ;;
    *) echo "build-web-engine: unknown option: $1" >&2; usage ;;
  esac
done

die() { echo "build-web-engine: $*" >&2; exit 1; }
step() { printf '\n=== %s ===\n' "$1"; }

if [ "$no_data" -eq 1 ]; then
  [ -z "$pinyin" ] && [ -z "$wubi" ] && [ -z "$japanese" ] && [ -z "$model" ] || die "--no-data cannot be combined with --pinyin, --wubi, --japanese or --model"
else
  [ -n "$pinyin" ] && [ -n "$wubi" ] && [ -n "$japanese" ] && [ -n "$model" ] || die "--pinyin, --wubi, --japanese and --model are required unless --no-data is given"
  [ -f "$pinyin" ] || die "pinyin dictionary not found: $pinyin"
  [ -f "$wubi" ] || die "wubi dictionary not found: $wubi"
  [ -f "$japanese" ] || die "japanese model not found: $japanese"
  [ -f "$model" ] || die "model not found: $model"
  [[ "$keep_multi" =~ ^[0-9]+$ ]] || die "--keep-multi must be a number, got '$keep_multi'"
  [[ "$keep_japanese" =~ ^[0-9]+$ ]] || die "--keep-japanese must be a number, got '$keep_japanese'"
fi

for tool in cargo jq gzip git node npm; do
  command -v "$tool" >/dev/null 2>&1 || die "$tool not found"
done
if command -v sha256sum >/dev/null 2>&1; then
  sha256_of() { sha256sum "$1" | cut -d' ' -f1; }
else
  sha256_of() { shasum -a 256 "$1" | cut -d' ' -f1; }
fi
size_of() { wc -c < "$1" | tr -d ' '; }

step "tools"
# 1. 工具版本。wasm-bindgen CLI 和 crate 版本不一致时生成的加载代码与 wasm 对不上，运行时才报错，所以这里先拦下。
command -v wasm-bindgen >/dev/null 2>&1 || die "wasm-bindgen not found (cargo install wasm-bindgen-cli --version $WASM_BINDGEN_VERSION --locked)"
command -v wasm-opt >/dev/null 2>&1 || die "wasm-opt not found (binaryen version_$WASM_OPT_VERSION)"
bindgen_version="$(wasm-bindgen --version)"
[ "$bindgen_version" = "wasm-bindgen $WASM_BINDGEN_VERSION" ] || die "need wasm-bindgen $WASM_BINDGEN_VERSION, found '$bindgen_version'"
opt_version="$(wasm-opt --version)"
case "$opt_version" in
  "wasm-opt version $WASM_OPT_VERSION"|"wasm-opt version $WASM_OPT_VERSION "*) ;;
  *) die "need wasm-opt version $WASM_OPT_VERSION, found '$opt_version'" ;;
esac
echo "$bindgen_version; $opt_version"

# LLVM：已经设置了 CC_/AR_wasm32_unknown_unknown 就用它们；否则先找 Homebrew 的 llvm，再找 PATH 上的 clang 和 llvm-ar。候选的 clang 必须真的能编 wasm32（Apple clang 不能）。
can_target_wasm() {
  printf 'int x;\n' | "$1" --target=wasm32-unknown-unknown -x c -c -o /dev/null - >/dev/null 2>&1
}
cc="${CC_wasm32_unknown_unknown:-}"
ar="${AR_wasm32_unknown_unknown:-}"
if [ -z "$cc" ] || [ -z "$ar" ]; then
  for prefix in /opt/homebrew/opt/llvm/bin /usr/local/opt/llvm/bin; do
    if [ -x "$prefix/clang" ] && [ -x "$prefix/llvm-ar" ]; then
      cc="${cc:-$prefix/clang}"
      ar="${ar:-$prefix/llvm-ar}"
      break
    fi
  done
fi
if [ -z "$cc" ]; then cc="$(command -v clang || true)"; fi
if [ -z "$ar" ]; then ar="$(command -v llvm-ar || true)"; fi
[ -n "$cc" ] && [ -n "$ar" ] || die "LLVM clang and llvm-ar not found (brew install llvm, or apt-get install clang llvm)"
can_target_wasm "$cc" || die "$cc cannot compile for wasm32-unknown-unknown; point CC_wasm32_unknown_unknown at an LLVM clang"
export CC_wasm32_unknown_unknown="$cc" AR_wasm32_unknown_unknown="$ar"
echo "CC_wasm32_unknown_unknown=$cc"
echo "AR_wasm32_unknown_unknown=$ar"

source_commit="$(git rev-parse HEAD)"
short_commit="$(git rev-parse --short HEAD)"
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "warning: the working tree has uncommitted changes; the manifest still names $source_commit"
fi
if [ -z "$version" ]; then
  version="$(cargo metadata --locked --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "msime-engine-wasm") | .version')"
fi
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]] || die "version must be X.Y.Z, got '$version'"

out="target/web-engine"
pkg="$out/pkg"
dist="$out/dist"
rm -rf "$pkg" "$dist"
mkdir -p "$pkg" "$dist"

step "cargo build (wasm-release)"
# 2. build_info() 从 MSIME_GIT_SHA 读提交号。
MSIME_GIT_SHA="$short_commit" cargo build --locked -p msime-engine-wasm --target wasm32-unknown-unknown --profile wasm-release
raw_wasm="target/wasm32-unknown-unknown/wasm-release/msime_engine_wasm.wasm"
[ -f "$raw_wasm" ] || die "$raw_wasm was not produced"

step "wasm-bindgen"
# 3. --omit-default-module-path 去掉加载代码里默认的 new URL('msime_engine_bg.wasm', import.meta.url)：TapTapGo 总是显式传入 wasm 的地址，留着它 Vite 会去解析一个不存在的文件。
wasm-bindgen --target web --omit-default-module-path --no-typescript --out-name msime_engine --out-dir "$pkg" "$raw_wasm"

step "wasm-opt"
# 4. 启用的特性与 wasm-bindgen 0.2.128 和 Rust 默认 wasm32 目标产出的指令一致。
wasm-opt -Oz --enable-bulk-memory --enable-sign-ext --enable-nontrapping-float-to-int \
  --enable-mutable-globals --enable-reference-types --enable-multivalue \
  -o "$dist/msime_engine_bg.wasm" "$pkg/msime_engine_bg.wasm"
cp "$pkg/msime_engine.js" "$dist/msime_engine.js"

if [ "$no_data" -eq 0 ]; then
  step "web dictionaries"
  # 5. 裁出拼音库、五笔 86 库和日语模型，再把它们和整句模型 gzip -9n（不记文件名和时间戳，结果可复现）。
  dict_dir="$out/dict"
  mkdir -p "$dict_dir"
  cargo run --locked --release -p msime-dict-builder --bin msime-dict-build -- \
    web --pinyin "$pinyin" --wubi "$wubi" --out-dir "$dict_dir" --keep-multi "$keep_multi" \
    --japanese "$japanese" --keep-japanese "$keep_japanese"
  gzip -9n -c "$dict_dir/msime-pinyin.db" > "$dist/msime-pinyin.db.gz"
  gzip -9n -c "$dict_dir/msime-wubi86.db" > "$dist/msime-wubi86.db.gz"
  gzip -9n -c "$dict_dir/msime-japanese.dat" > "$dist/msime-japanese.dat.gz"
  gzip -9n -c "$model" > "$dist/sentence-model.safetensors.gz"
fi

step "NOTICE"
# 6. NOTICE 必须覆盖每个链接进 wasm 的 crate；发布的那份把源码链接里的占位符换成本次构建的完整提交号。
scripts/web-engine-notice.sh --check
notice_source="resources/licenses/web-engine-NOTICE.md"
grep -q '{{source_commit}}' "$notice_source" || die "$notice_source has no {{source_commit}} placeholder for the source link"
sed "s/{{source_commit}}/$source_commit/g" "$notice_source" > "$dist/NOTICE.md"

step "manifest and checksums"
# 7. 清单的 artifacts 由 TapTapGo 原样抄进它的锁文件。size 是发布出去的文件大小，raw_size 是 gunzip 之后的大小（不是 gzip 的文件两者相同）。
artifacts="$out/artifacts.jsonl"
: > "$artifacts"
add_artifact() {
  local name="$1" role="$2" file="$dist/$1" raw
  [ -f "$file" ] || die "$file is missing"
  case "$name" in
    *.gz) raw="$(gzip -dc "$file" | wc -c | tr -d ' ')" ;;
    *) raw="$(size_of "$file")" ;;
  esac
  jq -cn --arg name "$name" --arg role "$role" --arg sha256 "$(sha256_of "$file")" \
    --argjson size "$(size_of "$file")" --argjson raw "$raw" \
    '{name: $name, role: $role, sha256: $sha256, size: $size, raw_size: $raw}' >> "$artifacts"
}
add_artifact msime_engine_bg.wasm wasm
add_artifact msime_engine.js glue
if [ "$no_data" -eq 0 ]; then
  add_artifact msime-pinyin.db.gz pinyin
  add_artifact msime-wubi86.db.gz wubi86
  add_artifact msime-japanese.dat.gz japanese
  add_artifact sentence-model.safetensors.gz model
  keep_multi_json="$keep_multi"
  keep_japanese_json="$keep_japanese"
else
  keep_multi_json=null
  keep_japanese_json=null
fi
add_artifact NOTICE.md notice
jq -s --arg version "$version" --arg commit "$source_commit" --argjson keep "$keep_multi_json" --argjson keep_japanese "$keep_japanese_json" \
  '{version: $version, source_commit: $commit, keep_multi: $keep, keep_japanese: $keep_japanese, artifacts: .}' "$artifacts" > "$dist/web-engine-manifest.json"
rm -f "$artifacts"
(
  cd "$dist"
  export LC_ALL=C
  for file in *; do
    printf '%s  %s\n' "$(sha256_of "$file")" "$file"
  done
) > "$out/SHA256SUMS.txt"
mv "$out/SHA256SUMS.txt" "$dist/SHA256SUMS.txt"

step "size gates"
# 8. 体积门槛。
wasm_bytes="$(size_of "$dist/msime_engine_bg.wasm")"
[ "$wasm_bytes" -le "$MAX_WASM_BYTES" ] || die "msime_engine_bg.wasm is $wasm_bytes bytes, over the $MAX_WASM_BYTES byte gate"
echo "msime_engine_bg.wasm: $wasm_bytes bytes (gate $MAX_WASM_BYTES)"
if [ "$no_data" -eq 0 ]; then
  pinyin_bytes="$(size_of "$dist/msime-pinyin.db.gz")"
  [ "$pinyin_bytes" -le "$MAX_PINYIN_GZ_BYTES" ] || die "msime-pinyin.db.gz is $pinyin_bytes bytes, over the $MAX_PINYIN_GZ_BYTES byte gate"
  echo "msime-pinyin.db.gz: $pinyin_bytes bytes (gate $MAX_PINYIN_GZ_BYTES)"
  japanese_bytes="$(size_of "$dist/msime-japanese.dat.gz")"
  [ "$japanese_bytes" -le "$MAX_JAPANESE_GZ_BYTES" ] || die "msime-japanese.dat.gz is $japanese_bytes bytes, over the $MAX_JAPANESE_GZ_BYTES byte gate"
  echo "msime-japanese.dat.gz: $japanese_bytes bytes (gate $MAX_JAPANESE_GZ_BYTES)"
fi

step "npm package"
# 9. SDK（packages/web-engine）和这次构建的加载代码、资源组装成 @msime/web-engine，打成 target/web-engine/npm/msime-web-engine-<版本>.tgz。它不进 dist/：dist 的清单和校验和是 TapTapGo 钉住的，多一个文件就会改变它们。assets.js 从清单生成，SDK 靠它知道每个资源的名字和大小，所以 SDK 与 wasm、词库永远是同一次构建。
npm_dir="$out/npm"
npm_pkg="$npm_dir/package"
rm -rf "$npm_dir"
mkdir -p "$npm_pkg/assets" "$npm_pkg/bin"
sdk="packages/web-engine"
cp "$sdk/src/index.js" "$sdk/src/index.d.ts" "$sdk/src/keys.js" "$sdk/src/input.js" "$sdk/src/skin.js" "$sdk/src/candidates.js" "$sdk/src/candidates.d.ts" "$sdk/src/worker.js" "$sdk/README.md" "$npm_pkg/"
# skin.js 导入的内置皮肤表从 packages/ui/src/theme/theme-catalog.json 和 crates/client-core/src/skin/catalog/windows_looks.rs 生成，SDK 里没有手写的配色副本；来源的格式变了生成器会报错，构建随之失败。
node "$sdk/tools/theme-catalog.mjs" "$npm_pkg/theme-catalog.js"
cp "$sdk/bin/msime-web-engine.mjs" "$npm_pkg/bin/"
cp LICENSE "$npm_pkg/LICENSE"
cp "$dist/msime_engine.js" "$npm_pkg/"
# 加载代码在包的根目录（worker.js 旁边）；SHA256SUMS.txt 按 dist 的目录结构列文件，放进 assets/ 就对不上了，包里的校验信息以 web-engine-manifest.json 为准。
for file in "$dist"/*; do
  case "$(basename "$file")" in
    msime_engine.js | SHA256SUMS.txt) ;;
    *) cp "$file" "$npm_pkg/assets/" ;;
  esac
done
jq --arg version "$version" '.version = $version' "$sdk/package.json" > "$npm_pkg/package.json"
# 辅助码表（全拼和双拼的辅助码，SDK 只在打开辅助码时下载）只进 npm 包，不进 dist：dist 的清单和校验和是 TapTapGo 钉住、release 逐个上传的那一套，release-web-engine.yml 也按角色核对清单，而辅助码只有 SDK 用得到。方案名和文件与引擎的 `assets::HELPCODES`（crates/engine/src/assets.rs）一一对应，tests/helpcodes.rs 核对这一点；和词库一样 gzip -9n，结果可复现。辅助码表的来源和分发限制写在 NOTICE 里。
helpcodes_jsonl="$npm_dir/helpcodes.jsonl"
: > "$helpcodes_jsonl"
for entry in lantian:helpcodes/helpcode.txt ziranma:helpcodes/zrm_helpcode_big_unique.txt shouyou2_0:helpcodes/shouyou2_0_helpcode.txt \
  shouyouplus:helpcodes/shouyouplus_helpcode.txt xiaohe:helpcodes/xiaohe_helpcode.txt jiajia:helpcodes/jiajia_helpcode.txt; do
  schema="${entry%%:*}"
  table="resources/${entry#*:}"
  [ -f "$table" ] || die "$table is missing"
  name="helpcode-$schema.txt.gz"
  gzip -9n -c "$table" > "$npm_pkg/assets/$name"
  jq -cn --arg schema "$schema" --arg name "$name" --argjson size "$(size_of "$npm_pkg/assets/$name")" --argjson raw "$(size_of "$table")" \
    '{key: $schema, value: {name: $name, size: $size, rawSize: $raw}}' >> "$helpcodes_jsonl"
done
{
  echo "// 由 scripts/build-web-engine.sh 从 web-engine-manifest.json 和辅助码表生成，不要手改。"
  jq -r '"export const version = \(.version | tojson);",
    "export const sourceCommit = \(.source_commit | tojson);",
    "export const files = \([.artifacts[] | select(.role != "glue" and .role != "notice") | {key: .role, value: {name, size, rawSize: .raw_size}}] | from_entries | tojson);"' \
    "$dist/web-engine-manifest.json"
  jq -rs '"export const helpcodes = \(from_entries | tojson);"' "$helpcodes_jsonl"
} > "$npm_pkg/assets.js"
rm -f "$helpcodes_jsonl"
npm pack --silent --pack-destination "$npm_dir" "$npm_pkg" > /dev/null
npm_tgz="$npm_dir/msime-web-engine-$version.tgz"
[ -f "$npm_tgz" ] || die "npm pack did not produce $npm_tgz"
echo "$npm_tgz: $(size_of "$npm_tgz") bytes"

step "done"
jq -r '.artifacts[] | "\(.name)\t\(.size)\t\(.raw_size)\t\(.sha256)"' "$dist/web-engine-manifest.json" |
  awk -F '\t' '{ printf "  %-32s %10d B  (raw %10d B)  %s\n", $1, $2, $3, $4 }'
for file in "$npm_pkg"/assets/helpcode-*.txt.gz; do
  printf '  %-32s %10d B  (npm package only)\n' "$(basename "$file")" "$(size_of "$file")"
done
echo "web engine $version ($short_commit) written to $dist"
echo "npm package @msime/web-engine $version written to $npm_tgz"
