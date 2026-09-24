#!/usr/bin/env bash
set -euo pipefail
umask 077

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
source_dir=${1:?usage: stage-resources.sh <verified-resource-directory> [settled-model-directory] [offline-glosses-directory] [pronunciations-directory] [character-glosses-directory] [word-glosses-directory]}
source_dir=$(cd "$source_dir" && pwd)
destination="$repo_root/target/macos/EngineResources"

# Use the shared verifier as the source of truth. The staged directory is ignored
# build output and is rebuilt as one unit, so a failed copy cannot look complete.
artifacts=$(cargo run --quiet -p msime-client-core --example verify_resources --locked -- "$source_dir")
rm -rf "$destination"
mkdir -p "$destination"
while IFS= read -r artifact; do
  cp "$source_dir/$artifact" "$destination/$artifact"
done <<< "$artifacts"
cargo run --quiet -p msime-client-core --example verify_resources --locked -- "$destination" >/dev/null
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

# Optional: the English pronunciation table built by scripts/build_pronunciations.py, read beside the resource directory like the glosses. Without it English gloss lines carry no IPA; Japanese romaji needs no data.
pronunciations_source=${4:-$repo_root/target/pronunciations}
pronunciations_destination="$repo_root/target/macos/pronunciations"
rm -rf "$pronunciations_destination"
if [ -f "$pronunciations_source/en-phonetic.db" ] && [ -f "$pronunciations_source/pronunciations-NOTICE.txt" ]; then
  mkdir -p "$pronunciations_destination"
  cp "$pronunciations_source/en-phonetic.db" "$pronunciations_source/pronunciations-NOTICE.txt" "$pronunciations_destination/"
  echo "pronunciations staged: $pronunciations_destination"
else
  echo "no pronunciations at $pronunciations_source; English glosses are shown without IPA"
fi
# Optional: English glosses for single characters built by scripts/build_character_glosses.py, which english.db leaves out; read beside the resource directory.
characters_source=${5:-$repo_root/target/character-glosses}
characters_destination="$repo_root/target/macos/character-glosses"
rm -rf "$characters_destination"
if [ -f "$characters_source/zh-en.db" ] && [ -f "$characters_source/character-glosses-NOTICE.txt" ]; then
  mkdir -p "$characters_destination"
  cp "$characters_source/zh-en.db" "$characters_source/character-glosses-NOTICE.txt" "$characters_destination/"
  echo "character glosses staged: $characters_destination"
else
  echo "no character glosses at $characters_source; single characters have no English gloss"
fi
# Optional: English glosses for words english.db does not cover, built from CC-CEDICT by scripts/build_word_glosses.py; read beside the resource directory.
words_source=${6:-$repo_root/target/word-glosses}
words_destination="$repo_root/target/macos/word-glosses"
rm -rf "$words_destination"
if [ -f "$words_source/zh-en.db" ] && [ -f "$words_source/word-glosses-NOTICE.txt" ]; then
  mkdir -p "$words_destination"
  cp "$words_source/zh-en.db" "$words_source/word-glosses-NOTICE.txt" "$words_destination/"
  echo "word glosses staged: $words_destination"
else
  echo "no word glosses at $words_source; English glosses come from english.db only"
fi
echo "macOS resources staged from the pinned dictionary release: $destination"
