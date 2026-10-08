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
"$tools_dir/aapt2" link -I "$android_jar" --manifest platforms/android/tests/device/AndroidManifest.xml -o "$build_dir/unsigned.apk"
(cd "$build_dir/dex" && zip -q -0 "$build_dir/unsigned.apk" classes.dex)
"$tools_dir/zipalign" -P 16 4 "$build_dir/unsigned.apk" "$build_dir/aligned.apk"
# Instrumentation that targets app.msime.android only runs when this package carries the app's certificate, so it signs with the same key (scripts/signing.sh).
source "$repo_root/platforms/android/scripts/signing.sh"
"$tools_dir/apksigner" sign "${android_signing[@]}" --out "$build_dir/signed.apk" "$build_dir/aligned.apk"
"$tools_dir/apksigner" verify "$build_dir/signed.apk"
cp "$build_dir/signed.apk" target/android/editor-test.apk
echo "Synthetic editor APK built; no device changed"
