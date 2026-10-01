#!/usr/bin/env bash
set -euo pipefail
umask 077
repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
resource_dir=${1:?usage: build-apk.sh <verified-resource-directory>}
resource_dir=$(cd "$resource_dir" && pwd)
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
tools_dir="$android_sdk/build-tools/35.0.0"
# The Gradle host module compiles against API 36. Check the same platform here so a
# partially installed SDK fails before Gradle starts resolving dependencies.
android_jar="$android_sdk/platforms/android-36/android.jar"
[[ -f "$android_jar" && -x "$tools_dir/d8" ]] || { echo "Android API 36 platform and build-tools 35 required" >&2; exit 1; }
artifacts=$(cargo run --quiet -p msime-client-core --example verify_resources --locked -- "$resource_dir")
for abi in arm64-v8a x86_64; do bash platforms/android/build-native.sh "$abi"; done

# The host is a Gradle build now: it uses AndroidX and Material, and those ship as AARs whose
# resources have to be merged and whose R classes have to be generated per package. The previous
# aapt2/d8 pipeline had no dependency resolution at all, so every one of those steps would have been
# hand-rolled here. Gradle and AGP are pinned to the versions the Tauri bundle already resolves.
assets="$repo_root/target/android/host-assets"
rm -rf "$assets"
mkdir -p "$assets/dictionary"
cp resources/desktop-dictionary.lock.json "$assets/"
while IFS= read -r artifact; do cp "$resource_dir/$artifact" "$assets/dictionary/"; done <<< "$artifacts"
cargo run --quiet -p msime-client-core --example verify_resources --locked -- "$assets/dictionary" >/dev/null
mkdir -p "$assets/native-notices"
cp -R target/android/notices/. "$assets/native-notices/"
cp LICENSE "$assets/client-LICENSE.txt"
# Helpcode tables are not part of the dictionary release; the repository carries them in resources/helpcodes. Bootstrap extracts them into helpcodes/ under the resource directory, where the Engine reads them (crates/engine/src/assets.rs names the six files), and replaces them whenever the package changes.
mkdir -p "$assets/helpcodes"
for table in helpcode.txt zrm_helpcode_big_unique.txt shouyou2_0_helpcode.txt shouyouplus_helpcode.txt xiaohe_helpcode.txt jiajia_helpcode.txt; do
  cp "resources/helpcodes/$table" "$assets/helpcodes/$table"
done
cp resources/helpcodes/ENGINE-NOTICE.md "$assets/helpcodes/NOTICE.md"
cp resources/helpcodes/NOTICE.md "$assets/helpcodes/NOTICE-jiajia.md"
# Optional non-English candidate glosses (scripts/build_offline_glosses.py). Bootstrap extracts them beside the resources, where the Engine looks for one zh-<lang>.db per target language; without them only English is glossed offline.
glosses_source=${MSIME_OFFLINE_GLOSSES:-$repo_root/target/offline-glosses}
rm -rf "$assets/offline-glosses"
if compgen -G "$glosses_source/zh-*.db" >/dev/null && [ -f "$glosses_source/offline-glosses-NOTICE.txt" ]; then
  mkdir -p "$assets/offline-glosses"
  cp "$glosses_source"/zh-*.db "$glosses_source/offline-glosses-NOTICE.txt" "$assets/offline-glosses/"
  echo "offline glosses packaged from $glosses_source"
else
  echo "no offline glosses at $glosses_source; candidates are glossed offline in English only"
fi
# Optional Cantonese and Zhuyin dictionaries fetched by scripts/fetch_language_dictionaries.py (or built by `msime-dict-build languages`), as on macOS, iOS and HarmonyOS. Bootstrap extracts them to language-dictionaries/ beside the resources, where host-api finds them and names them in the runtime options; the keyboard and the settings page leave a scheme whose dictionary is missing out. Each dictionary is packaged only with its licence text, which must travel with the data.
languages_source=${MSIME_LANGUAGE_DICTIONARIES:-$repo_root/target/language-dictionaries}
rm -rf "$assets/language-dictionaries"
staged_languages=()
for pair in cantonese.db:rime_cantonese_LICENSE.txt zhuyin.db:libchewing_data_LICENSE.txt; do
  database=${pair%%:*}
  license=${pair#*:}
  [ -f "$languages_source/$database" ] || continue
  if [ ! -f "$languages_source/$license" ]; then
    echo "$languages_source/$database has no $license beside it; refusing to ship the data without its licence" >&2
    exit 1
  fi
  mkdir -p "$assets/language-dictionaries"
  cp "$languages_source/$database" "$languages_source/$license" "$assets/language-dictionaries/"
  staged_languages+=("$database")
done
if [ "${#staged_languages[@]}" -gt 0 ]; then
  echo "language dictionaries packaged (${staged_languages[*]}) from $languages_source"
else
  echo "no language dictionaries at $languages_source; Cantonese and Zhuyin stay unavailable"
fi
if [ "${MSIME_REQUIRE_LANGUAGE_DICTIONARIES:-0}" = 1 ] && [ "${#staged_languages[@]}" -ne 2 ]; then
  echo "MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 but cantonese.db and zhuyin.db were not both packaged from $languages_source" >&2
  exit 1
fi

gradle_dir="$repo_root/platforms/android/gradle-app"
tauri_gradlew="$repo_root/apps/desktop/src-tauri/gen/android/gradlew"
[[ -x "$tauri_gradlew" ]] || { echo "Gradle wrapper required; run the Tauri Android init once" >&2; exit 1; }
ANDROID_HOME="$android_sdk" "$tauri_gradlew" --project-dir "$gradle_dir" --console=plain assembleRelease

unsigned="$gradle_dir/app/build/outputs/apk/release/app-release-unsigned.apk"
[[ -f "$unsigned" ]] || { echo "Expected host APK not produced" >&2; exit 1; }
keystore="$repo_root/target/android/development.keystore"
if [[ ! -f "$keystore" ]]; then
  keytool -genkeypair -keystore "$keystore" -storepass android -keypass android \
    -alias androiddebugkey -dname "CN=MSIME Development" -keyalg RSA -keysize 2048 -validity 3650
fi
"$tools_dir/zipalign" -P 16 4 "$unsigned" "$repo_root/target/android/aligned.apk"
"$tools_dir/apksigner" sign --ks "$keystore" --ks-key-alias androiddebugkey \
  --ks-pass pass:android --key-pass pass:android \
  --out target/android/msime-client.apk "$repo_root/target/android/aligned.apk"
"$tools_dir/apksigner" verify --verbose target/android/msime-client.apk
"$tools_dir/zipalign" -c -P 16 4 target/android/msime-client.apk
rm -f "$repo_root/target/android/aligned.apk"
echo "Development APK built: $repo_root/target/android/msime-client.apk; not installed or device-verified"
