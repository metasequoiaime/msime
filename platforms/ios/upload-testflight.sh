#!/usr/bin/env bash
# 把 build-app.sh testflight 准备好的 MSIMEApp（资源、真机原生库、语音运行时、xcodegen 工程和 CocoaPods 都已就绪）签名归档、导出 ipa 并上传 App Store Connect。由 build-app.sh 在 testflight 模式下调用，不单独运行。
#
# 签名用 App Store 发布证书（调用方已导入钥匙串）和两份描述文件：App（app.msime.ios）与键盘扩展（app.msime.ios.keyboard）。描述文件名通过 MSIME_IOS_APP_PROFILE、MSIME_IOS_KEYBOARD_PROFILE 传给 project.yml 里对应 target 的 PROVISIONING_PROFILE_SPECIFIER。
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
: "${MSIME_IOS_TEAM_ID:?MSIME_IOS_TEAM_ID is required}"
: "${MSIME_IOS_AUTH_KEY_ID:?MSIME_IOS_AUTH_KEY_ID is required}"
: "${MSIME_IOS_AUTH_KEY_ISSUER_ID:?MSIME_IOS_AUTH_KEY_ISSUER_ID is required}"
: "${MSIME_IOS_AUTH_KEY_PATH:?MSIME_IOS_AUTH_KEY_PATH is required}"
: "${MSIME_IOS_APP_PROFILE_PATH:?MSIME_IOS_APP_PROFILE_PATH is required}"
: "${MSIME_IOS_KEYBOARD_PROFILE_PATH:?MSIME_IOS_KEYBOARD_PROFILE_PATH is required}"
: "${MSIME_IOS_VERSION:?MSIME_IOS_VERSION is required}"
: "${MSIME_IOS_BUILD_NUMBER:?MSIME_IOS_BUILD_NUMBER is required}"
[[ "$MSIME_IOS_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "Invalid version: $MSIME_IOS_VERSION" >&2; exit 1; }
[[ "$MSIME_IOS_BUILD_NUMBER" =~ ^[1-9][0-9]{0,8}$ ]] || { echo "Invalid build number: $MSIME_IOS_BUILD_NUMBER" >&2; exit 1; }
for file in "$MSIME_IOS_AUTH_KEY_PATH" "$MSIME_IOS_APP_PROFILE_PATH" "$MSIME_IOS_KEYBOARD_PROFILE_PATH"; do
  [[ -s "$file" ]] || { echo "Missing $file" >&2; exit 1; }
done
workspace="$repo_root/platforms/ios/MSIMEClient.xcworkspace"
[[ -d "$workspace" ]] || { echo "Missing $workspace; run build-app.sh testflight, which installs the pods" >&2; exit 1; }

build_root="$repo_root/target/ios/testflight"
archive_path="$build_root/MSIMEApp.xcarchive"
export_path="$build_root/export"
rm -rf -- "$build_root"
mkdir -p "$export_path"

# xcodebuild 只在这个目录里按名字找描述文件。
profiles_dir="$HOME/Library/MobileDevice/Provisioning Profiles"
mkdir -p "$profiles_dir"
profile_field() { security cms -D -i "$1" | plutil -extract "$2" raw -o - -; }
for profile in "$MSIME_IOS_APP_PROFILE_PATH" "$MSIME_IOS_KEYBOARD_PROFILE_PATH"; do
  cp "$profile" "$profiles_dir/$(profile_field "$profile" UUID).mobileprovision"
done
app_profile=$(profile_field "$MSIME_IOS_APP_PROFILE_PATH" Name)
keyboard_profile=$(profile_field "$MSIME_IOS_KEYBOARD_PROFILE_PATH" Name)

# CODE_SIGN_STYLE=Manual 与证书在命令行上对所有 target 生效；描述文件只由 App 与键盘两个 target 的 PROVISIONING_PROFILE_SPECIFIER 引用，Pods 的静态库不签名。
xcodebuild archive \
  -workspace "$workspace" \
  -scheme MSIMEApp \
  -configuration Release \
  -destination 'generic/platform=iOS' \
  -archivePath "$archive_path" \
  -derivedDataPath "$repo_root/target/ios/derived-testflight" \
  MARKETING_VERSION="$MSIME_IOS_VERSION" \
  CURRENT_PROJECT_VERSION="$MSIME_IOS_BUILD_NUMBER" \
  CODE_SIGN_STYLE=Manual \
  CODE_SIGN_IDENTITY="Apple Distribution" \
  DEVELOPMENT_TEAM="$MSIME_IOS_TEAM_ID" \
  MSIME_IOS_APP_PROFILE="$app_profile" \
  MSIME_IOS_KEYBOARD_PROFILE="$keyboard_profile"

app="$archive_path/Products/Applications/MSIMEApp.app"
keyboard="$app/PlugIns/MSIMEKeyboardExtension.appex"
[[ -d "$app" && -d "$keyboard" ]] || { echo "The archive lacks MSIMEApp.app or its keyboard extension" >&2; exit 1; }
for bundle in "$app" "$keyboard"; do
  test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$bundle/Info.plist")" = "$MSIME_IOS_BUILD_NUMBER"
  test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$bundle/Info.plist")" = "$MSIME_IOS_VERSION"
done
# 键盘扩展读粤语和注音词库；发布包必须带着它们（与 build-app.sh 对 simulator/device 产物的检查相同）。
for pair in cantonese.db:rime_cantonese_LICENSE.txt zhuyin.db:libchewing_data_LICENSE.txt; do
  [[ -f "$repo_root/target/ios/language-dictionaries/${pair%%:*}" ]] || continue
  [[ -s "$keyboard/language-dictionaries/${pair%%:*}" && -f "$keyboard/language-dictionaries/${pair#*:}" ]] \
    || { echo "The archived keyboard extension lacks ${pair%%:*} or ${pair#*:}" >&2; exit 1; }
done

cat > "$build_root/ExportOptions.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>method</key>
    <string>app-store-connect</string>
    <key>signingStyle</key>
    <string>manual</string>
    <key>teamID</key>
    <string>$MSIME_IOS_TEAM_ID</string>
    <key>provisioningProfiles</key>
    <dict>
        <key>app.msime.ios</key>
        <string>$app_profile</string>
        <key>app.msime.ios.keyboard</key>
        <string>$keyboard_profile</string>
    </dict>
    <key>uploadSymbols</key>
    <true/>
</dict>
</plist>
EOF
xcodebuild -exportArchive \
  -archivePath "$archive_path" \
  -exportPath "$export_path" \
  -exportOptionsPlist "$build_root/ExportOptions.plist"
ipa=$(find "$export_path" -maxdepth 1 -type f -name '*.ipa' -print -quit)
[[ -n "$ipa" ]] || { echo "Xcode export produced no IPA" >&2; exit 1; }

# altool 只从 API_PRIVATE_KEYS_DIR 里按 AuthKey_<id>.p8 找密钥。
keys_dir="$build_root/private_keys"
mkdir -p "$keys_dir"
chmod 700 "$keys_dir"
trap 'rm -rf -- "$keys_dir"' EXIT
install -m 600 "$MSIME_IOS_AUTH_KEY_PATH" "$keys_dir/AuthKey_$MSIME_IOS_AUTH_KEY_ID.p8"
API_PRIVATE_KEYS_DIR="$keys_dir" xcrun altool --upload-app --file "$ipa" --type ios \
  --apiKey "$MSIME_IOS_AUTH_KEY_ID" --apiIssuer "$MSIME_IOS_AUTH_KEY_ISSUER_ID"
echo "Uploaded MSIMEApp $MSIME_IOS_VERSION ($MSIME_IOS_BUILD_NUMBER) to App Store Connect"
