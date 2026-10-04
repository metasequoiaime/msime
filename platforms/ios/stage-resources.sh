#!/usr/bin/env bash
set -euo pipefail
umask 077

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
source_dir=${1:?usage: stage-resources.sh <verified-resource-directory> [offline-glosses-directory] [language-dictionaries-directory]}
source_dir=$(cd "$source_dir" && pwd)
destination="$repo_root/target/ios/EngineResources"
artifacts=$(cargo run --quiet -p msime-client-core --example verify_resources --locked -- "$source_dir")
# Rebuilt as one unit, as on macOS: a directory staged for an earlier lock keeps files the current lock no longer names, and the verifier below refuses them.
rm -rf "$destination"
mkdir -p "$destination"
while IFS= read -r artifact; do
  cp "$source_dir/$artifact" "$destination/$artifact"
done <<< "$artifacts"
cargo run --quiet -p msime-client-core --example verify_resources --locked -- "$destination" >/dev/null
# Helpcode tables are not part of the dictionary release; the repository carries them in resources/helpcodes, and the Engine reads them from helpcodes/ under the resource directory (crates/engine/src/assets.rs names the six files). Without them the Engine has nothing to match: Shift letters are taken as helpcode and narrow nothing.
helpcodes="$repo_root/resources/helpcodes"
rm -rf "$destination/helpcodes"
mkdir -p "$destination/helpcodes"
for table in helpcode.txt zrm_helpcode_big_unique.txt shouyou2_0_helpcode.txt shouyouplus_helpcode.txt xiaohe_helpcode.txt jiajia_helpcode.txt; do
  cp "$helpcodes/$table" "$destination/helpcodes/$table"
done
cp "$helpcodes/ENGINE-NOTICE.md" "$destination/helpcodes/NOTICE.md"
cp "$helpcodes/NOTICE.md" "$destination/helpcodes/NOTICE-jiajia.md"
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
echo "iOS resources staged from the pinned dictionary release: $destination"
