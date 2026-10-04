# 实现说明

这份文档描述水杉输入法客户端的最终形态：共享层与各平台宿主分别承担什么、边界画在哪里、为什么这么画。功能对照与逐项覆盖情况见 [windows-parity](windows-parity.md)、[macos-parity](macos-parity.md)、[ios-parity](ios-parity.md)、[harmony-parity](harmony-parity.md) 以及对应的 feature inventory；构建与验证命令见 [ARCHITECTURE](../ARCHITECTURE.md) 和各平台 README。

## 一、目标与范围

一个仓库承载六个平台（Android、iOS、macOS、Linux、Windows、HarmonyOS）的输入法客户端：业务逻辑写一次放在 Rust，管理界面写一次放在 React，输入算法完全留在上游 C++ msime-engine，各平台只保留自己的系统入口和平台能力适配。

范围内的是「宿主编排」：会话生命周期、焦点、候选分页与身份、偏好的持久化与应用时机、账号与云服务、资源分代安装、面板与工具栏的展示状态。范围外的是「输入算法」：拼音切分、词格仲裁、候选排序、学习与调频、日文转换表，这些全部由 Engine 提供，客户端只传配置、取快照、按身份回选。简繁转换是共享层 `crates/client-core/src/chinese_conversion.rs` 按 OpenCC s2t 实现的词级转换，由宿主在显示与上屏边界调用。

一条贯穿全仓库的规则：**Tauri 是公共组件，不是任何平台的产品本体**。最终安装、启动、被系统识别为输入法的，始终是 `platforms/<os>` 下的原生宿主；Tauri/React 由它按需承载（`platforms/ios/build-app.sh` 顶部的注释写明了这一点，iOS 上只有 `MSIME_IOS_TAURI_COMPONENT=1` 才单独构建那部分）。

## 二、分层与目录

### Rust workspace

工作区共九个成员：八个 `crates/*` 加 `apps/desktop/src-tauri`。`default-members` 刻意排除 `msime-desktop`——它需要已构建的前端和已打包的平台 bundle 才能跑 build script，所以裸 `cargo test` 只覆盖库。工具链钉在 `rust-toolchain.toml` 的 1.97.1；`[workspace.lints.rust] unsafe_code = "deny"` 用 `deny` 而非 `forbid`，因为四个 FFI crate 需要在 crate root 写 `#![allow(unsafe_code)]` 作为有据可查的豁免，`forbid` 下它们只能整体不继承 lint 表。

| crate | 职责 |
| --- | --- |
| `msime-client-core` | 宿主无关的客户端业务：`preferences`、`account`、`ai`、`cloud`、`community`、`credential`、`dictionary`、`helpcode`、`skin`、`translation`、`voice`、`clipboard`、`punctuation`、`chinese_conversion`、`typing_statistics`、`resources`、`host_surface`、`panels`。不依赖 Tauri、React、Engine 或任何平台 API。 |
| `msime-engine` | 纯 Rust 输入引擎（由 C++ MSIME-Engine 移植）：组合状态、全拼／双拼／五笔／九键／日语等方案、词库查询与分代准备、学习日志与回放、手写识别；`examples/` 下是各类真实词库探针，`tests/golden/` 是从 C++ 参考实现录下的行为基准。 |
| `msime-input-runtime` | 输入宿主的会话编排：焦点、候选翻页、代次选择、全半角转换、在线候选调度。含重排模型 `Reranker` 的接入。 |
| `msime-host-api` | 版本化 C ABI（`msime_client_abi_version()` 返回 3），132 个 `msime_client_*` 导出（头文件另有 2 个 `static inline` 辅助函数），`crate-type = ["cdylib", "staticlib", "rlib"]`。`ffi/` 按 host/session/input/candidates/lifecycle/providers/translation/voice 分文件。 |
| `msime-host-macos` | macOS 宿主的 Objective-C++ 平台能力：键盘注入、账户、剪贴板、词库、文件选择器、卸载器、录音设备枚举，以及 `panel_session`、`cloud_clipboard`、`cloud_dictionary`。 |
| `msime-host-windows` | Windows 平台能力的安全封装：语音控制器与输出、粘贴策略、Windows Ink 手写。桌面 shell 禁 unsafe，所以 Win32 调用集中在这里。 |
| `msime-tauri-mobile-platform` | Tauri shell 在 Android/iOS 上的平台能力注入插件。 |
| `msime-ios-native-ffi` | 只包住 Swift `@_cdecl` 导出的那条 FFI 边界，使 Tauri 命令 crate 不必背 unsafe 豁免。 |

### 共享界面与 Tauri 承载层

`packages/ui`（`@msime/ui`）是唯一一份 React 设置界面实现，导出 `SettingsPage`，21 个页面 id 覆盖首页、账号、AI 对话、社区、打字统计、外观、输入、辅助码、快捷键、词库、皮肤、语音、屏幕键盘、手写、实用功能、AI 辅助、悬浮工具栏、更多、帮助、关于、反馈。移动端另有一套底部 tab 映射和窄屏导航收敛：桌面才有的表面（快捷键、悬浮工具栏等）不会在没有对应宿主表面的移动端出现。

`apps/desktop/src-tauri` 是所有平台共用的 Tauri 入口库，210 个 `#[tauri::command]` 按 `platform/{android,ios,linux,macos,windows,desktop}`、`shared/`、`tests/` 分层。目录名沿用 `desktop`，但它同时是 Android、iOS 上那份合包的 Rust 入口。`apps/harmony` 只有 `main.tsx` 一个胶水文件，把同一份 `@msime/ui` 打成 HarmonyOS 能加载的单文件设置页。

### 平台目录

`platforms/{android,ios,macos,linux,windows,harmony}` 各自持有系统入口、平台适配和该平台的测试；`platforms/common` 放桌面宿主之间共用的 C++：Linux 与 Windows 编译的 `Telemetry`，以及 Linux 与 macOS 共用的词库维护租约 `DictionaryQuiesceLease.h`（单测在 `tests/`）。`shared/` 是跨语言而非跨平台的共享层：`shared/apple` 与 `shared/apple-bridge` 被 macOS 宿主和 iOS 键盘扩展同时依赖，`shared/backend`、`shared/backend-ui` 是 Swift Package 形态的账号与界面后端，`shared/voice` 是 C++ 的 provider 注册表与录音常量，`shared/snapshot` 是词库快照，`shared/input` 是 `CompositionDisplay.h` 与 `EnglishModeOutput.h` 两个 C++ 头文件。

### 引擎与资源

输入引擎是仓库内的 `crates/engine`，不再从外部拉取、也没有锁文件和 overlay 脚本：原先由 overlay 改写的行为都已直接写进对应的 Rust 模块。移植的对照基准在 `crates/engine/tests/golden/`，录制方法见 `tools/engine-golden/README.md`。平台仍在用的非引擎文件随仓库提交：IPC 契约头文件在 `shared/contracts/`，Windows 提示音用的 miniaudio 在 `platforms/windows/third_party/miniaudio/`。

`resources/desktop-dictionary.lock.json` 锁定 9 个词库 artifact（合计约 175 MB，含 `msime-pinyin.db`、`msime-japanese.dat`、`msime-bigram.bin`/`msime-trigram.bin`、`msime-english.db`、`msime-others.db`、`sentence-model.safetensors`），每项带 sha256 和长度；`resources/neural-model.lock.json` 同时锁定键盘与桌面落定两个神经模型，`scripts/fetch_neural_model.py` 将它们原子下载到 `target/neural-model`；旧的 `resources/settled-model.lock.json` 和 `scripts/fetch_settled_model.py` 仍兼容只准备桌面模型的构建。`resources/eval/` 是四套转换质量数据集及其基线，`resources/helpcodes/` 是辅助码表与其 NOTICE。校验词库目录时 `client-core` 的 `ResourceStore::verify` 要求目录恰好是锁里的产物，唯一的例外是 Engine 的 `helpcodes/` 子目录。

## 三、共享层的最终形态

### 偏好

`client-core::preferences` 是唯一的配置真相来源：值、格式版本、revision 和文件存储。同目录临时文件替换避免半写 JSON，独立锁文件协调多个进程（输入法进程、设置进程、provider 进程可能同时在写），revision 比较拒绝覆盖过期设置。`load` 遇到损坏文件、未知字段和未来格式一律报错，不静默回落默认值。修复是单独的显式动作：`recover` 先把原字节原样备份为同目录的 `preferences.json.corrupt-YYYYMMDD-HHMMSS`（UTC 时间戳，重名时追加 `-N`），再从默认值出发逐个顶层字段尝试并入原文件中仍能解析且通过校验的值，对象字段整体不合格时再逐个子字段尝试，因此服务凭据等与坏字段同段的值也会保留；无法解析为 JSON 时写出默认值。新 revision 为原 revision + 1，读不出 revision 时取当前 Unix 秒数，保证按 revision 比较的宿主一定会应用修复结果。文件本来就能读取时不做任何事。`recover_malformed` 只处理不是合法 JSON 的字节，合法 JSON 但被拒绝的文档（未知字段、未来格式）原样返回错误，供输入法进程自动修复时使用，避免覆盖更新版本写出的设置。`try_load` 提供无等待读取，供设置监听线程在锁被占用时直接返回忙状态而不阻塞。

### C ABI

会话用线程局部注册表中的整数句柄表示，错误线程和已销毁句柄不会解引用陈旧对象指针。响应是库拥有的 UTF-8 JSON，配套显式释放入口。输入缓冲区有效性和输出指针单次释放仍是 C 调用方的责任。繁简转换是唯一直接返回字符串而非 JSON 的入口——宿主对每页每个候选和每次上屏都会调用它，为此绕一次 JSON 往返不值得。

### 输入运行时

运行时缓存 Engine 值快照，界面读取不再次推进引擎。分页、高亮、候选身份归共享运行时；候选 ID 是 `{session, generation, index}` 三元组，拒绝跨会话、旧代次和当前页以外的选择。失焦取消组词，未聚焦按键透传。数字选词先让 Engine 处理字符，未处理的 1–9 再映射到当前页的全局索引，平台不实现选词规则。标点、首尾汉字选择（`select_edge`）、结束组合（`Action::Finish`）都由共享层编排调用顺序，转换算法留在 Engine。

### 资源安装

`ResourceStore` 读取受信任的产品锁，通过宿主注入的传输流安装平面文件集合，严格校验长度与摘要，用独立文件锁和临时目录发布防止半安装，不覆盖旧资源代次。词库升级按代次进行：宿主建会话前比对编译进库的词库锁代次，原子替换运行配置里的 `resources`/`dictionaries` 两项。

`client-core::helpcode` 负责读取资源目录里的用户辅助码元数据。`helpcodes/custom/*.txt` 的文件名形成 `custom/<stem>` 标识，扫描结果按 stem 排序；可选的 `# name:`、`# name_en:` 头部只用于设置界面显示，路径校验拒绝目录穿越和平台文件名分隔符。实际码表解析与候选筛选仍由 Engine 负责，Rust 不复制输入算法。

### 账号、云与社区

`account` 提供客户端、会话、校验与 API 四层，凭据留在 Rust 侧，交给 WebView 的只有脱敏 DTO；各平台的原生登录（iOS 的 Apple 登录、Android 的 Google 凭据管理器）把身份 token 直接交回 Rust 会话，不经过 WebView，也不写日志。`cloud` 下是快照队列、云词典与云候选；`community` 是社区资源与资源库；`credential` 按 doubao / translation / asr / probe 分别提供凭据形态与探测。`skin` 覆盖内置皮肤目录、AI 生成皮肤、社区皮肤、自定义皮肤库与键盘试用；`translation` 持有译义存储与自定义译义；`voice` 是 controller 与豆包帧编解码；`dictionary` 是导入与个人词库；`typing_statistics` 按用户选择的保留时长（默认永久）保留按日聚合，只记字符计数与分类，不保存输入内容。

### 有界性

所有从网络或文件进来的东西都有显式上限，越界的那一条被跳过而不影响同批其他条目：云响应 256 KiB、AI 响应 1 MiB、AI JSON 内容 64 KiB、单条候选 4096 字节、单次最多 10 条候选；剪贴板历史 50 条、单条 4000 个 UTF-16 单元（超长复制截取前 4000 个单元、不拆代理对），只有含 NUL 的文本不落盘，其余控制字符作为用户内容保存；Windows 回复编码按 UTF-16 单元计容量，超长时明确失败而不截断文本。空白、控制字符、重复项在进入候选之前被过滤掉。

### 能力声明与界面注入

React 只依赖一个 `SettingsClient` 接口；所有平台动作（读写偏好、打开外链、打开系统键盘设置、启动面板、探测凭据、枚举字体、震动预览）都由宿主注入，缺哪一项就不渲染对应控件，而不是渲染一个点了没反应的按钮。哪些控件该出现由 `client_core::host_surface::HostCapabilities` 声明：平台名、是否用移动端导航（HarmonyOS 的同一个 HAP 会跑在手机也会跑在 2in1，所以它可以按实际形态覆盖这一项而不是跟着「是不是桌面」推断）、能否重启输入法服务、能否把共享面板开成独立置顶窗口、是否保存按应用/全局的中英模式、是否记录打字统计、能否枚举系统字体、是否自绘标题栏、悬浮工具栏的四档粒度（有没有、能不能调缩放与图标大小、能不能分别显隐组件、有没有手写与语音按钮）、是否消费共享的模式切换快捷键与数字行选词等。

这份能力表是有测试守着的：每加一个宿主真的接上的字段，就在 `client-core` 的能力回归里补一条断言，并在共享 UI 里补一条「这个平台显示 / 那个平台不显示」的用例。能力表和实际消费不一致就是 bug，不是「等以后再接」。

### 数据与网络边界

默认配置下只有云联想一个功能会把输入内容发出设备（当前正在组的拼音串，发给 Google 输入工具，设置页可关，关闭后宿主既不发起请求也拒绝任何云来源候选）。其余联网功能——语音识别、语音润色、候选翻译、AI 联想、账号同步——默认凭据为空，不填密钥就不会发出请求。另有一条自有的匿名使用统计路径（`crates/client-core/src/telemetry.rs`，各宿主经 Host API 的 `msime_client_telemetry_*` 调用，端点是本项目的 `api.msime.app`），默认开启、可由 `usage_reporting` 关闭；事件里只有随机事件 id、事件类型、平台名、版本号和本机随机生成的安装 id，崩溃事件另带截断过的异常摘要与去掉目录的调用栈，不含任何输入内容、候选、剪贴板或账号标识；仓库不接入任何第三方统计或崩溃上报 SDK。完整的逐条说明在 [PRIVACY.md](../PRIVACY.md)，这份文档不重复它。

实现上与之配套的约束有两条。一是凭据不进 WebView：探测、请求构造和 token 持久化都在 Rust 或原生侧完成，交给 React 的是脱敏 DTO 和「这个服务配没配好」的布尔结果。二是诊断日志不记内容：Linux 的 `diagnostic.log` 写在偏好目录下、1 MiB 轮转一份、只用户可读，各平台的错误路径一律记类别与错误码，不记响应正文、提示词、输入串或路径。

## 四、各平台宿主的最终形态

六个宿主连到共享层的方式各不相同，但连的都是同一个 `msime-host-api` C ABI：

| 平台 | 系统入口 | 与共享层的连接 |
| --- | --- | --- |
| macOS | InputMethodKit bundle `水杉输入法.app` | 静态链接 `libmsime_host_api.a`，Swift 后端另编成 `MSIMEBackend.dylib` |
| iOS | `MSIMEApp` + `MSIMEKeyboardExtension.appex` | 静态链接 `libmsime_host_api.a`，Swift `@_cdecl` 回传给 Rust |
| Android | `InputMethodService`（`:ime` 进程） | `NativeClient` 经 JNI 调 `libmsime_android.so`，后者链 `libmsime_host_api.so` |
| Linux | IBus 组件可执行文件与 Fcitx5 addon module（并列） | 都链同一份 `libmsime_host_api.so` |
| Windows | 进程内 TSF DLL + 进程外 Server，命名管道相连 | 两者各自链 `msime-host-api` 导入库，刻意是两个构建目标 |
| HarmonyOS | `InputMethodExtensionAbility` | ArkTS 经 NAPI 调 `libmsimeclient.so`，后者链 `libmsime_host_api.so` |

### macOS

InputMethodKit 宿主，产物 bundle 名为 `水杉输入法.app`，`CFBundleIdentifier` 为 `app.msime.inputmethod.MetasequoiaIME`，最低系统 13.0，控制器类 `MSIMEInputController`（实现在 `src/input/InputController.mm`）。源码按 `src/{backend,candidate,cloud,core,dictionary,input,settings,voice}` 分层；`backend/` 下的 Swift 文件与 `shared/backend`、`shared/backend-ui` 一起编成 `MSIMEBackend.dylib` 再链进 bundle，账号面板等符号以弱链接引入，使不链接 Swift 后端的隔离测试目标仍能成立。

输入源菜单收敛为工具入口加两条出口——中英文、英文候选模式，繁体输出、全角、中文标点、显示译文四个勾选项，输入方案与主题两个子菜单，悬浮工具栏，加表情面板、云剪贴板、屏幕键盘、手写、语音，最后是「水杉输入法设置…」和「关于水杉输入法…」。「云剪贴板…」是和表情面板同类的工具：它带着当前输入会话打开共享面板，点选条目直接上屏，安全输入期间不打开。所有管理页（候选、账号、云剪贴板的开关与清空、皮肤目录）统一进设置窗口，不在菜单里各开一条路；设置窗口优先启动共享 Tauri 页面，bundle 不可用时回退到原生视图。

原生实现的能力包括：候选面板（定位、分页、hover、翻页按钮、行宽测量、位置迟滞）、四套内置皮肤与外部 `skin.toml` 皮肤包、候选注释与翻译、云候选、语音（豆包 WSS、HTTP provider、波形面板、录音设备、润色、提示音）、手写、屏幕键盘、表情/颜文字/符号/剪贴板面板、悬浮工具栏、中英模式 HUD、双拼键位提示、打字统计、诊断日志、Sparkle 自动更新。发布安装来自 DMG（`platforms/macos/package-release.sh`）：设置应用内嵌 `水杉输入法.app`，每次启动都在后台比较随附与已安装 bundle 的版本，未安装时安装、随附的更新时替换，从不降级，只在打包好的应用里进行（开发运行与输入法拉起的面板进程跳过）；设置页的「安装 / 更新」是不比较版本的强制重装。两者走同一条路径：staging 目录里完整复制、原子替换到 `~/Library/Input Methods`，再启动已安装 bundle 的 `--register-input-source` 登记并启用自身，注册失败时回滚到旧安装（没有旧安装时保留新 bundle 等下次登录），不静默切换当前输入源。开发者路径是 `platforms/macos/scripts/install.sh`：用本机 Developer ID 重签、原子替换、失败回滚，并拒绝含受限 entitlement 的签名。卸载默认只把 bundle 移入废纸篓、保留词库与学习记录，用户明确勾选才一并清除状态目录、偏好域和语音密钥。

### iOS

工程由 XcodeGen 从 `project.yml` 生成（`project.yml` 是唯一权威来源），9 个 target：主 App `MSIMEApp`、键盘扩展 `MSIMEKeyboardExtension`（`com.apple.keyboard-service`）、测试宿主、四个测试 target 和独立的 `MSIMEDoubaoTransport` framework。App 与扩展共用 App Group `group.app.msime.ios`，最低 iOS 17.0；唯一的 CocoaPods 依赖是 ML Kit Digital Ink，手写按 SDK 切源文件（device 用真实识别器，simulator 用 fallback）。

`KeyboardExtension/Sources/` 是产品键盘：候选栏与展开候选面板、方案/布局/皮肤/更多选择器、符号面板、表情与颜文字、剪贴板、日语九宫、手写、云候选、候选翻译、词库快照工作线程、英文大写与建议策略、空格移光标、标点上下文。`SharedUI/` 是 App 与扩展共用的表现层与偏好适配器；`App/` 是主应用，承载欢迎引导、账号、社区皮肤、AI 皮肤生成、云词典、个人词典、云剪贴板、聊天和十余个设置页。测试宿主声明了与扩展相同的 App Group entitlement——被测键盘要靠这个容器读共享偏好，缺它会让宿主在套件中途被杀掉。

构建链是三步：`install_resources` 取得已校验资源目录 → `platforms/ios/stage-resources.sh` 暂存 → `platforms/ios/build-app.sh <资源目录> simulator|device`。`.cargo/config.toml` 只对两个 iOS target 延后解析四个由 Xcode 编译的 Swift `@_cdecl` 符号，不做工作区级放宽，以免在其他平台掩盖真正缺失的符号。

### Android

原生 `InputMethodService` 宿主，applicationId 与 namespace 都是 `app.msime.android`，minSdk 28。服务跑在 `android:process=":ime"` 独立进程：Tauri 最后一个窗口关闭会退出进程，设置页与输入法必须分开，跨进程共享的只有那份带文件锁的偏好。ML Kit 的初始化 provider 也在同一进程。四个 `activity-alias` 实现应用图标切换。

Java 侧按 `java/app/msime/android/<feature>/` 分层（core、home、keyboard、candidate、voice、account、dictionary、policy、handwriting、statistics、community、clipboard、settings），其中 `policy/` 是一批无 Android 依赖的纯模型类——键面大小写、硬件按键映射、候选导航、被拒标点、数字行选词、候选滚动、候选折行、智能标点上下文等规则都放在这里，因此可以用 JVM smoke 直接覆盖而不需要设备。`NativeClient.java` 是唯一的 JNI 声明处，对应 `native/client_jni.cpp`。

能力覆盖软键盘 26 键与符号层、全拼九键（含数字层）、日语九键、四套双拼加微软双拼分词键、86 五笔、手写、AI 回复键盘；候选条与展开面板、候选长按管理、离线英文释义、在线候选翻译、云与 AI 联想、剪贴板历史与云剪贴板、表情浏览器、八种本地输入模式、AI 润色、语音、打字统计、内置与自定义皮肤、社区资源、账号、硬件键盘快捷键与数字行选词、大屏居中外框、无障碍键盘尺寸调整。

有两个构建脚本产出同名同路径的 `target/android/msime-client.apk`：`build-apk.sh` 出的是本目录的原生 IME（Java 来自 `platforms/android/java`，无 WebView），`build-client-apk.sh` 出的是 Tauri + React 设置合包。两者包名相同、内容完全不同，README 为此专门写了一节。Gradle 工程 `gradle-app` 的 `srcDirs` 直接指向 `platforms/android/{AndroidManifest.xml,java,res}` 和 `target/android/{host-assets,jniLibs}`，不复制任何源码。

### Linux

IBus 与 Fcitx5 是**并列的两个系统入口**，不是宿主和它的插件——`CMakeLists.txt` 里的注释和 README 都写明了这一点，两者链的是同一个 `msime-host-api` ABI。IBus 侧是 `msime-linux-ibus` 可执行文件，由 `data/msime-linux.xml.in` 生成的 component 文件注册，`<exec>` 指向随装的 `msime-linux-ibus-launcher`；Fcitx5 侧是独立子工程编出的 `msime-fcitx5` MODULE，显式 `unset(CMAKE_CXX_STANDARD)` 以免继承上级钉死的 C++17（Fcitx5 5.1 的公开头用了 `std::span`）。装了 Fcitx5 开发包或开了打包就默认构建它，没装则打印获取方式而不是静默丢掉这一半。

在线候选、语音、剪贴板、手写、emoji、词典、翻译各有独立的可执行入口，其中在线候选、语音和剪贴板另配 systemd 用户单元；前两者是 socket 激活的，`ListenStream` 落在 `%t/msime-client/` 下、`SocketMode=0600`。浮层在 `src/overlay/`，提供模式徽章和语音波形，X11 与 Wayland layer-shell 两套后端（Wayland 协议代码由 `wayland-scanner` 从 `data/wayland/` 的 layer-shell 描述加系统 `xdg-shell.xml` 生成），缺依赖时退回面板文字。诊断日志写偏好目录下的 `diagnostic.log`，1 MiB 轮转一份，只用户可读。

安装后的首次配置收成一条命令：`msime-linux-setup` 按词库锁逐个核对名称、大小和 SHA-256，调 `msime-linux-prepare` 在 `$XDG_CONFIG_HOME/msime-client` 建状态并发布 `runtime-options.json`，再把输入法加入正在运行的宿主的输入法列表：Fcitx5 经 D-Bus 追加到当前输入法组并读回核对，IBus 在引擎未被列出时先 `ibus restart`，再追加到 GNOME 的输入源或 IBus 的 `preload-engines`；任何一步失败都退回打印手动步骤，`--no-register` 跳过这一步。默认不联网，取回词库要显式 `--download`；校验不过即中止，不留半份词库；状态目录已存在时报错而不覆盖，但安装流程预先写入的匿名账号文件可以保留并继续准备。升级后自行下载的词库过期时，宿主的刷新以 `dictionary_outdated:` 报告并继续用旧代次，经 `msime-linux-first-run-guide --reason dictionary-outdated` 发通知；`msime-linux-setup --update --download` 只取回过期的几项，再用 `msime-linux-prepare --refresh` 切到新代次并回放用户词库，随包提供的词库目录留给包管理器。图形入口是等价的：`msime-linux-settings` 在缺 `runtime-options.json` 时打开首次配置页，页面跑的就是同一个 `msime-linux-setup`。卸载走 `cmake --build <build-dir> --target uninstall`，会先逐个停掉 setup 启用过的用户单元，再用 `msime-linux-setup --unregister` 把输入法从 GNOME 输入源、IBus 预载引擎和当前 Fcitx5 输入法组中移除（会让列表变空时保持原样）；`.deb` 的 prerm 对每个已登录用户做同样两步。

打包由 `cmake/packaging.cmake` 提供（`-DMSIME_ENABLE_PACKAGING=ON`），四道硬性前置：必须 Linux、前缀必须是 `/usr`、运行配置文件必须为空、本地语音必须关。版本取 `-DMSIME_PACKAGE_VERSION`（发布工作流传 `platforms/linux/version.txt` 的版本），不传时同样读 `platforms/linux/version.txt`，与 Windows 的 `MSIME_WINDOWS_VERSION` 读自己的 `version.txt` 一致；这一步在顶层 `CMakeLists.txt` 里完成，IBus 宿主的遥测以 `MSIME_LINUX_VERSION` 报告同一个版本；产出 TGZ 或 DEB，Debian 依赖声明 ibus/python3，开了 Fcitx5 再追加 fcitx5。

### Windows

两个独立目标，边界写在 `tsf/CMakeLists.txt` 的注释里：TSF tip 是注入到每个应用进程里的 DLL（`MetasequoiaImeTsf`），Server 是进程外可执行文件（`MetasequoiaImeServer`），两者刻意是分开的构建目标而不是一个目标的两种配置。DLL 侧按 `Candidate/ Compartment/ Composition/ DisplayAttribute/ Edit/ Global/ IME/ IPC/ Key/ LanguageBar/ Register/ Tf/ Thread/ UI/ Utils/` 组织，导出四个未修饰的 COM 入口；Server 侧按 `entrypoints/ ipc/ input/ candidate/ voice/ clipboard/ system/` 组织。另有 `MetasequoiaImeWatchdog`（对应安装器的登录任务）和 `msime-client-prepare`（准备 `runtime-options.json`）。`msimeui/` 是独立的 Direct2D/DirectWrite UI 静态库，带自己的公开头、demo 和测试。

IPC 是三角色命名管道（Main / Aux / Diagnostic）。`PipePeer::bind` 校验客户端 PID、登录会话和 TokenUser SID；监听器用账户 SID 构造明确 DACL、保留受限 AppContainer 权限与低完整性标签、拒绝远程连接。注册表为三个角色各维护单调代次，替换反向端点使主注册失效并取消旧读取，旧票据既不能发送也不能清理新端点。握手、写入强制有限超时，主管道的空闲读取必须携带取消事件。

焦点与输入的编排分成清晰的几层：`FocusGate` 分离传输票据、Server activation epoch 与 TSF token，新激活先 pending、worker 确认完整成功后才 ready；`FocusRouter` 把登记、激活、按键、焦点恢复、挂起、终止、断连映射到门禁和精确的会话清理任务；`InputQueue` 在单一 worker 线程创建、使用、销毁真实共享会话；`SessionPump` 把已登记的 Main 连接串到输入队列，当前回复未结算就不读下一个键；`SessionWorkers` 用固定线程槽位运行 pump，每客户端最多一个活动连接和一个待接替连接。管道读写始终留在 I/O 线程，确认回到输入队列。

回复编码是显式的：`ReplyCodec` 按上游 opcode 逐字段小端生成，零填充协议空隙，UTF-8 严格转 UTF-16 并按 UTF-16 单元计容量，超长、坏编码、NUL 和无法无歧义表示的分隔字段明确失败而不截断。`ReplyComposer` 为已认证的单次客户端激活保存尚未上屏的选词前缀，分段、最终候选、标点、预编辑、导航、UILess 各有自己的回复语义；本地完成（`LocalCommit`）必须先核对键型和完整观察文本才允许无帧确认。

候选与模式的展示走单槽邮箱：`CandidateMailbox`/`ModeMailbox` 在回复发送并确认之后才发布快照，窗口线程只读不做 I/O。候选窗口与模式面板是原生 GDI 的不激活窗口，按实际 DPI 缩放（PMv2 只在窗口创建/布局/绘制期间临时采用并恢复线程上下文），点击由单任务后台线程执行、忙碌即拒绝且不排队，控制器在真正执行前再核对一次当前候选身份。展示侧的连接校验用专用的 `try_current`：注册握手会在不持焦点锁时持有连接锁做 I/O，只尝试焦点锁不足以避免等待，忙碌时宁可隐藏候选也不让 UI 线程阻塞。

安装器是完整的 Inno Setup 工程（`installer/msime_setup.iss`）：32/64 位 TSF DLL 分别装到 `{commonpf32|64}\metasequoiaime\msime_v<ver>\` 并带 `regserver` 注册 TIP，Server 装到 64 位目录，`config.toml` 只 `onlyifdoesntexist`、升级绝不覆盖，随包装 `THIRD_PARTY_NOTICES.txt` 与 `LICENSE.txt`（GPLv3 第 4/6 条），HKLM 下写 `VersionDir`/`ServerPath`/`DataDir`。发布链是 `Package-SimplySign.ps1`：双架构构建 → `Prepare-PackageFiles.ps1` 暂存 → 签 payload → `Compile-Installer.ps1` → 签安装包；本地测试链用自签证书走 `test.ps1` / `Invoke-LocalInstall.ps1`。

### HarmonyOS

ArkTS 宿主，`module.json5` 声明 `mainElement: "KeyboardExtensionAbility"`、`type: "inputMethod"`，`deviceTypes` 覆盖 phone/tablet/2in1，四项权限（INTERNET、VIBRATE、READ_PASTEBOARD、MICROPHONE）。`inputmethodextability/KeyboardExtensionAbility.ets` 继承 `InputMethodExtensionAbility`，负责 panel 创建、caret 跟随、selectionChanged/textChanged 回调、HUD 与悬浮工具栏面板、硬件键路由；`keyboard/KeyboardView.ets` 与 `keyboard/KeyboardSession.ets` 是键面与会话的主体。NAPI 边界在 `native/client_napi.cpp`，73 个导出覆盖会话、候选管理、翻译、云与 AI 联想、语音、皮肤库、社区资源、个人词库和打字统计。

能力按平台 API 实际具备的来：手写走 Core Vision Kit 的 `textRecognition`，语音走 Core Speech Kit 的离线短语音加 HTTP ASR 加豆包流式，录音提示音与静音走 AVPlayer 与 `CONCURRENCY_PAUSE_OTHERS`，录音设备用 `AudioRoutingManager`。AI 润色改为润色光标前文本，因为 `InputClient` 只提供 `getForwardSync`/`getBackwardSync`。明确不做的几项在 README 里带 SDK 查证记录：应用图标切换（`bundleManager` 与 `shortcutManager` 都没有对应 API）、独立的表情/剪贴板窗口（在本宿主它们是键面而不是窗口）。

设置页的形态是 HarmonyOS 特有的：`resource://` 的 origin 为 null，WebView 拒绝跨 origin 拉模块脚本和样式，所以 `apps/harmony` 把同一份 `@msime/ui` 打成**单文件** `entry/src/main/resources/rawfile/settings/index.html`，由 `platforms/harmony/stage-settings.sh` 在每次打 HAP 前构建，不提交进仓库（`entry/hvigorfile.ts` 在它缺失时拒绝打包）。它以前是提交的产物，改共享 UI 的 PR 之间因此反复冲突；`scripts/test-harmony-settings-bundle.py` 现在检查它没有被重新提交，并且仍能构建成单文件。

`ohpm install` 与 `hvigorw assembleHap` 两步缺一不可：缺前者则 `import client from 'libmsimeclient.so'` 解析不到 `.d.ts`，ArkTS 会把整个 NAPI 边界当无类型并照样打包成功；`tests/run.sh` 只编译 `.ts` 不编译 `.ets`，ArkTS 对 `@Builder`/`build` 体内局部变量、对象字面量必须对应已声明接口等限制只有真打包才抓得到。原生库产出 `libmsime_host_api.so`、`libmsimeclient.so` 与 `libc++_shared.so` 三个文件，最后一个必须随包——OpenHarmony 不给应用提供系统副本。

## 五、关键设计决策

**候选身份是三元组围栏。** 候选 ID 携带 `session`、`generation`、`index`，任何选择都要同时匹配这三项和当前页。这一条约束覆盖了所有异步来源：迟到的云候选、AI 候选、在线翻译、后台释义都用代次判断是否还属于当前视图，属于旧代次就丢弃而不是写进新会话。同一条围栏也用在展示上——Android 的候选条以 `{session, generation, page}` 作为滚动身份，任一项变化就回到当前页首项，而同代次内的释义或翻译重绘不抢用户的横向位置。

**偏好用 revision CAS，不是 last-write-wins。** 输入法进程、设置进程和各 provider 进程会同时读写同一份 `preferences.json`。保存时带上读到的 revision，不匹配就拒绝并把冲突交回调用方；界面保留用户编辑而不是静默覆盖。原生宿主自己的设置窗口（macOS 的语音 provider 窗口、外观页）也走同一条 CAS 快照路径，不另开第二套配置事务，否则两边各存一份很快就会互相覆盖。

**需要重建 Engine 的配置等到组词结束才应用。** 快照进来时若组合处于活动状态，只保留最新的一份，等结束组合或失焦后再应用；拒绝旧 revision、同 revision 不同内容和非法配置。替换 Engine 前先验证新实例，保留会话句柄与焦点、推进视图代次；重建失败保留旧会话、待应用配置和已完成提交，等下一次重试。纯展示字段（字号、颜色、皮肤、布局）不走这条路，直接刷新渲染状态，因为它们不需要重建任何东西。

**Tauri 是公共组件，不是产品本体。** 六个平台的产品形态都是 `platforms/<os>` 下的原生宿主，Tauri/React 由宿主按需承载。这条决策直接决定了几个看起来不一致的选择：iOS 的默认构建产物是 XcodeGen 工程的 `MSIMEApp` 而不是 Tauri 归档；Android 把 `InputMethodService` 隔离到 `:ime` 进程，因为 Tauri 最后一个窗口关闭会退出进程；macOS 的原生入口在 Tauri bundle 不可用时全部有回退路径；HarmonyOS 干脆把设置页打成随包的单文件 HTML。

**展示状态只在回复被确认之后发布。** Windows 的候选窗口、模式面板和展示快照都不在 Engine 刚算完时更新，而是等回复写进管道并被对端确认、且焦点门禁仍然有效之后才发布。这条顺序换来两件事：窗口上看到的候选一定是编辑器里正在生效的那一组；投递失败或焦点过期时没有一个「已显示但没送到」的中间态需要回滚。隐藏事件同样是语义动作而不是视觉动作——宿主发来隐藏就调共享取消命令清掉组合与已选前缀，此后的显示/移动不复活旧内容，新输入重新发布。

**UILess 标志按激活继承，不只看单个包。** 宿主可能只在激活时声明自己接管绘制，之后的按键包里不再带这个标志。会话泵为每个已登记的 Main 流保存已接受激活的标志并向后续键与候选事件合并，因此这类宿主不会被误判成 Local 无回复编辑；拒绝路由不改变状态，接受失活才清除。

**「不确定」是一种必须表达的投递结果。** 管道写入分 `Sent`、`DefinitelyNotSent` 与 `DeliveryAmbiguous` 三种，平台适配层用单元测试锁定这三者的区别。歧义投递沿既有 epoch 恢复路径处理，不自动重发也不伪造确认——重发一次已上屏的提交比丢一次提交更糟。

**从共享设置页移除没有宿主表面的入口。** 一个能保存但没人消费的开关比没有这个开关更糟。所以：`menu_theme` 只在 macOS/Windows 有原生菜单，移动端就不显示；macOS 关于页不显示 Windows Server、TSF 或 Linux IBus 的诊断开关；macOS 语音设置页不显示无法提交到 IMK 会话的 Tauri 语音面板按钮，改为指向输入法快捷键或悬浮工具栏；辅助码页按宿主而不是按形态划分——Android 键盘在发 Shift 辅码所以可进入，Apple 键盘扩展没有辅助码输入所以继续隐藏。反过来，Android 候选栏真的消费了字体、颜色和皮肤字段之后，`HostCapabilities` 才把对应能力置为可用。

**平台不复制输入算法，只维护绑定。** 以词定字调 Engine 的首尾字接口；数字选词先让 Engine 处理字符再映射当前页索引；全拼纠错、混输、调频、辅助码筛选、日文转换全部是传配置给 Engine；简繁由宿主在显示与上屏边界调用共享导出 `msime_client_simplified_to_traditional` 转换，平台侧不持有转换表；标点由共享层编排「先结束组合再转换」的顺序，转换表在 Engine。平台侧留下的是按键身份判定、修饰键规则、回复类型选择这类真正属于宿主的东西。

**硬件快捷键按宿主能力分别实现，语义对齐。** 裸 Shift / 裸 Ctrl 切中英有 500 ms 上限和「期间无其他键」约束，避免普通大写字母或编辑器组合误触发；Ctrl+Shift+F 切简繁；数字行选词按物理数字行取键（AZERTY 也能用）而不是按字符。Windows 从 TSF 键包取布局转换后的 wch 并规范化小键盘数字，Android 用 `HardwareShortcutPolicy` 与 `HardwareKeyPolicy` 映射到共享命令，HarmonyOS 用 `HardwareKeyDispatch` 处理 2in1 的和弦，Linux 在 Fcitx5 侧消费共享的四个模式快捷键与作用域设置。

**文件锁是跨进程协作的基础设施，不是可选优化。** 偏好、剪贴板历史、打字统计、词库快照都有多个进程同时访问：Android 的 `:ime` 与设置进程、iOS 的键盘扩展与主 App、Linux 的宿主与各 provider、macOS 的输入法与设置应用、Windows 的 TSF DLL 与 Server。每一处共享状态都配了独立锁文件和原子替换，写入路径不因为「大概不会同时发生」而省掉。

**外部皮肤包的资源解析在 host 边界完成。** 浏览器按样式表位置解析相对 `@import` 与 `url()`，而共享预览用 constructed stylesheet，`@import` 会被直接丢弃、子目录里的资源地址还会被误当成相对包根。所以改为在 host 边界先读同一包内的 CSS，递归展开导入并把每份样式表的资源路径归一到包根；远程、绝对和越出包根的导入删除并报 partial，循环、深度、文件数与总字节都有上限。Tauri 命令只接受 package id 与已归一化的相对路径，Rust 侧重新验证 manifest、目录 containment、MIME、单文件大小与 UTF-8，不向 webview 暴露文件系统路径。

**引擎改动直接改 `crates/engine`，改完对照基准。** 引擎曾是钉死的外部 C++ 归档，行为改动只能靠取回后改写源码的 overlay 脚本叠上去；移植成 Rust 后这一层整个去掉了。代价是没有上游可以同步：引擎的行为以 `crates/engine/tests/golden/` 的基准和各模块单测为准，改动行为就同时更新基准，并写明为什么。

**候选窗口的位置有记忆，翻页没有。** 竖排候选在一次组词期间记录出现过的最高卡片高度，用最高高度决定是否从光标下方翻到上方，实际放置仍用当前页高度；候选面板隐藏后清除该记忆。这样短页不会在同一次组词里因为暂时变矮而跳回光标下方，也不会在翻转后留下按最高页算出来的空洞。`candidate_follow_cursor` 关闭时，面板在一次组词期间锁定首次有效光标位置，切换会话、结束组合或重新开启跟随才清除锚点。Linux 的候选位置由桌面 panel 管理，这套逻辑不适用，也没有硬塞进去。

**触屏候选和硬件候选是两套语义，共用一份身份。** 触屏 chip 上不画序号——序号属于无障碍描述和硬件数字行选择，画在候选文字前面会让用户以为数字是候选内容。所以 Android 普通候选条去掉视觉序号，content description 保留「候选 N」，`{session, generation, index}` 不变，无障碍树和外接键盘数字选词都不受影响。同理，候选词本身不省略也不折行（横向条靠外层滚动承载宽度，展开面板按候选格换行），但开了双语言释义时按钮允许多行——否则第二行释义会被单行约束裁掉。

**设置外壳是单实例加短暂驻留。** Linux 与 Windows 的设置外壳收到二次启动时在主线程切换页面或重新打开辅助面板，而不是再起一个进程；主设置窗口关闭后隐藏并保留十分钟才真正退出，避免从输入法菜单每点一次就付一次冷启动。受限的 surface route 同时放进环境变量和 argv，这样二次启动能把「要打开哪个面板」带过来。

**语音 provider 的凭据按 provider 分槽，不共用。** 两个 OpenAI 兼容服务可能用同一个 origin，Keychain account 因此带上 provider id；切换 provider 时把草稿放回原槽位，返回时恢复，保存只删除同一 provider 被替换的旧 origin。默认 endpoint 与 model 随 provider 更新，用户手填的值保留。这套规则在共享设置页、macOS 原生备用窗口和各平台的配置解析里用的是同一份表。

## 六、测试套件的形态

测试跟着边界走：能在宿主机上跑的就别要求设备，需要设备才成立的就明确标成设备套件，两者不互相冒充。

**共享 Rust。** 裸 `cargo test` 覆盖八个 `default-members`（`client-core`、`engine`、`input-runtime`、`host-api`、`host-macos`、`host-windows`、`tauri-mobile-platform`、`ios-native-ffi`）；`verify-local.sh` 另外把 `msime-desktop` 一并跑上，并单独检测「编译失败」而不是把它误读成「零个失败测试名」。真实词库不是单测的前提：`crates/engine/examples/` 下是一批探针，接收一个已备齐的资源目录后在临时用户目录里验证具体行为（九键候选顺序、调频五种模式、混输优先级、辅助码五种方案、双拼四套键位、首尾字抽取），原资源不被改写。`crates/input-runtime/examples/` 另有 `convert_eval` 与 `rerank_latency`，前者跑 `resources/eval/` 的四套数据集比对基线，后者量重排的逐键延迟。

**共享界面。** `apps/desktop/tests/` 下 99 个 Vitest 文件按 account / candidate / chat / community / core / dictionary / emoji / input / settings / skin / support / voice 十二个域组织。写这些用例有一条必须遵守的时序：`render(<SettingsPage …>)` 之后要先等初始加载落定再去点侧栏，页面此时还在解析快照，加载完成后自己做的选择会覆盖掉提前点下的分类——不等就会得到一个随机失败、看起来像产品回归的用例。

**macOS。** `platforms/macos/CMakeLists.txt` 注册 127 项 CTest，加上 `ClipboardTests.cmake` 与 `shared/voice` 共约 141 项，另有三个标签（`emoji-local`、`clipboard-local`、`handwriting-local`）用于隔离需要本机环境的那部分。`tests/settings/` 下还有一批 Python 校验型用例，检查设置路由覆盖、偏好覆盖、bundle 内容、Info.plist 的图标/用途/名称键和 entitlements 守卫——这些是「打包结果对不对」的检查，不是行为测试。

**Windows。** `platforms/windows/CMakeLists.txt` 96 项、`tsf/CMakeLists.txt` 19 项，加上 `msimeui` 自己的套件。关键的一点是这些套件在非 Windows 机器上也真的跑：把 Rust `msime-host-api` 编出来指给 `-DMSIME_HOST_LIBRARY`，边界测试就在本机驱动真实的共享库和真实 engine。要跑交叉产物本身则走 `run-tests-wine.sh`，它把 C++ 套件与 `cargo test --no-run` 产出的 Rust 套件一并在 `xvfb-run wine` 下执行，每个 120 秒超时，结果与基线清单比对。`tsf/tests/exports/` 那组只读 PE 导出表、不加载 DLL，因此不需要 Windows。

**Linux。** `platforms/linux/CMakeLists.txt` 35 项 CTest 加 `fcitx5/` 的 4 项（后者条件注册，需要已校验词库），另有约三十个 Python 用例分布在 candidate / clipboard / core / dictionary / input / provider / runtime / voice 下。编译门禁与隔离验收分开：`build-container.sh` 用的镜像刻意不装 X11/XFixes/Fcitx5 开发包，`check-container.sh` 才起独立 D-Bus 与 IBus daemon 做真实输入。两个容器脚本都按 `$repo_root/vendor` → 主 worktree 的 `vendor` 顺序找 Engine 并只读挂进去，构建镜像按仓库路径哈希打 tag，避免多个 worktree 互相覆盖。

**Android。** `platforms/android/tests/<feature>/*Smoke.java` 共 73 个文件，`check-host.sh` 编译并执行其中 71 个——它们全部不依赖 Android 运行时，因为被测对象是 `policy/` 下那批纯模型类。同一个脚本还做十二组契约守卫，逐条比对 Android 侧与 `crates/host-api/src/ffi/input.rs`：命令 9 必须仍是 `Action::Finish`、命令 10 必须仍是 `CycleKanaVariant`、`KEYCODE_FORWARD_DEL` 必须映射到 `DeleteForward`，并禁止 Android 自己复制假名变体表、双拼 profile 表和润色 prompt 表。设备套件在 `tests/device/`，17 个类，默认跑 11 个 instrumentation，统计、设置和手写三组各自用开关追加；`start-emulator.sh` 校验设备类型和专用 AVD 名，不操作用户现有真机。

**HarmonyOS。** `platforms/harmony/tests/keyboard-logic.test.ts` 用 1608 条断言覆盖键面布局、候选、输入模式与策略层，`tests/run.sh` 只要 tsc 加 node 就能跑，因此可以进常规门禁。它编译 `.ts` 但不编译 `.ets`，所以 ArkTS 的语法限制仍然只有真打包才抓得到——这正是 `hvigorw assembleHap` 必须在门禁里的原因。

**iOS。** `MSIMEClientTests` 聚合键盘、服务、共享三个单测 target，`MSIMEDoubaoTransportTests` 与 `MSIMEClientUITests` 各自是独立 scheme；`tests/settings/ProjectConfigurationTests.py` 是纯 Python 的工程配置校验，不需要模拟器。模拟器套件需要已暂存的词库资源，单次约十分钟量级。

**契约门禁。** `scripts/` 下的每个 `test-*.py` 都由 `scripts/run-checks.sh` 自动发现执行（少数需要参数或由别的门禁负责的列在它的 `special_checks` 里），`verify-local.sh` 与 `Core CI` 的 contracts job 都调用它，多数不需要任何工具链，检查的是「这个东西是否还接在一起」：路径与能力映射、配置 key 是否都落到共享设置页上、界面动作是否都能在仓库里检索到对应 token、生成产物是否与源重建后逐字节一致。其中一组依赖一份外部参考实现检出（由 `MSIME_REFERENCE_DIR` 指定），没有检出时它们自报所需条件并通过，不阻塞其他人。

## 七、本地验证入口

`scripts/verify-local.sh` 是统一入口，三种用法：`--quick` 只跑编译阶段（pre-merge 门禁，`.githooks/pre-push` 与 `pre-merge-commit` 直接 exec 它），无参数跑全量，`--update-baseline` 把本次新观察到的失败追加进基线清单（只追加，不删除也不重排）。

它的核心设计是**把失败的测试名集合与 `scripts/known-failures.txt` 比对，只对不在清单里的名字失败**。多个套件有长期失败，裸 pass/fail 没有信息量；清单里每一条都带完整的取证记录，文件开头写明「Every line here is debt, not an exemption」。

阶段顺序：自动发现的 `scripts/test-*.py` 静态与契约门禁 → Rust workspace 编译 → Android 目标编译与 host 检查 → HarmonyOS ArkTS 打包 → Linux 桌面 shell 与原生宿主 → Windows 交叉构建与 Wine 套件 → macOS 与 shared apple bridge → pipe-only 配置（`--quick` 到此为止）→ Rust 测试、fmt、clippy（七个 crate 的 `-D warnings` 硬门禁，无基线）→ 前端 lint 与格式 → 依赖 advisory → 整句转换质量评测与重排延迟 → 各平台 ctest → TypeScript 与 Vitest。缺少某个工具链时该阶段明确跳过并打印所需条件，不静默通过。

各平台的直接入口：

```sh
bash scripts/verify-local.sh --quick            # 合并前门禁
bash scripts/verify-local.sh                    # 全量

ANDROID_SDK_ROOT=<SDK> bash platforms/android/check-host.sh          # 契约守卫 + JVM smoke
ANDROID_SDK_ROOT=<SDK> bash platforms/android/tests/device/smoke.sh emulator-5580 [--settings --statistics --handwriting]

bash platforms/linux/build-container.sh                              # 容器内构建 + ctest，非 Linux 机器也能跑
bash platforms/linux/tests/tools/check-container.sh /abs/verified-resources [--fcitx5|--ibus-1.5.32]

bash platforms/windows/build-cross.sh x64                            # MinGW 交叉构建，产物在 target/windows-full/x64
bash platforms/windows/run-tests-wine.sh x64                         # 在 Wine 里跑交叉产物
.\platforms\windows\Build-Client.ps1 -X64Dependencies ... -X86Dependencies ...   # Windows 上的 MSVC 全量构建

bash platforms/harmony/tests/run.sh                                  # ArkTS 逻辑测试

platforms/ios/build-app.sh "$resource_dir" simulator|device
xcodebuild test -project platforms/ios/MSIMEClient.xcodeproj -scheme MSIMEClientTests ...

cmake -S platforms/macos -B target/macos-isolated -DMSIME_HOST_LIBRARY=... -DMSIME_SPARKLE_ROOT=...
ctest --test-dir target/macos-isolated --output-on-failure
```

几个跨平台的前置条件是硬性的，脚本会直接报错而不是继续：macOS 的 CMake 配置要求 Sparkle 2.9.6 就位且 `msime-host-api` 已用 Cargo 构建；Android 的原生构建校验 NDK 的 `source.properties` 必须是钉死的那个版本、vcpkg 检出必须是钉死的那个提交；HarmonyOS 要求 `MSIME_OHOS_NDK` 指向含 `ohos.toolchain.cmake` 的 native SDK；Windows 的 `build-cross.sh` 校验 vcpkg HEAD 必须是清单里那个提交且工作区干净。

macOS 上设置最低系统版本要**按目标语言分别下发**：`CFLAGS`/`CXXFLAGS` 给 C 与 C++，`CMAKE_OSX_DEPLOYMENT_TARGET` 给 CMake 构建，不要用 `MACOSX_DEPLOYMENT_TARGET`。在当前 rustc 上，那个变量会一并作用到为宿主编译的 proc-macro 动态库，产出带 `minos 13.0` 的产物之后 rustc 自己就加载不了它；cargo 不把该变量算进指纹，于是某个产物目录里留下一份坏的 proc-macro 就会被后续构建一直复用，失败看起来时有时无。

`.githooks/`（需 `git config core.hooksPath .githooks` 启用）提供两道更轻的网：`pre-commit` 是亚秒级的冲突标记扫描加 staged 文件的 rustfmt 与前端格式检查；`pre-merge-commit` 与 `pre-push` 都直接跑 `verify-local.sh --quick`。
