# Agent Note: Android 原生设置逐项选择模糊音

Status: implemented

## Problem

共享偏好已有 `fuzzy_pinyin.enabled`、`rules` 和一次性种子 `seeded`，引擎也逐项读取规则；Android 原生「输入」页只有总开关，不能只选 `z-zh` 或 `an-ang`。用户为了一个规则开启模糊音时会同时启用全部规则。

## Decision

在 Android 原生「输入」页增加「模糊音规则」入口，独立页面按共享设置的四组、11 个规则 ID 展示开关。每次编辑从最新偏好快照保存单个规则，保留其他选择；首次手动选规则时写 `seeded=true`，避免之后开启总开关被共享存储自动补成全选。现有首次启用全选、关闭保留勾选、默认关闭的语义不变。设置首页索引覆盖新页面和规则行。

## Alternatives considered

- 把 11 个规则直接展开在「输入」页：不用新导航页，但会让已有很长的输入页再增加四组设置，常用语言与方案、辅助码入口更难找到。
- 修改共享偏好默认值为只开常用规则：能降低重码，但会改变所有平台的新装体验，也超出 Android 设置入口的范围。

## Consequences

页面直接使用共享设置的 11 个规则 ID，按四组展示，逐项保存时从最新偏好快照修改一条规则。首次手动选规则时标记 `seeded`，之后开启总开关保留选择；从未手动选规则的用户首次开启仍由共享存储选中全部，默认值不变。代价是 Android 原生设置多一个子页面，规则目录未来变更时需同步这个页面与共享设置。

## Verification

设置搜索回归在旧实现中因缺少规则页失败，新增页面后通过。`bash platforms/android/check-host.sh` 的 279 项 JVM 冒烟与 245 个设备用例源码编译通过；`bash scripts/verify-local.sh --quick`、笔记与差异校验通过。API 28 模拟器上，`FuzzyPinyinDeviceSmoke` 输出 `MSIME_DEVICE_SMOKE_PASSED`，覆盖禁用状态下逐项选择、其他规则保留、重开与首次启用不覆盖手动选择。装包和 `smoke.sh --core` 的前序用例通过；它在 `NumberPadDeviceSmoke` 的笔画键面检查停止，本次 APK 未暂存笔画语言词库，应用按既有规则回落到拼音 26 键；单独重跑同一用例仍得到该结果，不能把整套 core 记为通过。
