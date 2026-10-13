#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
resource_dir=${1:?usage: build-app.sh <verified-resource-directory> [device|simulator|testflight]}
variant=${2:-simulator}
# testflight 的准备步骤与 device 相同，最后一步改为签名归档并上传 App Store Connect（upload-testflight.sh）。
native_variant=$variant
case "$variant" in
  device) tauri_target=aarch64 ;;
  simulator) tauri_target=aarch64-sim ;;
  testflight) tauri_target=aarch64; native_variant=device ;;
  *) echo "Unsupported iOS variant: $variant" >&2; exit 2 ;;
esac

# 完整开发包预取附加词库，瘦包由主应用按用户操作下载安装。
if [ "${MSIME_IOS_OMIT_ON_DEMAND:-0}" != 1 ]; then
  python3 "$repo_root/scripts/fetch_language_dictionaries.py" >/dev/null
fi
bash "$repo_root/platforms/ios/stage-resources.sh" "$resource_dir"
bash "$repo_root/platforms/ios/build-native.sh" "$native_variant"

# iOS 的产品本体是 platforms/ios 下的原生宿主 MSIMEApp：装机、启动、被系统识别为输入法的都是它，所以它是这个脚本的默认产物。Tauri/React 是它承载的公共组件，不是 iOS 的产品本体；只在需要单独构建那部分时用 MSIME_IOS_TAURI_COMPONENT=1 显式选择，不拿它作为 iOS 的产品去启动或验收。
if [ "${MSIME_IOS_TAURI_COMPONENT:-0}" = 1 ]; then
  if [ "$variant" = device ]; then
    command -v pod >/dev/null || { echo "CocoaPods is required for the device handwriting build" >&2; exit 1; }
    (cd "$repo_root/apps/desktop/src-tauri/gen/apple" && pod install --deployment)
  fi
  (cd "$repo_root" && pnpm --filter @msime/desktop tauri ios build \
    --target "$tauri_target" --no-sign --ci)
  exit 0
fi

sdk=$([ "$native_variant" = device ] && echo iphoneos || echo iphonesimulator)
# MSIMEApp 嵌入的 sherpa-onnx 运行时（本地语音识别）：按 resources/voice-runtime.lock.json 下载并校验到 target/voice-runtime/ios，已有时直接复用。
python3 "$repo_root/scripts/fetch_voice_runtime.py" --platform ios
(cd "$repo_root/platforms/ios" && xcodegen generate -s project.yml -p .)
build_container=(-project "$repo_root/platforms/ios/MSIMEClient.xcodeproj")
if [ "$native_variant" = device ]; then
  command -v pod >/dev/null || { echo "CocoaPods is required for the device handwriting build" >&2; exit 1; }
  (cd "$repo_root/platforms/ios" && pod install --deployment)
  build_container=(-workspace "$repo_root/platforms/ios/MSIMEClient.xcworkspace")
fi
if [ "$variant" = testflight ]; then
  exec bash "$repo_root/platforms/ios/upload-testflight.sh"
fi
xcodebuild "${build_container[@]}" \
  -scheme MSIMEApp -sdk "$sdk" -configuration Release \
  -derivedDataPath "$repo_root/target/ios/derived-$variant" \
  CODE_SIGNING_ALLOWED=NO ARCHS=arm64 ONLY_ACTIVE_ARCH=YES build

# 完整包核对扩展内每份已暂存的语言词库及许可证。
staged_languages="$repo_root/target/ios/language-dictionaries"
keyboard_languages="$repo_root/target/ios/derived-$variant/Build/Products/Release-$sdk/MSIMEApp.app/PlugIns/MSIMEKeyboardExtension.appex/language-dictionaries"
for pair in msime-cantonese.db:msime-rime_cantonese_LICENSE.txt msime-zhuyin.db:msime-libchewing_data_LICENSE.txt msime-stroke.db:msime-rime_stroke_LICENSE.txt; do
  [ -f "$staged_languages/${pair%%:*}" ] || continue
  if [ ! -s "$keyboard_languages/${pair%%:*}" ] || [ ! -f "$keyboard_languages/${pair#*:}" ]; then
    echo "$keyboard_languages lacks ${pair%%:*} or ${pair#*:} although both were staged" >&2
    exit 1
  fi
done

# 验收最终内嵌扩展，防止旧构建资源残留让瘦包继续带附加词库。
if [ "${MSIME_IOS_OMIT_ON_DEMAND:-0}" = 1 ]; then
  keyboard_resources="${keyboard_languages%/language-dictionaries}/EngineResources"
  for name in msime-japanese.dat msime-mozc_LICENSE.txt msime-mozc_dictionary_oss_README.txt; do
    [ ! -e "$keyboard_resources/$name" ] || { echo "slim iOS bundle still contains $name" >&2; exit 1; }
  done
  for name in msime-cantonese.db msime-zhuyin.db msime-stroke.db; do
    [ ! -e "$keyboard_languages/$name" ] || { echo "slim iOS bundle still contains $name" >&2; exit 1; }
  done
fi
