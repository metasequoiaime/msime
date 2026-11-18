# Agent Note: Android 没有输入类别的输入框按文字框组字

Status: implemented

## Problem

抖音极速版 41.2.0 直播间的评论框是一个 `EditText`，聚焦时 `EditorInfo.inputType` 报 `0x20000`：类别位（`TYPE_MASK_CLASS`）为 0，只带 `TYPE_TEXT_FLAG_MULTI_LINE`；`imeOptions` 是 `IME_ACTION_SEND | IME_FLAG_NO_EXTRACT_UI`。`EditorPolicy.useEngine` 原来只放行两种：inputType 恰为 0（[TYPE_NULL 聊天框](2026-10-14-android-type-null-editor.md)，#7755），以及类别为 `TYPE_CLASS_TEXT` 的非密码框。`0x20000` 两条都不满足，宿主不建引擎会话：工具栏显示「直接输入」，中文键变灰，字母键直接上屏字母。#11103、#11064、#11627 报的就是这个现象（抖音极速版和抖音谷歌版、vivo 与 Redmi、Android 11 到 16）。百度、Fcitx5 和系统键盘在同一个框里能打中文。

输入框本身能组字：`TextView` 只在 inputType 恰为 0 时不提供可编辑的 InputConnection，`0x20000` 拿到的是普通的 `EditableInputConnection`，`EditorInfo` 也报了真实选区（`initialSelStart=0`）。问题只在宿主的分类：先认定「只有文字类别才算文字框」，没声明类别的框就被排除了。

## Decision

`useEngine` 按类别排除已知的非文字框，其余都组字：

- `TYPE_CLASS_NUMBER`、`TYPE_CLASS_PHONE`、`TYPE_CLASS_DATETIME` 不走引擎，无论带什么 flag。
- 三种文字密码 variation（`PASSWORD`、`VISIBLE_PASSWORD`、`WEB_PASSWORD`）不走引擎。这个判断对所有其余类别生效：没有类别的框若带着密码 variation 的位，同样不组字。
- 其余都走引擎：`TYPE_CLASS_TEXT`、类别为 0 的框（整数 0 和只带 flag 的都算）、以及将来新增的类别。

原来单列的 `type == 0` 分支并入类别规则。其他判断不变：`numberInput`、`password`、`prefersLatin`、`capitalizationMode` 仍按原来的类别与 variation；`InputViewRefreshPolicy.defersEngineStartup` 仍只对恰为 0 的 inputType 推迟建会话（桌面、设置页这类不打字的窗口报的是整数 0）；`TextCursorEdgePolicy` 对类别为 0 的框照原样发方向键。所有决定「建不建会话」的调用（`MSIMEInputService` 的开始与重启输入、会话保留、手写）都经过 `useEngine`，没有别处另判。

## Alternatives considered

- **只补一条「类别为 0 就组字」，文字类别以外仍一律排除。** 最强理由是改动最小，只放行这次实际见到的形态；但它保留了白名单的结构，下一个不按约定声明类别的自定义控件还会以同样的「直接输入」来报 bug。改成按已知非文字类别排除，与其他键盘遇到不认识的类别时退回文字键盘的做法一致。
- **按包名给抖音开例外。** 最强理由是对其他应用零影响；但根因是分类规则而不是某个应用，抖音的几个版本和发行渠道各自报着不同的形态（#7755 是整数 0，现在是 `0x20000`），按包名追只会一版一版地补。
- **把各处对 inputType 的判断收拢成一个统一的编辑器分类器再改。** 最强理由是长期一致；但决定建不建会话的判断已经只在 `useEngine` 一处，其余几处各管各的事（推迟建会话、方向键、大写），把不改变行为的重构和这次的行为修复放进同一个改动，只会让审查和回滚更难。

## Consequences

- **收益**：没有声明类别的自定义聊天控件（直播评论、弹幕等）能正常组中文；不再依赖控件恰好报整数 0。
- **代价与已知上限**：类别为 0 或未知类别、但实际只收按键的控件（终端一类）现在也会建引擎会话；它们原本报整数 0 时就已如此（见 [TYPE_NULL 聊天框](2026-10-14-android-type-null-editor.md)），用户仍可切到英文模式发送字面字符。`TYPE_CLASS_DATETIME` 的行为不变：不组字，也不显示数字九宫格（`numberInput` 只认数字和电话），是否改成数字九宫格另作决定。若将来 Android 新增的类别是只收数字的，需要把它加进排除列表。
- **Enter 键**：评论框声明了 `IME_ACTION_SEND` 而没有 `IME_FLAG_NO_ENTER_ACTION`，组字后按回车照常执行编辑器动作（发送），与其他键盘一致。

## Verification

- `platforms/android/tests/core/EditorSmoke.java`：整数 0、`0x20000`、`0x10000`、`0x20000 | CAP_SENTENCES`、未知类别 `0x5` 走引擎；`TYPE_CLASS_DATETIME`、带 flag 的电话和数字类别、没有类别但带文字密码 variation 的框不走引擎；原有的文字、不要建议、网址和密码用例不变。由 `platforms/android/check-host.sh` 运行。
- 抖音极速版 41.2.0（Android 16 真机）：评论框 `inputType=0x20000`，改前工具栏为「直接输入」、字母直接上屏；改后中文键可用，拼音在评论框里组字，选候选后上屏中文。
