# Agent Note: Android 快捷短语混入普通候选

Status: implemented

## Problem

共享引擎已经支持 `mixed_input.quick_phrase` 和 `quick_phrase_frequency`，但 Android 宿主没有请求 `quick_phrase_candidates`，设置页也没有这两个用户入口。因此 Android 不能按 issue #11566 使用快捷短语混输或调整其候选位置。

## Decision

Android 宿主声明 `quick_phrase_candidates`，并在每次建 Engine 会话时请求该能力。共享层继续用宿主能力与 `mixed_input.quick_phrase` 相与，未开启用户开关时不加入普通候选；`quick_phrase_frequency` 继续由共享偏好控制调频。Android 表达页在「智能」组增加「快捷短语混输」和「快捷短语参与调频」两个原生开关，说明收进标题旁的「?」，调频开关在混输关闭时置灰。K 模式入口保留。

macOS、Linux 的旧偏好缺 `quick_phrase` 时仍按开启处理，以保持它们接入开关前的行为；Android 是新接入能力，旧 Android 文档缺这个字段时按关闭处理，避免用户原本没有的候选来源突然出现。`quick_phrase_frequency` 在三者缺字段时都按开启处理；新装文档由共享默认值写出关闭混输、开启调频。这个差异不改共享默认值。Android 的共享文档始终明确写出 `quick_phrase` 的值。若沿用桌面「开启时省略」的写法，Android 下一次加载会把用户刚打开的开关读回关闭。桌面继续允许省略已开启的值。

## Alternatives considered

- **只在 Android 页面增加开关**：宿主没有请求能力时开关不会影响引擎，属于无效设置，不采用。
- **Android 无条件混入快捷短语**：会改变现有候选列表并绕过用户偏好，不采用。
- **把快捷短语改成 Android 的 K 模式替代品**：会改变现有 K 模式和普通拼音输入边界，不采用。

## Consequences

Android 用户可以在普通拼音候选中使用快捷短语，并控制其调频；关闭混输时 K 模式仍可用。Windows、macOS、Linux 的现有能力和默认值保持不变。issue #11566 还要求各平台的自定义辅助码表导入；Android 现有插件包导入链路已存在，本次不重写它，也不把 macOS/Windows 未补齐的原始码表入口宣称完成。

## Verification

- `HostCapabilities` 平台矩阵测试把 Android 的 `quick_phrase_candidates` 断言为开启，其他未接入宿主保持关闭。
- Android 建会话代码请求 `quick_phrase_candidates`，设置页接入两个原生偏好行；完整 quick 和 Android Java 门禁在 PR 上执行。
