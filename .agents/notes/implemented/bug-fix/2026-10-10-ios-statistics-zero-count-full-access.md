# Agent Note: iOS 统计计数为零时先提醒完全访问

Status: implemented

## Problem

#6476 让记录关闭时的 iOS 统计页只显示「记录已关闭」，不再同时提示去开完全访问。审查（#6476 的评论 6081239133）指出这之后，没开「允许完全访问」的用户整条路径上都看不到相关提示：在 app 里点「开启记录」会经 `client-core` 的 `set_enabled` 无条件写出 `typing-statistics.json`，页面于是走到「统计文件最后写入于…但计数为零…请反馈」；而键盘只在 `hasFullAccess` 时记录，数字永远是零。

共享设置页 `availabilityNotice` 有同样的前提错误，`statistics-full-access.test.tsx` 还把它写成了用例：「文件已建立时计数为零说明是别的原因，不能指向完全访问」。文件存在只说明有人写过，不说明键盘写过。

## Decision

计数为零时（`.ready` 且 `total == 0`，有无最后写入时间都算），iOS 原生页和共享设置页的 iOS 分支都先提醒「请先确认已在系统设置……为水杉键盘开启“允许完全访问”，未开启时键盘不记录统计」，再说清空过统计、请反馈等其他可能。这句始终出现，不按完全访问状态条件显示。

- 原生页的文案从 `TypingStatisticsView` 移到 `SharedUI/core/TypingStatistics.swift` 的 `TypingStatisticsStore.emptyStatisticsAdvice`，以便 `MSIMESharedTests` 直接测它（`App/Sources` 不在单测 target 里）。记录关闭时不出这张卡的行为（#6476）不变。
- 共享设置页在 iOS 上只要出现「统计没有数据」就给「打开系统键盘设置」按钮，原先只在 `neverWritten` 时给；原生页一直是每种情况都带「前往系统设置」。
- 其他平台的零计数文案不变，也不提完全访问。

## Alternatives considered

- **只在键盘没有完全访问时显示这句** — 最准确，有完全访问的用户不会被误导去查一个已经开着的开关。但 app 拿不到这个状态：iOS 没有公开 API；键盘没有完全访问时写不了 App Group，所以无法由键盘写一个「当前没有完全访问」的标记；有完全访问时写下的标记在权限被收回后会过期，读它反而给出错误的否定（这也是引导页不显示 ✓ / ! 的原因，见 [ios-harmony-all-platform-design](../feature/2026-10-07-ios-harmony-all-platform-design.md)）。`diagnostic.log` 的 `keyboard_loaded full_access=` 只在用户开着诊断日志时才有，而且同样需要完全访问才写得进去。
- **在「记录已关闭」卡里补一句「开启后需要完全访问才会计数」** — 审查给的另一条路，提示出现得更早。但它不能覆盖已经开启记录、数字一直为零的用户，而这正是会来反馈「统计一直是 0」的那批人；维护者选择放在零计数提示里。

## Consequences

- **收益**：没开完全访问的用户开启记录后，零计数页面的第一条可操作建议就是正确原因，并且有直达系统设置的按钮。
- **代价与已知上限**：开着完全访问、刚清空过统计的用户也会看到这句提醒，要多读一句才看到「清空过统计是正常的」。如果以后键盘能可靠地把当前完全访问状态交给 app（例如 iOS 提供公开 API），应改成按状态显示并重访这篇。

## Verification

- `MSIMESharedTests/TypingStatisticsTests/testZeroCountAdviceAfterEnablingInAppNamesFullAccessFirst` 在临时目录里复现「app 开启记录 → 文件存在、计数为零」，断言提示里「允许完全访问」排在「清空过统计」之前；`testEmptyStatisticsAdviceStaysQuietWhenItHasNothingToExplain` 守住有计数和记录关闭时不出声。
- `MSIMEClientUITests/OnboardingUITests/testEnablingRecordingWithZeroCountNamesFullAccess` 在模拟器的真实 app 里点「开启记录」，断言「统计没有数据」卡里出现「允许完全访问」和「前往系统设置」；模拟器上已有计数时跳过。PR 上的 `iOS Simulator` 不跑界面用例，要本机跑，且需签名（`CODE_SIGN_IDENTITY=-`），`CODE_SIGNING_ALLOWED=NO` 时 app 拿不到 App Group，页面只会显示「无法访问共享存储」。
- `apps/desktop/tests/settings/statistics-full-access.test.tsx` 覆盖共享页 iOS 有无写入时间两种零计数，以及其他平台不提完全访问。
