# Agent Note: 剪贴板多选按序号插入（Android、iOS、鸿蒙）

Status: implemented

## Problem

#10411：要把几条剪贴板记录接连插进同一个输入框，只能开一次面板插一条、面板关上、再开再插。报告者希望在 Android 剪贴板面板的「多选」里加「插入」：选中的条目右上角标出 ①②③④ 表示插入顺序，取消某一项后排在它后面的序号依次前移，最多 4 条，按序号插入；现有的多选删除、导出和计数不能受影响，访客模式下也要能用。

Android 已有本机历史的多选（[多选删除](2026-10-17-clipboard-multi-select.md)、[多选导出](2026-10-27-android-clipboard-multi-export.md)），只能导出和删除，选中只有高亮没有序号。iOS（`KeyboardClipboardView` 只能点一条插一条）和鸿蒙的触屏剪贴板面板都没有多选。维护者定为三端在同一个 PR 里一起做：iOS 和鸿蒙补一个最小的多选（入口、选中高亮、序号角标、插入、退出），不搬 Android 独有的导出和访客模式。

## Decision

- **复用现有「多选」，不另开模式。** Android 在多选顶行的「取消」和「导出」之间加「插入」；iOS 和鸿蒙在本机历史的顶行加「多选」，进入后顶行换成「N 项已选择」「取消」「插入」，和 Android 多选顶行「N 项已选择 / 取消 / 插入 / 导出 / 删除」的先后一致。三端的多选都只作用于本机历史：云端分段没有入口，搜索中（iOS）也不进入。
- **上限分两层。** 选择上限仍是 `MAX_SELECTIONS = 20`（导出和删除共用，不变）；插入另有 `MAX_INSERTIONS = 4`，选中 1 到 4 项时「插入」可用（`canInsert`），超过 4 项时置灰并给提示：Android 在选中第 5 项那一下用 `notice` 提示一次 `INSERTION_LIMIT_HINT`（顶行套在横向滚动里按内容宽度量，把提示塞进计数会把「删除」挤到窄屏屏外），iOS 和鸿蒙写在已有的状态行里，一直显示到选中项回到 4 项以内。iOS 和鸿蒙目前只有插入，仍按 20 项上限，以后补删除不用再改规则。
- **序号就是选中集合的迭代下标。** Android 的选中项本来就是 `LinkedHashSet<String>`，iOS 和鸿蒙用按点选先后排列的 `[String]`；序号是条目在集合里的下标（`ordinal`），角标文字是 U+2460 加下标（`ordinalLabel`，① 到 ⑳，正好和 20 的选择上限对上）。取消一项只是从集合里删掉它，重画时后面各项的下标自然前移，不另外维护序号。所有选中项都画角标，不只前 4 项。多选中选中的记录可能被删掉（Android 多选时仍可左滑或长按删除，别处也可能删掉或清空），所以每次重画都按刚读出的历史收窄选中集合（Android 在 `renderClipboardHistory` 里 `retainAll`，iOS 在 `reload()` 里过滤；鸿蒙多选时顶行整行换掉、长按管理行不开，没有删除或保存的入口，重开面板会退出多选，用不着收窄），否则序号会跳号、已删的文字仍占 4 项上限、「插入」还会把它插进去。
- **角标位置与样式。** 选中卡片铺强调色、文字换成强调色上的文字色，右上角画序号。Android 的本机记录无论单列还是多列网格都放在同一个左滑格（`clipboardSwipeCell` 的 `FrameLayout`）里，角标在这一格里建出来，所以三种排法都有。它是一个 `Drawable`（`ClipboardOrdinalBadgeDrawable`），加进卡片自己的浮层（`card.getOverlay()`），不做这一格里的兄弟视图：带阴影的皮肤经 `ImeStyler.applyShadow` 给卡片抬高度，`FrameLayout` 按高度从低到高画子视图，高度为 0 的兄弟角标会被不透明的卡片盖住；画在浮层里则和卡片同一高度、总在卡片内容之上，左滑和按压缩放时也跟着卡片走。它自己画字而不是用 `TextView`，否则换肤遍历会把它改成按键前景色、压在强调色底上看不清。角标占的正是置顶角标的位置，选中时不再叠画置顶角标，「已置顶」仍写在卡片元信息里。iOS 是 `ClipboardCardCell` 里的一个 `UILabel`，多选时行菜单（附件位）让开；鸿蒙是卡片外包一层 `Stack({ alignContent: Alignment.TopEnd })`。三端正文右边都多让出一截，长文字不压在角标下。
- **读屏。** 选中卡片的说明前缀是「已选择第 N 项；」（`selectedDescription`），替换原来的「已选择；」；角标本身不进读屏。「插入」不可用时三端都标成不可用（Android `setEnabled`、iOS `isEnabled`、鸿蒙 `.enabled`），读屏读得出它点不了。
- **一次提交，不加分隔，不分条延迟。** 按序号把各条首尾相接（`insertionText`），中间不加任何分隔符，走各平台单条插入的同一条路径一次提交：Android 新增 `MSIMEInputService.insertClipboardTexts(List<String>)`，与 `insertClipboardText` 一样先 `command(2)`，再一次 `commitText`，然后 `recentClip.dismiss()`、`closeClipboardHistory()`、`render()`，面板关闭；iOS 把拼好的文字交给面板的 `onInsert`（结束组字、`insertOwnText`、关面板）；鸿蒙交给 `KeyboardSession.shared.insert`，和鸿蒙的单条插入一样面板留着，只退出多选。面板在调用前先把选择快照成列表、退出多选，和导出的写法一样。
- **不认证。** 单条插入不经系统认证（`authenticateClipboard` 只用于搜索、导出、清空），多选插入照单条处理。
- **访客模式不另做。** Android 的访客模式只是存储隔离（`ClipboardHistoryStore`），面板代码是同一套，多选插入在访客模式下自然可用。
- **规则在三端各一份纯逻辑，逐条对应。** Android `ClipboardSelectionPolicy`（Java，`ClipboardSelectionPolicySmoke` 覆盖），iOS `ClipboardSelectionPolicy`（Swift，写在 `KeyboardClipboardView.swift` 里，免得为一个 enum 改提交在仓库里的 Xcode 工程；`ClipboardHistoryTests` 覆盖），鸿蒙 `keyboard/clipboard/ClipboardSelectionPolicy.ts`（`tests/clipboard-selection.test.ts` 覆盖）。接线分别由 `platforms/android/check-host.sh` 和 `platforms/harmony/tests/run.sh` 钉住。

## Alternatives considered

- **分条插入、每条之间延迟一会儿**：这是报告者的设想，理由是照顾低端机，延迟时长交给维护者定。逐条 `commitText` 在编辑器里是几次独立提交，用户若想撤回要撤几次；更要紧的是延迟期间键盘可能收起、编辑器可能换成另一个输入框，后面几条会插错地方或只插进去一部分，每次回调都得重新核对连接和编辑器会话。一次提交没有这些窗口，也就用不着延迟，低端机上也只是一次 IPC。
- **条目之间加换行或空格**：换行在单行输入框里可能被吞掉，在聊天类应用里可能直接触发发送；空格对中文内容是多余的字符。维护者定为不加分隔符，需要分隔的用户可以在复制时自己带上。
- **插入上限也放到 20、取消 4 条的限制**：一次插 20 条长文本更容易碰到编辑器的长度限制，报告者的标题明确是「最多 4 条」，导出和删除仍保持 20 条，两者各管各的。
- **角标只给前 4 项画**：超出插入上限的选中项没有序号，用户看不出取消哪几项能让「插入」恢复可用，所有选中项都画序号更一致。序号只表示插入顺序：导出仍按共享历史的顺序输出（`ClipboardExportFormat.select`），不按序号。
- **iOS 和鸿蒙补全整套多选（删除、导出）再加插入**：维护者只要插入所需的最小多选，导出依赖 Android 的目录授权和后台写文件流程，删除在 iOS、鸿蒙上还缺批量删除和撤销的入口，范围会大一倍；规则已按 20 项上限写好，以后补删除时直接复用。

## Consequences

- **收益**：几条记录一次插进输入框，一个撤销步就能撤回；三端的多选入口、角标、上限和读屏说明一致；Android 现有的多选删除、导出、20 项上限和访客模式行为不变。
- **代价**：Android 多选顶行多一个按钮（「N 项已选择 / 取消 / 插入 / 导出 / 删除」），窄屏加大字号时要横滑才能点到「删除」，顶行本来就套在横向滚动里，不会被截断。选中项以文字为键，历史本身按文字去重，目前不会冲突；以后若允许同文多条，序号要改按条目标识算。拼接后一次提交的长度没有额外上限，几条很长的记录拼起来可能被个别编辑器截断。鸿蒙只在触屏面板（手机）上提供多选，2in1 表情面板里的剪贴板页不变。
- **未在设备上验收**：本次只跑了三端的单元测试和接线检查。设备上要用合成文本走一遍：选 A、B、C、D 看到 ①②③④，取消 B 后变成 A① C② D③，插入后编辑器里依次是 A、C、D 连在一起、一次撤销撤回；选第 5 项时「插入」置灰、导出和删除照常可用到 20 项；Android 用访客模式重复一遍；打开 TalkBack / VoiceOver / 鸿蒙读屏确认读出「已选择第 N 项」。
