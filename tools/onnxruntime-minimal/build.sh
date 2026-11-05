#!/bin/zsh
# 按 required_operators_and_types.config 精简构建 ONNX Runtime 的静态库，一次一个平台，产物合并成一个 libonnxruntime.a 再 gzip。用法和前置条件见同目录的 README.md。
#
#   build.sh <ONNX Runtime 源码检出> <工作目录> <平台>
#   平台：android-arm64 android-armv7 android-x86_64 ohos-arm64 ios-arm64 ios-sim-arm64 macos-arm64 macos-x86_64 linux-aarch64 linux-x86_64
set -euo pipefail
SOURCE=${1:?ONNX Runtime 源码检出}
WORK=${2:?工作目录}
TARGET=${3:?平台}
HERE=${0:A:h}
CONFIG=$HERE/required_operators_and_types.config
NDK=${ANDROID_NDK_HOME:-$HOME/Library/Android/sdk/ndk/28.2.13676358}
SDK=${ANDROID_HOME:-$HOME/Library/Android/sdk}
# 鸿蒙的 NDK 随 DevEco Studio 装，与 platforms/harmony/build-native.sh 的默认位置相同；单独的命令行 SDK 用 MSIME_OHOS_NDK 指过去（指到含 build/cmake/ohos.toolchain.cmake 的 native 目录）。
OHOS_NDK=${MSIME_OHOS_NDK:-/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/native}
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
  # build.py 没有鸿蒙选项，按普通的交叉编译把 NDK 的 CMake 工具链交给它；工具链默认 c++_shared，与 libmsimeclient.so 随 HAP 带的 libc++_shared.so 是同一个运行时。四处补丁都只加构建参数，不改 ONNX Runtime 的源码：
  # - 工具链给每条编译命令（包括 MLAS 的汇编文件）都加 --gcc-toolchain，clang 编译时用不上它、报 -Wunused-command-line-argument，ONNX Runtime 又按 -Werror 编，所以只关这一个警告。
  # - 打开 bf16 这个目标特性：MLAS 的 mlasi.h 在 aarch64 Linux（鸿蒙的编译器也定义 __linux__）上无条件用 bfloat16_t，NDK 自带的 clang 15 只在开了 bf16 时才定义它（clang 17 起无条件定义，Android NDK 28 是 clang 19），不开时除 bf16 内核之外每个 include 它的文件都编不过。用 cc1 的 -target-feature 而不是 -march=armv8.2-a+bf16：指令集基线仍是 armv8-a，也不会被 MLAS 给 fp16 文件单独加的 -march 覆盖掉；bf16 指令只出现在 sbgemm 内核里，MLAS 运行时按 hwcap 判断 CPU 支持才走它，重排模型的 fp32 MatMul 不走。
  # - 鸿蒙的 musl 没有 pthread_setaffinity_np，ohos-compat.h 把它换成作用于调用线程的 sched_setaffinity，理由写在那个文件里。
  # - 关掉 cpuinfo：它的 CMake 只认 Linux 和 Android 这两个系统名，遇到 OHOS 时编出一个没有 ARM Linux 探测代码的库（cpuinfo_arm_linux_init 未定义），ONNX Runtime 却按 __linux__ 去调它，链接失败。关掉以后 CPUIDInfo::ArmLinuxInit 走它本来就有的回退，按 getauxval 的 AT_HWCAP / AT_HWCAP2 判断 dotprod、i8mm、bf16 这些指令能不能用，少的只是大小核的微架构信息。
  # 只有 arm64：鸿蒙 x86_64 只给模拟器用，MLAS 的 x86_64/cvtfp16Avx.S 用了 AVX-NE-CONVERT 指令，clang 15 的汇编器不认（clang 16 起才认），ONNX Runtime 又没有不编它的开关。
  ohos-arm64)
    OHOS_WARNINGS=-Wno-unused-command-line-argument
    OHOS_FEATURES="-Xclang -target-feature -Xclang +bf16"
    EXTRA=(--cmake_extra_defines CMAKE_TOOLCHAIN_FILE=$OHOS_NDK/build/cmake/ohos.toolchain.cmake OHOS_ARCH=arm64-v8a OHOS_STL=c++_shared onnxruntime_ENABLE_CPUINFO=OFF
      "CMAKE_ASM_FLAGS=$OHOS_WARNINGS" "CMAKE_C_FLAGS=$OHOS_WARNINGS $OHOS_FEATURES" "CMAKE_CXX_FLAGS=$OHOS_WARNINGS $OHOS_FEATURES -include $HERE/ohos-compat.h");;
  ios-arm64) EXTRA=(--ios --apple_sysroot iphoneos --osx_arch arm64 --apple_deploy_target 15.0 --cmake_generator Xcode);;
  ios-sim-arm64) EXTRA=(--ios --apple_sysroot iphonesimulator --osx_arch arm64 --apple_deploy_target 15.0 --cmake_generator Xcode);;
  # build.py 只在 --ios 或 --build_apple_framework 时把 --apple_deploy_target 交给 CMake，普通的 macOS 构建会忽略它，编出的目标文件按编译机的系统版本标最低版本（在 macOS 27 上编就是 27.0）。所以直接给 CMake 最低版本，与 msime macOS 版的最低版本（tauri.macos.conf.json 的 13.0）一致。
  macos-arm64) EXTRA=(--osx_arch arm64 --cmake_extra_defines CMAKE_OSX_DEPLOYMENT_TARGET=13.0);;
  macos-x86_64) EXTRA=(--osx_arch x86_64 --cmake_extra_defines CMAKE_OSX_DEPLOYMENT_TARGET=13.0);;
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
  elif [[ $TARGET == ohos-* ]]; then
    { echo "CREATE $OUT/libonnxruntime.a"; archives | sed 's/^/ADDLIB /'; echo SAVE; echo END; } | $OHOS_NDK/llvm/bin/llvm-ar -M
  else
    libtool -static -o $OUT/libonnxruntime.a $(archives) 2>/dev/null
  fi
fi
gzip -9 -n -k -f $OUT/libonnxruntime.a
ls -l $OUT
