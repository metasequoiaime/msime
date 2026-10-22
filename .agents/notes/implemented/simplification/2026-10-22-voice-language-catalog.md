# Agent Note: 语音识别语言目录归入共享服务层

Status: implemented

## Problem

共享 UI 的桌面语言建议、手机语言选项和 Windows 原生设置分别写着 `zh-cn`、`zh-CN`、`en`、`en-US`、`ja`、`ja-JP`、`auto` 以及各自的标题。iOS 和 Android 也重复维护手机四项，语言值会写入同一份 `voice_input.language`。修改一项时，选项顺序、标题或系统识别器的区域代码可能漂移。

## Decision

在 `crates/client-providers/src/voice/language_catalog.rs` 持有语言的稳定 id、云端短代码、系统识别区域代码、桌面标题和移动标题。独立导出器生成共享 UI JSON 与 Windows 头文件；UI 和 Windows 按宿主能力投影有值的列。移动端本片只生成目录字段，下一片接入移动宿主，保留系统识别只支持普通话、未知保存值原样显示和本地模型自己的语言映射。

## Alternatives considered

- 继续让各平台手写选项并增加逐项快照：改动较小，但新语言仍要在多种语言和多个宿主同步，快照只能发现漂移，不能提供唯一来源。
- 把所有语言规则放进 `client-core` 偏好模型：偏好负责持久化字段，但服务选项和各识别器的投影属于 provider 层；把展示目录放进偏好会让不联网的 core 依赖服务目录语义。

## Verification

- 共享 Rust 目录生成 UI JSON 和 Windows 头文件，生成器支持 `--check`。
- UI 桌面/移动选项和 Windows 系统/云端选项的实际输出与迁移前逐字一致。
- `voice_input.language` 的存储值、系统识别区域值、未知值回显规则不变。
- 生成物漂移、缺失和消费者未接线时有自动检查。

Rust 目录单元测试通过，生成器 `--check` 通过；合成 C++ 消费者运行检查通过。旧 UI 的桌面语言选项和 Windows 的系统/云端选项逐字核对，当前目录投影保持相同顺序和值；本片尚未改移动端消费者。

## Consequences

共享 UI 与 Windows 不再维护语言值和标题的第二份表；导出生成物必须通过 `--check` 保持同步。移动端的 iOS/Android 识别器仍保留平台特有的映射，本片先接 UI 与 Windows，后续片分别接入移动宿主并用运行基线证明没有改变系统、本地模型和云端请求策略。
