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
javac --release 17 -Xlint:all -Werror -cp "$android_jar" -d "$build_dir/classes" platforms/android/tests/device/core/EditorActivity.java platforms/android/tests/device/core/DeviceSmoke.java platforms/android/tests/device/candidate/CandidatePanelDeviceSmoke.java platforms/android/tests/device/keyboard/MoreToolsDeviceSmoke.java platforms/android/tests/device/keyboard/EmojiPickerDeviceSmoke.java platforms/android/tests/device/keyboard/ChineseHelpcodeDeviceSmoke.java platforms/android/tests/device/keyboard/MicrosoftShuangpinDeviceSmoke.java platforms/android/tests/device/voice/HandwritingDeviceSmoke.java platforms/android/tests/device/settings/PreferencesDeviceSmoke.java platforms/android/tests/device/keyboard/KeyboardHeightDeviceSmoke.java platforms/android/tests/device/keyboard/FuzzyPinyinDeviceSmoke.java platforms/android/tests/device/candidate/CandidateGlossDeviceSmoke.java platforms/android/tests/device/keyboard/NineKeyEnglishDeviceSmoke.java platforms/android/tests/device/settings/SettingsDeviceSmoke.java platforms/android/tests/device/settings/SettingsLifecycleSmoke.java platforms/android/tests/device/voice/TypingStatisticsDeviceSmoke.java platforms/android/tests/device/settings/AccountStorageDeviceSmoke.java platforms/android/tests/device/settings/BackendAccountRefreshDeviceSmoke.java platforms/android/java/app/msime/android/account/BackendAccount.java platforms/android/java/app/msime/android/account/AccountTokenPolicy.java platforms/android/java/app/msime/android/TextPolicy.java platforms/android/java/app/msime/android/clipboard/CloudClipboardTextPolicy.java platforms/android/java/app/msime/android/core/AndroidAccountSessionStorage.java platforms/android/java/app/msime/android/keyboard/ChineseSymbolFaces.java platforms/android/java/app/msime/android/core/WindowLayout.java
jar --create --file "$build_dir/classes.jar" -C "$build_dir/classes" .
"$tools_dir/d8" --release --min-api 28 --lib "$android_jar" --output "$build_dir/dex" "$build_dir/classes.jar"
"$tools_dir/aapt2" link -I "$android_jar" --manifest platforms/android/tests/device/AndroidManifest.xml -o "$build_dir/unsigned.apk"
(cd "$build_dir/dex" && zip -q -0 "$build_dir/unsigned.apk" classes.dex)
"$tools_dir/zipalign" -P 16 4 "$build_dir/unsigned.apk" "$build_dir/aligned.apk"
keystore="$repo_root/target/android/development.keystore"
if [[ ! -f "$keystore" ]]; then
  keytool -genkeypair -keystore "$keystore" -storepass android -keypass android \
    -alias androiddebugkey -dname "CN=MSIME Development" -keyalg RSA -keysize 2048 -validity 3650
fi
"$tools_dir/apksigner" sign --ks "$keystore" --ks-key-alias androiddebugkey \
  --ks-pass pass:android --key-pass pass:android --out "$build_dir/signed.apk" "$build_dir/aligned.apk"
"$tools_dir/apksigner" verify "$build_dir/signed.apk"
cp "$build_dir/signed.apk" target/android/editor-test.apk
echo "Synthetic editor APK built; no device changed"
