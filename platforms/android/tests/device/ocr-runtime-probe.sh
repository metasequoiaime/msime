#!/usr/bin/env bash
# 独立 SDK/资源加载试验，不安装产品 APK，也不代替相机与真实编辑器验收。
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
serial=${1:?usage: ocr-runtime-probe.sh <emulator-serial>}
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
[[ -x "$adb" ]] || { echo "Android platform-tools required" >&2; exit 1; }
# 模块带 arm64-v8a 与 x86_64 两套原生库，两种专用 AVD 都能跑。
abi=$("$adb" -s "$serial" shell getprop ro.product.cpu.abi | tr -d '\r')
[[ "$abi" == x86_64 || "$abi" == arm64-v8a ]] || {
  echo "This isolation probe requires an arm64-v8a or x86_64 test AVD, not $abi" >&2; exit 1;
}
gradlew=apps/desktop/src-tauri/gen/android/gradlew
gradle_project=platforms/android/gradle-app
mkdir -p target/android/ocr-probe-assets
"$gradlew" --project-dir "$gradle_project" --console=plain -PmsimeOcrProbe=true \
  :ocr-module:assembleDebug :ocr-module:lintDebug
"$gradlew" --project-dir "$gradle_project" --console=plain -PmsimeOcrProbe=true \
  :ocr-module:dependencies --configuration debugRuntimeClasspath > target/android/ocr-module-dependencies.txt
cp "$gradle_project/ocr-module/build/outputs/apk/debug/ocr-module-debug.apk" \
  target/android/ocr-probe-assets/ocr-module.apk
"$gradlew" --project-dir "$gradle_project" --console=plain -PmsimeOcrProbe=true \
  :ocr-probe:assembleDebug :ocr-probe:lintDebug
"$adb" -s "$serial" install -r "$gradle_project/ocr-probe/build/outputs/apk/debug/ocr-probe-debug.apk"
"$adb" -s "$serial" shell pm clear app.msime.ocr.probe
result=$("$adb" -s "$serial" shell am instrument -w app.msime.ocr.probe/app.msime.ocr.probe.OcrProbe)
printf '%s\n' "$result" | tee target/android/ocr-runtime-probe.log
"$adb" -s "$serial" exec-out run-as app.msime.ocr.probe cat files/ocr-synthetic-result.json \
  > target/android/ocr-synthetic-isolated.json
# 独立安装模块，用同一夹具核对普通加载方式，避免把模型误识别当作动态加载缺陷。
"$adb" -s "$serial" install -r "$gradle_project/ocr-module/build/outputs/apk/debug/ocr-module-debug.apk"
"$adb" -s "$serial" shell pm clear app.msime.ocr.module
direct=$("$adb" -s "$serial" shell am instrument -w app.msime.ocr.module/app.msime.ocr.module.DirectOcrProbe)
printf '%s\n' "$direct" | tee target/android/ocr-direct-probe.log
"$adb" -s "$serial" exec-out run-as app.msime.ocr.module cat files/ocr-synthetic-result.json \
  > target/android/ocr-synthetic-direct.json
# 0–2 是三种字号的中英文合成图，3 是竖排两列。
for index in 0 1 2 3; do
  "$adb" -s "$serial" exec-out run-as app.msime.ocr.probe cat "files/ocr-synthetic-$index.png" \
    > "target/android/ocr-synthetic-isolated-$index.png"
  "$adb" -s "$serial" exec-out run-as app.msime.ocr.module cat "files/ocr-synthetic-$index.png" \
    > "target/android/ocr-synthetic-direct-$index.png"
  cmp "target/android/ocr-synthetic-isolated-$index.png" "target/android/ocr-synthetic-direct-$index.png"
done
cmp target/android/ocr-synthetic-isolated.json target/android/ocr-synthetic-direct.json
echo 'Synthetic pixels and all recognition results match the directly installed module baseline'
[[ "$result" == *MSIME_OCR_PROBE_PASSED* && "$result" != *MSIME_OCR_PROBE_FAILED* ]]
[[ "$direct" == *MSIME_OCR_DIRECT_PASSED* && "$direct" != *MSIME_OCR_DIRECT_FAILED* ]]
