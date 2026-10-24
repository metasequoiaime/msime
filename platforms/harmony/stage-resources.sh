#!/usr/bin/env bash
# Stages the pinned dictionary release into the HAP, the counterpart of what build-apk.sh does for
# Android. Run it before hvigorw assembleHap; the natives come from build-native.sh.
#
# The artifacts land in resfile rather than rawfile because OpenHarmony extracts resfile to
# context.resourceDir at install time, which gives the Engine a real filesystem path. Nothing writes
# back into it: msime_client_prepare_host only verifies this directory and puts its state elsewhere,
# so the read-only extraction is enough and the 180MB first-run copy Android needs is avoided.
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
resource_dir=${1:?usage: stage-resources.sh <verified-resource-directory> [offline-glosses-directory] [language-dictionaries-directory] [wordbooks-directory]}
resource_dir=$(cd "$resource_dir" && pwd)
# The directory has to match the lock exactly, down to containing no extra file, because the same
# check runs again on the device inside prepare_host. Failing here is far cheaper than failing there.
artifacts=$(cargo run --quiet -p msime-client-core --example verify_resources --locked -- "$resource_dir")
staged="$repo_root/platforms/harmony/entry/src/main/resources/resfile/engine"
rm -rf "$staged"
mkdir -p "$staged"
while IFS= read -r artifact; do cp "$resource_dir/$artifact" "$staged/"; done <<< "$artifacts"
# 辅助码表不在词库发布里，由仓库自带在 resources/helpcodes，Engine 从资源目录下的 helpcodes/ 读它们（crates/engine/src/assets.rs 列出七个文件）。共享校验放行真实的 helpcodes/ 目录，StagedResources 把它和其余文件一起复制出去。
mkdir -p "$staged/helpcodes"
for table in helpcode.txt zrm_helpcode_big_unique.txt shouyou2_0_helpcode.txt shouyouplus_helpcode.txt xiaohe_helpcode.txt jiajia_helpcode.txt wubi86_helpcode.txt; do
  cp "resources/helpcodes/$table" "$staged/helpcodes/$table"
done
cp resources/helpcodes/ENGINE-NOTICE.md "$staged/helpcodes/NOTICE.md"
cp resources/helpcodes/NOTICE.md "$staged/helpcodes/NOTICE-jiajia.md"
cp resources/helpcodes/NOTICE-wubi86.md "$staged/helpcodes/NOTICE-wubi86.md"
cargo run --quiet -p msime-client-core --example verify_resources --locked -- "$staged" >/dev/null
echo "Staged for the HAP: $staged"

# Licences of what the native library carries that its own notices do not cover: the input engine embeds the Korean Hanja table from libhangul's data/hanja/hanja.txt, whose BSD-3-Clause licence requires the notice in binary distributions, its Cantonese and Zhuyin schemes take their syllables and words from data derived from rime-cantonese (CC BY 4.0) and libchewing-data (LGPL-2.1-or-later), and its Stroke scheme takes its stroke orders from rime-stroke (LGPL-3.0, with the CNS11643 attribution), whose notices travel with every engine build even where those schemes are not offered. Beside the engine directory rather than in it, which the lock check above would reject; resfile extracts it to context.resourceDir/licenses.
licenses_staged="$repo_root/platforms/harmony/entry/src/main/resources/resfile/licenses"
rm -rf "$licenses_staged"
mkdir -p "$licenses_staged"
cp resources/licenses/libhangul-hanja-BSD-3-Clause.txt "$licenses_staged/"
cp resources/licenses/librime-octagram-BSD-3-Clause.txt "$licenses_staged/"
cp resources/licenses/rime-cantonese-CC-BY-4.0.txt resources/licenses/libchewing-data-LGPL-2.1.txt resources/licenses/rime-stroke-LGPL-3.0.txt "$licenses_staged/"
# 越南文和藏文方案编入原生库的 vi crate（MIT）和 ewts crate（MIT OR Apache-2.0，按 MIT 使用）的许可证全文。
cp resources/licenses/vi-MIT.txt resources/licenses/ewts-MIT.txt "$licenses_staged/"
# 拆字反查的部件拼音表由 kfcd/chaizi《漢語拆字字典》（CC BY 3.0）改编后编进原生库，第 4(b) 条要求随改编作品署名并说明改动，许可证文件里写着这些和法律文本全文。
cp resources/licenses/kfcd-chaizi-CC-BY-3.0.txt "$licenses_staged/"
# 符号模式（S 模式）的分组码表来自 rime-wanxiang 的 wanxiang_symbols.yaml（CC BY 4.0），第 3(a) 条要求随改编数据署名并说明改动，许可证文件里写着这些和法律文本全文。
cp resources/licenses/rime-wanxiang-CC-BY-4.0.txt "$licenses_staged/"
# 同一张拆字表里每个字的读音、名次和繁体字形取自 Unicode 汉字数据库（Unihan 17.0.0），Unicode License V3 要求随数据附上它的版权与许可声明。
cp resources/licenses/Unicode-License-V3.txt "$licenses_staged/"
# 简繁转换内嵌的 OpenCC 词表许可证随 HAP 分发。
cp resources/licenses/OpenCC-Apache-2.0.txt "$licenses_staged/"
cp resources/licenses/world-clock-NOTICE.txt "$licenses_staged/"
echo "Licences staged for the HAP: $licenses_staged"

# 内置音效包，由 scripts/generate_sound_packs.py 合成。放在 engine 目录旁边而不是里面，因为放进去会被上面的锁文件校验拒绝；resfile 会把它们解压到 context.resourceDir/sound-packs，也就是键盘告诉 client-services 的内置包根目录。
sound_packs_staged="$repo_root/platforms/harmony/entry/src/main/resources/resfile/sound-packs"
rm -rf "$sound_packs_staged"
mkdir -p "$sound_packs_staged"
cp -R resources/sound-packs/. "$sound_packs_staged/"
echo "Sound packs staged for the HAP: $sound_packs_staged"

# 表情面板里的水杉娘表情包（66 张透明 WebP 加 manifest.json），iOS 与 Android 打包的是同一份。与音效包一样放在 engine 目录旁边，resfile 会把它解压到 context.resourceDir/stickers/shuishan，键盘的 MascotStickerStore 从那里读清单，ArkUI Image 直接按沙箱路径加载图片。
stickers_staged="$repo_root/platforms/harmony/entry/src/main/resources/resfile/stickers/shuishan"
rm -rf "$stickers_staged"
mkdir -p "$stickers_staged"
cp -R resources/stickers/shuishan/. "$stickers_staged/"
echo "Mascot stickers staged for the HAP: $stickers_staged"

# Optional non-English candidate glosses built by scripts/build_offline_glosses.py. They sit beside the engine directory rather than in it, which the lock check above would reject; the keyboard copies them next to its copy of the resources, where the Engine looks for one zh-<lang>.db per target language. Without them only English is glossed offline.
glosses_source=${2:-$repo_root/target/offline-glosses}
glosses_staged="$repo_root/platforms/harmony/entry/src/main/resources/resfile/offline-glosses"
rm -rf "$glosses_staged"
if compgen -G "$glosses_source/zh-*.db" >/dev/null && [ -f "$glosses_source/offline-glosses-NOTICE.txt" ]; then
  mkdir -p "$glosses_staged"
  cp "$glosses_source"/zh-*.db "$glosses_source/offline-glosses-NOTICE.txt" "$glosses_staged/"
  echo "Offline glosses staged for the HAP: $glosses_staged"
else
  echo "no offline glosses at $glosses_source; candidates are glossed offline in English only"
fi

# Optional: the Cantonese, Zhuyin and Stroke dictionaries fetched by scripts/fetch_language_dictionaries.py (or built by `msime-dict-build languages`), as on macOS and iOS. Beside the engine directory rather than in it, which the lock check above would reject; resfile extracts them to context.resourceDir/language-dictionaries, the keyboard copies them to language-dictionaries/ beside its copy of the resources, where host-api finds them and names them in the runtime options, and the keyboard and the settings page leave a scheme whose dictionary is missing out. Each dictionary is staged only with its licence text, which must travel with the data.
languages_source=${3:-$repo_root/target/language-dictionaries}
languages_staged="$repo_root/platforms/harmony/entry/src/main/resources/resfile/language-dictionaries"
rm -rf "$languages_staged"
staged_languages=()
for pair in msime-cantonese.db:msime-rime_cantonese_LICENSE.txt msime-zhuyin.db:msime-libchewing_data_LICENSE.txt msime-stroke.db:msime-rime_stroke_LICENSE.txt; do
  database=${pair%%:*}
  license=${pair#*:}
  [ -f "$languages_source/$database" ] || continue
  if [ ! -f "$languages_source/$license" ]; then
    echo "$languages_source/$database has no $license beside it; refusing to ship the data without its licence" >&2
    exit 1
  fi
  mkdir -p "$languages_staged"
  cp "$languages_source/$database" "$languages_source/$license" "$languages_staged/"
  staged_languages+=("$database")
done
if [ "${#staged_languages[@]}" -gt 0 ]; then
  echo "Language dictionaries staged for the HAP (${staged_languages[*]}): $languages_staged"
else
  echo "no language dictionaries at $languages_source; Cantonese, Zhuyin and Stroke stay unavailable"
fi
# A release requires every dictionary resources/language-dictionaries.lock.json pins, not a fixed list: a dictionary that has not been released yet is staged when present but cannot fail a release, and the lock bump that publishes it makes it required.
if [ "${MSIME_REQUIRE_LANGUAGE_DICTIONARIES:-0}" = 1 ]; then
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

# 背单词的内置词书（scripts/fetch_wordbooks.py 从 ECDICT 生成），和 engine 目录并列放在 resfile/wordbooks：设置页把 context.resourceDir 交给共享层，共享层在它下面的 wordbooks/ 里找书（VocabularyReviewRequest）。八本书只和许可声明一起暂存；没有时背单词只列用户导入的词表。MSIME_REQUIRE_WORDBOOKS=1 时缺了 resources/wordbook.lock.json 列出的任何一本就失败。
wordbooks_source=${4:-$repo_root/target/wordbooks}
wordbooks_staged="$repo_root/platforms/harmony/entry/src/main/resources/resfile/wordbooks"
rm -rf "$wordbooks_staged"
if compgen -G "$wordbooks_source/*.json" >/dev/null && [ -f "$wordbooks_source/wordbooks-NOTICE.txt" ]; then
  mkdir -p "$wordbooks_staged"
  cp "$wordbooks_source"/*.json "$wordbooks_source/wordbooks-NOTICE.txt" "$wordbooks_staged/"
  echo "Wordbooks staged for the HAP: $wordbooks_staged"
else
  echo "no wordbooks at $wordbooks_source; 背单词 offers only imported books"
fi
if [ "${MSIME_REQUIRE_WORDBOOKS:-0}" = 1 ]; then
  for id in $(python3 -c 'import json, sys; print(" ".join(book["id"] for book in json.load(open(sys.argv[1], encoding="utf-8"))["books"]))' "$repo_root/resources/wordbook.lock.json"); do
    [ -f "$wordbooks_staged/$id.json" ] || { echo "MSIME_REQUIRE_WORDBOOKS=1 but $id.json, listed by resources/wordbook.lock.json, was not staged from $wordbooks_source; run python3 scripts/fetch_wordbooks.py first" >&2; exit 1; }
  done
fi
