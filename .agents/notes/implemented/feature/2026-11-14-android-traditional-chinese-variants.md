# Agent Note: Android 繁体输出接入 OpenCC 臺灣变体

Status: implemented

## Problem

共享偏好已经保存 `traditional_chinese_variant`，但 Android 宿主只调用 `msime_client_simplified_to_traditional`，设置页也只有「简体 / 繁体」。Android 用户无法选择 `s2tw` 的台湾正体或 `s2twp` 的台湾用词，候选显示和最终上屏会继续使用标准繁体。

## Decision

Android 把 `HostCapabilities::traditional_chinese_variants` 打开，并在原生「中文字符集」面板提供简体、繁体、台湾正体、台湾用词四项。偏好仍保持 `traditional_chinese_output` 布尔开关与 `traditional_chinese_variant` 独立保存；选择简体只关闭开关，切回繁体时保留原来的变体。

Android 键盘在候选显示、候选提交和普通上屏的同一边界按变体名称映射为 host-api 的 0/1/2 编号，统一调用 `msime_client_convert_to_traditional`。缺键或未知值按 `s2t`，转换失败保留原文；日语、临时日语、英文和其他既有不转换方案仍由 `ChineseOutputPolicy` 拦截。

## Alternatives considered

- **只修改 Android 转换调用，不打开能力位或设置项** — 已写入的台湾变体会生效，但用户无法在 Android 选择它，能力字段与设置页契约也会继续把 Android 当成不支持。
- **把变体并入 `traditional_chinese_output`** — 会改变旧版本和快捷切换读取的布尔字段，破坏旧宿主对偏好文档的兼容；沿用共享层已有的独立字段风险更小。
- **继续保留 Android 自己的 ICU 转换作为回退** — ICU 按字符转换，无法处理词级字形（例如「头发」），且低于 API 29 不可用；共享 C ABI 已提供词级 OpenCC 表，不再复制一套规则。

## Consequences

- **收益**：Android 设置、候选显示与最终上屏使用同一套共享 OpenCC 变体；`s2tw` 可保留「吃」并输出「裡面」，`s2twp` 还可把「软件」转换为「軟體」。共享设置页也会按能力位列出台湾两档。
- **代价**：JNI 增加一个带编号的直接转换入口，Android 首次使用台湾变体会承担共享词表解析成本；无法在本机磁盘不足时完成完整 APK 和设备验收。
- **边界**：标准 `s2t` 默认值不变，菜单和快捷键仍只翻转布尔开关；账号同步和其他平台行为不在本 issue 的代码范围内。
