#!/usr/bin/env bash
set -euo pipefail
umask 077

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
source_dir=${1:?usage: stage-resources.sh <verified-resource-directory> [settled-model-directory] [offline-glosses-directory] [language-dictionaries-directory]}
source_dir=$(cd "$source_dir" && pwd)
destination="$repo_root/target/macos/EngineResources"

# Use the shared verifier as the source of truth. The staged directory is ignored build output and is rebuilt as one unit, so a failed copy cannot look complete.
# MSIME_MACOS_OMIT_ON_DEMAND=1 按发布包的规则只暂存核心词库：日文词典那一对文件（resources.rs 的 MACOS_ON_DEMAND_ARTIFACTS）不进包，由 App 在用户选日文方案时下载到 resource-packs/japanese。默认的开发流程仍暂存全部文件，作为下载之外的内置兜底。
verify_flags=()
if [ "${MSIME_MACOS_OMIT_ON_DEMAND:-0}" = 1 ]; then
  verify_flags=(--omit-on-demand)
fi
artifacts=$(cargo run --quiet -p msime-client-core --example verify_resources --locked -- ${verify_flags[@]+"${verify_flags[@]}"} "$source_dir")
rm -rf "$destination"
mkdir -p "$destination"
while IFS= read -r artifact; do
  cp "$source_dir/$artifact" "$destination/$artifact"
done <<< "$artifacts"
cargo run --quiet -p msime-client-core --example verify_resources --locked -- ${verify_flags[@]+"${verify_flags[@]}"} "$destination" >/dev/null
# Helpcode tables are not part of the dictionary release; the repository carries them in resources/helpcodes, and the Engine reads them from helpcodes/ under the resource directory (crates/engine/src/assets.rs names the six files). Without them the Engine has nothing to match: Shift letters are taken as helpcode and narrow nothing. The shared verifier lets a real helpcodes/ directory through.
helpcodes="$repo_root/resources/helpcodes"
mkdir -p "$destination/helpcodes"
for table in helpcode.txt zrm_helpcode_big_unique.txt shouyou2_0_helpcode.txt shouyouplus_helpcode.txt xiaohe_helpcode.txt jiajia_helpcode.txt; do
  cp "$helpcodes/$table" "$destination/helpcodes/$table"
done
cp "$helpcodes/ENGINE-NOTICE.md" "$destination/helpcodes/NOTICE.md"
cp "$helpcodes/NOTICE.md" "$destination/helpcodes/NOTICE-jiajia.md"

# The settled-rerank model, staged as a sibling of the bundle rather than a member of it.
#
# A member would fail the verifier immediately above, which requires this directory to hold exactly
# the artifacts the dictionary lock pins — the check whose job is to prove a shipped dictionary is
# intact. `prepare_host_configuration` looks for the sibling and names it in the runtime options.
#
# Optional: 25 MB buying a desktop-only improvement, fetched by scripts/fetch_settled_model.py.
# Without it the host behaves exactly as it does today.
settled_source=${2:-$repo_root/target/settled-model}
# fetch_neural_model.py keeps both presets together in target/neural-model. Keep the historical
# settled-model argument working, but use the shared neural staging directory when that older
# one-artifact directory was not prepared.
if [ ! -f "$settled_source/sentence-model-desktop.safetensors" ] &&
   [ -f "$repo_root/target/neural-model/sentence-model-desktop.safetensors" ]; then
  settled_source="$repo_root/target/neural-model"
fi
settled_destination="$repo_root/target/macos/settled-model"
rm -rf "$settled_destination"
if [ -f "$settled_source/sentence-model-desktop.safetensors" ]; then
  mkdir -p "$settled_destination"
  cp "$settled_source/sentence-model-desktop.safetensors" "$settled_destination/"
  echo "settled model staged: $settled_destination"
else
  echo "no settled model at $settled_source; the desktop rerank pass stays off"
fi

# Optional: non-English candidate glosses built by scripts/build_offline_glosses.py. Engine looks for them beside the resource directory, one zh-<lang>.db per target language; without them only English is glossed offline.
glosses_source=${3:-$repo_root/target/offline-glosses}
glosses_destination="$repo_root/target/macos/offline-glosses"
rm -rf "$glosses_destination"
if compgen -G "$glosses_source/zh-*.db" >/dev/null && [ -f "$glosses_source/offline-glosses-NOTICE.txt" ]; then
  mkdir -p "$glosses_destination"
  cp "$glosses_source"/zh-*.db "$glosses_source/offline-glosses-NOTICE.txt" "$glosses_destination/"
  echo "offline glosses staged: $glosses_destination"
else
  echo "no offline glosses at $glosses_source; candidates are glossed offline in English only"
fi

# Optional: the Cantonese, Zhuyin and Stroke dictionaries fetched by scripts/fetch_language_dictionaries.py (or built by `msime-dict-build languages`). prepare_host_configuration finds them beside the resource directory and names them in the runtime options; a scheme whose dictionary is missing is shown as unavailable and falls back. Each dictionary is staged only with its licence text, which must travel with the data.
# 这里暂存的是开发构建用的内置兜底。macOS 发布包（package-release.sh）不带它们，App 在用户选粤拼、注音或笔画时把 resources/language-dictionaries.lock.json 固定的文件下载到 resource-packs/language-dictionaries，查找时下载的优先、这里的次之。
languages_source=${4:-$repo_root/target/language-dictionaries}
languages_destination="$repo_root/target/macos/language-dictionaries"
rm -rf "$languages_destination"
staged_languages=()
for pair in cantonese.db:rime_cantonese_LICENSE.txt zhuyin.db:libchewing_data_LICENSE.txt stroke.db:rime_stroke_LICENSE.txt; do
  database=${pair%%:*}
  license=${pair#*:}
  [ -f "$languages_source/$database" ] || continue
  if [ ! -f "$languages_source/$license" ]; then
    echo "$languages_source/$database has no $license beside it; refusing to ship the data without its licence" >&2
    exit 1
  fi
  mkdir -p "$languages_destination"
  cp "$languages_source/$database" "$languages_source/$license" "$languages_destination/"
  staged_languages+=("$database")
done
if [ "${#staged_languages[@]}" -gt 0 ]; then
  echo "language dictionaries staged (${staged_languages[*]}): $languages_destination"
else
  echo "no language dictionaries at $languages_source; Cantonese, Zhuyin and Stroke stay unavailable"
fi
# A release requires every dictionary resources/language-dictionaries.lock.json pins, not a fixed list: a dictionary that has not been released yet (stroke.db until a langdict release carries it) is staged when present but cannot fail a release, and the lock bump that publishes it makes it required.
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
echo "macOS resources staged from the pinned dictionary release: $destination"
