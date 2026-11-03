# Agent Note: Android 候选条、表情和手写面板跟随键盘的分明暗皮肤选择，候选行纵向居中

Status: implemented

部分取代[Android 键盘皮肤按明暗分别选择](../feature/2026-10-26-android-keyboard-skin-mode-slots.md)里「候选窗、表情、手写继续按现有全局主题读取」这一条；两个槽位的格式、缺槽回退、设计库引用和候选皮肤包槽位保持不变。

## Problem

#10749：Android nightly 上，打字时候选条的底色和键盘对不上，候选词也贴在候选条的上半截。报告者附的截图里键盘是内置「浅色」，候选条却是「跟随系统」套上秋杉应用主题后的浅色底（#EED7C7），在不同皮肤下候选条都是这个颜色。

颜色不对的原因：#9924 给 Android 加了 `android_keyboard_skin_light` / `android_keyboard_skin_dark` 两个槽位，皮肤页和键盘里的皮肤面板只写槽位，不再写 `global_theme`。`MSIMEInputService.surfaceSkin` 只在 `screen_keyboard_theme` 时读槽位，候选条（`candidate_theme`）、表情（`emoji_theme`）和手写（`handwriting_theme`）仍读 `global_theme`。旧决定的前提是「候选窗有自己的全局主题可选」，可 Android 上已经没有任何入口写 `global_theme`：`SkinsPage` 里的那处只是生成预览用的 probe，云同步的 `platform.android.global_theme` 也只有 Android 自己写。结果是候选条一直停在默认的 `system` 上，经 `ImeStyler.themed` 按应用主题着色，换什么键盘皮肤都不变，用户也没有办法改它。

位置不对的原因（0.6.0 正式版就有）：候选行 `candidates` 由 `LayoutPolicy.row` 建，没有设 gravity；`horizontalCandidateScroll.addView(candidates)` 不带参数，这一行按 MATCH_PARENT 量出整条候选行的高度（`ImeToolbar.CANDIDATE_LINE_DP` 随候选栏大小缩放），候选按钮却按 WRAP_CONTENT 加入、只有一行字高，LinearLayout 默认从顶端摆，chip 和文字都贴在上半截。外层 `line` 的 `setCenteredVertically` 只管 viewport 本身，管不到 viewport 里的这一行。九键拼音行在 `ImeLayoutRows` 里设了 `CENTER_VERTICAL`，没有这个问题。

## Decision

- **四块面板都读槽位**：`surfaceSkin` 对每块面板都经 `KeyboardSkinModeChoice.selected(preferences, dark, modeSkinLibrary)` 取主题，不再按 key 分支。`dark` 仍按这块面板自己的明暗（`candidate_theme` 等）解析，所以候选条设成固定深色时读的是深色槽位，和键盘此刻的明暗无关。缺槽或引用的设计不在库里时，`selected` 自己回到 `global_theme` 和 `custom_theme.keyboard`，旧偏好的表现不变。
- **候选皮肤包仍然优先**：`candidateAppearanceFor` 先用上面的皮肤算出回退配色，再交给 `CandidateSkinResolver.paletteFor`；`global_theme` 为 `custom` 且 `custom_theme.candidate_skin(_dark)` 指向一个包时，包的配色和画布照旧盖在上面，判定条件没有改。
- **设计库读完时一起重算**：槽位是我的设计时，设计库在偏好工作线程上异步读取。`refreshModeSkinLibrary` 的回调原先只重算键盘 `skin`；现在候选条（`candidateAppearanceFor`）、表情、手写一起重算，按 `displayKey` 和 `candidateAppearance.key()` 比较，有变化才 `applySkin` 和 `render`。不这样做，冷启动后候选条会一直停在旧主题上，直到下一次偏好变化。冷启动的皮肤提示 `SKIN_HINT_KEYS` 已经带着两个槽位，`onCreate` 里的 `applyCandidateAppearance(hint)` 走的就是 `surfaceSkin`，不用另外改。
- **在键盘里选设计时先补进设计库**：`saveKeyboardSkin` 选的是我的设计时，键盘直接用 `KeyboardSkin.custom(design, …)` 画，候选条、表情和手写却经 `surfaceSkin` 按 `modeSkinLibrary` 解析 `design:` 槽位。原槽位是内置主题时 `refreshModeSkinLibrary` 把库置成空表，刚从社区装进来的设计也还不在库里，这两种情况下槽位解析不到，候选条会回到旧主题，直到保存往返结束、`applyPreferencesSnapshot` 重读设计库。所以库里没有这项时先补一份临时副本再算三块面板；写入成功后重读的库会替换它，写入失败时槽位回到原值，多出的这一项没人引用。另一种做法是接受这段错配、只在这里记下，没有采用：改动前候选条从不跟键盘走，这段错配是这次才引入的，补一项的代价只是一份短命的列表副本。
- **候选行纵向居中**：`candidates` 建好后调 `ViewPolicy.setCenteredVertically(candidates)`，与九键拼音行和 `ImeToolbar` 的各行做法相同。这一行里的其他内容（手写候选、义项页、九键常用标点、邮箱后缀、英文建议）都按 WRAP_CONTENT 高加入，一样居中。

## Alternatives considered

- **只改候选条，表情和手写照旧读 `global_theme`**：改动面最小。没有采用：三块面板是同一个函数里的同一个分支，表情和手写在 Android 上同样没有入口能改 `global_theme`，换了键盘皮肤后打开表情面板会出现同样的错配。只修一块等于把同一个缺陷留在另外两块。
- **在 Android 上恢复一个候选窗主题入口，或者选键盘皮肤时顺带写 `global_theme`**：后者正是旧笔记否掉的做法，会丢掉另一种明暗的选择；前者让用户在两个地方分别挑皮肤，而报告者要的是候选条和键盘一致，触屏键盘上候选条本来就是键盘的一部分。
- **候选按钮按整行高（MATCH_PARENT）铺满，chip 背景加上下 inset 保持原来大小，下半截也能点中**：这次没有做：候选条的按下、悬停和首选 chip 由 `ImeCandidates.chip` 和 `ImeStyler.candidateDrawable` 等多份 drawable 画，候选皮肤包的画布资源也贴在按钮边界上，inset 要按随字号和候选栏大小变化的内容高度计算，改动面和回归风险都比居中大得多，需要在设备上逐种皮肤核对。居中之后，按钮上下各留出大约同样宽的空白，原来集中在下半截的点不中区域减半；整行可点留作单独的改动，回复里不承诺这一点。

## Consequences

- 收益：在皮肤页或键盘里换键盘皮肤后，候选条（含展开面板、读音行、页码）、表情和手写面板跟着变，和 #9924 之前选一个皮肤全套跟着走的表现一致，同时保留浅色、深色各选各的。候选词在候选条里上下居中。
- 代价：候选条、表情和手写面板不能再单独选一套与键盘不同的内置主题；在 Android 上这本来就做不到。桌面平台不受影响，它们没有这两个槽位。
- 测试：`ThemeModeDeviceSmoke` 的阶段改成「keyboard skin choices repaint the candidate strip」，断言换成纸白后候选 1 的配色和跟随系统时不同（原来断言不变）；去掉两个槽位后再输入一次，候选 1 回到跟随系统时的配色，旧偏好的兼容路径因此也在设备上覆盖；浅色基线阶段量候选 1 和外层横向候选条的纵向中心，相差不超过 2 px，以后再把候选行包进别的容器、正则还能过时，这一步会失败。`platforms/android/check-host.sh` 钉住接线：`surfaceSkin` 不再按 `screen_keyboard_theme` 分支、也不直接读 `global_theme`；设计库回调重算候选条、表情和手写；`saveKeyboardSkin` 先补设计库再算候选条；候选行纵向居中。JVM 冒烟读不了 `org.json`，槽位解析本身只能在设备上测。
- 未验证：设备冒烟和截图对比留给之后的设备批次，需要在 Android 手机上依次核对浅色、深色、水杉、纸白、夜墨和一份我的设计下候选条与键盘的配色，冷启动后第一次弹出（槽位为我的设计）的候选条颜色，横屏、九键、候选栏大小 80% 与 130%、读音行或释义行打开时的居中，旧偏好里 `global_theme = custom` 配候选皮肤包时包仍然生效，以及在键盘皮肤面板里从内置主题直接选一款还没用过的我的设计（含刚装的社区皮肤）时，候选条和键盘同时换、中途不闪回旧主题。
