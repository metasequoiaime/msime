#!/bin/zsh
# 按 required_operators_and_types.config 精简构建 ONNX Runtime 的静态库，一次一个平台，产物合并成一个 libonnxruntime.a 再 gzip。用法和前置条件见同目录的 README.md。
#
#   build.sh <ONNX Runtime 源码检出> <工作目录> <平台>
#   平台：android-arm64 android-armv7 android-x86_64 ios-arm64 ios-sim-arm64 macos-arm64 macos-x86_64 linux-aarch64 linux-x86_64
set -euo pipefail
SOURCE=${1:?ONNX Runtime 源码检出}
WORK=${2:?工作目录}
TARGET=${3:?平台}
HERE=${0:A:h}
CONFIG=$HERE/required_operators_and_types.config
NDK=${ANDROID_NDK_HOME:-$HOME/Library/Android/sdk/ndk/28.2.13676358}
SDK=${ANDROID_HOME:-$HOME/Library/Android/sdk}
# Linux 用 msime 发 Linux 版的同一个镜像编（Dockerfile.build-gate），静态库链接的 glibc 版本才对得上。
LINUX_IMAGE=rust:1.97.1-bookworm@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
COMMON=(--config MinSizeRel --update --build --parallel ${JOBS:-4} --minimal_build --disable_ml_ops --disable_rtti --disable_exceptions --skip_tests
  --include_ops_by_config $CONFIG --enable_reduced_operator_type_support
  --cmake_extra_defines CMAKE_POLICY_VERSION_MINIMUM=3.5 onnxruntime_BUILD_UNIT_TESTS=OFF CMAKE_POSITION_INDEPENDENT_CODE=ON)
BUILD=$WORK/build/$TARGET
OUT=$WORK/out/$TARGET
mkdir -p $OUT

case $TARGET in
  android-arm64) EXTRA=(--android --android_abi arm64-v8a --android_api 28 --android_sdk_path $SDK --android_ndk_path $NDK);;
  android-armv7) EXTRA=(--android --android_abi armeabi-v7a --android_api 28 --android_sdk_path $SDK --android_ndk_path $NDK);;
  android-x86_64) EXTRA=(--android --android_abi x86_64 --android_api 28 --android_sdk_path $SDK --android_ndk_path $NDK);;
  ios-arm64) EXTRA=(--ios --apple_sysroot iphoneos --osx_arch arm64 --apple_deploy_target 15.0 --cmake_generator Xcode);;
  ios-sim-arm64) EXTRA=(--ios --apple_sysroot iphonesimulator --osx_arch arm64 --apple_deploy_target 15.0 --cmake_generator Xcode);;
  macos-arm64) EXTRA=(--osx_arch arm64 --apple_deploy_target 12.0);;
  macos-x86_64) EXTRA=(--osx_arch x86_64 --apple_deploy_target 12.0);;
  linux-aarch64|linux-x86_64) EXTRA=();;
  *) echo "未知平台 $TARGET" >&2; exit 2;;
esac

archives() {
  find $BUILD/MinSizeRel -name "*.a" | grep -viE "test|gtest|gmock|benchmark"
}

if [[ $TARGET == linux-* ]]; then
  PLATFORM=linux/amd64; [[ $TARGET == linux-aarch64 ]] && PLATFORM=linux/arm64
  docker run --rm --platform $PLATFORM -v $SOURCE:$SOURCE -v $WORK:$WORK -v $HERE:$HERE -w $SOURCE $LINUX_IMAGE bash -euo pipefail -c "
    # 镜像里的 Python 不带 venv 模块，建出的虚拟环境没有 pip；装上它，再在虚拟环境里装 ONNX Runtime 1.24 要的 CMake（不低于 3.28，bookworm 自带的是 3.25）。
    apt-get update -qq && apt-get install -y -qq python3-venv > /dev/null
    python3 -m venv /tmp/venv && /tmp/venv/bin/pip install -q flatbuffers cmake==3.31.6
    export PATH=/tmp/venv/bin:\$PATH
    # 源码检出归宿主上的用户，容器里的 root 跑 git 会报 dubious ownership；build.py 要在检出里跑 git submodule sync，所以把它标成可信。macOS 上的 Docker 会把挂载目录映射成 root 的，看不出这个问题，Linux runner 上才会撞上。
    git config --global --add safe.directory $SOURCE
    # 一次性容器里本来就是 root；ONNX Runtime 的构建脚本默认拒绝 root。
    python3 tools/ci_build/build.py --build_dir $BUILD ${COMMON[*]} ${EXTRA[*]} --allow_running_as_root
    { echo CREATE $OUT/libonnxruntime.a; find $BUILD/MinSizeRel -name '*.a' | grep -viE 'test|gtest|gmock|benchmark' | sed 's/^/ADDLIB /'; echo SAVE; echo END; } | ar -M"
else
  # 构建脚本要 flatbuffers 来读算子清单；放进这次构建自己的虚拟环境，不碰系统的 Python。
  [[ -x $WORK/venv/bin/python ]] || { python3 -m venv $WORK/venv && $WORK/venv/bin/pip install -q flatbuffers; }
  (cd $SOURCE && $WORK/venv/bin/python tools/ci_build/build.py --build_dir $BUILD "${COMMON[@]}" "${EXTRA[@]}")
  rm -f $OUT/libonnxruntime.a
  if [[ $TARGET == android-* ]]; then
    { echo "CREATE $OUT/libonnxruntime.a"; archives | sed 's/^/ADDLIB /'; echo SAVE; echo END; } | $NDK/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ar -M
  else
    libtool -static -o $OUT/libonnxruntime.a $(archives) 2>/dev/null
  fi
fi
gzip -9 -n -k -f $OUT/libonnxruntime.a
ls -l $OUT
