# Agent Note: Android JNI 产物与 Java 声明错配

Status: implemented

## Problem

Android 真机上的 Java `NativeClient` 声明了 `candidateRangeRaw` 和 `setKeyGeometryRaw`，但安装包内的 `libmsime_android.so` 没有这两个导出。输入服务在第一次调用时收到 `UnsatisfiedLinkError`，进入“输入连接失败：仅直接输入”，九键因此只能直通。源码契约检查和 `build-native.sh` 的库检查本身已经覆盖这些名字，但直接运行 Gradle 或复用 worktree 的旧 `target/android/jniLibs` 仍能绕过它们。

## Decision

Android Gradle 应用在合并 `jniLibs` 前调用与 native 构建相同的 `platforms/android/verify-native.sh`，按本次 `msimeAbis` 对 `target/android/jniLibs/<abi>` 做 ELF、依赖、Host API 和全部 JNI 导出检查。缺库或缺少当前 Java 所需的 JNI 导出时，Gradle 在打包前失败。Java 编译和 JVM 门禁不需要 native 产物，因此不触发这项检查。`build-native.sh` 在每个 ABI 开始时先删除旧目录；native 构建中途失败后，后续 Gradle 不会误把旧库当作有效输入。JNI 名字合同测试同时固定 Gradle 门禁必须存在并接到 native 合并任务。

## Alternatives considered

- **只依赖 `build-native.sh` 的现有检查**：构建脚本路径本身是可靠的，但维护者和设备验收经常直接运行 Gradle 复用 native 输出；这条路径正是本次错配进入 APK 的入口，因此不足以作为唯一门禁。
- **在 Java 端为缺少方法提供降级实现**：能隐藏 `UnsatisfiedLinkError`，却不能提供候选范围或键位几何，九键仍会失去组字能力，也会让 Java 与 Rust ABI 继续漂移，因此不采用。
- **每次 Gradle 构建都强制重新编译 Rust 和 JNI**：能避免旧库，但会显著增加普通 Java/资源构建时间，并且不能解决人工复制的错误库；在打包前复用完整的 ELF/JNI 校验更适合现有构建分层。

## Verification

`scripts/test-android-jni-names.py` 检查 Java、C++、导出清单和 Gradle 门禁；用旧 worktree 的 APK 抽出的 `libmsime_android.so` 重跑 `verify-native.sh` 时会明确报缺少 `candidateRangeRaw`/`setKeyGeometryRaw`，证明门禁能拦住报告中的产物。重新构建 native 与 APK 后还需在 Pixel 真机安装，确认 logcat 不再出现 `No implementation found` 或 `input_failure reason=jni_link`，并用合成编辑器验证九键组字。

## Consequences

直接 Gradle 打包现在需要先准备并验证 native 库；这会让缺少 Android NDK 或未构建 native 输出的命令更早失败，但避免生成安装后才在输入连接中降级的 APK。native 输出目录被清空后，中断构建不会留下可被误用的旧库。
