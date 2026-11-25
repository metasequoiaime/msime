# Agent Note: 手写时保留工具栏那一行，别再进得去出不来

Status: implemented

## Problem

2026-11-25 用户报告：从键盘的功能菜单点「手写」进入手写，机器上还没下载中文手写模型，书写区只有一个「下载中文手写模型」按钮，而键盘顶栏整行不见了 —— 没有品牌键、没有功能菜单，也就没有回到 26 键、九键这些布局的入口。截图里顶栏是空的，底行是 `123 | ， | 手写 | 。 | 中 | 重写 | ⌫ | 回车`。

原因是顶栏那一行受 `toolbarRowHidden` 管：显示方式为「隐藏」（`TouchToolbarLocalPreference.hidden`）或可选按钮一个都没打开时，`shortcutBar` 整行收起，连品牌键一起藏掉。iOS 保留顶栏高度，只把内容收起来，所以看上去就是一条空白。原来只有 26 键「长空格」样式用 `longSpaceToolbarRowKept` 让这一行一直留着，因为它把 符 挪到了工具栏上；手写不在其中。

手写底行没有方案换键盘的入口（三个移动端共用的 `LAYER | COMMA | SPACE | PERIOD | LANGUAGE | REWRITE | DELETE | RETURN`），中/英键只是切中英文、方案仍是手写，切回来还是手写。于是「隐藏」用户进了手写就没有一步到位的出路，未下载模型时更明显：书写区没东西可写，只剩一个下载按钮。

## Decision

把 `longSpaceToolbarRowKept` 扩成 `layoutKeepsToolbarRow`：长空格样式（原来那条）**或**正停在手写方案上（`handwritingKeepsToolbarRow` = `inputScheme == .handwriting && !isInLocalMode`）时，这一行都留着。手写这条只看方案，不看此刻的中/英，也不看画的是不是 123 / #+= 层，与长空格「切到英文 26 键时这一行还在」同一套规则。

显示方式为「隐藏」时这一行仍只放品牌键和收起键（`updateShortcutButtons` 里 `toolbarRowKept && toolbarHidden` 那一段），中间的可选按钮一个都不放。品牌键打开的功能菜单里有「手写」磁贴，再点一次按 `toggleHandwritingScheme` 回到进入前的布局（#9703 那次做的），所以品牌键就是手写的出口。

命中时机由 `updateToolbarSymbolsEntry()` 负责，它已经在 `updateKeyboardLayout()` 末尾调用，进出 手写 / 长空格 时 `kept` 变化会触发重排并重算 `shortcutBar.isHidden`。

## Alternatives considered

- **在手写底行加一个方案入口（比如把「中」换成方案键，或加一个键盘图标的键）**：一步到位。不采用：底行是三个移动端共用的 8 个键，320 pt 上加键要重新分份额、改 Android 与 HarmonyOS 的 `designEntries` / `handwritingActionRow`，为一个「隐藏工具栏」下的兜底改三端布局不划算。
- **只在「未下载模型」的空状态里加一个「返回键盘」按钮**：正好覆盖截图那一幕。不采用：它只治这一种状态，模型下载完之后（或者出错时）照样没有出路，而且空状态已经有下载按钮和一行状态说明，再叠一个按钮更挤。
- **强制把手写的显示方式当作「显示」处理**：等于忽略用户的设置。不采用：用户选「隐藏」是要顶栏少占地方，我们只在他没有别的路可走时留一个品牌键和收起键，中间按钮照旧不放。

## Consequences

- 手写（含中文、英文、123 / #+= 层）时顶栏不再是一条空白，左端有品牌键。设了「隐藏」的用户也会在这里看到水杉标志，这是有意的：没有它就没有回到其他布局的入口。
- 离开手写、回到 26 键标准样式或九键时，`layoutKeepsToolbarRow` 变回假，顶栏按用户的显示方式重新收起，中间的可选按钮也跟着回来。
- 长空格样式、密码框和只收数字的框这些既有规则没有变：前三者的判断照旧在 `layoutKeepsToolbarRow` 的第一个分支里。

## Verification

- 新增 `HandwritingTests.testHandwritingKeepsTheToolbarRowSoTheBrandMenuCanSwitchBack`：显示方式设为「隐藏」时，九键的顶栏收起；进手写后这一行不收起、品牌键可见、`schemeButton` 仍然藏着；点品牌键 → 功能菜单的「手写」磁贴 → 方案回到全拼 26 键，顶栏重新收起。同一文件补了 `toolbarBar(in:)` 取顶栏这一行本身（`shortcutBar` 是 `UIStackView`，不是按钮）。
- `scripts/test-mobile-handwriting-pad-parity.py` 通过（底行键序与份额、书写区铺满、竖屏多一行都没动）；`scripts/test-ios-key-layout-settings.py` 32 项通过；`git diff --check` clean。
- 编译与单测在 fork 的 iOS CI 上跑（本机是 Windows，没有 Xcode），结果见 PR 的验证一节。
