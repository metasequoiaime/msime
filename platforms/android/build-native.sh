#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
abi=${1:-arm64-v8a}
# 第二个参数 --omit-voice-runtime 由 build-apk.sh 在 MSIME_ANDROID_OMIT_ON_DEMAND=1 时传入：本地语音运行库（libsherpa-onnx-c-api.so 与 libonnxruntime.so）不进 jniLibs，由应用在用户打开离线识别时作为资源包 voice-runtime 下载（固定在 resources/voice-runtime-android.lock.json）。不带这个参数时照旧取出这两个库，build-client-apk.sh 的 Tauri 合包靠的就是这条默认路径。
include_voice_runtime=1
case "${2:-}" in
  '') ;;
  --omit-voice-runtime) include_voice_runtime=0 ;;
  *) echo "usage: build-native.sh [arm64-v8a|x86_64] [--omit-voice-runtime]" >&2; exit 1 ;;
esac
case "$abi" in
  arm64-v8a) rust_target=aarch64-linux-android; compiler_target=aarch64-linux-android; triplet=arm64-msime-android; cargo_linker=CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER ;;
  x86_64) rust_target=x86_64-linux-android; compiler_target=x86_64-linux-android; triplet=x64-msime-android; cargo_linker=CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER ;;
  *) echo "Supported ABIs: arm64-v8a, x86_64" >&2; exit 1 ;;
esac
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
ndk=${MSIME_ANDROID_NDK:-${android_sdk}/ndk/28.2.13676358}
vcpkg_root=${MSIME_VCPKG_ROOT:-$repo_root/target/tooling/vcpkg}
if [[ ! -f "$ndk/source.properties" ]] || ! grep -q '28.2.13676358' "$ndk/source.properties"; then
  echo "Install pinned NDK 28.2.13676358 and set ANDROID_SDK_ROOT or MSIME_ANDROID_NDK" >&2; exit 1
fi
if [[ ! -x "$vcpkg_root/vcpkg" ]] || [[ $(git -C "$vcpkg_root" rev-parse HEAD) != ef7dbf94b9198bc58f45951adcf1f041fcbc5ea0 ]]; then
  echo "Provide bootstrapped vcpkg at pinned commit ef7dbf94b9198bc58f45951adcf1f041fcbc5ea0 using MSIME_VCPKG_ROOT" >&2; exit 1
fi
case $(uname -s) in
  Darwin) host_tag=darwin-x86_64 ;;
  Linux) host_tag=linux-x86_64 ;;
  *) echo "Use this build script on macOS or Linux" >&2; exit 1 ;;
esac
toolchain="$ndk/toolchains/llvm/prebuilt/$host_tag"
compiler="$toolchain/bin/${compiler_target}28-clang"
# 旧 worktree 的库不能作为本次构建的回退输入。先清空 ABI 目录，任何中途失败都会让
# 随后的 Gradle 校验明确报缺产物，而不是继续把旧 JNI 库打进 APK。
output="$repo_root/target/android/jniLibs/$abi"
rm -rf "$output"
mkdir -p "$output"
# 真正构建该 ABI 时必须运行身份和源码行回归，工具缺失不能作为跳过。
MSIME_ANDROID_NDK="$ndk" python3 scripts/test-android-crash-symbols.py --require-tools --target "$rust_target"
# vcpkg provides only nlohmann-json, which shared/voice/LocalAsr.cpp includes. Manifest installs reconcile their root, so the ABIs are kept apart and one build cannot uninstall the other ABI's copy.
deps="$repo_root/target/android-deps/$abi"
ANDROID_NDK_HOME="$ndk" VCPKG_DISABLE_METRICS=1 "$vcpkg_root/vcpkg" install \
  --x-manifest-root="$repo_root/platforms/android" --x-install-root="$deps" \
  --overlay-triplets="$repo_root/platforms/android/triplets" --triplet="$triplet"
env "CC_${rust_target//-/_}=$compiler" "CXX_${rust_target//-/_}=$compiler++" \
  "AR_${rust_target//-/_}=$toolchain/bin/llvm-ar" "$cargo_linker=$compiler" \
  CARGO_TARGET_DIR="$repo_root/target/android-cargo" \
  CARGO_PROFILE_RELEASE_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_STRIP=none \
  RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,--build-id=sha1" \
  cargo build -p msime-host-api --target "$rust_target" --release --locked
cp "$repo_root/target/android-cargo/$rust_target/release/libmsime_host_api.so" "$output/"
cp "$toolchain/sysroot/usr/lib/$compiler_target/libc++_shared.so" "$output/"
# On-device speech recognition: the sherpa-onnx C API and ONNX Runtime come prebuilt from the pinned release in resources/voice-runtime.lock.json (fetched and hash-checked, never committed). Only the two native libraries are taken from the .aar; LocalAsr loads the C API with dlopen, so a missing runtime disables local recognition instead of the keyboard.
if [ "$include_voice_runtime" = 1 ]; then
  python3 scripts/fetch_voice_runtime.py --platform android --out "$repo_root/target/voice-runtime/android"
  voice_aar=$(ls "$repo_root"/target/voice-runtime/android/sherpa-onnx-*.aar)
  unzip -o -j -q "$voice_aar" "jni/$abi/libsherpa-onnx-c-api.so" "jni/$abi/libonnxruntime.so" -d "$output"
else
  # jniLibs 目录跨构建保留，上一次不省略时取出的两个库要删掉，否则 Gradle 会把它们照样打进 APK。
  rm -f "$output/libsherpa-onnx-c-api.so" "$output/libonnxruntime.so"
  echo "speech runtime omitted from $output; the app downloads it as the voice-runtime resource pack"
fi
"$compiler++" -std=c++17 -g -Wl,--build-id=sha1 -shared -fPIC -Wall -Wextra -Werror \
  -Wl,--no-undefined -Wl,-z,max-page-size=16384 -Wl,-soname,libmsime_android.so \
  platforms/android/native/client_jni.cpp platforms/android/native/JniCallbacks.cpp platforms/common/CrashRecorder.cpp shared/voice/LocalAsr.cpp \
  -Icrates/host-api/include -Ishared -Ishared/voice/third_party -I"$deps/$triplet/include" \
  -L"$output" -lmsime_host_api -ldl -o "$output/libmsime_android.so"
bash platforms/android/verify-native.sh "$toolchain/bin/llvm-readelf" "$output" "$abi"
notices="$repo_root/target/android/notices/$abi"
mkdir -p "$notices"
for copyright_file in "$deps/$triplet"/share/*/copyright; do
  package=$(basename "$(dirname "$copyright_file")")
  cp "$copyright_file" "$notices/$package.txt"
done
cp "$toolchain/NOTICE" "$notices/ndk-toolchain.txt"
# 省略语音运行库时这三份声明照样随包：下载的资源包是同一份上游库，加载它的仍是这个应用。
cp shared/voice/third_party/sherpa-onnx/LICENSE "$notices/sherpa-onnx.txt"
cp platforms/linux/data/licenses/onnxruntime-MIT.txt "$notices/onnxruntime.txt"
cp platforms/linux/data/licenses/onnxruntime-ThirdPartyNotices.txt "$notices/onnxruntime-third-party.txt"
cp "$toolchain/sysroot/NOTICE" "$notices/ndk-sysroot.txt"
# The input engine in libmsime_host_api.so embeds the Korean Hanja table from libhangul's data/hanja/hanja.txt, which is BSD-3-Clause: clause 2 requires its notice in every binary distribution, so it travels with the library's other notices into assets/native-notices.
cp resources/licenses/libhangul-hanja-BSD-3-Clause.txt "$notices/libhangul-hanja.txt"
# 语法模型查询逻辑的移植声明随引擎分发，文件末尾附着双数组读取所依的 darts-clone（BSD-2-Clause）条款。
cp resources/licenses/librime-octagram-BSD-3-Clause.txt "$notices/librime-octagram-BSD-3-Clause.txt"
# 引擎的 cxx_sort.rs 逐步复现了 libc++ 的排序算法，源码级翻译要按 Apache-2.0 第 4 条随附许可证（LLVM 例外不覆盖）。
cp resources/licenses/libcxx-Apache-2.0-WITH-LLVM-exception.txt "$notices/libcxx-Apache-2.0-WITH-LLVM-exception.txt"
# 符号模式按它查命令名的 LaTeX 表是本项目自写的 MIT 数据，编进了引擎，MIT 全文随引擎分发。
cp resources/licenses/latex-symbols-MIT.txt "$notices/latex-symbols-MIT.txt"
# 输入引擎本身（crates/engine、crates/path-trust，源码在本仓库）和它链接的 rink-core（单位换算，源码在 crates.io）是 MPL-2.0，二进制分发要附上许可证全文。
cp resources/licenses/MPL-2.0.txt "$notices/MPL-2.0.txt"
# The engine's Cantonese and Zhuyin schemes take their Jyutping and bopomofo syllables and words from data derived from rime-cantonese (CC BY 4.0, which requires attribution) and libchewing-data (LGPL-2.1-or-later, which requires the licence text and a source pointer). build-apk.sh and build-client-apk.sh package the dictionaries themselves, each beside its own licence text, when target/language-dictionaries (or MSIME_LANGUAGE_DICTIONARIES) holds them; the notices travel with every engine build either way so that one notice list covers every platform.
cp resources/licenses/rime-cantonese-CC-BY-4.0.txt "$notices/rime-cantonese.txt"
cp resources/licenses/libchewing-data-LGPL-2.1.txt "$notices/libchewing-data.txt"
# The Stroke scheme's stroke orders come from rime-stroke (LGPL-3.0, whose main table also carries the CNS11643 attribution); its notice travels the same way.
cp resources/licenses/rime-stroke-LGPL-3.0.txt "$notices/rime-stroke.txt"
# 越南文和藏文方案分别用 vi crate（MIT）和 ewts crate（MIT OR Apache-2.0，按 MIT 使用）编进 libmsime_host_api.so；ewts 没有自带许可证文件，仓库里这份全文是它的声明唯一能随二进制分发的途径。
cp resources/licenses/vi-MIT.txt "$notices/vi.txt"
cp resources/licenses/ewts-MIT.txt "$notices/ewts.txt"
# 拆字反查的部件拼音表由 kfcd/chaizi《漢語拆字字典》（CC BY 3.0）改编后编进 libmsime_host_api.so，第 4(b) 条要求随改编作品署名并说明改动；许可证文件里写着署名、固定提交、改动说明和法律文本全文。
cp resources/licenses/kfcd-chaizi-CC-BY-3.0.txt "$notices/kfcd-chaizi.txt"
# 同一张拆字表里每个字的读音、名次和繁体字形取自 Unicode 汉字数据库（Unihan 17.0.0），符号模式（S 模式）的分组表按 Unicode 字符数据库 17.0.0 挑选组员，Unicode License V3 要求随数据附上它的版权与许可声明。
cp resources/licenses/Unicode-License-V3.txt "$notices/unicode-unihan.txt"
# 简繁转换内嵌的 OpenCC 词表许可证与原生库一并打包。
cp resources/licenses/OpenCC-Apache-2.0.txt "$notices/OpenCC-Apache-2.0.txt"
# 随 APK 和资源包分发的词库里有 MIT 与 Apache-2.0 的数据，两种许可都要求随副本附上全文：msime-pinyin.db 的单字读音（mozillazg/pinyin-data）、msime-english.db 的中英释义（skywind3000/ECDICT）、msime-others.db 的颜文字（aoguai/rime_kaomoji_dict）与关键词整词读音（mozillazg/python-pinyin）、msime-wubi.db 的 86 码表（KyleBing/rime-wubi86-jidian，Apache-2.0）。
cp resources/licenses/pinyin-data-MIT.txt "$notices/pinyin-data-MIT.txt"
cp resources/licenses/ECDICT-MIT.txt "$notices/ECDICT-MIT.txt"
cp resources/licenses/rime_kaomoji_dict-MIT.txt "$notices/rime_kaomoji_dict-MIT.txt"
cp resources/licenses/python-pinyin-MIT.txt "$notices/python-pinyin-MIT.txt"
cp resources/licenses/rime-wubi86-jidian-Apache-2.0.txt "$notices/rime-wubi86-jidian-Apache-2.0.txt"
# msime-pinyin.db 与语言词库资源包的 msime-stroke.db 里与 rime-wanxiang 共有的词条和单字，权重改编自 amzxyz/rime-wanxiang（CC BY 4.0），署名、改动说明与许可证全文随副本分发。
cp resources/licenses/rime-wanxiang-CC-BY-4.0.txt "$notices/rime-wanxiang-CC-BY-4.0.txt"
# 随 APK 和资源包分发的词库（msime-pinyin.db、msime-english.db、msime-others.db、msime-bigram.bin、msime-trigram.bin，按需下载或随非瘦包带上的 msime-wubi.db 与日文词典）逐个来源的上游、提交与条款写在 msime-dictionary 的 NOTICE.md 里，这是它的副本，另有 msime 一侧加进同一批词库的来源：其中有 msime-bigram.bin 与 msime-trigram.bin 要求的中文维基百科（CC BY-SA 4.0）署名，以及 msime-wubi.db 各份码表的出处。
cp resources/licenses/msime-engine-dictionary-NOTICE.md "$notices/msime-engine-dictionary-NOTICE.md"
# 世界时钟内嵌 IANA 城市目录，随原生库分发公共领域许可。
cp resources/licenses/world-clock-NOTICE.txt "$notices/world-clock-NOTICE.txt"
echo "Android native libraries built: $output (not yet device-verified)"

python3 scripts/crash-symbols.py collect --platform android --version "${MSIME_ANDROID_VERSION:-$(cat platforms/android/version.txt)}" \
  --root "$output" --tool-prefix "$toolchain/bin/llvm-" \
  --output "target/android-symbols/android-${MSIME_ANDROID_VERSION:-$(cat platforms/android/version.txt)}-${MSIME_EDITION:-full}-$abi-${MSIME_BUILD_RUN:-${GITHUB_RUN_ID:-local}}-${GITHUB_RUN_ATTEMPT:-1}.zip"
