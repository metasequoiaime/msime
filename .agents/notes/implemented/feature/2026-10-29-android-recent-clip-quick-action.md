# Agent Note: Android 最近复制一行的分词或分类快捷键

Status: implemented

## Problem

#10079：复制一段文字后想分词或移动分类，要先点剪贴板进面板，长按这条记录，再在操作行里点「分词」或「移动分类」，四步才到。报告者希望在键盘顶部「最近复制」那一行的预览右边直接放一个快捷键，并且在设置里能开关、能在「分词」和「分类」之间二选一；总开关默认开、默认「分类」，让用户知道有这个功能，不想要的自己关。

## Decision

- 「最近复制」那一行（[工具栏显示最近复制](2026-10-08-android-recent-clip-toolbar.md)）在预览和 × 之间多一个文字快捷键（`ImeToolbar.addRecentClipRow` 的 `recentClipActionButton`），写「分词」或「分类」，宽度随字、至少 44 dp，预览仍占剩下的全部宽度。它用 `GLYPH` 角色，和 × 同色、不画键帽。
- 显不显示、显示哪个只由 `RecentClipboardSuggestion.quickAction(actionSetting, kind, image)` 判断：总开关关着返回 null；这一条是图片时也返回 null（图片不能分词，剪贴板历史也不记图片，没有记录可移动分类）；不认识的取值按出厂默认的分类。最近复制本身不显示时（历史关着、隐私规则、正在组词等）快捷键跟着整行一起不显示，规则不重复。
- 两个本地设置，只在本机、不随账号同步：`platform.android.clipboard_suggestion_action`（布尔，默认开）和 `platform.android.clipboard_suggestion_action_kind`（`segment` / `category`，默认 `category`）。出厂默认开，是 issue 明确要的产品决定。
- 点快捷键（`MSIMEInputService.runRecentClipQuickAction`）：这一条算用过，和粘贴一样 `recentClip.dismiss()`；收起别的工具栏面板（最近复制也显示在文本编辑面板上），再由 `ImePanels.openRecentClipAction` 打开剪贴板面板。分词和分类都只画在本机分段里，所以先把分段切回本机。之后走长按菜单同一条路：分词调 `startClipboardSegmentation(text)`，分类把 `clipboardCategoryMovingText` 设成这段文字后重画，进「移动到分类」选择器。
- 移动分类按文字找历史里的记录。复制监听和打开面板时的补读都已同步把这一条写进历史，正常情况下记录一定在；共享存储拒收的内容（过长、含控制字符）不在历史里，选了分类也只会得到「移动失败，记录或分类可能已改变」。所以分类前先查一次本机历史（`localHistoryContains`），不在时提示「这条内容不在剪贴板历史里，不能移动分类」，停在历史列表上。分词不依赖历史记录，不查。
- 设置在「键盘 › 工具栏 › 剪贴板」：「工具栏显示最近复制」下面是开关「复制后自动显示快捷键」和导航行「快捷键功能」（点开是和「排列」同样的底部单选：分词、分类）。剪贴板历史或最近复制关着时两行都置灰，快捷键总开关关着时「快捷键功能」置灰。开关保存成功后页面不重画，所以开关的回调里直接更新这两行的置灰状态。设置搜索的关键词补上两行标题和「分词」。
- `check-host.sh` 钉住 JVM 冒烟碰不到的接线：服务经 `RecentClipboardSuggestion.quickAction` 用这两个设置、工具栏按 `s.recentClipQuickAction(image)` 显隐、点按经 `imePanels.openRecentClipAction` 进入分词和移动分类。

## 平台范围

只有 Android 有「最近复制」那一行和剪贴板分词、分类（iOS、HarmonyOS 的宿主里都没有这几项），所以这不是三端键盘 UI 一致性要同步的改动。

## Alternatives considered

- **一个三值设置（`off` / `segment` / `category`）** — 一个键就说清楚，不会出现「总开关关着、功能选了分词」这种无效组合。不用的原因是 issue 要的界面就是一个总开关加一个二选一，而本页其余「开关 + 细项」（「滑动输入符号」和「滑动方向」）也都存成两个键；拆成两个键，关掉再打开时用户选过的功能还在。
- **同时放「分词」「分类」两个按钮，不让选** — 省掉设置项。不用的原因是这一行本来就挤（剪贴板、预览、×、收起），再放两个字按钮，窄屏上预览只剩几个字；issue 也明确要求二选一。
- **点快捷键不收起最近复制** — 关掉面板回到键盘时还能粘贴它。不用的原因是这一条已经被用过了（分词后可以插入、复制，分类后在历史里），和点预览粘贴后同一条不再出现的规则一致；不收的话面板一关它又占着工具栏。

## Consequences

- **收益**：复制后一次点按就进分词点选或分类选择器；默认就显示，用户能发现这个功能。
- **代价**：默认开意味着所有开着最近复制的老用户升级后都会多看到一个「分类」字样的按钮，预览少了约 44 dp 的宽度；不想要的需要去设置关掉。点快捷键会把剪贴板面板的分段切回本机，下次打开面板不再停在上次的云端分段。共享存储拒收的长文本只能分词，点「分类」只得到一句提示。

## Verification

`platforms/android/tests/clipboard/RecentClipboardSuggestionSmoke.java` 验证两个设置的默认值（开、`category`）、只在本机、功能只能是 `segment` 或 `category`，以及 `quickAction` 在总开关关着、这一条是图片时不显示、不认识的取值退回分类；`SettingsSearchIndexSmoke` 要求两行新设置都能被搜到。`bash platforms/android/check-host.sh` 运行这些冒烟和上面的接线守卫（在把 `clipboardCategoryMovingText = text` 和 `recentClipQuickAction(image)` 分别改坏的副本上确认守卫会失败）；设置页由 Gradle 的 `compileFullReleaseJavaWithJavac` 编译。真机或模拟器上的显示、点按进入分词和分类、窄屏下预览的可读性没有在设备上验证。
