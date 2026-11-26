# Agent Note: 快速删除上滑按屏幕坐标判断，不再用窗口坐标

Status: implemented

## Problem

#11313 跟踪的 nightly Android 设备红里，`QuickDeleteUndoDeviceSmoke` 反复失败，失败串是 `Synthetic editor length=4;cursor=4;accepted=2;expected-length=0`：上滑快速清除只完成了按下删除键时的那一次普通删除，松手没有清空光标前文字。

设备日志把原因定住了。这条用例排在 `EnglishCorrectionDeviceSmoke` 之后，而后者把输入模式切到英文后没有还原，全局模式跨 `am instrument` 存活，所以它开始时是英文模式：`setup` 的 `setText` 让光标末尾的 `X` 被当成正在输入的英文词，候选区多出一行英文建议。按下删除键会立刻删掉 `X`，这一行在同一次手势里收起，键盘矮了 57 px（`keyTop` 从 460 变成 403），视图按新高度重新排版；但同一次触摸里输入系统还在用按下时的窗口位置把屏幕坐标换成窗口坐标，于是注入在同一点上的移动事件被算成 `getY()=-53.25`，而按真实位置应当是 `-110.25`，正好偏 57 px（约 22 dp）。待命线在键上沿上方 40 dp，用例把手指送到 42 dp，偏掉的这 22 dp 让状态只到 `REVEALED`，松手什么也不清空。用例在两次运行里分别红在「editor switch invalidates backup」和「quick-delete backup survives short press until release」，就是哪一次手势正好碰上重新排版。

键盘在实时输入里也会这样变高变矮（英文建议行收起、候选行出现），所以这不是设备验收的注入方式独有的问题：手势中途重新排版时，按窗口坐标算出来的指尖位置会整体偏掉这段高度，用户得再多滑一段才待命。

只用 raw 坐标还不够。上滑那一段改完之后，同一个套件的重建运行（`android-device` API 35 完整套件，run 45719780915）在「held downward undo restores deleted prefix」一阶段又红了：用例把指尖送到布局键下沿 18 dp，撤销的待命线是键下沿 16 dp，本应留 2 dp 余量，实际停在 `REVEALED`（`yInKey=177.25`，待命线 179.4）。差出来的 7 px 正好是 `KeyboardPressFeedback` 按住时给键的变换：缩放 0.94（绕中心，上沿下移 `0.06 * 137 / 2` px）加下移 1 dp，`getLocationOnScreen` 拿到的是变换后的上沿，而待命线用的 `getHeight()` 是布局高度，两者混在一起就少算了约 2.5 dp。按下事件里的 `getY()` 会把按键自己的变换逆回去，所以以前没有这个问题；改用 raw 坐标时把这一层也丢了。

## Decision

上滑判断一律改用 `MotionEvent#getRawX/getRawY` 和键盘外框**此刻**的屏幕位置：`QuickDeleteOverlay.fingerY(rawY)` 把屏幕坐标换到覆盖层坐标系，`ImeLetterRows.fingerYInKey()` 再换算到删除键坐标系；按下点、水平滑动的横向距离、点按取消的「离开键」判断和撤回下滑判断同样改用 raw 坐标。raw 坐标是手指在屏幕上的真实位置，不受窗口位置换算的影响，而覆盖层的位置每次都按当前布局读。

`BackspaceSwipePolicy.fingerYInKey()` 再把按键自己的按压缩放撤掉（`(fingerY - keyTop) / scaleY`）：待命线按布局高度量，位置也必须回到布局坐标系。没有缩放时结果不变，所以正常手势和以前完全一样。

键盘没有动时这套换算和原实现代数相同（`keyTop + getY()` 等于 `rawY - 键在屏幕上的 y`），所以不改变正常手势的判定；只有手势中途重新排版时结果才不同，而且是有据可查的那个正确值。设备日志里这次手势改判为待命，正在待命线以上 5 px。

## Alternatives considered

- **只在测试里先切回中文模式**：最强的理由是不碰产品代码，一行就能让该套件不再继承英文模式。放弃它，因为英文模式下建议行收起、键盘变矮是真实输入里会发生的事，按窗口坐标判断指尖位置偏掉这段高度是产品缺陷；切回中文只是让验收不再经过这条路径，把缺陷留在原处。
- **让 `quickDelete()` 按下之后重新读删除键位置再注入移动**：最强的理由是删掉键位缓存这个变量。放弃它，因为日志证明键在屏幕上的位置没变（一直是 1973），偏掉的是输入系统的窗口换算，测试重新读位置也修正不了；而且这会让用例对键盘几何变化过度敏感。
- **在输入系统同步新窗口位置之前把手势事件暂缓处理**：最强的理由是能从源头等坐标变正确。放弃它，因为输入法只能在收到的事件里判断，暂缓意味着丢掉一段手指轨迹，取消或加速的时机也会跟着延迟，代价远大于改用自己的换算。

## Verification

- `platforms/android/check-host.sh` 通过：387 条 JVM 冒烟、215 个设备套件源码编译，服务和键盘的 Java/API 契约检查全过。
- 设备日志核对：失败运行里该套件的移动事件与重新排版重叠，`keyTop` 差 57 px，算出阶段的指尖 y 恰好在待命线下方约 5 px；改后同一手势在待命线上方 5 px。
- 下滑撤回也核对过：run 45719780915 里 `yInKey=177.25` 与 16 dp 待命线 179.4 差 7 px，正好是按住时 0.94 缩放加 1 dp 下移让变换后的上沿比布局上沿低的那一段；`BackspaceSwipePolicySmoke` 用同一组数值钉住（173.6 → `REVEALED`，173.6 / 0.94 = 184.7 → `ARMED`）。
- 远端 `android-device(API 35)` 完整套件手动触发验证（结果见 PR 的验证段）。

## Consequences

- 手势中途键盘变高变矮（英文建议行收起、候选区行数变化、工具栏收起）时，上滑与撤回的判断跟着布局走，不会再少算或多算这段高度。
- 按下反馈什么时候把键缩小、缩多少都从 `getScaleY()` 读，以后改 `KeyboardPressFeedback` 的缩放不用再动这里的换算；换成缩放以外的新变换（旋转、非中心 pivot）时要重新看一遍。
- 这条设备用例对缺陷的覆盖依赖「英文模式从上一个套件继承下来」。如果以后有人在 `EnglishCorrectionDeviceSmoke` 结束时还原中文，这条覆盖会静默消失，届时要补一条显式的英文模式快速删除阶段。

## Related decisions

- [Android 编辑器无法读取光标时仍执行快速清除](2026-11-15-android-quick-delete-unreadable-cursor.md) 记录同一条用例里另一个与位置读取有关的边界。
- [Android 顶部删除键保留紧凑快速删除提示](2026-11-19-android-quick-delete-compact-preview.md) 记录待命线随候选区高度变化的约束。

## Follow-ups

- #11313 由下一次 nightly 确认后关闭；同一用例若换了阶段再红，先看该阶段的手势是否与一次键盘重新排版重叠。
