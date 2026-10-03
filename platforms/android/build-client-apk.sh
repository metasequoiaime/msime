#!/usr/bin/env bash
# Tauri management UI and the native IME share one package and private state.
set -euo pipefail
umask 077
repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
resource_dir=${1:?usage: build-client-apk.sh <verified-resource-directory> [arm64-v8a|x86_64]}
abi=${2:-arm64-v8a}
case "$abi" in
  arm64-v8a) tauri_target=aarch64 ;;
  x86_64) tauri_target=x86_64 ;;
  *) echo "Unsupported ABI" >&2; exit 1 ;;
esac
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
[[ -n "$android_sdk" ]] || { echo "Android SDK required" >&2; exit 1; }
[[ -f "$android_sdk/platforms/android-36/android.jar" && -x "$android_sdk/build-tools/35.0.0/apksigner" ]] || { echo "Android API 36 and build-tools 35 required" >&2; exit 1; }
android_ndk=${MSIME_ANDROID_NDK:-$android_sdk/ndk/28.2.13676358}
tauri_android_dir=${TAURI_ANDROID_DIR:-}
if [[ -z "$tauri_android_dir" ]]; then
  tauri_manifest=$(cargo metadata --locked --format-version 1 | node -e '
    let input = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", chunk => input += chunk);
    process.stdin.on("end", () => {
      const matches = JSON.parse(input).packages.filter(item => item.name === "tauri");
      if (matches.length !== 1) process.exit(1);
      process.stdout.write(matches[0].manifest_path);
    });
  ')
  tauri_android_dir="$(dirname "$tauri_manifest")/mobile/android"
fi
[[ -f "$tauri_android_dir/build.gradle.kts" ]] || { echo "Locked Tauri Android sources required" >&2; exit 1; }
artifacts=$(cargo run --quiet -p msime-client-core --example verify_resources --locked -- "$resource_dir")
bash platforms/android/build-native.sh "$abi"
tauri_jni="$repo_root/target/android/tauri-jniLibs/$abi"
rm -rf "$tauri_jni"
mkdir -p "$tauri_jni"
cp "$repo_root/target/android/jniLibs/$abi/libmsime_android.so" \
  "$repo_root/target/android/jniLibs/$abi/libmsime_host_api.so" \
  "$repo_root/target/android/jniLibs/$abi/libsherpa-onnx-c-api.so" \
  "$repo_root/target/android/jniLibs/$abi/libonnxruntime.so" "$tauri_jni/"
assets="$repo_root/target/android/tauri-assets"
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
# Optional Cantonese, Zhuyin and Stroke dictionaries fetched by scripts/fetch_language_dictionaries.py (or built by `msime-dict-build languages`), as on macOS, iOS and HarmonyOS. Bootstrap extracts them to language-dictionaries/ beside the resources, where host-api finds them and names them in the runtime options; the keyboard and the settings page leave a scheme whose dictionary is missing out. Each dictionary is packaged only with its licence text, which must travel with the data.
languages_source=${MSIME_LANGUAGE_DICTIONARIES:-$repo_root/target/language-dictionaries}
# Each dictionary beside the licence file that must travel with it; the staging below and the APK check at the end read the same list.
language_pairs="cantonese.db:rime_cantonese_LICENSE.txt zhuyin.db:libchewing_data_LICENSE.txt stroke.db:rime_stroke_LICENSE.txt"
rm -rf "$assets/language-dictionaries"
staged_languages=()
for pair in $language_pairs; do
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
  echo "no language dictionaries at $languages_source; Cantonese, Zhuyin and Stroke stay unavailable"
fi
# A release requires every dictionary resources/language-dictionaries.lock.json pins, not a fixed list: a dictionary that has not been released yet (stroke.db until a langdict release carries it) is packaged when present but cannot fail a release, and the lock bump that publishes it makes it required.
if [ "${MSIME_REQUIRE_LANGUAGE_DICTIONARIES:-0}" = 1 ]; then
  required_languages=$(python3 "$repo_root/scripts/fetch_language_dictionaries.py" --list-databases)
  if [ -z "$required_languages" ]; then
    echo "MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 but resources/language-dictionaries.lock.json pins no dictionary" >&2
    exit 1
  fi
  for database in $required_languages; do
    if [[ " ${staged_languages[*]:-} " != *" $database "* ]]; then
      echo "MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 but $database, pinned by resources/language-dictionaries.lock.json, was not packaged from $languages_source" >&2
      exit 1
    fi
  done
fi
ANDROID_HOME="$android_sdk" NDK_HOME="$android_ndk" TAURI_ANDROID_DIR="$tauri_android_dir" \
  pnpm --filter @msime/desktop tauri android build --apk --target "$tauri_target" --ci
unsigned="$repo_root/apps/desktop/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk"
[[ -f "$unsigned" ]] || { echo "Expected Tauri APK not produced" >&2; exit 1; }
keystore=$(bash "$repo_root/platforms/android/scripts/dev-keystore.sh")
output="$repo_root/target/android/msime-client.apk"
"$android_sdk/build-tools/35.0.0/apksigner" sign --ks "$keystore" --ks-key-alias androiddebugkey \
  --ks-pass pass:android --key-pass pass:android --out "$output" "$unsigned"
"$android_sdk/build-tools/35.0.0/apksigner" verify "$output"
"$android_sdk/build-tools/35.0.0/zipalign" -c -P 16 4 "$output"
# The package is only worth shipping if it carries each dictionary staged above, beside its licence (a release, MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1, has already refused to stage fewer than the lock pins).
apk_entries=$(unzip -Z1 "$output")
for pair in $language_pairs; do
  [ -f "$assets/language-dictionaries/${pair%%:*}" ] || continue
  for entry in "${pair%%:*}" "${pair#*:}"; do
    grep -qxF "assets/language-dictionaries/$entry" <<< "$apk_entries" || { echo "$output has no assets/language-dictionaries/$entry although it was staged" >&2; exit 1; }
  done
done
echo "Tauri + native IME development APK built for $abi; no device changed"
