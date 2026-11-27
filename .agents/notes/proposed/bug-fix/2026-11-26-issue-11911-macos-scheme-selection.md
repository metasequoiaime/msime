# Agent Note: 调查 macOS 方案选择后显示与实际方案不一致

Status: proposed

## Problem

Issue #11911 报告 macOS 设置界面选择全拼并保存后再次唤起输入法仍显示双拼，但实际按键按全拼处理。报告环境为水杉 IME 0.60.1、macOS 27.2.1，现有材料没有给出完整构建号、偏好文档内容、输入源模式标识或诊断日志，因此还不能区分设置显示滞后、系统输入源报告覆盖偏好，还是旧版本进程未重新载入共享偏好。

## Proposal

把 #11911 作为方案偏好持久化与 macOS 输入源回报的调查记录，先要求一轮不含真实输入的最小证据，再决定是否修改代码。

当前代码核查得到以下事实：

- 共享偏好文档的 `preferences.scheme` 是设置页、命令行、云同步和输入法共同使用的方案值。macOS 的 `NSUserDefaults` 键 `MSIMEClientInputScheme` 只承担输入法自己的旧本地回退。
- 输入法进程启动并在共享文档载入前，可能暂时读到本地回退。`ShortcutTest.mm` 的 `TestFreshProcessActsOnTheSharedSchemeNotTheStaleLocalOne` 已覆盖本地值为 `shuangpin`、共享文档为 `quanpin` 时，首次激活采用全拼，并验证后续异步保存不会把双拼写回文档。
- 系统输入源报告由 `InputModeIdentifiers.h` 的 `switchedIn` 与 `left` 状态区分系统把用户送回原入口和用户明确选择另一个水杉模式。已有快捷键回归覆盖从双拼入口回到全拼入口及设置中改为全拼的路径。
- `AppearancePreferences` 读取共享文档后以共享值覆盖本地值，设置窗口保存方案时会写入共享快照。当前没有证据表明这条路径在 0.60.1 之后仍会把全拼稳定改回双拼。

需要报告者补充：

1. `水杉 IME 0.60.1` 的完整构建号，以及问题是否在最新 nightly 或 develop 构建仍存在。
2. 选择全拼后，从设置应用退出并重新打开时页面显示的方案，和输入法菜单栏显示的模式名称。
3. 设置应用维护与诊断中的输入模式报告行，重点是 `input_mode_report` 的 `mode`、`scheme`、`target`、`switched_in` 和 `left` 字段。日志中不要包含输入内容、账号或私人路径。
4. 复现后偏好目录中 `preferences.json` 的 `preferences.scheme` 值和输入法本地 `MSIMEClientInputScheme` 值。只需提供两个方案名，不要上传整份文件。

## Alternatives considered

- 直接把所有系统输入源报告强制改成全拼。报告可能来自用户明确选择的双拼入口，这会破坏已有的输入源切换语义，也无法解释设置应用自身显示旧值，因此不采用。
- 删除 `MSIMEClientInputScheme` 并只读共享文档。输入法首次启动、共享文档暂时不可用及旧版本升级仍需要本地回退，删除它会扩大启动失败范围，因此不采用。

## Acceptance criteria

- 获得一组不含真实输入的最小复现，能够说明问题发生在共享偏好载入、系统输入源回报或设置应用显示中的一个具体阶段。
- 若最新版本已经不再复现，则用现有回归测试和报告者复验结果关闭 issue。
- 若日志确认某条路径覆盖了共享方案，则新增针对该路径的行为回归并另开 `Fixes #11911` 修复 PR。

## Risks

没有报告者的构建号、模式回报和两份方案值时，任何修复都可能把系统明确选择的双拼误判为错误状态，或重新引入 #4288 的旧回归。
