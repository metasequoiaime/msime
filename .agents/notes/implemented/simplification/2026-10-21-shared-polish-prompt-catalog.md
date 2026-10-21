# Agent Note: 润色内置提示词归共享 Rust 目录

Status: implemented

## Problem

四种内置润色提示词分别存在 C++ 共享头、React 和 Linux Python 中；改正文需要跨三种语言同步。预设名称也与正文分开保存。iOS、Android、macOS 和 Windows 已共用 C++ 头，但仍不能和共享设置页、Linux 服务共用事实源。

## Decision

在 `client-providers::voice::polish_catalog` 持有内置方案的 id、名称和原始正文。Cargo example 一次导出共享 C++ 窄字符串目录头、Windows 设置用的宽字符串目录头、React JSON、React 的字面量 id 元组、Linux Python 字符串模块、iOS 内置方案的 Swift 名称/id 表和 Android 内置方案的 Java 名称/id 表。生成物随源码提交；Linux 设置应用 CI 在既有依赖构建后运行 `--check`，JSON 做语义比较，其余文件逐字比较。

C++ 请求头按共享目录查正文，已有自定义槽位与未知 id 的 cleanup 回退保持。Windows 设置页的 `PolishPreset` 与 `polish_presets` 引用宽字符串生成表，`LR` 字面量保持 `wstring_view` 的静态生命周期、编译期常量和原有同步访问；名称、顺序和正文随 Rust 目录生成，不在页面内手写。iOS 的 `VoicePolishSettings.presets` 先引用生成的 `VoicePolishPresetCatalog.builtIns`，再追加原有三个自定义槽位；App 的目录源文件与服务测试 target 都编译同一生成物。React 的公开 id 元组及其联合类型保持，名称和正文从 JSON 投影。Linux 仅导入生成的 `PROMPTS`，不增运行时 JSON 读取；CMake 把模块装到原有脚本目录。运行时不引入网络、偏好或新的解析失败。

Android 的 `VoicePolishSettingsPolicy` 读取包内生成的 `VoicePolishPresetCatalog.ids()` 和 `titles()`，再追加原有三个自定义槽位；生成方法每次返回独立数组，不暴露共享可变表。原有公开 `PROMPTS` 数组接口和各自定义槽位适配保持。Java 源码的自动发现把新生成物同时编入 APK 和 JVM 冒烟，不新增运行时 JNI 初始化、文件读取或解析失败。

## Alternatives considered

- 直接把 C++ 头当事实源并解析源码导出：延续现有所有权，但需要自行解释 raw string 和 C++ 表达式，且不能让 Rust 层直接拥有方案。
- 用 JSON 做唯一源、各宿主运行时解析：导出链最短，但 C++ 需要新增解析/分配与失败路径，Python 还需要定位安装的数据文件。静态生成保持现有宿主的同步常量读取。
- 让 Windows 设置页运行时把 UTF-8 目录转成宽字符串：少一份生成物，但要增加分配、字符串持有和首次访问初始化，改变现有编译期常量表；宽字符串静态投影沿用既有导出命令，不新增产品运行时的失败路径。
- iOS 通过 C ABI 动态取名称目录：可以沿用现有的 Swift/Rust 桥，但读取失败会让原本同步可靠的选择表新增初始化与错误处理；名称与 id 不需要运行时变化，沿用静态导出更适合本片。
- Android 沿用服务目录的 JNI 动态快照：可以复用现有服务桥，但读取名称会新增 native 库加载和解析依赖，原来无需桥的选择表也无法直接由纯 JVM 冒烟覆盖。内置 id 和名称随版本固定，静态导出与其他宿主一致；生成方法只返回调用方独有的数组，避免额外引入可修改的共享目录。
- 同时把提示词选择策略全部迁入新 C ABI：能减少更多宿主规则，但要重接六个平台的调用、边界检查、分配与释放；本片先统一正文和名称，已有槽位规则继续由现有回归保护。

## Consequences

本片迁移的 C++ 请求正文、React 与 Linux Python 消费路径读取同一个 Rust 目录。Windows 原生设置页、iOS 与 Android 内置方案选择表同样读取共享导出；macOS 回退页的方案名称及鸿蒙独立的简版正文尚未迁移，它们继续需要同步，后续逐片处理。维护七份生成物与一个导出命令，跨语言发布时必须带齐；真实导出检查、跨语言契约和 Linux 安装接线承担这项维护成本。没有把提示词选择规则移到 Rust，宿主仍保留已有自定义槽位适配，后续可在统一 C ABI 时继续收敛。

检索活跃笔记中的 `PolishPrompt`、`polish-presets` 和「润色提示词」后分类：

- [Windows 语音页](../feature/2026-10-11-windows-settings-voice-page.md)与正文来源部分重叠，请求发送用的 C++ 共享头与原生设置页均读取共享静态导出；原生页面布局、选择规则和服务目录决定继续适用。
- [共享设置页语音目录](2026-10-21-shared-ui-voice-catalog.md)拥有服务元数据的 Rust 导出，本片复用静态投影方式，提示词正文是独立目录，不取代服务目录。
- [Android 润色配置](../feature/2026-10-12-android-voice-polish-and-credential-test.md)、[Windows 语音页布局](../feature/2026-10-20-windows-settings-voice-ai-mac-parity.md)保留各自的设置接线与页面归属，与正文来源无关。
- [iOS 与鸿蒙设计](../feature/2026-10-07-ios-harmony-all-platform-design.md)、[工具与支持设置精简](2026-10-16-macos-tools-support-settings-prune.md)、[Android 社区内容归属](../feature/2026-10-11-android-community-into-feature-pages.md)命中其他 AI 提示词或润色入口，不属于语音内置正文目录，保留原决定。

## Verification

- 合成 React 目录测试在旧常量实现上先失败，接入 JSON 后通过；连同槽位和设置页回归共 3 文件、10 项通过。生产构建含类型检查、定向 lint 和 exporter 严格 clippy 通过。
- 一次性基线比对确认四篇正文与重构前 C++、Python、React 全部逐字相等。跨语言契约编译真实 C++ 消费头，再逐项对照 UI JSON 和 Python 字符串；既有 C++ 槽位/回退回归及 Linux provider 回归通过。
- 五个生成物分别注入陈旧值，`--check` 均非零退出并指出对应路径；恢复后通过。JSON 修改实际字段值，其余文件修改字节，覆盖语义比较与逐字比较。
- 独立审查通过，核对正文、公开字面量类型、未知与自定义槽位、空白正文、包含链、Linux 安装规则及 CI 检查入口。没有执行设备验收；本片只更换静态正文来源，不增加产品运行时文件访问。
- 完整 `bash scripts/verify-local.sh --quick` 通过：Rust workspace、Android 交叉检查与宿主契约、Wasm、Linux 设置应用、Linux 原生 108 项测试与版本构建、macOS 原生构建及共享 Apple bridge。完整版与五笔版安装目录中的新 Python 模块均与源文件逐字一致。Windows 原生交叉构建因缺少 MinGW、HarmonyOS 原生包因缺少 staged 输入按门禁明确跳过。

### Windows 原生设置目录验证

- 跨语言契约先在缺少宽字符串导出时编译失败，接入后通过；它编译真实 `VoiceSettings.h` 并逐项输出宽字符码点，在 Python 中与 JSON 的 id、名称、顺序和完整正文比较，不依赖系统控制台区域设置。
- 一次性编译改动前与改动后的 Windows 设置头，四项 id、名称和正文逐字相等。`voice_settings.cpp` 以 `c++ -std=c++20 -Wall -Wextra -Werror` 编译运行通过，既有选择和自定义槽位规则保持。
- 把生成宽表的一项名称改成合成值后，Rust 导出检查与实际消费契约都失败；恢复后两者通过。Rust 格式和 exporter 严格 clippy 通过。
- 独立审查通过：另行编译改动前后头文件并逐字对照，核对五份导出、MSVC 的 C++20 与 `/utf-8`、相对包含链、静态引用生命周期及页面遍历和字符串复制调用。没有在本机运行 Windows 窗口，真实 MSVC 构建与页面验收由 PR CI 运行。
- 本片完整 `bash scripts/verify-local.sh --quick` 通过，覆盖 176 项 Windows 可移植测试、仓库契约、Rust workspace、Android 交叉检查与宿主测试、Wasm、Linux 设置外壳、Linux 原生 108 项测试及版本安装检查、macOS 原生构建和共享 Apple bridge。缺少 MinGW 的 Windows 原生交叉构建与缺少 staged 输入的鸿蒙原生包按门禁明确跳过。

### iOS 内置方案目录验证

- 新增 Swift 静态导出检查先在缺少生成文件时编译失败；生成后用真实 Swift 编译器执行目录并对照共享 JSON，四项 id、名称和顺序一致。没有 Swift 编译器的环境明确跳过这一部分，已有 C++ 与 Python 消费检查继续运行。
- iOS 服务测试核对实际 `VoicePolishSettings.presets` 的内置目录、自定义槽位名称与顺序、以及各方案从偏好读回后的 id；未知 id 与正文回退继续由既有用例保护。原生模拟器测试由 PR CI 执行，本机不据静态目录检查宣称窗口验收。
- 抽取改动前后真实 Swift `presets` 声明并编译执行，七项 id、名称和顺序逐字一致。临时把生成名称改成合成值，导出检查和 Swift 编译消费契约同时失败；恢复后通过。Rust 格式、exporter 严格 clippy 与 25 项 iOS 项目配置测试通过；本机 XcodeGen 校验因未暂存 `target/ios/EngineResources` 被资源前置阻止。
- 独立审查通过：另行编译七项选择表对照 base，验证 Rust Debug 字符串经 Swift 编译往返，覆盖引号、反斜杠、控制字符、Unicode 与插值形状；核对 App 与服务测试的源包含、真实选择器消费和原有槽位/回退。
- 本片完整 `bash scripts/verify-local.sh --quick` 通过：176 项 Windows 可移植测试、仓库契约、Rust workspace、Android 交叉检查及宿主测试、Wasm、Linux 设置外壳、Linux 原生 108 项测试及版本安装检查、macOS 原生构建和共享 Apple bridge。缺少 MinGW 的原生 Windows 构建与缺少 staged 输入的鸿蒙包按门禁明确跳过。

### Android 内置方案目录验证

- 新增 Java 消费契约先在缺少生成文件时编译失败；接入后编译真实 `VoicePolishSettingsPolicy` 并执行，七项 id、名称和顺序与共享 JSON 加三个自定义槽位一致。生成数组被调用方改写后再次读取保持原值，没有 JDK 17 的环境明确跳过这一部分。
- 改动前后的真实设置策略分别以 `javac --release 17 -Xlint:all -Werror` 编译，并运行同一份完整七项回归；null/空选择、未知 id 原样显示、自定义字段与状态提示均保持。临时改写 Java 生成名称为合成值，Rust `--check` 与实际 Java 消费契约均非零退出并指出不一致，恢复后通过。
- 独立审查通过：另行编译 base 与当前策略，七项 id、名称和顺序逐字一致；真实 `serde_json` 字面量经 `javac --release 17` 往返核对全部 ASCII 控制字符、中文、补充平面字符、U+2028/U+2029 和反斜杠加 Unicode 转义形状。APK 的 Gradle Java 源根与 JVM 冒烟的源码发现均包含新类。
- 完整 `bash scripts/verify-local.sh --quick` 通过：176 项 Windows 可移植测试、仓库契约、Rust workspace、Android 交叉与 JVM 检查、Wasm、Linux 设置外壳、Linux 原生 108 项测试、macOS 原生构建及共享 Apple bridge。缺少 MinGW 的 Windows 原生交叉构建与缺少 staged 输入的鸿蒙包按门禁明确跳过；本片只更换静态目录来源，没有新增运行时文件访问或 Android 资源，未做设备界面验收。Rust 格式、exporter 严格 clippy 和笔记校验通过。
