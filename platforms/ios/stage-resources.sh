#!/usr/bin/env bash
set -euo pipefail
umask 077

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
source_dir=${1:?usage: stage-resources.sh <verified-resource-directory> [offline-glosses-directory] [language-dictionaries-directory] [wordbooks-directory]}
source_dir=$(cd "$source_dir" && pwd)
destination="$repo_root/target/ios/EngineResources"
# 发布用瘦包只省略附加语言资源，日文独立版本的主词库不能省。
omit_on_demand=${MSIME_IOS_OMIT_ON_DEMAND:-0}
verify_flags=()
case "$omit_on_demand" in
  0) ;;
  1)
    case "${MSIME_EDITION:-full}" in
      full|pinyin) verify_flags=(--omit-on-demand) ;;
      *) echo "MSIME_IOS_OMIT_ON_DEMAND only supports full and pinyin" >&2; exit 2 ;;
    esac ;;
  *) echo "MSIME_IOS_OMIT_ON_DEMAND must be 0 or 1" >&2; exit 2 ;;
esac
artifacts=$(cargo run --quiet -p msime-client-core --example verify_resources --locked -- ${verify_flags[@]+"${verify_flags[@]}"} "$source_dir")
# Rebuilt as one unit, as on macOS: a directory staged for an earlier lock keeps files the current lock no longer names, and the verifier below refuses them.
rm -rf "$destination"
mkdir -p "$destination"
while IFS= read -r artifact; do
  cp "$source_dir/$artifact" "$destination/$artifact"
done <<< "$artifacts"
cargo run --quiet -p msime-client-core --example verify_resources --locked -- ${verify_flags[@]+"${verify_flags[@]}"} "$destination" >/dev/null
# 辅助码表不在词库发布里，由仓库自带在 resources/helpcodes，Engine 从资源目录下的 helpcodes/ 读它们（crates/engine/src/assets.rs 列出七个文件）。没有它们 Engine 就没有可匹配的码：Shift 字母被当作辅助码，却什么也筛不掉。
helpcodes="$repo_root/resources/helpcodes"
rm -rf "$destination/helpcodes"
mkdir -p "$destination/helpcodes"
for table in helpcode.txt zrm_helpcode_big_unique.txt shouyou2_0_helpcode.txt shouyouplus_helpcode.txt xiaohe_helpcode.txt jiajia_helpcode.txt wubi86_helpcode.txt; do
  cp "$helpcodes/$table" "$destination/helpcodes/$table"
done
cp "$helpcodes/ENGINE-NOTICE.md" "$destination/helpcodes/NOTICE.md"
cp "$helpcodes/NOTICE.md" "$destination/helpcodes/NOTICE-jiajia.md"
cp "$helpcodes/NOTICE-wubi86.md" "$destination/helpcodes/NOTICE-wubi86.md"
# Optional: non-English candidate glosses built by scripts/build_offline_glosses.py, bundled beside EngineResources because host-api looks for them next to the resource directory. The directory is always created, empty when there are none, since the keyboard target bundles it as a folder; without the databases only English is glossed offline.
glosses_source=${2:-$repo_root/target/offline-glosses}
glosses_destination="$repo_root/target/ios/offline-glosses"
rm -rf "$glosses_destination"
mkdir -p "$glosses_destination"
if compgen -G "$glosses_source/zh-*.db" >/dev/null && [ -f "$glosses_source/offline-glosses-NOTICE.txt" ]; then
  cp "$glosses_source"/zh-*.db "$glosses_source/offline-glosses-NOTICE.txt" "$glosses_destination/"
  echo "offline glosses staged: $glosses_destination"
else
  echo "no offline glosses at $glosses_source; candidates are glossed offline in English only"
fi
# Optional: the Cantonese, Zhuyin and Stroke dictionaries fetched by scripts/fetch_language_dictionaries.py (or built by `msime-dict-build languages`), as on macOS. host-api finds them in language-dictionaries/ beside EngineResources and names them in the runtime options; the keyboard leaves an enabled scheme whose dictionary is missing out of its picker. The directory is always created, empty when there are none, since the keyboard target bundles it as a folder. Each dictionary is staged only with its licence text, which must travel with the data.
languages_source=${3:-$repo_root/target/language-dictionaries}
languages_destination="$repo_root/target/ios/language-dictionaries"
rm -rf "$languages_destination"
mkdir -p "$languages_destination"
staged_languages=()
for pair in msime-cantonese.db:msime-rime_cantonese_LICENSE.txt msime-zhuyin.db:msime-libchewing_data_LICENSE.txt msime-stroke.db:msime-rime_stroke_LICENSE.txt; do
  database=${pair%%:*}
  license=${pair#*:}
  [ "$omit_on_demand" != 1 ] || continue
  [ -f "$languages_source/$database" ] || continue
  if [ ! -f "$languages_source/$license" ]; then
    echo "$languages_source/$database has no $license beside it; refusing to ship the data without its licence" >&2
    exit 1
  fi
  cp "$languages_source/$database" "$languages_source/$license" "$languages_destination/"
  staged_languages+=("$database")
done
if [ "${#staged_languages[@]}" -gt 0 ]; then
  echo "language dictionaries staged (${staged_languages[*]}): $languages_destination"
else
  echo "no language dictionaries at $languages_source; Cantonese, Zhuyin and Stroke stay unavailable"
fi
# 完整发布包按语言锁要求带齐数据库；瘦包通过下载提供它们，不要求随包。
if [ "${MSIME_REQUIRE_LANGUAGE_DICTIONARIES:-0}" = 1 ] && [ "$omit_on_demand" != 1 ]; then
  required_languages=$(python3 "$repo_root/scripts/fetch_language_dictionaries.py" --list-databases)
  if [ -z "$required_languages" ]; then
    echo "MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 but resources/language-dictionaries.lock.json pins no dictionary" >&2
    exit 1
  fi
  for database in $required_languages; do
    if [[ " ${staged_languages[*]:-} " != *" $database "* ]]; then
      echo "MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 but $database, pinned by resources/language-dictionaries.lock.json, was not staged from $languages_source" >&2
      exit 1
    fi
  done
fi
# 背单词的内置词书（scripts/fetch_wordbooks.py 从 ECDICT 生成），和 EngineResources 一起放进键盘扩展：设置 App 在它的同级 wordbooks/ 里找书（VocabularyReviewStore）。目录总是建出来，没有词书时为空，因为键盘扩展把它当文件夹打包；八本书只和许可声明一起暂存，没有时背单词只列用户导入的词表。词书不分版本，也不是按需下载的资源，瘦包同样带。MSIME_REQUIRE_WORDBOOKS=1 时缺了 resources/wordbook.lock.json 列出的任何一本就失败。
wordbooks_source=${4:-$repo_root/target/wordbooks}
wordbooks_destination="$repo_root/target/ios/wordbooks"
rm -rf "$wordbooks_destination"
mkdir -p "$wordbooks_destination"
if compgen -G "$wordbooks_source/*.json" >/dev/null && [ -f "$wordbooks_source/wordbooks-NOTICE.txt" ]; then
  cp "$wordbooks_source"/*.json "$wordbooks_source/wordbooks-NOTICE.txt" "$wordbooks_destination/"
  echo "wordbooks staged: $wordbooks_destination"
else
  echo "no wordbooks at $wordbooks_source; 背单词 offers only imported books"
fi
if [ "${MSIME_REQUIRE_WORDBOOKS:-0}" = 1 ]; then
  for id in $(python3 -c 'import json, sys; print(" ".join(book["id"] for book in json.load(open(sys.argv[1], encoding="utf-8"))["books"]))' "$repo_root/resources/wordbook.lock.json"); do
    [ -f "$wordbooks_destination/$id.json" ] || { echo "MSIME_REQUIRE_WORDBOOKS=1 but $id.json, listed by resources/wordbook.lock.json, was not staged from $wordbooks_source; run python3 scripts/fetch_wordbooks.py first" >&2; exit 1; }
  done
fi
# 内置按键音包：resources/sound-packs 里 `mode = "keys"` 的包整目录（样本连同写着许可的 plugin.toml）放进键盘扩展的 sound-packs/，与 Android 的 assets/sound-packs 是同一批。键盘把这个目录作为 `sound_packs` 交给 `msime_client_key_sound_pack` 取校验过的样本路径，设置 App 从它内嵌的键盘扩展里找同一个目录列出音效、试听。旋律包和音乐包 iOS 不播，不打包：两个音乐包各 1.6 MB，键盘扩展的内存和体积都不该为用不到的东西付钱。
sound_packs_destination="$repo_root/target/ios/sound-packs"
rm -rf "$sound_packs_destination"
mkdir -p "$sound_packs_destination"
for pack_dir in "$repo_root"/resources/sound-packs/*/; do
  grep -qx 'kind = "sound"' "$pack_dir/plugin.toml" || continue
  grep -qx 'mode = "keys"' "$pack_dir/plugin.toml" || continue
  cp -R "${pack_dir%/}" "$sound_packs_destination/$(basename "$pack_dir")"
done
[ -f "$sound_packs_destination/default/plugin.toml" ] || { echo "the default key sound pack was not staged into $sound_packs_destination" >&2; exit 1; }
echo "key sound packs staged: $sound_packs_destination"
echo "iOS resources staged from the pinned dictionary release: $destination"
