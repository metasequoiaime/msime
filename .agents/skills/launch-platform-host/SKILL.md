---
name: launch-platform-host
description: Build, launch and verify a 水杉输入法 platform host — macOS, iOS, Android, HarmonyOS, Linux or Windows. Use whenever the task is to run the product, see a change in the real app, screenshot a settings page, type on a real device with a script, or close an "untested on device" gap. Records the commands that work and the traps that do not announce themselves.
---

# 启动平台宿主

**产品本体是 `platforms/<os>` 下的原生宿主。** [AGENTS.md](../../../AGENTS.md) 开篇就写着这一条：不要把 Tauri 生成的工程当成某个平台的产品去启动、调试或验收；`apps/desktop` 的目录名和 Tauri 工程自带的 bundle id 都不构成例外。

所以「跑一下看看」永远从 `platforms/<os>` 开始，`pnpm tauri dev` 不是答案。

共享 React 设置页（打字统计、词库、背单词……）是**原生宿主按需拉起的公共组件**，而且只在三个平台上是设置界面：macOS 与 Linux 由宿主拉起 Tauri 设置应用，HarmonyOS 由 `pages/Settings.ets` 用 ArkWeb 加载、不经过 Tauri。Windows、iOS 与 Android 发布包的设置是原生界面，在那里看不到这些页面（归属见 [ARCHITECTURE.md](../../../docs/ARCHITECTURE.md#设置界面按平台归属)）。要看某个共享设置页，先把这三个平台之一的宿主编出来，再让宿主打开它。

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

开发构建照旧暂存全部资源（含日文词典，和 `target/language-dictionaries` 里有的粤拼、注音、笔画词库），它们是按需下载之外的内置兜底，所以开发包里这几个方案装好就能用。发布包只带核心词库，日文词典、五笔码表（水杉五笔除外）、粤拼/注音/笔画词库和手写模型由设置应用下载到 `~/Library/Application Support/app.msime.macos/resource-packs/<id>/`；`resources/voice-runtime-macos.lock.json` 固定了重签的语音运行库之后，签名的发布包也不带 sherpa-onnx 运行库，改由资源包 `voice-runtime-macos` 下载。开发构建默认照旧内置运行库，要看省略之后的样子就在 CMake 配置时加 `-DMSIME_BUNDLE_VOICE_RUNTIME=OFF`，此时只有本机装着由同一 Team ID 签名的 `voice-runtime-macos` 资源包，本地识别才加载得了。要在本机测试下载流程，暂存时改用 `MSIME_MACOS_OMIT_ON_DEMAND=1 bash platforms/macos/stage-resources.sh <目录>`（不准备 `target/language-dictionaries`），并先删掉 `resource-packs/` 下已装的资源包；`cargo run --quiet --locked -p msime-client-providers --example install_resource_pack -- <绝对路径的 state_root> [japanese|language-dictionaries|handwriting]` 用 App 同一个安装器直接装资源包。

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
- `xcodegen generate` 会做 spec 校验，缺 `target/ios/EngineResources` 及其中的 `msime-dictionary-manifest.json` 直接失败——先 stage 再生成工程。
- 改了 `App/Sources`、`SharedUI`、`KeyboardTests` 下的文件要 `xcodegen generate` 并提交 `project.pbxproj`：它逐个列出源文件，不重新生成，新文件不会被编译。
- Xcode 27 的模拟器界面是 `DeviceHub.app`，`Simulator.app` 已不存在；`xcrun simctl` 一切照常，设备状态以 `xcrun simctl list devices` 为准。
- 验收系统键盘与主应用的共享设置时，模拟器构建使用 `CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- CODE_SIGNING_REQUIRED=YES`。`CODE_SIGNING_ALLOWED=NO` 的产物能启动，但没有 Xcode 链入的 `*-Simulated.xcent` 权限，主应用添加的方案可能传不到扩展；事后仅用 `codesign --entitlements` 重签也不能补回这段链接输入。安装并打开主应用后，用 `xcrun simctl get_app_container <设备> app.msime.ios groups` 确认有 `group.app.msime.ios`，再开始跨进程验收。进程内构造键盘控制器的测试不能代替这一步。

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

- **复用缓存也必须带齐包内容。** Hvigor 不会把顶层符号链接形式的 `entry/libs/` 当作原生库输入；另一检出的 `resources/resfile/` 缺失时也能构建和签名成功，但键盘缺词库就起不来。复用既有产物时把这两个暂存目录作为真实目录交给打包，安装前打开 HAP 核对原生库、核心词库、音效与许可证；仅改 ArkTS 时可与已验证的包逐项比较，除代码和源码映射外的内容应相同。签名校验与安装成功都不能代替这个检查。
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

## 在别人的真机上自动打字

**脚本打出来的字会变成用户的个人数据。** 在用户自己的手机上跑测速或回归时，首选上屏的每个词都会被「记忆新词」记住，打字时自动造的词进了「我的词条」，云同步再把它们推到同一账号的其他设备，候选翻译把每页候选发给服务端。2026-10-29 在用户的 Pixel 上跑测速时这几项都开着：约一千次空格上屏全被学进了用户的词库，其中四到六成是错的首选。

所以动手打字之前：

- **先关掉三项、记下原值、结束后按原值恢复：** 记忆新词、云同步、用水杉账号翻译候选。记录要落到文件里（读界面就存 uiautomator dump 或截图），恢复放在 `trap … EXIT INT TERM` 里，脚本中途失败或被打断也要还原；恢复后冷启动设置应用再读一遍，确认真的回到原值。
- **恢复之前先停掉设备端的输入注入，确认停了再动设置。** Android 的 `monkey`、HarmonyOS 的 `uitest` 跑在设备上，杀掉本机的 adb / hdc 不会结束它们（脚本没有终端时 adb 不分配 PTY）。不先停，剩下的点击会落到正在恢复的设置页上误切别的开关，键盘重新起来后打进去的字又会被学进词库。Android 用 `adb shell pidof com.android.commands.monkey` 找到进程逐个 `kill`，再查一次确认没有了；还在就别动设置，记录留着待恢复。
- **记录写明是谁持有的。** 记录里写持有它的进程号；另一个脚本遇到持有者还活着的记录，不接管也不恢复，否则它结束时会把别人还在用的隔离提前撤掉。
- **先关云同步，最后再开。** 记忆新词随账号同步（Android 的 `input.learning`，iOS 的 `platform.ios.dictionary_learning`），云同步开着时先关学习，会把「关」传到用户的其他设备上。用水杉账号翻译候选（`translation_account`）不在同步字段里，关它不受同步影响。重新打开云同步会把全部分类标成待上传并马上比对一次；如果弹出「合并 / 使用云端」，停下来交给用户选，不要替用户点。这个对话框可能晚几秒才出来：每次读屏后都查一遍，恢复期间冷启动设置应用之前也先看一眼，否则 force-stop 会把它关掉，用户看不到这个选择。
- **同样会外发或学习输入的开关一并看一眼：** 云候选（`cloud_candidates`，把当前拼写发给所选的云候选服务；旧偏好文档缺这个键时按开启处理，用户手机上很可能开着）、AI 候选（`ai_assistant`，把拼音和前文发给用户配置的 AI 服务）、AI 辅助翻译（把候选交给 `ai_assistant` 里配置的服务翻译；各端的开关不同：桌面与 HarmonyOS 走 host-api 的 `selected_translation_services`，看的是 `translation_ai`、不看 `ai_assistant.enabled`，只关 AI 候选挡不住它；Android 的 `CandidateTranslationPolicy.route` 与 iOS 的 `TranslationProviderPreference` 在没选别的翻译服务时按 `ai_assistant.enabled` 走 AI 翻译、不看 `translation_ai`，只关 `translation_ai` 挡不住它。所以两项都关）、用小牛 / 腾讯 / 自定义服务的候选翻译（`candidate_translations` 加 `niutrans` / `tencent_tmt` / `custom_translation`）、「我的词条」的 WebDAV 词库同步。测试不是在测它们，就同样记录、关闭、恢复。匿名使用统计（`usage_reporting`）只发当天是否使用、会话是否正常结束和崩溃摘要，打字统计的「上传到云端」只传每日计数，都不含输入内容，不用为隔离去动。但测试打的字会计入用户本机的打字统计（字数、活跃时长、候选位次），开着「上传到云端」时还会合进账号的统计，显示在 QQ 群助手的 `/stats` 里：测试期间关掉上传并按原值恢复，或者在报告里说明哪几天的统计被测试污染了。
- **能用测试账号就用测试账号**，最好是没登录账号的专用设备或模拟器；用户的手机只在必须真机时借用。
- **不要把测试文本写进用户的个人数据。** 词库、常用语、剪贴板、皮肤都不往里加东西；已经学进去的，报告给用户并说明范围，不要自己去删用户的词。

三端的位置与偏好键（键都在共享偏好里，`crates/client-core/src/preferences.rs`；云同步不是偏好，开关在 Rust 的 `cloud-sync/state.json`，经 `msime_client_cloud_sync` 的 `set_enabled` 改）：

| | 记忆新词（`learning`，默认开） | 云同步 | 水杉账号翻译（`translation_account`，默认关） |
| --- | --- | --- | --- |
| Android | 词库 › 学习 › 记忆新词 | 我的 › 同步 › 云同步：词库、自造词和设置 | 我的 › 隐私 › 联网功能 › 用水杉账号翻译候选 |
| iOS | 词库 › 学习 › 学习常用词 | 我的 › 同步 › 云同步：只同步设置和无编码常用语，个人词库不经它 | 输入 › 翻译 › 翻译服务 选「水杉账号」；关掉是改选「不使用在线翻译」 |
| HarmonyOS | 词库 › 学习 › 记忆新词（共享设置页） | 没有自动云同步；账号页的「设置同步」是手动上传下载。自动外发「我的词条」的是词库页的 WebDAV「词库同步」，设置应用开着时按间隔运行 | 键盘不走水杉账号翻译（`TranslationPolicy.provider` 没有这一支）；在线翻译经小牛、腾讯、自定义服务，或者 AI 辅助（`ai_assistant` 开着且 AI 总闸 `AiGenerationGate` 开时），关输入 › 翻译 › 候选词翻译（`candidate_translations`） |

Android 的发布 APK 不可调试，`run-as` 读不到偏好文件，只能经界面读写：开关行在 uiautomator dump 里是 `checkable="true"` 的 `Switch` 节点，`checked` 就是当前值，点行的中心切换。

## 证据分级

[ARCHITECTURE.md](../../../docs/ARCHITECTURE.md) 的五级，报告时不能跨级：

1. 源码与单元测试
2. 跨目标编译或容器构建
3. 模拟器运行
4. 真机或系统入口
5. 安装、签名与真实编辑器验收

交叉编译通过不等于系统入口可用，模拟器跑通不等于真机验收完成。说到第几级就写第几级。

## 起了进程就要收尾

`pnpm dev` 的 Vite、直接跑起来的宿主二进制、Playwright / CDP 起的浏览器，都要在同一次会话里关掉。macOS 上无头 Chrome 会以同 bundle id 注册成前台实例，导致点 Dock 图标毫无反应；异常退出后按临时 profile 特征兜底清理：`pkill -f playwright_chromiumdev_profile`。
