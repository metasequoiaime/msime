#!/usr/bin/env bash
# 原生产品专项：只在专用 AVD 中运行，临时锁固定当次构建的合成模块，不发布模块。
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
serial=${1:?usage: ocr-product-smoke.sh <dedicated-emulator-serial>}
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
[[ "$serial" == emulator-* ]] || { echo 'Dedicated AVD required' >&2; exit 1; }
avd=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd" == msime-client-test || "$avd" == msime-client-test-api28 ]] || { echo 'Unexpected AVD' >&2; exit 1; }
[[ $("$adb" -s "$serial" shell getprop ro.product.cpu.abi | tr -d '\r') == x86_64 ]] || exit 1
mkdir -p target/android
lock=resources/ocr-module-android.lock.json
backup=target/android/ocr-product-production-lock.json
cp "$lock" "$backup"
restore() {
  cp "$backup" "$lock"
  "$adb" -s "$serial" shell svc wifi enable >/dev/null 2>&1 || true
  "$adb" -s "$serial" shell svc data enable >/dev/null 2>&1 || true
}
trap restore EXIT
gradlew=apps/desktop/src-tauri/gen/android/gradlew
"$gradlew" --project-dir platforms/android/gradle-app --console=plain -PmsimeOcrModule=true \
  :ocr-module:assembleRelease :ocr-module:lintRelease
"$gradlew" --project-dir platforms/android/gradle-app --console=plain -PmsimeOcrModule=true \
  :ocr-module:dependencies --configuration releaseRuntimeClasspath > target/android/ocr-product-dependencies.txt
cp platforms/android/gradle-app/ocr-module/build/outputs/apk/release/ocr-module-release-unsigned.apk \
  target/android/ocr-product-module.apk
# 共享层整套单测读取正式锁；合成地址只属于后续产品专项，不改变发布 URL 契约。
core_status=0
MSIME_ANDROID_TEST_SERIAL="$serial" bash platforms/android/tests/device/build-core-test.sh x86_64 2>&1 | tee target/android/ocr-product-core.log || core_status=$?
python3 - <<'PY'
import hashlib, json, subprocess
from pathlib import Path
path = Path('resources/ocr-module-android.lock.json')
lock = json.loads(path.read_text())
module = Path('target/android/ocr-product-module.apk')
lock['source_commit'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
lock['artifact'] = {
    'name': 'msime-ocr-v1.apk',
    # 保留域名只标识合成测试；OCR 的共享安装入口拒绝下载，不会访问该地址。
    'url': 'https://example.test/ocr-test-only.apk',
    'size': module.stat().st_size,
    'sha256': hashlib.sha256(module.read_bytes()).hexdigest(),
}
path.write_text(json.dumps(lock, indent=2) + '\n')
Path('target/android/ocr-product-test-lock.json').write_text(path.read_text())
PY
# x86_64 验收使用普通开发包；发布瘦包仅支持 arm64，不能绕过产品的 ABI 限制。
python3 scripts/fetch_language_dictionaries.py
python3 scripts/fetch_wordbooks.py
bash platforms/windows/bootstrap-vcpkg.sh
resource_dir=$(cargo run --quiet -p msime-client-core --example install_resources --locked -- target/resources)
MSIME_ANDROID_ABIS=x86_64 MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 MSIME_REQUIRE_WORDBOOKS=1 \
  bash platforms/android/build-apk.sh "$resource_dir"
api=$("$adb" -s "$serial" shell getprop ro.build.version.sdk | tr -d '\r')
baseline_status=0
bash platforms/android/tests/device/smoke.sh "$serial" --core 2>&1 | tee target/android/ocr-product-core-smoke.log || baseline_status=$?
printf 'core=%s baseline=%s\n' "$core_status" "$baseline_status" > target/android/ocr-product-baseline-status.txt
MSIME_ANDROID_OCR_SMOKE=1 bash platforms/android/tests/device/build-editor.sh
"$adb" -s "$serial" install -r target/android/editor-test.apk
# release APK 不提供 run-as；专项在目标进程内清理自己先前生成的固定模块。
"$adb" -s "$serial" shell svc wifi disable
"$adb" -s "$serial" shell svc data disable
instrument_options=()
if (( api >= 31 )); then
  "$adb" -s "$serial" shell am start -W -n app.msime.android/app.msime.android.home.HomeActivity >/dev/null
  instrument_options=(--no-restart)
fi
result=$("$adb" -s "$serial" shell am instrument "${instrument_options[@]}" -w app.msime.android.test/app.msime.android.test.OcrProductDeviceSmoke)
printf '%s\n' "$result" | tee target/android/ocr-product-smoke.log
product_status=0
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* && "$result" != *MSIME_DEVICE_SMOKE_FAILED* ]] || product_status=1
# 运行时撤权会杀死目标进程，另起专项验证撤权后的原生入口，不让 instrumentation 自杀。
# 撤权专项独立运行，前一个用例失败也保留其结果，不让串行断言掩盖权限覆盖。
"$adb" -s "$serial" shell pm grant app.msime.android android.permission.CAMERA
"$adb" -s "$serial" shell pm revoke app.msime.android android.permission.CAMERA
if (( api >= 31 )); then
  "$adb" -s "$serial" shell am start -W -n app.msime.android/app.msime.android.home.HomeActivity >/dev/null
fi
denied=$("$adb" -s "$serial" shell am instrument "${instrument_options[@]}" -w -e ocr-permission-denied true \
  app.msime.android.test/app.msime.android.test.OcrProductDeviceSmoke)
printf '%s\n' "$denied" | tee target/android/ocr-product-permission.log
[[ "$denied" == *MSIME_DEVICE_SMOKE_PASSED* && "$denied" != *MSIME_DEVICE_SMOKE_FAILED* ]] || product_status=1
"$adb" -s "$serial" shell svc wifi enable
"$adb" -s "$serial" shell svc data enable
for frame in captured preview result editor selection hidden resumed permission failure; do
  "$adb" -s "$serial" pull "/sdcard/Android/data/app.msime.android/files/ocr-product-$frame.png" \
    "target/android/ocr-product-$frame.png" >/dev/null 2>&1 || true
done
[[ "$product_status" == 0 ]]

if [[ "$api" == 35 ]]; then
  bash platforms/android/tests/device/smoke.sh "$serial" || baseline_status=$?
fi
printf 'core=%s baseline=%s\n' "$core_status" "$baseline_status" | tee target/android/ocr-product-baseline-status.txt
[[ "$core_status" == 0 && "$baseline_status" == 0 ]]
