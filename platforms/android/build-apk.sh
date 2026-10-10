#!/usr/bin/env bash
set -euo pipefail
umask 077
repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
resource_dir=${1:?usage: [MSIME_EDITION=<id>] build-apk.sh <verified-resource-directory>}
resource_dir=$(cd "$resource_dir" && pwd)
# MSIME_EDITION=<id> 选产品版本（版本表 shared/contracts/editions.json 里有 Android 段的 id，也是 gradle-app 的 flavor 名），缺省是 full。每个版本是一个独立的包：本版本的 applicationId、资源锁里列的词库、本版本要的语言词库，APK 也按版本命名。full 的 applicationId 和资源与引入版本之前相同，APK 名是 target/android/msime-android.apk。
edition=${MSIME_EDITION:-full}
edition_tool="$repo_root/platforms/android/scripts/edition_android.py"
apk_name=$(python3 "$edition_tool" field --edition "$edition" apk_name)
edition_lock=$(python3 "$edition_tool" field --edition "$edition" resource_lock)
edition_languages=$(python3 "$edition_tool" field --edition "$edition" language_dictionaries)
edition_offline_glosses=$(python3 "$edition_tool" field --edition "$edition" features.offline_glosses)
flavor="$(tr '[:lower:]' '[:upper:]' <<< "${edition:0:1}")${edition:1}"
# MSIME_ANDROID_OMIT_ON_DEMAND=1 打发布用的瘦包，对照 macOS 的 MSIME_MACOS_OMIT_ON_DEMAND：日文词典那一组（词典与两份 Mozc 许可文本，resources.rs 的 ON_DEMAND_JAPANESE_ARTIFACTS）、粤拼注音笔画语言词库、非英文离线释义和本地语音运行库都不进包，由应用在用户添加日语或这些语言、打开离线释义、打开离线识别时作为资源包下载到 files/bootstrap/state/resource-packs/。只对 full 和 pinyin 生效：它们的日文只是附加功能；日文版的主词库就是 msime-japanese.dat，不能省。默认不省略，开发构建照旧全带。
omit_on_demand=${MSIME_ANDROID_OMIT_ON_DEMAND:-0}
verify_flags=()
native_flags=()
if [ "$omit_on_demand" = 1 ]; then
  case "$edition" in
    full|pinyin) ;;
    *) echo "MSIME_ANDROID_OMIT_ON_DEMAND=1 applies to the full and pinyin editions only, not $edition" >&2; exit 1 ;;
  esac
  if [ "${MSIME_REQUIRE_LANGUAGE_DICTIONARIES:-0}" = 1 ]; then
    echo "MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 contradicts MSIME_ANDROID_OMIT_ON_DEMAND=1, which leaves the language dictionaries to download on demand" >&2
    exit 1
  fi
  verify_flags=(--omit-on-demand)
  native_flags=(--omit-voice-runtime)
elif [ "$omit_on_demand" != 0 ]; then
  echo "MSIME_ANDROID_OMIT_ON_DEMAND must be 0 or 1" >&2
  exit 1
fi
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
tools_dir="$android_sdk/build-tools/35.0.0"
# The Gradle host module compiles against API 36. Check the same platform here so a
# partially installed SDK fails before Gradle starts resolving dependencies.
android_jar="$android_sdk/platforms/android-36/android.jar"
[[ -f "$android_jar" && -x "$tools_dir/d8" ]] || { echo "Android API 36 platform and build-tools 35 required" >&2; exit 1; }
# 带 --omit-on-demand 时完整的源目录照样整体校验，输出的只是去掉日文词典那一组后的文件名。
artifacts=$(cargo run --quiet -p msime-client-core --example verify_resources --locked -- ${verify_flags[@]+"${verify_flags[@]}"} "$resource_dir")
# 默认只构建 arm64-v8a：这个包面向的手机都是 arm64，多带一份 x86_64 原生库会让 APK 大约翻倍。MSIME_ANDROID_ABIS（空格或逗号分隔，例如 "arm64-v8a x86_64"）可以为 x86_64 模拟器加上 x86_64；Gradle 的 abiFilters 经 -PmsimeAbis 收到同一份列表，第三方库（ML Kit）也按它过滤。
read -r -a abis <<< "$(tr ',' ' ' <<< "${MSIME_ANDROID_ABIS:-arm64-v8a}")"
[ "${#abis[@]}" -gt 0 ] || { echo "MSIME_ANDROID_ABIS names no ABI" >&2; exit 1; }
for abi in "${abis[@]}"; do
  case "$abi" in
    arm64-v8a|x86_64) ;;
    *) echo "MSIME_ANDROID_ABIS: unsupported ABI $abi (supported: arm64-v8a, x86_64)" >&2; exit 1 ;;
  esac
done
# 瘦包只能是 arm64-v8a 单 ABI：资源包 voice-runtime 只取 .aar 里 arm64-v8a 的两个库（resources/voice-runtime-android.lock.json），x86_64 上下载下来也加载不了，离线识别永远失败。
if [ "$omit_on_demand" = 1 ] && [ "${abis[*]}" != arm64-v8a ]; then
  echo "MSIME_ANDROID_OMIT_ON_DEMAND=1 builds arm64-v8a only (the voice-runtime pack carries arm64-v8a libraries), not MSIME_ANDROID_ABIS=${abis[*]}" >&2
  exit 1
fi
for abi in "${abis[@]}"; do bash platforms/android/build-native.sh "$abi" ${native_flags[@]+"${native_flags[@]}"}; done
abi_list=$(IFS=,; echo "${abis[*]}")

# The host is a Gradle build now: it uses AndroidX and Material, and those ship as AARs whose
# resources have to be merged and whose R classes have to be generated per package. The previous
# aapt2/d8 pipeline had no dependency resolution at all, so every one of those steps would have been
# hand-rolled here. Gradle and AGP are pinned to the versions the Tauri bundle already resolves.
assets="$repo_root/target/android/host-assets"
rm -rf "$assets"
mkdir -p "$assets/dictionary"
# APK 里的资源锁一律叫 desktop-dictionary.lock.json（Bootstrap 按这个名字读），内容是本版本的锁：full 就是 resources/desktop-dictionary.lock.json 本身，其他版本是 scripts/editions.py 生成的 resources/editions/<id>.lock.json。暂存的词库恰好是锁里列的文件，再按本版本的锁校验一遍。
edition_flags=()
if [ "$edition" != full ]; then
  edition_artifacts=$(python3 -c 'import json, sys; print("\n".join(artifact["name"] for artifact in json.load(open(sys.argv[1]))["artifacts"]))' "$edition_lock")
  artifacts=$(grep -Fx -f <(printf '%s\n' "$edition_artifacts") <<< "$artifacts")
  edition_flags=(--edition "$edition")
fi
# Bootstrap 只按这份锁解包，锁里列了而 APK 里没有对应 asset 会直接失败，所以省略日文词典时随包的锁是本版本的锁去掉没暂存的条目，其余字段原样保留。host-api 仍按编译进二进制的完整锁校验，日文那一组整体缺席时放行，用户词库的代次不变。
if [ "$omit_on_demand" = 1 ]; then
  python3 -c 'import json, sys
lock = json.load(open(sys.argv[1]))
shipped = set(sys.stdin.read().split())
lock["artifacts"] = [artifact for artifact in lock["artifacts"] if artifact["name"] in shipped]
if {artifact["name"] for artifact in lock["artifacts"]} != shipped: sys.exit("the staged artifacts are not a subset of " + sys.argv[1])
with open(sys.argv[2], "w") as output: json.dump(lock, output, indent=2); output.write("\n")' "$edition_lock" "$assets/desktop-dictionary.lock.json" <<< "$artifacts"
else
  cp "$edition_lock" "$assets/desktop-dictionary.lock.json"
fi
while IFS= read -r artifact; do cp "$resource_dir/$artifact" "$assets/dictionary/"; done <<< "$artifacts"
cargo run --quiet -p msime-client-core --example verify_resources --locked -- ${verify_flags[@]+"${verify_flags[@]}"} ${edition_flags[@]+"${edition_flags[@]}"} "$assets/dictionary" >/dev/null
mkdir -p "$assets/native-notices"
cp -R target/android/notices/. "$assets/native-notices/"
cp LICENSE "$assets/client-LICENSE.txt"
# 辅助码表不在词库发布里，由仓库自带在 resources/helpcodes。Bootstrap 把它们解到资源目录下的 helpcodes/，Engine 从那里读（crates/engine/src/assets.rs 列出七个文件），安装包每次变化都会替换它们。
mkdir -p "$assets/helpcodes"
for table in helpcode.txt zrm_helpcode_big_unique.txt shouyou2_0_helpcode.txt shouyouplus_helpcode.txt xiaohe_helpcode.txt jiajia_helpcode.txt wubi86_helpcode.txt; do
  cp "resources/helpcodes/$table" "$assets/helpcodes/$table"
done
cp resources/helpcodes/ENGINE-NOTICE.md "$assets/helpcodes/NOTICE.md"
cp resources/helpcodes/NOTICE.md "$assets/helpcodes/NOTICE-jiajia.md"
cp resources/helpcodes/NOTICE-wubi86.md "$assets/helpcodes/NOTICE-wubi86.md"
# 内置按键音包：resources/sound-packs 里 `mode = "keys"` 的包整目录打进 assets/sound-packs（样本连同写着许可的 plugin.toml），Bootstrap 解到 files/sound-packs，键盘经 NativeClient.keySoundPack 校验后播放。旋律包和音乐包 Android 不播，不打包。
mkdir -p "$assets/sound-packs"
for pack_dir in resources/sound-packs/*/; do
  pack=$(basename "$pack_dir")
  grep -qx 'kind = "sound"' "$pack_dir/plugin.toml" || continue
  grep -qx 'mode = "keys"' "$pack_dir/plugin.toml" || continue
  cp -R "${pack_dir%/}" "$assets/sound-packs/$pack"
done
[ -f "$assets/sound-packs/default/plugin.toml" ] || { echo "the default key sound pack was not packaged" >&2; exit 1; }
# Optional non-English candidate glosses (scripts/build_offline_glosses.py). Bootstrap extracts them beside the resources, where the Engine looks for one zh-<lang>.db per target language; without them only English is glossed offline.
glosses_source=${MSIME_OFFLINE_GLOSSES:-$repo_root/target/offline-glosses}
rm -rf "$assets/offline-glosses"
# 它们按中文候选查释义，不提供中文方案的版本（版本表 features.offline_glosses 为 false：日文、越南文和藏文版）不带。
if [ "$edition_offline_glosses" != true ]; then
  echo "edition $edition offers no Chinese scheme; offline glosses are not packaged"
elif [ "$omit_on_demand" = 1 ]; then
  echo "MSIME_ANDROID_OMIT_ON_DEMAND=1: offline glosses are downloaded on demand (resource pack offline-glosses), not packaged"
elif compgen -G "$glosses_source/zh-*.db" >/dev/null && [ -f "$glosses_source/offline-glosses-NOTICE.txt" ]; then
  mkdir -p "$assets/offline-glosses"
  cp "$glosses_source"/zh-*.db "$glosses_source/offline-glosses-NOTICE.txt" "$assets/offline-glosses/"
  echo "offline glosses packaged from $glosses_source"
else
  echo "no offline glosses at $glosses_source; candidates are glossed offline in English only"
fi
# Optional Cantonese, Zhuyin and Stroke dictionaries fetched by scripts/fetch_language_dictionaries.py (or built by `msime-dict-build languages`), as on macOS, iOS and HarmonyOS. Bootstrap extracts them to language-dictionaries/ beside the resources, where host-api finds them and names them in the runtime options; the keyboard and the settings page leave a scheme whose dictionary is missing out. Each dictionary is packaged only with its licence text, which must travel with the data.
languages_source=${MSIME_LANGUAGE_DICTIONARIES:-$repo_root/target/language-dictionaries}
# Each dictionary beside the licence file that must travel with it; the staging below and the APK check at the end read the same list.
language_pairs="msime-cantonese.db:msime-rime_cantonese_LICENSE.txt msime-zhuyin.db:msime-libchewing_data_LICENSE.txt msime-stroke.db:msime-rime_stroke_LICENSE.txt"
rm -rf "$assets/language-dictionaries"
staged_languages=()
# 只带本版本要的语言词库（版本表的 language_dictionaries）：full 是粤拼、注音和笔画三个，其他版本一个也不带。
for pair in $language_pairs; do
  database=${pair%%:*}
  license=${pair#*:}
  [ "$omit_on_demand" != 1 ] || continue
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
if [ "$omit_on_demand" = 1 ]; then
  echo "MSIME_ANDROID_OMIT_ON_DEMAND=1: language dictionaries are downloaded on demand (resource pack language-dictionaries), not packaged"
elif [ "${#staged_languages[@]}" -gt 0 ]; then
  echo "language dictionaries packaged (${staged_languages[*]}) from $languages_source"
elif [ -z "$edition_languages" ]; then
  echo "edition $edition packs no language dictionaries"
else
  echo "no language dictionaries at $languages_source; Cantonese, Zhuyin and Stroke stay unavailable"
fi
# A release requires every dictionary this edition packs (the edition table's language_dictionaries) that resources/language-dictionaries.lock.json pins, not a fixed list: a dictionary that has not been released yet is packaged when present but cannot fail a release, and the lock bump that publishes it makes it required.
if [ "${MSIME_REQUIRE_LANGUAGE_DICTIONARIES:-0}" = 1 ]; then
  required_languages=$(python3 "$repo_root/scripts/fetch_language_dictionaries.py" --list-databases)
  if [ -z "$required_languages" ]; then
    echo "MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 but resources/language-dictionaries.lock.json pins no dictionary" >&2
    exit 1
  fi
  for database in $required_languages; do
    grep -qxF "$database" <<< "$edition_languages" || continue
    if [[ " ${staged_languages[*]:-} " != *" $database "* ]]; then
      echo "MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 but $database, which edition $edition packs and resources/language-dictionaries.lock.json pins, was not packaged from $languages_source" >&2
      exit 1
    fi
  done
fi

gradle_dir="$repo_root/platforms/android/gradle-app"
tauri_gradlew="$repo_root/apps/desktop/src-tauri/gen/android/gradlew"
[[ -x "$tauri_gradlew" ]] || { echo "Gradle wrapper required; run the Tauri Android init once" >&2; exit 1; }
# The APK's versionName comes from version.txt; MSIME_ANDROID_VERSION (the release workflow's version input) overrides it.
version_args=()
[[ -n "${MSIME_ANDROID_VERSION:-}" ]] && version_args+=("-PmsimeVersion=$MSIME_ANDROID_VERSION")
ANDROID_HOME="$android_sdk" "$tauri_gradlew" --project-dir "$gradle_dir" --console=plain "-PmsimeAbis=$abi_list" ${version_args[@]+"${version_args[@]}"} "assemble${flavor}Release"

unsigned="$gradle_dir/app/build/outputs/apk/$edition/release/app-$edition-release-unsigned.apk"
[[ -f "$unsigned" ]] || { echo "Expected host APK not produced" >&2; exit 1; }
# 签名密钥的选择（发布、beta、本地构建都用同一把发布密钥）见 scripts/signing.sh。
source "$repo_root/platforms/android/scripts/signing.sh"
output="$repo_root/target/android/$apk_name.apk"
aligned="$repo_root/target/android/$apk_name-aligned.apk"
"$tools_dir/zipalign" -P 16 4 "$unsigned" "$aligned"
"$tools_dir/apksigner" sign "${android_signing[@]}" --out "$output" "$aligned"
"$tools_dir/apksigner" verify --verbose --print-certs "$output"
"$tools_dir/zipalign" -c -P 16 4 "$output"
# The package is only worth shipping if it carries each dictionary staged above, beside its licence (a release, MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1, has already refused to stage fewer than the lock pins for this edition).
apk_entries=$(unzip -Z1 "$output")
for pair in $language_pairs; do
  [ -f "$assets/language-dictionaries/${pair%%:*}" ] || continue
  for entry in "${pair%%:*}" "${pair#*:}"; do
    grep -qxF "assets/language-dictionaries/$entry" <<< "$apk_entries" || { echo "$output has no assets/language-dictionaries/$entry although it was staged" >&2; exit 1; }
  done
done
# APK 带的原生库必须恰好是上面构建的这些 ABI：少一个的话能装上，但第一次 JNI 调用就崩溃；多一个（某个依赖的 x86 或 armeabi-v7a 副本漏过了 abiFilters）则是白占体积，还会让包装到它根本跑不了的设备上。
packaged_abis=$(sed -n 's|^lib/\([^/]*\)/.*|\1|p' <<< "$apk_entries" | sort -u)
expected_abis=$(printf '%s\n' "${abis[@]}" | sort -u)
if [ "$packaged_abis" != "$expected_abis" ]; then
  echo "$output carries native libraries for ABIs [$(tr '\n' ' ' <<< "$packaged_abis")], expected [$(tr '\n' ' ' <<< "$expected_abis")]" >&2
  exit 1
fi
# 不提供中文方案的版本不带非英文离线释义（见上面的暂存），打出来的 APK 里也不能有。
if [ "$edition_offline_glosses" != true ] && grep -q '^assets/offline-glosses/' <<< "$apk_entries"; then
  echo "$output carries offline glosses, but edition $edition offers no Chinese scheme" >&2
  exit 1
fi
# 省略按需资源的瘦包里不能混进任何一样该下载的东西：日文词典那一组、语言词库、非英文离线释义和两个语音运行库。
if [ "$omit_on_demand" = 1 ]; then
  for pattern in '(^|/)msime-japanese\.dat$' '^assets/dictionary/msime-mozc_' '^assets/language-dictionaries/' '^assets/offline-glosses/' '^lib/[^/]+/libonnxruntime\.so$' '^lib/[^/]+/libsherpa-onnx-c-api\.so$'; do
    if grep -Eq "$pattern" <<< "$apk_entries"; then
      echo "$output carries $(grep -E -m 1 "$pattern" <<< "$apk_entries"), which MSIME_ANDROID_OMIT_ON_DEMAND=1 leaves to download on demand" >&2
      exit 1
    fi
  done
  # 体积上限挡的是整组资源或运行库悄悄回到包里（日文词典约 20 MB、语音运行库约 27 MB）。瘦包只有 arm64-v8a 单 ABI（见上面的 ABI 检查），默认上限按它定，MSIME_ANDROID_MAX_APK_BYTES 可改。
  max_apk_bytes=${MSIME_ANDROID_MAX_APK_BYTES:-130000000}
  [[ "$max_apk_bytes" =~ ^[0-9]+$ ]] || { echo "MSIME_ANDROID_MAX_APK_BYTES must be a byte count" >&2; exit 1; }
  apk_bytes=$(wc -c < "$output" | tr -d '[:space:]')
  if [ "$apk_bytes" -gt "$max_apk_bytes" ]; then
    echo "$output is $apk_bytes bytes, above the $max_apk_bytes-byte ceiling for a package without on-demand resources" >&2
    exit 1
  fi
  echo "APK size $apk_bytes bytes, within the $max_apk_bytes-byte ceiling"
fi
rm -f "$aligned"
echo "APK for edition $edition built and signed with the $android_signed_with: $output; not installed or device-verified"
