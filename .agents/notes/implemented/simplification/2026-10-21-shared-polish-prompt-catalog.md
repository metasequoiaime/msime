# Agent Note: 润色内置提示词归共享 Rust 目录

Status: implemented

## Problem

四种内置润色提示词分别存在 C++ 共享头、React 和 Linux Python 中；改正文需要跨三种语言同步。预设名称也与正文分开保存。iOS、Android、macOS 和 Windows 已共用 C++ 头，但仍不能和共享设置页、Linux 服务共用事实源。

## Decision

在 `client-providers::voice::polish_catalog` 持有内置方案的 id、名称和原始正文。Cargo example 一次导出共享 C++ 目录头、React JSON、React 的字面量 id 元组和 Linux Python 字符串模块。生成物随源码提交；Linux 设置应用 CI 在既有依赖构建后运行 `--check`，JSON 做语义比较，其余文件逐字比较。

C++ 头按共享目录查正文，已有自定义槽位与未知 id 的 cleanup 回退保持。React 的公开 id 元组及其联合类型保持，名称和正文从 JSON 投影。Linux 仅导入生成的 `PROMPTS`，不增运行时 JSON 读取；CMake 把模块装到原有脚本目录。运行时不引入网络、偏好或新的解析失败。

## Alternatives considered

- 直接把 C++ 头当事实源并解析源码导出：延续现有所有权，但需要自行解释 raw string 和 C++ 表达式，且不能让 Rust 层直接拥有方案。
- 用 JSON 做唯一源、各宿主运行时解析：导出链最短，但 C++ 需要新增解析/分配与失败路径，Python 还需要定位安装的数据文件。静态生成保持现有宿主的同步常量读取。
- 同时把提示词选择策略全部迁入新 C ABI：能减少更多宿主规则，但要重接六个平台的调用、边界检查、分配与释放；本片先统一正文和名称，已有槽位规则继续由现有回归保护。

## Consequences

本片迁移的 C++ 请求正文、React 与 Linux Python 消费路径读取同一个 Rust 目录。Windows 原生设置页的宽字符串展示副本、其他原生页的方案名称及鸿蒙独立的简版正文尚未迁移；它们继续需要同步，后续逐片处理。新增四个生成物与一个导出命令，跨语言发布时必须带齐；真实导出检查、跨语言契约和 Linux 安装接线承担这项维护成本。没有把提示词选择规则移到 Rust，宿主仍保留已有自定义槽位适配，后续可在统一 C ABI 时继续收敛。

检索活跃笔记中的 `PolishPrompt`、`polish-presets` 和「润色提示词」后分类：

- [Windows 语音页](../feature/2026-10-11-windows-settings-voice-page.md)与正文来源部分重叠，本片仅迁移请求发送用的 C++ 共享头，原生设置页的展示副本仍由该篇持有；互链标明边界，原生页面和服务目录决定继续适用。
- [共享设置页语音目录](2026-10-21-shared-ui-voice-catalog.md)拥有服务元数据的 Rust 导出，本片复用静态投影方式，提示词正文是独立目录，不取代服务目录。
- [Android 润色配置](../feature/2026-10-12-android-voice-polish-and-credential-test.md)、[Windows 语音页布局](../feature/2026-10-20-windows-settings-voice-ai-mac-parity.md)保留各自的设置接线与页面归属，与正文来源无关。
- [iOS 与鸿蒙设计](../feature/2026-10-07-ios-harmony-all-platform-design.md)、[工具与支持设置精简](2026-10-16-macos-tools-support-settings-prune.md)、[Android 社区内容归属](../feature/2026-10-11-android-community-into-feature-pages.md)命中其他 AI 提示词或润色入口，不属于语音内置正文目录，保留原决定。

## Verification

- 合成 React 目录测试在旧常量实现上先失败，接入 JSON 后通过；连同槽位和设置页回归共 3 文件、10 项通过。生产构建含类型检查、定向 lint 和 exporter 严格 clippy 通过。
- 一次性基线比对确认四篇正文与重构前 C++、Python、React 全部逐字相等。跨语言契约编译真实 C++ 消费头，再逐项对照 UI JSON 和 Python 字符串；既有 C++ 槽位/回退回归及 Linux provider 回归通过。
- 四个生成物分别注入陈旧值，`--check` 均非零退出并指出对应路径；恢复后通过。JSON 修改实际字段值，其余文件修改字节，覆盖语义比较与逐字比较。
- 独立审查通过，核对正文、公开字面量类型、未知与自定义槽位、空白正文、包含链、Linux 安装规则及 CI 检查入口。没有执行设备验收；本片只更换静态正文来源，不增加产品运行时文件访问。
- 完整 `bash scripts/verify-local.sh --quick` 通过：Rust workspace、Android 交叉检查与宿主契约、Wasm、Linux 设置应用、Linux 原生 108 项测试与版本构建、macOS 原生构建及共享 Apple bridge。完整版与五笔版安装目录中的新 Python 模块均与源文件逐字一致。Windows 原生交叉构建因缺少 MinGW、HarmonyOS 原生包因缺少 staged 输入按门禁明确跳过。
