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
resource_dir=${1:?usage: stage-resources.sh <verified-resource-directory> [offline-glosses-directory] [language-dictionaries-directory]}
resource_dir=$(cd "$resource_dir" && pwd)
# The directory has to match the lock exactly, down to containing no extra file, because the same
# check runs again on the device inside prepare_host. Failing here is far cheaper than failing there.
artifacts=$(cargo run --quiet -p msime-client-core --example verify_resources --locked -- "$resource_dir")
staged="$repo_root/platforms/harmony/entry/src/main/resources/resfile/engine"
rm -rf "$staged"
mkdir -p "$staged"
while IFS= read -r artifact; do cp "$resource_dir/$artifact" "$staged/"; done <<< "$artifacts"
# Helpcode tables are not part of the dictionary release; the repository carries them in resources/helpcodes, and the Engine reads them from helpcodes/ under the resource directory (crates/engine/src/assets.rs names the six files). The shared verification lets a real helpcodes/ directory through, and StagedResources copies it out with the rest.
mkdir -p "$staged/helpcodes"
for table in helpcode.txt zrm_helpcode_big_unique.txt shouyou2_0_helpcode.txt shouyouplus_helpcode.txt xiaohe_helpcode.txt jiajia_helpcode.txt; do
  cp "resources/helpcodes/$table" "$staged/helpcodes/$table"
done
cp resources/helpcodes/ENGINE-NOTICE.md "$staged/helpcodes/NOTICE.md"
cp resources/helpcodes/NOTICE.md "$staged/helpcodes/NOTICE-jiajia.md"
cargo run --quiet -p msime-client-core --example verify_resources --locked -- "$staged" >/dev/null
echo "Staged for the HAP: $staged"

# Licences of what the native library carries that its own notices do not cover: the input engine embeds the Korean Hanja table from libhangul's data/hanja/hanja.txt, whose BSD-3-Clause licence requires the notice in binary distributions, its Cantonese and Zhuyin schemes take their syllables and words from data derived from rime-cantonese (CC BY 4.0) and libchewing-data (LGPL-2.1-or-later), and its Stroke scheme takes its stroke orders from rime-stroke (LGPL-3.0, with the CNS11643 attribution), whose notices travel with every engine build even where those schemes are not offered. Beside the engine directory rather than in it, which the lock check above would reject; resfile extracts it to context.resourceDir/licenses.
licenses_staged="$repo_root/platforms/harmony/entry/src/main/resources/resfile/licenses"
rm -rf "$licenses_staged"
mkdir -p "$licenses_staged"
cp resources/licenses/libhangul-hanja-BSD-3-Clause.txt "$licenses_staged/"
cp resources/licenses/rime-cantonese-CC-BY-4.0.txt resources/licenses/libchewing-data-LGPL-2.1.txt resources/licenses/rime-stroke-LGPL-3.0.txt "$licenses_staged/"
echo "Licences staged for the HAP: $licenses_staged"

# The built-in sound packs, synthesized by scripts/generate_sound_packs.py. Beside the engine directory rather than in it, which the lock check above would reject; resfile extracts them to context.resourceDir/sound-packs, the built-in pack root the keyboard names to client-core.
sound_packs_staged="$repo_root/platforms/harmony/entry/src/main/resources/resfile/sound-packs"
rm -rf "$sound_packs_staged"
mkdir -p "$sound_packs_staged"
cp -R resources/sound-packs/. "$sound_packs_staged/"
echo "Sound packs staged for the HAP: $sound_packs_staged"

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
for pair in cantonese.db:rime_cantonese_LICENSE.txt zhuyin.db:libchewing_data_LICENSE.txt stroke.db:rime_stroke_LICENSE.txt; do
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
if [ "${MSIME_REQUIRE_LANGUAGE_DICTIONARIES:-0}" = 1 ] && [ "${#staged_languages[@]}" -ne 3 ]; then
  echo "MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 but cantonese.db, zhuyin.db and stroke.db were not all staged from $languages_source" >&2
  exit 1
fi
