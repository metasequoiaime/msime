#!/usr/bin/env bash
set -euo pipefail
umask 077
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
tools_dir="$android_sdk/build-tools/35.0.0"
android_jar="$android_sdk/platforms/android-35/android.jar"
mkdir -p "$repo_root/target/android"
build_dir=$(mktemp -d "$repo_root/target/android/editor-build.XXXXXX")
mkdir -p "$build_dir/classes" "$build_dir/dex"
sources=()
while IFS= read -r source; do
  [[ -z $source || $source == \#* ]] || sources+=("$source")
done < platforms/android/tests/device/editor-sources.txt
javac --release 17 -Xlint:all -Werror -cp "$android_jar" -d "$build_dir/classes" "${sources[@]}"
jar --create --file "$build_dir/classes.jar" -C "$build_dir/classes" .
"$tools_dir/d8" --release --min-api 28 --lib "$android_jar" --output "$build_dir/dex" "$build_dir/classes.jar"
# 设备冒烟读的共享用例（SessionStoreDeviceSmoke 的 android_v2 会话）打进测试包的 assets，设备上没有仓库。
mkdir -p "$build_dir/assets/contracts/account-session-layouts"
cp shared/contracts/account-session-layouts/cases.json "$build_dir/assets/contracts/account-session-layouts/cases.json"
editor_manifest=platforms/android/tests/device/AndroidManifest.xml
# 世界时钟专项复用一个产品宿主入口，避免超过 API 35 的 100 项 instrumentation 上限。
if [[ "${MSIME_ANDROID_WORLD_CLOCK:-0}" == 1 ]]; then
  editor_manifest="$build_dir/world-clock-manifest.xml"
  sed 's/app.msime.android.test.CandidateCanvasDeviceSmoke/app.msime.android.test.WorldClockDeviceSmoke/' \
    platforms/android/tests/device/AndroidManifest.xml > "$editor_manifest"
fi
# OCR 专项只装入固定合成模块，不改变产品 APK 的信任入口。
if [[ "${MSIME_ANDROID_OCR_SMOKE:-0}" == 1 ]]; then
  cp target/android/ocr-product-module.apk "$build_dir/assets/ocr-product-module.apk"
  sed 's/app.msime.android.test.CandidateCanvasDeviceSmoke/app.msime.android.test.OcrProductDeviceSmoke/' \
    "$editor_manifest" > "$build_dir/ocr-manifest.xml"
  editor_manifest="$build_dir/ocr-manifest.xml"
fi
# 完整上游皮肤仅用于显式验收；固定提交归档，不能复制检出目录里的未提交资源，也不进入默认测试包。
if [[ -n "${MSIME_ANDROID_UPSTREAM_SKINS:-}" ]]; then
  upstream_commit=dd018f79858314ed542a47c528608752b13bd261
  mkdir -p "$build_dir/assets/upstream-candidate-skins"
  git -C "$MSIME_ANDROID_UPSTREAM_SKINS" archive "$upstream_commit" skins/green_leaf skins/sakura-night \
    | tar -xf - -C "$build_dir/assets/upstream-candidate-skins" --strip-components=1
  # Android 35 每个清单最多 100 项 instrumentation；专项替换一个入口，不增加默认清单的数量。
  editor_manifest="$build_dir/upstream-manifest.xml"
  sed 's/app.msime.android.test.CandidateCanvasDeviceSmoke/app.msime.android.test.CandidateUpstreamSkinDeviceSmoke/' \
    platforms/android/tests/device/AndroidManifest.xml > "$editor_manifest"
fi
# 联系人专项复用一个原生宿主入口，避免超过 Android 35 的 100 项限制。
if [[ "${MSIME_ANDROID_CONTACTS_SMOKE:-0}" == 1 || "${MSIME_ANDROID_CONTACTS_SMOKE:-0}" == pinyin ]]; then
  contacts_target=app.msime.android
  if [[ "$MSIME_ANDROID_CONTACTS_SMOKE" == pinyin ]]; then contacts_target=app.msime.android.pinyin; fi
  contacts_manifest="$build_dir/contacts-manifest.xml"
  sed "s/app.msime.android.test.CandidateCanvasDeviceSmoke\" android:targetPackage=\"app.msime.android\"/app.msime.android.test.ContactsDeviceSmoke\" android:targetPackage=\"$contacts_target\"/" \
    "$editor_manifest" > "$contacts_manifest"
  editor_manifest="$contacts_manifest"
fi
if [[ "${MSIME_ANDROID_INLINE_AUTOFILL:-0}" == 1 ]]; then
  # 专项复用一个入口，保持清单在系统允许的 100 项以内。
  sed 's/app.msime.android.test.DeviceSmoke/app.msime.android.test.InlineAutofillDeviceSmoke/' \
    "$editor_manifest" > "$build_dir/autofill-manifest.xml"
  editor_manifest="$build_dir/autofill-manifest.xml"
fi
if [[ "${MSIME_ANDROID_CLIPBOARD_CATEGORIES_SMOKE:-0}" == 1 ]]; then
  # 分类专项复用一个原生宿主入口，不突破系统 instrumentation 数量上限。
  sed 's/app.msime.android.test.CandidateCanvasDeviceSmoke/app.msime.android.test.ClipboardCategoriesDeviceSmoke/' \
    "$editor_manifest" > "$build_dir/clipboard-categories-manifest.xml"
  editor_manifest="$build_dir/clipboard-categories-manifest.xml"
fi
if [[ "${MSIME_ANDROID_RELATIVE_DATE:-0}" == 1 ]]; then
  # 专项复用现有入口，避免超过系统清单项数限制。
  sed 's/app.msime.android.test.CandidateCanvasDeviceSmoke/app.msime.android.test.RelativeDateDeviceSmoke/' \
    "$editor_manifest" > "$build_dir/relative-date-manifest.xml"
  editor_manifest="$build_dir/relative-date-manifest.xml"
fi
"$tools_dir/aapt2" compile --dir platforms/android/tests/device/res -o "$build_dir/resources.zip"
"$tools_dir/aapt2" link -I "$android_jar" --manifest "$editor_manifest" -A "$build_dir/assets" \
  -R "$build_dir/resources.zip" -o "$build_dir/unsigned.apk"
(cd "$build_dir/dex" && zip -q -0 "$build_dir/unsigned.apk" classes.dex)
"$tools_dir/zipalign" -P 16 4 "$build_dir/unsigned.apk" "$build_dir/aligned.apk"
# Instrumentation that targets app.msime.android only runs when this package carries the app's certificate, so it signs with the same key (scripts/signing.sh).
source "$repo_root/platforms/android/scripts/signing.sh"
"$tools_dir/apksigner" sign "${android_signing[@]}" --out "$build_dir/signed.apk" "$build_dir/aligned.apk"
"$tools_dir/apksigner" verify "$build_dir/signed.apk"
cp "$build_dir/signed.apk" target/android/editor-test.apk
# 首帧回归需要能独立强停的编辑器；复用合成控件的 dex，另用专用包名。
"$tools_dir/aapt2" link -I "$android_jar" --manifest platforms/android/tests/device/InitialKeyboardEditorManifest.xml -o "$build_dir/first-frame-unsigned.apk"
(cd "$build_dir/dex" && zip -q -0 "$build_dir/first-frame-unsigned.apk" classes.dex)
"$tools_dir/zipalign" -P 16 4 "$build_dir/first-frame-unsigned.apk" "$build_dir/first-frame-aligned.apk"
"$tools_dir/apksigner" sign "${android_signing[@]}" --out target/android/first-frame-editor.apk "$build_dir/first-frame-aligned.apk"
"$tools_dir/apksigner" verify target/android/first-frame-editor.apk
echo "Synthetic editor APKs built; no device changed"

# 应用兼容专项只在显式请求时生成；无网络权限的合成搜索框使用被测应用包名。
if [[ "${MSIME_ANDROID_APP_COMPOSITION:-0}" == 1 ]]; then
  "$tools_dir/aapt2" link -I "$android_jar" --manifest platforms/android/tests/device/CompositionSearchEditorManifest.xml -o "$build_dir/search-unsigned.apk"
  (cd "$build_dir/dex" && zip -q -0 "$build_dir/search-unsigned.apk" classes.dex)
  "$tools_dir/zipalign" -P 16 4 "$build_dir/search-unsigned.apk" "$build_dir/search-aligned.apk"
  "$tools_dir/apksigner" sign "${android_signing[@]}" --out target/android/composition-search-editor.apk "$build_dir/search-aligned.apk"
  "$tools_dir/apksigner" verify target/android/composition-search-editor.apk
fi
