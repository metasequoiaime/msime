# Agent Note: Android 剪贴板面板顶行显示已存条数和上限

Status: implemented

## Problem

#5905：剪贴板面板顶行「本机 / 云端」分段和「清空」之间是一块空白。用户不知道历史最多存几条（举例时猜的 100，实际是 50），希望在这块空白里一眼看到「10/50」这样的已存条数和上限，并且不额外占地方。

## Decision

- 顶行原来那块加权空白换成一个同样加权、文字居中的单行标签，写「已存条数/上限」，例如 `10/50`；同一个计数给读屏的说法是「本机已存 10 条，最多 50 条」。没有可数的内容时标签为空串、对读屏隐藏，布局和原来一样。
- 规则在 `CloudClipboardPanelPolicy`（`limit` / `cloudCount` / `countLabel` / `countDescription`），不依赖 Android：
  - 本机分段的上限是 `ClipboardHistoryPolicy.LIMIT`，计数是这次渲染读到的条目数；历史未开启或读取失败时不显示。
  - 云端分段的上限是 `CloudClipboardApi.MAX_ITEMS`，只在这次拉取答复为 `READY` 或 `EMPTY` 时显示条数，读取中、未登录、未开启和失败时不显示。
- `ImePanels.renderClipboardHistory` 把本机历史的 `load()` 提到画顶行之前，计数和下面的卡片用同一份结果，每次渲染仍只读一次；读取失败时下面照旧画「历史记录无法读取，请清空后重试」。
- 等待确认清空时顶行整行给问句，不显示计数；分词界面不画顶行，也没有计数。删除、固定、清空、新复制和云端刷新都会重新渲染，计数随之更新，不另存状态。
- 标签的颜色和面板里的提示一样，在换肤遍历之后设成前景色 60% 透明度，读起来是次要信息。

## Alternatives considered

- **把云端已有的「N 条 · 点按插入」状态行也画在本机分段** — 现成的文案；但状态行占一整行，用户明确要求不额外占地方，而且云端列表有内容时那一行本来就不画。
- **让共享存储经 JNI 返回上限** — 分母永远和 `crates/client-core/src/clipboard.rs` 的 `MAX_ENTRIES` 一致；但要改 host-api 的响应和三个移动宿主，上限多年没变过，`ClipboardHistoryPolicy.LIMIT` 已是安卓里唯一的一份副本。
- **只显示条数、不显示上限** — 更短；但用户要的正是「还能存多少」，只有分子回答不了。

## Consequences

- **收益**：不用猜上限，满了之前就看得到；不占新的一行。
- **代价**：安卓的分母是 client-core 和服务端上限的副本，以后只改那两处，界面上的分母会不准。窄屏或分离键盘上顶行很挤时，标签按单行省略号截断。

## Verification

`platforms/android/tests/keyboard/CloudClipboardPanelPolicySmoke.java` 验证本机 0/50、10/50、50/50，云端 READY、EMPTY 计数，其余状态和未开启时不显示，以及读屏说法；`bash platforms/android/check-host.sh` 编译面板代码并运行它。在 API 35 的专用模拟器上看到本机顶行居中显示 1/50、2/50，删掉一条后变回 1/50；云端计数、窄屏和分离键盘上的排布没有在设备上验证。
