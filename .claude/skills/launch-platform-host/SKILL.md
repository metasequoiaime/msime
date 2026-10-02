---
name: launch-platform-host
description: Build, launch and verify a 水杉输入法 platform host — macOS, iOS, Android, HarmonyOS, Linux or Windows. Use whenever the task is to run the product, see a change in the real app, screenshot a settings page, or close an "untested on device" gap. Records the commands that work and the traps that do not announce themselves.
---

# 启动平台宿主

**产品本体是 `platforms/<os>` 下的原生宿主。** [AGENTS.md](../../../AGENTS.md) 开篇就写着这一条：不要把 Tauri 生成的工程当成某个平台的产品去启动、调试或验收；`apps/desktop` 的目录名和 Tauri 工程自带的 bundle id 都不构成例外。

所以「跑一下看看」永远从 `platforms/<os>` 开始，`pnpm tauri dev` 不是答案。

共享 React 设置页（打字统计、词库、背单词……）确实由 Tauri 层渲染，但它是**原生宿主按需拉起的公共组件**：要看某个共享设置页，先把宿主编出来，再让宿主打开它。

## 先确认前置条件，不要假设没有

每条都有固定版本或固定来源。动手前先查本机有没有，**查过再说「本机不具备」**——凭印象断言缺依赖，代价是整块验证被跳过而没人知道。

| 依赖 | 用途 | 怎么找 |
| --- | --- | --- |
| Sparkle 2.9.6 | macOS 更新控制器，不可省略 | 按 [platforms/macos/README.md](../../../platforms/macos/README.md) 下载校验后解压到独立目录，`MSIME_SPARKLE_ROOT` 指向含 `Sparkle.framework` 的那一层 |
| Android SDK + NDK 28.2.13676358 | Android 原生构建 | `ANDROID_SDK_ROOT`；NDK 在 `$ANDROID_SDK_ROOT/ndk/28.2.13676358`，版本由 `build-native.sh` 钉死 |
| vcpkg（锁定 commit） | Android 依赖 | `MSIME_VCPKG_ROOT`，或 `target/tooling/vcpkg`；commit 不符会直接拒绝 |
| DevEco 命令行 `hvigorw` / `ohpm` | HarmonyOS 打包 | DevEco Studio 自带，把其 `command-line-tools/bin` 加进 `PATH` |
| Docker | Linux 容器构建 | `docker info` |
| MinGW `x86_64-w64-mingw32-g++` | Windows 交叉编译 | `platforms/windows/build-cross.sh` 会引导 vcpkg |

## 产物会消失，把构建和使用串成一条命令

`target/` 是被忽略的构建输出。并发 worktree 多、磁盘接近满时，它会被清理掉——包括刚编好的 `.app`、静态库和 staged 资源。分两步跑，第二步很可能发现第一步的产物已经不在。

```sh
# 反例：两条命令，中间产物可能蒸发
bash platforms/ios/build-native.sh simulator
xcodebuild ... test        # ld: library 'msime_host_api' not found

# 正例
bash platforms/ios/build-native.sh simulator && xcodebuild ... test
```

空间不够时先清自己的：`target/{android-cargo,android-deps,macos-cargo,ios-cargo,resources}` 都能重新生成。不要清别的 worktree。

## macOS

产出 `水杉输入法.app`（IMK 输入法 bundle）。

```sh
CFLAGS="-mmacosx-version-min=13.0" CXXFLAGS="-mmacosx-version-min=13.0" \
CMAKE_OSX_DEPLOYMENT_TARGET=13.0 CMAKE_PREFIX_PATH="$(brew --prefix)" \
CARGO_TARGET_DIR=target/macos-cargo cargo build -p msime-host-api --locked \
&& cmake -S platforms/macos -B target/macos-isolated \
     -DMSIME_HOST_LIBRARY="$PWD/target/macos-cargo/debug/libmsime_host_api.a" \
     -DMSIME_SPARKLE_ROOT="<Sparkle-2.9.6 所在目录>" \
&& cmake --build target/macos-isolated --parallel
```

- **不要**把最低版本写成全局 `MACOSX_DEPLOYMENT_TARGET`：rustc 会把它一并应用到为宿主编译的 proc-macro 动态库上，冷缓存构建以 `can't find crate for zerofrom_derive` 失败；cargo 不把该变量算进指纹，坏掉的产物会被后续构建继续复用，失败因此看起来时有时无。
- bundle 名字是中文。`cp -R target/macos-isolated/水杉输入法.app …` 会因 APFS 的 NFC/NFD 归一化报 `No such file or directory`——用 `find target/macos-isolated -maxdepth 1 -name "*.app" -exec cp -R {} <目标> \;`。
- 安装与输入源注册走 `platforms/macos/scripts/install.sh`。注册后有分钟级不稳定窗口：`check_input_source.swift` 要隔几秒多查几次再下结论，只查一次两个方向都可能误判。
- **装完还打不出中文是正常的：`install.sh` 不准备词库。** 输入会话要 `~/Library/Application Support/app.msime.macos/runtime-options.json`，开发构建里没有（发布包由设置应用首次启动时写）。没有会话的控制器把按键原样交给应用，看起来就是「选了中文却打出英文」。补齐的三条命令在 `platforms/macos/README.md` 的《安装与输入源注册》开头；资源别留在 `target/resources`，那里会被清掉。查现象用 `log show --predicate 'process == "水杉输入法"'` 找 `MSIME has no input session`。
- 产品是 `src/input/InputController.mm` 和 `src/core/ClientDictionaryRuntime.mm`，改动要落在这两处；`scripts/test-macos-orphan-sources.py` 守着不让不参与构建的源文件留在树里。
- 换新 bundle identifier 需要重新登录一次，这是 macOS 本身的限制，与签名和 plist 无关。
- `check_input_source.swift` 报某个模式 `disabled`（典型是英文模式 `.Roman`）而中文模式 enabled 时，安装是成功的，别去重新登录或反复重装：macOS 27 上进程启用不了键盘输入模式，`TISEnableInputSource` 返回 noErr 而状态不变，苹果自己的模式一样如此。该脚本为此退 2（可用但有源未启用），退 1 才是不可用。测量见 `docs/macos-parity.md`。
- 钥匙串里有两张同名 Developer ID Application 证书时，`codesign` 会因名字歧义拒签。`install.sh` 已改为按 SHA-1 取身份，要指定就把 `MSIME_SIGNING_IDENTITY` 设成哈希而不是名字。

看共享设置页时，Tauri 设置壳把 `target/macos/水杉输入法.app` 和 `target/macos/EngineResources` 列为 bundle 资源，两者必须先就位：

```sh
cargo run --quiet -p msime-client-core --example install_resources --locked -- target/resources
bash platforms/macos/stage-resources.sh target/resources/<上一步返回的目录>
mkdir -p target/macos && find target/macos-isolated -maxdepth 1 -name "*.app" -exec cp -R {} target/macos/ \;
```

## iOS

```sh
bash platforms/ios/build-native.sh simulator
```

跑模拟器测试要静态库、staged 资源、生成好的工程三样同时在场，串成一条：

```sh
cargo run --quiet -p msime-client-core --example install_resources --locked -- target/resources > /tmp/ir.log \
&& RES=$(tail -1 /tmp/ir.log) && bash platforms/ios/stage-resources.sh "$RES" \
&& bash platforms/ios/build-native.sh simulator \
&& (cd platforms/ios && xcodebuild -project MSIMEClient.xcodeproj -scheme MSIMEClientTests \
      -destination 'platform=iOS Simulator,name=<模拟器名>' test)
```

- scheme 是 `MSIMEClientTests`，`MSIMEKeyboardTests` 是 target 名，直接用它报 "does not contain a scheme"。`xcodebuild -list` 查全部。
- `xcodegen generate` 会做 spec 校验，缺 `target/ios/EngineResources` 及其中的 `dictionary-manifest.json` 直接失败——先 stage 再生成工程。
- 改了 `App/Sources`、`SharedUI`、`KeyboardTests` 下的文件要 `xcodegen generate` 并提交 `project.pbxproj`：它逐个列出源文件，不重新生成，新文件不会被编译。
- Xcode 27 的模拟器界面是 `DeviceHub.app`，`Simulator.app` 已不存在；`xcrun simctl` 一切照常，设备状态以 `xcrun simctl list devices` 为准。

## Android

```sh
ANDROID_SDK_ROOT=<SDK> MSIME_VCPKG_ROOT=<vcpkg> bash platforms/android/build-native.sh arm64-v8a
```

导出与 ABI 核验（`verify-native.sh` 要三个参数）：

```sh
READELF=$(find "$ANDROID_SDK_ROOT/ndk/28.2.13676358" -name llvm-readelf | head -1)
bash platforms/android/verify-native.sh "$READELF" target/android/jniLibs/arm64-v8a arm64-v8a
```

- vcpkg 树跨 worktree 共享，并发构建会撞 `Failed to take the filesystem lock / Resource busy`，等一会儿重试。
- 两个 APK 同包名同输出路径：`build-apk.sh` 是无 WebView 的原生壳，`build-client-apk.sh` 是 Tauri 合包。哪一个都不是「那个 Android 应用」。
- 不需要 NDK 的纯 Java 门禁：`ANDROID_SDK_ROOT=… bash platforms/android/check-host.sh`。它同一次运行里还会用 NDK 对 `client_jni.cpp` 做 `-fsyntax-only`——那是唯一能抓出 Java `native` 声明与 C++ 导出不匹配的地方，没有 NDK 时它跳过并打印提示。

## HarmonyOS

```sh
export PATH="<DevEco command-line-tools>/bin:$PATH"
bash platforms/harmony/stage-settings.sh   # settings page from packages/ui; hvigor refuses to package without it
cd platforms/harmony && ohpm install && hvigorw assembleHap --no-daemon
```

产出 `entry/build/default/outputs/default/entry-default-unsigned.hap`。

- **`ohpm install` 不能省**：它建立 `entry/oh_modules/` 里的链接，`import client from 'libmsimeclient.so'` 才能解析到 `.d.ts`。不跑它，ArkTS 把整个 NAPI 边界当成无类型，**构建照样成功**——于是「构建通过」可能意味着一个原生调用都没被检查。
- `hvigorw assembleHap` 是唯一编译 ArkTS 的地方。`tsc` 全过、单测全绿、`verify-local.sh --quick` 通过，都不代表 HAP 打得出来：曾有 13 个 ArkTS 错误进入 develop 而没有任何门禁发现。改过 `.ets` 就跑一次。
- 设置页 `rawfile/settings/index.html` 不提交，由 `stage-settings.sh` 在打包前从 `packages/ui` 构建；改了 `packages/ui` 重跑一次它再打包，设备上才是新界面。不要把这个文件加回版本库。

## Linux

```sh
bash platforms/linux/build-container.sh    # 固定容器内构建 IBus/Fcitx5 并跑 ctest
```

Tauri 外壳的 Linux 编译是唯一能看见 `#[cfg(target_os = "linux")]` 分支的地方——macOS 上 `cargo check --workspace` 根本走不到 `msime-desktop`，这个盲区曾攒下 36 个编译错误：

```sh
image="msime-linux-desktop-check:$(printf %s "$PWD" | shasum | cut -c1-12)"
docker build -q -t "$image" -f platforms/linux/tests/tools/Dockerfile.desktop-check platforms/linux/tests
docker run --rm -v "$PWD":/source \
  -v "$PWD/target/linux-desktop-check":/ctarget -w /source \
  -e CARGO_TARGET_DIR=/ctarget \
  "$image" cargo check -p msime-desktop --locked --all-targets --message-format short
```

镜像与 `verify-local.sh` 用的是同一个 tag，apt 只在 Dockerfile 变化后装一次。

zsh 下不要照抄 `verify-local.sh` 里的 `${var:+-v "$x":/path}` 写法——它会把一个前导空格带进参数，docker 报 `includes invalid characters for a local volume name`。把挂载路径写死。

## Windows

```sh
bash platforms/windows/build-cross.sh x64    # MinGW 交叉编译，不需要 Windows 机器
```

跑过一次后 `verify-local.sh` 会在任何 worktree 里自动接管这条路径。

## 证据分级

[ARCHITECTURE.md](../../../ARCHITECTURE.md) 的五级，报告时不能跨级：

1. 源码与单元测试
2. 跨目标编译或容器构建
3. 模拟器运行
4. 真机或系统入口
5. 安装、签名与真实编辑器验收

交叉编译通过不等于系统入口可用，模拟器跑通不等于真机验收完成。说到第几级就写第几级。

## 起了进程就要收尾

`pnpm dev` 的 Vite、直接跑起来的宿主二进制、Playwright / CDP 起的浏览器，都要在同一次会话里关掉。macOS 上无头 Chrome 会以同 bundle id 注册成前台实例，导致点 Dock 图标毫无反应；异常退出后按临时 profile 特征兜底清理：`pkill -f playwright_chromiumdev_profile`。
