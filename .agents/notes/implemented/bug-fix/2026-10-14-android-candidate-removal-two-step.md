# Agent Note: Android 删除候选词的两步菜单确认

Status: implemented

## Problem

用户报告：Android 长按候选、在菜单里选「删除词条…」并点确认框里的「删除」之后，键盘被收起；期望是删掉候选词只刷新候选列表，键盘保持打开。

原因是 `MSIMEInputService.confirmCandidateRemoval` 用 `new AlertDialog.Builder(this)`，`this` 是 `InputMethodService`。输入法服务没有 Activity 的窗口令牌，从它弹出的对话框要么加不上窗口、把输入法进程带崩，要么抢走编辑器的窗口焦点：Via 这类 WebView 浏览器一见窗口失焦就让输入框失焦、收起键盘，普通编辑器则多半在对话框关闭后回不到键盘（#5605、#5653 已经就剪贴板面板的确认记下同一条约束）。删除本身没有问题：`NativeClient.removeCandidate` 加 `apply` 只刷新候选列表，关键盘的是那个对话框。

## Decision

- 删除候选词的确认改成长按菜单里的两步：第一次选「删除词条…」只记下这一个候选并在诊断行提示「再次选择「删除词条…」以确认」，第二次选到同一个候选才交给 `editCandidate` 真正删除。两步都走原本的 `PopupMenu`，不新开任何窗口。与 iOS 的 `UIMenu` 嵌套子菜单、鸿蒙的「再次选择以确认」是同一个交互。
- 状态机在 `CandidateRemovalConfirmation`（`platforms/android/java/app/msime/android/candidate/`），不依赖 Android 运行时：`confirms(session, generation, index)` 与上一次记下的身份相同就返回 true 并清空，否则记下身份返回 false；`clear()` 作废。
- 候选身份沿用 Engine 给的 session/generation/index，与 `editCandidate` 的其它操作（优先显示、固定、取消固定）同一套判断；不额外比较候选文字，简繁转换或释义变化不会把待确认的候选判成另一个。换一次引擎候选（`apply` 开头）或结束输入会话（`stop`）都会作废，用户重新走两步。
- 提示画在诊断行（`notice`），它是 `ACCESSIBILITY_LIVE_REGION_POLITE`，读屏用户同样能听到这一步，不再另发 `announceForAccessibility`。
- `check-host.sh` 守住两条：`MSIMEInputService.java` 里不出现 `new AlertDialog`；`requestCandidateRemoval` 经 `CandidateRemovalConfirmation.confirms` 判断、且文件里出现 `candidateRemovalConfirmation.clear()`。

## Alternatives considered

- **给对话框设置输入法窗口的令牌**（`lp.token = keyboardRoot.getWindowToken()`，`TYPE_APPLICATION_ATTACHED_DIALOG`，AOSP LatinIME 的做法）— 改动最小，确认框还是系统样式；但对话框仍是可获得焦点的窗口，会把窗口焦点从编辑器拿走，WebView 一类的编辑器失焦就收起键盘（#5653），用户要求的「删除时键盘不关闭」做不到。剪贴板清空当时也是这个理由被否掉的。
- **键盘内联确认**（选「删除词条…」后把候选条换成一行「删除“词”？ 取消 / 删除」，与剪贴板面板清空的方式一致）— 一次长按就能完成，确认和结果都在键盘里；但候选条是单行横向滚动，没有像剪贴板面板那样可以放按钮的地方，要在键面上新画一层确认 UI，比两步菜单多一套布局、换肤和无障碍。
- **嵌套子菜单**（照搬 iOS 的 `UIMenu` 子菜单：点「删除词条…」再展开「确认删除此词条」）— iOS 上是这一个菜单里的两步；Android 的 `PopupMenu` 渲染子菜单要再开一个可获得焦点的弹窗，正是 #5653 里让 WebView 收起键盘的机制，等于把刚拆掉的问题换个地方装回去。
- **不加确认直接删除** — 界面最简单，也和用户「删掉候选词只需要刷新候选列表」的描述最贴；但删除用户词条在键盘里不可撤销（要回词库页才能恢复），误触一次选错候选就没了，其它平台都保留确认。

## Consequences

- **收益**：删除候选词的确认和删除整个过程都在输入法窗口内完成，任何编辑器里键盘都不再被收起；也不再有对话框加不上窗口、把输入法进程带崩的路径。删除完成后只经 `apply` 刷新候选列表。
- **代价**：删一个词要长按两次（第二次仍要展开菜单再选「删除词条…」），比一次点确认多一步；提示只在诊断行停留几秒（`InputDiagnosticPolicy.DISMISS_DELAY_MILLIS`）就消失，用户没跟上就要重新走两步。`apply` 一清状态，任何引擎转换（包括异步到达的释义/云候选结果）都会让待确认作废。

## Verification

`platforms/android/tests/candidate/CandidateRemovalConfirmationSmoke.java`（臂化、确认、换槽位、换代次、换会话、清空）；`bash platforms/android/check-host.sh` 编译服务代码并跑上面的源码守卫。模拟器/真机上的长按两次流程没有在设备上验证。
