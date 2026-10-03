#!/usr/bin/env bash
set -euo pipefail
umask 077
repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
resource_dir=${1:?usage: [MSIME_EDITION=<id>] build-apk.sh <verified-resource-directory>}
resource_dir=$(cd "$resource_dir" && pwd)
# MSIME_EDITION=<id> 选产品版本（版本表 shared/contracts/editions.json 里有 Android 段的 id，也是 gradle-app 的 flavor 名），缺省是 full。每个版本是一个独立的包：本版本的 applicationId、资源锁里列的词库、本版本要的语言词库，APK 也按版本命名。full 的 applicationId、资源和 APK 名（target/android/msime-client.apk）与引入版本之前相同。
edition=${MSIME_EDITION:-full}
edition_tool="$repo_root/platforms/android/scripts/edition_android.py"
apk_name=$(python3 "$edition_tool" field --edition "$edition" apk_name)
edition_lock=$(python3 "$edition_tool" field --edition "$edition" resource_lock)
edition_languages=$(python3 "$edition_tool" field --edition "$edition" language_dictionaries)
flavor="$(tr '[:lower:]' '[:upper:]' <<< "${edition:0:1}")${edition:1}"
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
# APK 里的资源锁一律叫 desktop-dictionary.lock.json（Bootstrap 按这个名字读），内容是本版本的锁：full 就是 resources/desktop-dictionary.lock.json 本身，其他版本是 scripts/editions.py 生成的 resources/editions/<id>.lock.json。暂存的词库恰好是锁里列的文件，再按本版本的锁校验一遍。
cp "$edition_lock" "$assets/desktop-dictionary.lock.json"
edition_flags=()
if [ "$edition" != full ]; then
  edition_artifacts=$(python3 -c 'import json, sys; print("\n".join(artifact["name"] for artifact in json.load(open(sys.argv[1]))["artifacts"]))' "$edition_lock")
  artifacts=$(grep -Fx -f <(printf '%s\n' "$edition_artifacts") <<< "$artifacts")
  edition_flags=(--edition "$edition")
fi
while IFS= read -r artifact; do cp "$resource_dir/$artifact" "$assets/dictionary/"; done <<< "$artifacts"
cargo run --quiet -p msime-client-core --example verify_resources --locked -- ${edition_flags[@]+"${edition_flags[@]}"} "$assets/dictionary" >/dev/null
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
# 只带本版本要的语言词库（版本表的 language_dictionaries）：full 是粤拼和注音两个，五笔版和拼音版一个也不带。
for pair in cantonese.db:rime_cantonese_LICENSE.txt zhuyin.db:libchewing_data_LICENSE.txt; do
  database=${pair%%:*}
  license=${pair#*:}
  grep -qxF "$database" <<< "$edition_languages" || continue
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
elif [ -z "$edition_languages" ]; then
  echo "edition $edition packs no language dictionaries"
else
  echo "no language dictionaries at $languages_source; Cantonese and Zhuyin stay unavailable"
fi
required_languages=$(grep -c . <<< "$edition_languages" || true)
if [ "${MSIME_REQUIRE_LANGUAGE_DICTIONARIES:-0}" = 1 ] && [ "${#staged_languages[@]}" -ne "$required_languages" ]; then
  echo "MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 but the language dictionaries edition $edition needs ($(tr '\n' ' ' <<< "$edition_languages")) were not all packaged from $languages_source" >&2
  exit 1
fi

gradle_dir="$repo_root/platforms/android/gradle-app"
tauri_gradlew="$repo_root/apps/desktop/src-tauri/gen/android/gradlew"
[[ -x "$tauri_gradlew" ]] || { echo "Gradle wrapper required; run the Tauri Android init once" >&2; exit 1; }
# The APK's versionName comes from version.txt; MSIME_ANDROID_VERSION (the release workflow's version input) overrides it.
version_args=()
[[ -n "${MSIME_ANDROID_VERSION:-}" ]] && version_args+=("-PmsimeVersion=$MSIME_ANDROID_VERSION")
ANDROID_HOME="$android_sdk" "$tauri_gradlew" --project-dir "$gradle_dir" --console=plain ${version_args[@]+"${version_args[@]}"} "assemble${flavor}Release"

unsigned="$gradle_dir/app/build/outputs/apk/$edition/release/app-$edition-release-unsigned.apk"
[[ -f "$unsigned" ]] || { echo "Expected host APK not produced" >&2; exit 1; }
# 正式包用发布密钥签名：release-android.yml 从仓库 secrets 解出 PKCS12 文件，把路径和口令放进这两个环境变量。发布密钥一旦用于发版就不能更换，否则已安装的用户无法覆盖升级；没有设置时退回所有 worktree 共用的开发密钥。
if [[ -n "${MSIME_ANDROID_RELEASE_KEYSTORE:-}" ]]; then
  [[ -f "$MSIME_ANDROID_RELEASE_KEYSTORE" && -n "${MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD:-}" ]] || { echo "MSIME_ANDROID_RELEASE_KEYSTORE needs an existing file and MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD" >&2; exit 1; }
  signing=(--ks "$MSIME_ANDROID_RELEASE_KEYSTORE" --ks-key-alias msime-release
    --ks-pass env:MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD --key-pass env:MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD)
  signed_with="release key"
else
  keystore=$(bash "$repo_root/platforms/android/scripts/dev-keystore.sh")
  signing=(--ks "$keystore" --ks-key-alias androiddebugkey --ks-pass pass:android --key-pass pass:android)
  signed_with="development key"
fi
output="$repo_root/target/android/$apk_name.apk"
aligned="$repo_root/target/android/$apk_name-aligned.apk"
"$tools_dir/zipalign" -P 16 4 "$unsigned" "$aligned"
"$tools_dir/apksigner" sign "${signing[@]}" --out "$output" "$aligned"
"$tools_dir/apksigner" verify --verbose --print-certs "$output"
"$tools_dir/zipalign" -c -P 16 4 "$output"
# The package is only worth shipping if it carries each dictionary staged above, beside its licence (a release, MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1, has already refused to stage fewer than both).
apk_entries=$(unzip -Z1 "$output")
for pair in cantonese.db:rime_cantonese_LICENSE.txt zhuyin.db:libchewing_data_LICENSE.txt; do
  [ -f "$assets/language-dictionaries/${pair%%:*}" ] || continue
  for entry in "${pair%%:*}" "${pair#*:}"; do
    grep -qxF "assets/language-dictionaries/$entry" <<< "$apk_entries" || { echo "$output has no assets/language-dictionaries/$entry although it was staged" >&2; exit 1; }
  done
done
rm -f "$aligned"
echo "APK for edition $edition built and signed with the $signed_with: $output; not installed or device-verified"
