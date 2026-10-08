# Agent Note: Android 剪贴板历史的分词插入

Status: implemented

## Problem

#5645：从网页复制的一段文字常常带着不要的头尾。用户希望在键盘的剪贴板历史里长按一条、选「分词」，把它切成词，点选需要的那几个，再「确认」插入，也能「取消」。

## Decision

- 长按一条本机历史打开的操作行里有「分词」（`ImePanels.renderClipboardItemActions`，操作行本身见 [剪贴板面板的确认和操作画在面板里](../bug-fix/2026-10-08-android-clipboard-panel-in-place.md)）。点它后面板换成分词界面：上面是按行折排（`WrapRowLayout`）的词片，点一下选中、再点取消；底部固定一行「取消 · 已选几个 · 全选/全不选 · 插入」，和 issue 里「下面有确认和取消」一致。「插入」把选中的词片拼起来，经 `insertClipboardText` 上屏并关闭面板；原来那条历史不变。换分段、重开面板都退出分词。
- 切词和拼接在 `ClipboardSegmentation`（`platforms/android/java/app/msime/android/clipboard/`），不依赖 Android：
  - 词边界来自 `java.text.BreakIterator.getWordInstance(Locale.CHINESE)`。Android 上它由 ICU 实现，中文按 ICU 自带的中日文词典切；超过 6 个字的纯汉字片段再拆成单字，词典里没有的长串也能逐字掐头去尾。
  - 空白是分隔，不显示、不可点选，相邻的空白合并成一段。拼接时只有夹在两个相邻已选词片之间的空白保留，所以选中连续的英文单词得到带空格的原文。
  - 只对开头 2000 个 UTF-16 单元分词，不切开代理对；超出时界面上说明。
- 词片在面板里自己的滚动区里滚，操作行在滚动区外、贴着面板底边，长文字滚到末尾「去尾」时「插入」「取消」仍在屏幕上。第一版把操作行做成面板的第一行，跟着词片一起滚，几百个词片时滚到末尾就看不到它了。剪贴板面板外面还套着一层开了 fillViewport 的滚动视图：词片滚动区的高度基数给 1 px、权重 1，外层第一次不限高测量时面板只报很小的高度，外层再按一屏高精确测量，剩余高度全部给滚动区；基数给 0 的话不限高测量会按全部词片报高，面板比一屏高，操作行又被推出屏幕。面板不比一屏高时外层滚不动，也不拦截触摸，滑动由词片滚动区处理。
- 这是对一段已经存在的文字找词边界，不是输入时的组词：宿主 must not 为此调用或复制 Engine 的组词、词库逻辑。

## Alternatives considered

- **经 Engine 分词**（新增一个 Host API，用拼音词库做最大匹配）— 和输入时的词库一致，专名、用户词也能切对；但 Engine 现在没有对任意文字分词的入口，要跨 crates/engine、host-api、JNI 三层加接口，而 ICU 词典对这里「掐头去尾」的需求已经够用。日后要更准时再加。
- **按字切**（每个字、每个字母一片）— 最简单，不依赖任何词典；但一段英文要逐个字母点，中文也要一个字一个字点，长文字里几百个片段，失去「分词」的意义。
- **把操作行做成悬在键盘表面上的独立视图**（加在 `keyboardSurface` 里、贴底显示）— 不用嵌套滚动；但剪贴板面板的显示和隐藏散在 `closeClipboardHistory`、`alignOverlaysBelowTopRow`、`anyToolbarPanelOpen` 等多处，另一个视图要在每一处跟着显隐，漏一处就会在键盘上留下一条孤零零的操作行。
- **Material 的 `ChipGroup` 做折行**— 现成的流式布局；但 `core/` 里的面板代码要被 `check-host.sh` 的 JVM 冒烟编译，那一步跳过任何引用 Material 的源文件，所以自己写了一个只会折行的 `WrapRowLayout`。

## Consequences

- **收益**：长文字可以只插入其中一段，不用先整段粘贴再删；中英混排、数字、版本号（`os1.0`）都按词切。
- **代价**：分词质量取决于设备的 ICU 版本；词典里没有的专名会被切开或拆成单字。超长文字只切开头 2000 个字。分词界面不支持拖动连选，选一大段要逐个点或先「全选」再去掉头尾。

## Verification

`platforms/android/tests/clipboard/ClipboardSegmentationSmoke.java` 在 JVM 上验证片段覆盖原文、空白合并、拼接保留与丢弃空白、长串汉字拆字、超长截断和代理对边界（JVM 的 `BreakIterator` 不按词典切中文，ICU 的切词效果没有在设备上验证）。`bash platforms/android/check-host.sh` 编译面板和 `WrapRowLayout` 并运行它，另有源码守卫要求 `renderClipboardSegmentation` 给词片单独建滚动区。操作行固定在底部的实际布局（含 fillViewport 的两次测量）没有在设备上看过。
