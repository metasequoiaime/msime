# Agent Note: macOS 繁体输出接入 OpenCC 臺灣变体

Status: implemented

## Problem

繁体输出的 OpenCC 臺灣变体（`s2tw` / `s2twp`）在 [2026-10-15 的核心实现](2026-10-15-traditional-chinese-variants.md) 里已经进共享层，当时只把 Linux 宿主接上。macOS 0.57.0 会把 `traditional_chinese_variant` 写进偏好，但候选和上屏仍调用永远走 `s2t` 的 `msime_client_simplified_to_traditional`：全拼 `chi` 第一名引擎是「吃」，候选窗显示「喫」。设置页因为 `HostCapabilities::traditional_chinese_variants` 为假，也不列出台湾正体。

## Decision

macOS IMK 宿主按 Linux 同一条契约转换：

- `MetasequoiaChineseOutputString` 增加 `variant` 编号，内部改调 `msime_client_convert_to_traditional`。缺省 0（`s2t`），未知编号时 C 接口返回空、宿主保留原文。
- `MSIMEAppearancePreferences` 从共享快照读 `traditional_chinese_variant`；缺键或认不出的值按 `s2t`，避免上次的 `s2tw` 粘在会话里。旧平台云快照不包含变体，因此账号同步或导入旧快照保留当前共享文档的选择，不清空它的缓存。菜单、工具栏和 Ctrl+Shift+F 仍然只翻转 `traditional_chinese_output`。
- `InputController` 把变体传给候选显示、上屏、释义和装饰字。日语方案与来源 18（运行时已转换）仍不整行再转。
- `HostCapabilities::traditional_chinese_variants` 对 macOS 为真，共享设置页因此列出台湾正体 / 台湾用词，不必改 React 组件。

原生 AppKit 设置窗口仍是回退，不加变体选择器。

## Verification

`traditional-variant-preferences` 通过真实 `MSIMEAppearancePreferences` 覆盖三种合法值、共享快照缺键、未知值及非字符串值，并在每种合法变体下套用旧平台云快照，核对变体保持不变。未修复代码在 `s2tw` 套用云快照后的断言失败；转换函数测试另行覆盖输出字形，不能代替宿主偏好状态的回归。

## Alternatives considered

- **进程内全局记住变体，两参数接口不动** — 调用点最少。架构明确不靠全局变量传设置，Windows Server 也不经过同一份偏好加载；#7908 已经否过这条。
- **只改转换函数、不打开能力位** — 偏好写了 `s2tw` 就会生效，但设置页仍只显示「繁体」，用户看不到、也选不了台湾正体。能力位是设置页露出选项的约定。
- **顺手加 `s2hk`** — OpenCC 有香港字形表，全拼用户也需要「吃」而不是「喫」。`TraditionalVariant` 没有这一档，粤语词库另用香港字形；加新配置要改共享枚举、词典和三个宿主，超出这次「把 macOS 接到现有三档」的范围。

## Consequences

- **收益**：macOS 繁体输出可以选臺灣字形；已写入的 `s2tw` 在升级后立刻生效，「吃」「裡面」不再被 `s2t` 换成「喫」「裏面」。`s2twp` 才会把「软件」换成「軟體」。
- **代价**：臺灣变体比 `s2t` 多一次分段和一遍扫描，与 Linux 相同。Windows、iOS、Android、HarmonyOS 仍按 `s2t`。
- **重访信号**：其余平台接入时只打开能力位并改各自转换调用；不要在 macOS 上再加一份变体换算。
