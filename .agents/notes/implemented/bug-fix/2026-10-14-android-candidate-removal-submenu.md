# Agent Note: Android 删除候选词的子菜单确认

Status: implemented

## Problem

用户报告：Android 长按候选、在菜单里选「删除词条…」并点确认框里的「删除」之后，键盘被收起；期望是删掉候选词只刷新候选列表，键盘保持打开。

原因是 `MSIMEInputService.confirmCandidateRemoval` 用 `new AlertDialog.Builder(this)`，`this` 是 `InputMethodService`。输入法服务没有 Activity 的窗口令牌，从它弹出的对话框要么加不上窗口、把输入法进程带崩，要么抢走编辑器的窗口焦点：Via 这类 WebView 浏览器一见窗口失焦就让输入框失焦、收起键盘，普通编辑器则多半在对话框关闭后回不到键盘（#5605、#5653 已经就剪贴板面板的确认记下同一条约束）。删除本身没有问题：`NativeClient.removeCandidate` 加 `apply` 只刷新候选列表，关键盘的是那个对话框。

## Decision

- 删除候选词的确认与 iOS 一致，收进长按菜单的子菜单：`ImeCandidates.showCandidateMenu` 把「删除词条…」建成 `SubMenu`（`popup.getMenu().addSubMenu`），子菜单里只有一项「确认删除此词条」，文字来自 `CandidateManagementAction.confirmationTitle()`，与 iOS 的 `UIMenu` 嵌套子菜单逐字相同。点开子菜单、再点确认，全程一次长按，键盘之外的窗口一个都不开。
- 候选身份不用额外保存：子菜单的确认项和同一份菜单里的其它操作一样，用的是长按那一刻从 `visibleCandidate` 读到的 `id`（Engine 的 session/generation/index）。没有待确认状态，也就没有要作废它的时机；执行前宿主复核 session/generation，过期菜单操作直接返回，Engine 继续作最终校验，与「优先显示/固定」相同。拒绝旧身份不能停止有效会话，见[过期候选回调](2026-10-14-android-stale-candidate-callbacks.md)。
- 单字候选不提供删除（引擎侧保护），与 iOS 的 `candidate.unicodeScalars.count != 1` 一致。
- `check-host.sh` 守住两条：`MSIMEInputService.java` 里不出现 `new AlertDialog`；`ImeCandidates.java` 用 `addSubMenu(Menu.NONE, Menu.NONE, action.ordinal(), action.title())` 建删除确认，文字来自 `CandidateManagementAction.confirmationTitle()`。

## Alternatives considered

- **退出菜单再长按一次的两步确认**（这个分支上先做过的实现）：第一次选「删除词条…」只记下候选并在诊断行提示再选一次，第二次选到同一个候选才真正删除。不新开窗口，实现也只是一个小状态机；但菜单在第一次选择后就关掉了，提示说「再次选择以确认」而候选条上没有可点的确认项，用户实际只能退出去重新长按——实测「交互有点奇怪，没有地方再次选择」。iOS 是同一个菜单里当场展开确认项，两者差得远，所以换掉。
- **给对话框设置输入法窗口的令牌**（`lp.token = keyboardRoot.getWindowToken()`，`TYPE_APPLICATION_ATTACHED_DIALOG`，AOSP LatinIME 的做法）— 改动最小，确认框还是系统样式；但对话框仍是可获得焦点的窗口，会把窗口焦点从编辑器拿走，WebView 一类的编辑器失焦就收起键盘（#5653），用户要求的「删除时键盘不关闭」做不到。剪贴板清空当时也是这个理由被否掉的。
- **键盘内联确认**（选「删除词条…」后把候选条整行换成「删除“词”？ 取消 / 删除」，与剪贴板面板清空的方式一致）— 一个窗口都不开，任何编辑器里都稳；但候选条是单行横向滚动，展开候选面板又是另一层全屏覆盖，两处都要新画确认 UI 和它的无障碍描述，比子菜单多一套布局、换肤和无障碍。
- **不加确认直接删除** — 界面最简单，也和用户「删掉候选词只需要刷新候选列表」的描述最贴；但删除用户词条在键盘里不可撤销（要回词库页才能恢复），误触一次选错候选就没了，其它平台都保留确认。

## Consequences

- **收益**：与 iOS 同款交互，一次长按就能删完；确认和删除全程在输入法窗口内，任何编辑器里键盘都不再被收起，删除后只经 `apply` 刷新候选列表。没有待确认状态，也没有状态过期、异步结果把它冲掉的边角情况。
- **代价**：「删除词条…」比别的操作多一级（要在它展开的子菜单里点确认），菜单项比平铺多一层；Android 的 `PopupMenu` 子菜单是系统另开的弹窗（与长按菜单本身同一类窗口），窄屏上贴在父菜单旁边、必要时靠边翻转。

## Verification

`platforms/android/tests/candidate/CandidateManagementSmoke.java`（菜单顺序、动作映射、确认文字与它对非删除操作的拒绝）；`bash platforms/android/check-host.sh` 编译服务代码并跑上面的源码守卫。设备上的长按（父项 + 子项）流程由 CI 的 Android 模拟器作业做装包验收，本机没有 Android SDK，没有本地验证。
