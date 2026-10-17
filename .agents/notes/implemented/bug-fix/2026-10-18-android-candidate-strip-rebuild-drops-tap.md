# Agent Note: Android 候选条按下期间不重画，点击不再被吞掉

Status: implemented

## Problem

用户反馈 Android 26 键上「候选区感觉不够大，容易点不上」：点得很准也偶尔没有反应。

候选按钮本身不小。26 键的候选 chip 与 iOS 是同一套尺寸：最小宽 30 dp、左右内边距各 11 dp、字号 17（iOS 18）、候选行高 42 dp（iOS 38，读取行 14 dp 两边相同）。所以问题不在目标尺寸。

`MSIMEInputService` 画候选条时，每一帧都先 `candidates.removeAllViews()` 再把 `candidateButtons` 里缓存的按钮逐个挂回去。按钮对象是复用的，但摘下来的那一刻，框架会把正在进行的按压转成 `ACTION_CANCEL`，`OnClickListener` 不再触发——这一次点击就丢了。而候选内容几乎每帧都在变（组字时 Engine 的异步回包、译文角标、页码），render 的触发者也很多（每次按键、每次回包、工具栏状态），所以按下到抬起之间撞上一次 render 并不罕见，表现就是「偶尔点不上」。

## Decision

`render` 在重画候选条和展开面板之前递归检查候选视图树里有没有按下的视图（`candidateTreeHeld`，逐个子视图 `isPressed()`）。有的话这一帧整体跳过，并置 `candidateRenderDeferred`；所有候选按钮和展开面板动作按钮共用服务里的触摸监听（返回 `false`，不影响点击、长按和滚动取消），在 `ACTION_UP` / `ACTION_CANCEL` 时看到这个标记就补一次 `render`，画面只晚这一下按压。顶部候选条、完整候选网格和全拼九键三栏面板都走同一条保护路径。

跳过的是整个候选区那一帧的收尾（页码、展开键、`applySkin`、`styleTopRow`），由松开后补的那次 render 一起补上；标记也在每次正常重画开头清掉，所以触摸被窗口带走、监听没等到抬起时不会一直卡住。

字母键滑动符号的判定和三平台默认值都不改。iOS 的 `SwipeHintPolicy.threshold` 与 Android 的 `SwipeHintPolicy.THRESHOLD_DP` 同为 14、默认都是开着、方向都是向下，这条规则本身没有 Android 独有的偏差；把默认值改成关闭会破坏 iOS、鸿蒙注释里写明的三平台一致，用户要关可以在设置里自己关。

## Alternatives considered

- **把候选行调高**：直接但错。Android 已经是 42 dp，比 iOS 的 38 dp 更高，继续加高只会把键盘顶上去，也解释不了「点得准也没反应」。
- **加宽候选 chip**：chip 尺寸已经和 iOS 一致（30 dp 最小宽、11 dp 内边距），再宽会挤掉一行里能看到的候选数，收益小于代价。
- **按下时禁止滚动候选条**：候选条要能用同一根手指按下后滚动，禁掉会换来另一个「拖不动」的问题。
- **候选按钮改为不复用、每次新建**：新对象同样要 `addView`，按下中的那个照样被摘掉，还多出每帧分配。
- **只保护顶部候选条**：改动小，但完整候选网格和九键三栏面板同样会在 `render` 时清空子树，按下其中的候选仍会收到 `ACTION_CANCEL`。
- **改 `KeyboardPressButton` 的 `staleRelease` 机制来兜住这次按下**：那套逻辑处理的是「同一个键连按」时迟到的 `setPressed(false)`，不是视图被摘下；而且它作用于所有键，改动的爆炸半径远大于候选条这一处。

## Consequences

- **收益**：候选条和展开候选面板上的按压不再被 render 打断，点击不再随机丢失；候选内容最多晚一帧更新，用户看不出来。
- **代价**：按下候选的那一小段时间里，候选区和工具栏的收尾绘制推迟到松手之后。若这次按压以滚动取消结束，期间到达的新候选会在下一次 render 时出现，不会漏内容。
- **验证**：`check-host.sh` 的 `AndroidLocalSettingsSmoke` 与候选相关 JVM 冒烟；顶部、完整候选网格和九键三栏按下期间的重画路径要真机或模拟器才碰得到，按 `platforms/android/tests/device` 的 `smoke.sh` 在 API 35 上验收。
