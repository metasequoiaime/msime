# Agent Note: Android 设备门禁重试损坏的 SDK 包

Status: implemented

## Problem

Android 设备门禁在安装模拟器和系统镜像时直接调用一次 `sdkmanager`。下载或解压归档损坏后，`sdkmanager` 会在 SDK 的临时目录留下半截归档、完成标记或不完整的目标目录；同一 job 没有重试，因此 API 28/35 还没启动模拟器就失败。2026-11-04 的 develop CI（#10831）在 API 35 复现了 `Error on ZipFile unknown archive`。

## Decision

设备门禁通过 `platforms/android/install-sdk-packages.sh` 安装固定的 build-tools、platform、emulator、platform-tools 和指定 API 的 x86_64 system image。脚本先检查每个目标包的 `package.xml` 以及 `emulator`、`adb` 可执行文件；检查完整时直接复用。否则最多尝试三次，第二次起清理 sdkmanager 的 `.temp`、`.downloadIntermediates` 和本次目标包目录，再重新下载，并在每次成功返回后再次检查完整性。只有完整性检查通过才返回成功，保留 `sdkmanager` 的失败结果并在三次后失败。

设备 workflow 复用该脚本，NDK 的既有安装器继续负责 NDK。合成测试 `scripts/test-android-install-sdk.py` 覆盖首次失败后成功、清理残片后成功、持续失败和复用完整安装四条路径；测试不访问真实 SDK 或网络。

## Alternatives considered

- **继续在 workflow 内一次调用 `sdkmanager`**：改动最小，但已经无法处理本次归档损坏，也会让同一类随机下载错误持续阻塞 develop，因此不采用。
- **只扩大 workflow 的超时或重新运行整个 job**：可以偶尔绕过网络抖动，但坏归档会被复用，不能保证下一次使用完整包，因此不采用。
- **把所有 SDK 包预装到 runner 镜像**：能减少下载，但 runner 镜像和 API 版本会变化，且 release 与 PR 仍需要自包含安装流程，因此不采用。

## Consequences

损坏的 SDK 归档现在会在同一 job 内清理并重试，完整性检查避免半截安装进入后续模拟器步骤；网络或镜像连续失败仍会在三次后明确失败。每次失败重试会重新下载本次 API 的目标包，增加少量时间和带宽。真实 GitHub runner 的 API 28/35 设备验收仍需后续 develop 运行确认。
