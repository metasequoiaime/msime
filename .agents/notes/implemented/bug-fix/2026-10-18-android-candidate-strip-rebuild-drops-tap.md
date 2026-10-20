# Agent Note: Android 候选条按下期间不重画，点击不再被吞掉

Status: implemented

## Problem

用户反馈 Android 26 键上「候选区感觉不够大，容易点不上」：点得很准也偶尔没有反应。

候选按钮本身不小。26 键的候选 chip 与 iOS 是同一套尺寸：最小宽 30 dp、左右内边距各 11 dp、字号 17（iOS 18）、候选行高 42 dp（iOS 38，读取行 14 dp 两边相同）。所以问题不在目标尺寸。

`MSIMEInputService` 画候选条时，每一帧都先 `candidates.removeAllViews()` 再把 `candidateButtons` 里缓存的按钮逐个挂回去。按钮对象是复用的，但摘下来的那一刻，框架会把正在进行的按压转成 `ACTION_CANCEL`，`OnClickListener` 不再触发——这一次点击就丢了。而候选内容几乎每帧都在变（组字时 Engine 的异步回包、译文角标、页码），render 的触发者也很多（每次按键、每次回包、工具栏状态），所以按下到抬起之间撞上一次 render 并不罕见，表现就是「偶尔点不上」。

#8983 还暴露了补帧时机的遗漏：`ACTION_UP` 的触摸监听先于框架自己的点击处理，点击更新候选后 `View` 仍可能处于按下状态，`render` 再次被延后；随后的 `UnsetPressedState` 才真正松开按钮，此时没有触摸事件唤醒补帧，于是选词已上屏，候选区却要等另一次操作才更新。只在 UP 时 `post(render)` 也可能排在框架点击和松开之前，不能保证解决。

## Decision

`render` 在重画候选条和展开面板之前递归检查候选视图树里有没有按下的视图（`candidateTreeHeld`，逐个子视图 `isPressed()`）。有的话这一帧整体跳过，并置 `candidateRenderDeferred`；所有候选按钮和展开面板动作按钮通过 `bindCandidateRenderRelease(KeyboardPressButton)` 绑定实际松开通知。`KeyboardPressButton.setPressed(false)` 在状态确实改变、且没有被 `staleRelease` 忽略时才发通知，服务向主队列提交补帧任务，在任务执行时检查标记；不接管点击、长按或滚动。顶部候选条、完整候选网格、全拼九键三栏、手写、英文建议和候选辅助页都走这条保护路径，绑定参数的类型让新增普通按钮无法悄悄绕过通知契约。

跳过的是整个候选区那一帧的收尾（页码、展开键、`applySkin`、`styleTopRow`），由实际松开后补的那次 `render` 一起补上。标记只在正常重画时清掉；补帧时若还有另一根手指按住候选，仍然延后，由最后一次实际松开唤醒。通知用服务的主队列，不依赖按钮还挂在窗口上；同一次松开不重复通知。普通键未绑定监听，连按的 `staleRelease` 判定和框架的点击、滑出、取消语义保持原样。

字母键滑动符号的判定和三平台默认值都不改。iOS 的 `SwipeHintPolicy.threshold` 与 Android 的 `SwipeHintPolicy.THRESHOLD_DP` 同为 14、默认都是开着、方向都是向下，这条规则本身没有 Android 独有的偏差；把默认值改成关闭会破坏 iOS、鸿蒙注释里写明的三平台一致，用户要关可以在设置里自己关。

## Alternatives considered

- **在 UP 后固定延迟再绘制，或一直轮询按下状态**：改动集中在服务内，也容易覆盖一次触摸；但框架队列和按压反馈时长并不固定，固定延迟仍有竞态，轮询又引入无必要的唤醒和生命周期管理。直接观察实际松开不需要猜时长。

- **把候选行调高**：直接但错。Android 已经是 42 dp，比 iOS 的 38 dp 更高，继续加高只会把键盘顶上去，也解释不了「点得准也没反应」。
- **加宽候选 chip**：chip 尺寸已经和 iOS 一致（30 dp 最小宽、11 dp 内边距），再宽会挤掉一行里能看到的候选数，收益小于代价。
- **按下时禁止滚动候选条**：候选条要能用同一根手指按下后滚动，禁掉会换来另一个「拖不动」的问题。
- **候选按钮改为不复用、每次新建**：新对象同样要 `addView`，按下中的那个照样被摘掉，还多出每帧分配。
- **只保护顶部候选条**：改动小，但完整候选网格和九键三栏面板同样会在 `render` 时清空子树，按下其中的候选仍会收到 `ACTION_CANCEL`。
- **改 `KeyboardPressButton` 的 `staleRelease` 机制来兜住这次按下**：那套逻辑处理的是「同一个键连按」时迟到的 `setPressed(false)`，不是视图被摘下；而且它作用于所有键，改动的爆炸半径远大于候选条这一处。

## Consequences

- **收益**：候选条和展开候选面板上的按压不再被 render 打断，点击不再随机丢失；选词后候选区在实际松开后的主队列任务中更新，不再需要下一次按键唤醒。
- **代价**：按下候选的那一小段时间里，候选区和工具栏的收尾绘制推迟到松手之后。滚动取消也会清除按下状态并唤醒补帧；服务需要保留这个松开通知契约。
- **验证入口**：`check-host.sh` 覆盖宿主契约与 JVM 冒烟；`CandidatePanelDeviceSmoke` 用真实 DOWN/UP 选取顶部候选和展开后的第 10 个候选，分别覆盖 26 键和九键，要求编辑器精确上屏且候选界面不经其他输入就收起。无障碍 `ACTION_CLICK` 绕过框架按压生命周期，不能替代这项回归。设备内另检查迟到松开不通知、滑出和取消只通知一次且不点击。


## 设备验证

#8983 的回归在未修复松开通知的宿主上复现：编辑器已收到展开面板第 10 个候选，但 15 秒内面板仍未收起（API 28）。修复后的 `CandidatePanelDeviceSmoke` 和既有 `CandidateExpansionDeviceSmoke` 在 API 28、35 专用模拟器均通过；后者覆盖释义、展开、快速输入及反复选词。设备包由本分支 Java 原生宿主、开发签名和固定 `android-v0.5.0` 的库与资源组成，基线 APK SHA-256 为 `924d1727b9ecd1099c7a96d1acaadec4a9cfc95c8a54d2f075e21c2a0a3238a1`；不把这项验证扩写为当前 Rust 整包或真机验证。原生 Gradle 宿主打包、250 项 JVM 冒烟及 196 份设备源码编译通过。

远程镜像的 `pnpm run verify-notes`、`bash scripts/verify-local.sh --quick`（运行 `20261021-000401`，退出 0）及本地 `git diff --check` 通过。quick 覆盖仓库契约、笔记、Rust 工作区、Android 目标与 Java 宿主、共享 Apple bridge；其他未受影响的平台按范围跳过，缺少原生资源的桌面 Tauri crate 明确跳过。
