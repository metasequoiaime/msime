#!/usr/bin/env bash
# 生成 resources/licenses/web-engine-NOTICE.md 里两段由脚本维护的内容，网页引擎（msime-engine-wasm）的 release 把这份文件改名为 NOTICE.md 一起发布。
#
# - `repo` 段：仓库里随 wasm 分发的许可证全文，逐字复制自 LICENSE 和 resources/licenses/（GPL-3.0、libhangul 汉字表、行政区划数据、Zinnia、日语模型的 Mozc 与 IPAdic 条款），以及 npm 包所带辅助码表的来源说明（resources/helpcodes/ 的两份 NOTICE）。
# - `crates` 段：链接进 wasm 的每个 crate、它声明的许可证表达式，以及它随包附带的许可证文件全文（内容相同的文件只收一份）。crate 集合来自 `cargo tree -p msime-engine-wasm --target wasm32-unknown-unknown -e normal,no-proc-macro`：过程宏及其依赖只在编译期运行，不进 wasm，所以不列；本 workspace 的成员是本项目自己的 GPL-3.0 代码，许可证全文在 `repo` 段。
#
# 做法和 platforms/linux/collect-notices.py、Windows 的 Collect-Notices.ps1 相同：许可证文件原样复制，没有附带许可证文件的 crate 单独列出它声明的表达式，缺口看得见而不是被悄悄丢掉。两段以外的手写内容不动。
#
# 用法：
#   scripts/web-engine-notice.sh           重新生成并写回 NOTICE
#   scripts/web-engine-notice.sh --check   重新生成到临时文件，与仓库里的 NOTICE 有任何不同就失败（依赖变了却没重新生成）
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
notice="resources/licenses/web-engine-NOTICE.md"

check=0
for argument in "$@"; do
  case "$argument" in
    --check) check=1 ;;
    *) echo "unknown option: $argument" >&2; exit 2 ;;
  esac
done

for tool in cargo jq awk; do
  command -v "$tool" >/dev/null 2>&1 || { echo "web-engine-notice: $tool not found" >&2; exit 2; }
done
[ -f "$notice" ] || { echo "web-engine-notice: $notice is missing" >&2; exit 2; }

if command -v sha256sum >/dev/null 2>&1; then
  hash_file() { sha256sum "$1" | cut -d' ' -f1; }
else
  hash_file() { shasum -a 256 "$1" | cut -d' ' -f1; }
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
export LC_ALL=C

# 许可证文本统一成 LF、去掉行尾空白和末尾空行，这样不同 crate 里只差换行符的同一份文本合并成一份，生成结果也不随签出方式变化。
normalize() {
  tr -d '\r' < "$1" | sed -e 's/[[:space:]]*$//' | awk '{ lines[NR] = $0 } END { last = NR; while (last > 0 && lines[last] == "") last--; for (i = 1; i <= last; i++) print lines[i] }'
}

# 用四个反引号围栏：许可证正文里偶尔会出现三个反引号。
fence_open='````text'
fence_close='````'

# ---- repo 段 ----
repo_section="$work/repo.md"
{
  echo "<!-- web-engine-notice:repo:begin -->"
  echo "<!-- 本段由 scripts/web-engine-notice.sh 生成，不要手改。 -->"
  for entry in \
    "LICENSE|GPL-3.0-only：引擎本身（msime-engine、msime-engine-wasm 等本项目 crate）" \
    "resources/licenses/libhangul-hanja-BSD-3-Clause.txt|BSD-3-Clause：libhangul 汉字表（crates/engine/src/korean/hanja.tsv）" \
    "resources/licenses/Administrative-divisions-of-China-WTFPL.txt|WTFPL：中国行政区划数据（crates/engine/src/local/places.tsv）" \
    "resources/licenses/Zinnia-LICENSE.txt|BSD-3-Clause：Zinnia（手写识别模块的 Rust 移植）" \
    "resources/licenses/mozc-BSD-3-Clause.txt|BSD-3-Clause：Mozc（日语模型 msime-japanese.dat 的来源，词库 release 的 msime-mozc_LICENSE.txt）" \
    "resources/licenses/mozc-dictionary_oss-README.txt|IPAdic、ICOT 与冲绳词典的条款（日语模型 msime-japanese.dat，词库 release 的 msime-mozc_dictionary_oss_README.txt）" \
    "resources/helpcodes/ENGINE-NOTICE.md|辅助码表 helpcode-lantian、helpcode-ziranma、helpcode-shouyou2_0、helpcode-shouyouplus、helpcode-xiaohe（只在 npm 包里）的来源与权利说明" \
    "resources/helpcodes/NOTICE.md|辅助码表 helpcode-jiajia（只在 npm 包里）的来源、权利与分发限制" \
    "resources/helpcodes/NOTICE-wubi86.md|辅助码表 helpcode-wubi86（只在 npm 包里）的来源、许可与生成方法"; do
    path="${entry%%|*}"
    title="${entry#*|}"
    [ -f "$path" ] || { echo "web-engine-notice: $path is missing" >&2; exit 1; }
    echo
    echo "### \`$path\`"
    echo
    echo "$title"
    echo
    echo "$fence_open"
    normalize "$path"
    echo "$fence_close"
  done
  echo
  echo "<!-- web-engine-notice:repo:end -->"
} > "$repo_section"

# ---- crates 段 ----
cargo tree --locked -p msime-engine-wasm --target wasm32-unknown-unknown -e normal,no-proc-macro \
  --prefix none --format '{p}' > "$work/tree.txt"
# `{p}` 是 `name vX.Y.Z` 后面可能跟来源和 `(*)`；只取前两个字段。
awk 'NF >= 2 && substr($2, 1, 1) == "v" { print $1 "\t" substr($2, 2) }' "$work/tree.txt" | sort -u > "$work/crates.tsv"
[ -s "$work/crates.tsv" ] || { echo "web-engine-notice: cargo tree listed no crates" >&2; exit 1; }

cargo metadata --locked --format-version 1 --filter-platform wasm32-unknown-unknown > "$work/metadata.json"
# name、version、许可证表达式、license-file、crate 目录、是否本 workspace 成员。
jq -r '(.workspace_members) as $members | .packages[]
  | [.name, .version, (.license // ""), (.license_file // ""), (.manifest_path | sub("/Cargo.toml$"; "")),
     (if (.id as $id | $members | index($id)) then "member" else "external" end)] | @tsv' \
  "$work/metadata.json" | sort -u > "$work/packages.tsv"

# crate 自己没有附带许可证文件、而仓库里有对应全文的，用仓库的那一份（docs/third-party.md 记录了这两份的来历）。
fallback_license() {
  case "$1" in
    ewts) echo "resources/licenses/ewts-MIT.txt" ;;
    rink-core) echo "resources/licenses/MPL-2.0.txt" ;;
    *) echo "" ;;
  esac
}

: > "$work/rows.tsv"
: > "$work/files.tsv"
: > "$work/missing.tsv"
mkdir -p "$work/texts"
while IFS="$(printf '\t')" read -r name version; do
  line="$(awk -F '\t' -v n="$name" -v v="$version" '$1 == n && $2 == v { print; exit }' "$work/packages.tsv")"
  [ -n "$line" ] || { echo "web-engine-notice: $name $version is not in cargo metadata" >&2; exit 1; }
  license="$(printf '%s' "$line" | cut -f3)"
  license_file="$(printf '%s' "$line" | cut -f4)"
  directory="$(printf '%s' "$line" | cut -f5)"
  kind="$(printf '%s' "$line" | cut -f6)"
  if [ "$kind" = member ]; then
    printf '%s\t%s\t%s\t%s\n' "$name" "$version" "${license:-GPL-3.0-only}" "本项目，见上文 \`LICENSE\`" >> "$work/rows.tsv"
    continue
  fi
  [ -n "$license" ] || license="见 license-file"
  : > "$work/paths"
  find "$directory" -maxdepth 1 -type f | while IFS= read -r path; do
    base="$(basename "$path")"
    if printf '%s\n' "$base" | grep -qiE '^(licen[cs]e|copying|copyright|notice|unlicense)([-._].*)?$'; then
      echo "$path"
    fi
  done | sort >> "$work/paths"
  if [ -n "$license_file" ] && [ -f "$directory/$license_file" ] && ! grep -qxF "$directory/$license_file" "$work/paths"; then
    echo "$directory/$license_file" >> "$work/paths"
  fi
  if [ ! -s "$work/paths" ]; then
    fallback="$(fallback_license "$name")"
    [ -n "$fallback" ] && echo "$root/$fallback" >> "$work/paths"
  fi
  if [ ! -s "$work/paths" ]; then
    printf '%s\t%s\t%s\n' "$name" "$version" "$license" >> "$work/missing.tsv"
    printf '%s\t%s\t%s\t%s\n' "$name" "$version" "$license" "未附带" >> "$work/rows.tsv"
    continue
  fi
  while IFS= read -r path; do
    normalize "$path" > "$work/text"
    digest="$(hash_file "$work/text")"
    [ -f "$work/texts/$digest" ] || cp "$work/text" "$work/texts/$digest"
    printf '%s\t%s\t%s\t%s\n' "$name" "$version" "$(basename "$path")" "$digest" >> "$work/files.tsv"
  done < "$work/paths"
  printf '%s\t%s\t%s\t%s\n' "$name" "$version" "$license" "-" >> "$work/rows.tsv"
done < "$work/crates.tsv"

# 按首次出现的顺序给每份不同的文本编号 L01、L02……
awk -F '\t' '!($4 in id) { id[$4] = sprintf("L%02d", ++n); print $4 "\t" id[$4] "\t" $3 }' "$work/files.tsv" > "$work/ids.tsv"

crates_section="$work/crates.md"
{
  echo "<!-- web-engine-notice:crates:begin -->"
  echo "<!-- 本段由 scripts/web-engine-notice.sh 生成，不要手改。 -->"
  echo
  echo "| crate | 版本 | 声明的许可证 | 许可证文本 |"
  echo "| --- | --- | --- | --- |"
  while IFS="$(printf '\t')" read -r name version license texts; do
    if [ "$texts" = "-" ]; then
      texts="$(awk -F '\t' -v n="$name" -v v="$version" 'NR == FNR { id[$1] = $2; next } $1 == n && $2 == v { out = out (out == "" ? "" : "、") id[$4] } END { print out }' "$work/ids.tsv" "$work/files.tsv")"
    fi
    # shellcheck disable=SC2016  # 反引号是 Markdown 的行内代码，不是命令替换
    printf '| `%s` | %s | %s | %s |\n' "$name" "$version" "$license" "$texts"
  done < "$work/rows.tsv"
  if [ -s "$work/missing.tsv" ]; then
    echo
    echo "下列 crate 的发布包里没有许可证文件，按它们声明的许可证表达式使用；对应许可证的标准条款见下面同名许可证的文本："
    echo
    while IFS="$(printf '\t')" read -r name version license; do
      # shellcheck disable=SC2016  # 同上
      printf -- '- `%s` %s（%s）\n' "$name" "$version" "$license"
    done < "$work/missing.tsv"
  fi
  while IFS="$(printf '\t')" read -r digest id file; do
    users="$(awk -F '\t' -v d="$digest" '$4 == d { key = "`" $1 "` " $2; if (!(key in seen)) { seen[key] = 1; out = out (out == "" ? "" : "、") key } } END { print out }' "$work/files.tsv")"
    echo
    echo "### ${id}（${file}）"
    echo
    echo "用于：$users"
    echo
    echo "$fence_open"
    cat "$work/texts/$digest"
    echo "$fence_close"
  done < "$work/ids.tsv"
  echo
  echo "<!-- web-engine-notice:crates:end -->"
} > "$crates_section"

# 把两段拼回 NOTICE：标记之间的内容整段替换，标记之外原样保留。缺标记或标记成对不全就失败。
for marker in repo crates; do
  begins="$(grep -c "^<!-- web-engine-notice:$marker:begin -->\$" "$notice" || true)"
  ends="$(grep -c "^<!-- web-engine-notice:$marker:end -->\$" "$notice" || true)"
  if [ "$begins" != 1 ] || [ "$ends" != 1 ]; then
    echo "web-engine-notice: $notice must contain exactly one $marker begin and end marker" >&2
    exit 1
  fi
done
awk -v repo="$repo_section" -v crates="$crates_section" '
  /^<!-- web-engine-notice:(repo|crates):begin -->$/ {
    file = ($0 ~ /:repo:/) ? repo : crates
    while ((getline line < file) > 0) print line
    close(file)
    skipping = 1
    next
  }
  /^<!-- web-engine-notice:(repo|crates):end -->$/ { skipping = 0; next }
  !skipping { print }
' "$notice" > "$work/notice.md"

if [ "$check" -eq 1 ]; then
  if ! cmp -s "$work/notice.md" "$notice"; then
    diff -u "$notice" "$work/notice.md" | head -40 >&2 || true
    echo "web-engine-notice: $notice is out of date; run scripts/web-engine-notice.sh and commit the result" >&2
    exit 1
  fi
  echo "web-engine-notice: $notice is up to date ($(wc -l < "$work/crates.tsv" | tr -d ' ') crates)"
else
  cp "$work/notice.md" "$notice"
  echo "web-engine-notice: wrote $notice ($(wc -l < "$work/crates.tsv" | tr -d ' ') crates)"
fi
