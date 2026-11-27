# Agent Note: macOS 长按左 Option 打开表情与符号面板

Status: implemented

## Problem

#11859 希望在 macOS 上长按 Option 调出标点符号，报告者后续明确要左 Option。全局拦截修饰键会影响所有应用的 Option 字符层和快捷键，所以先要定下阈值、透传、吞键范围、失焦和安全输入的语义。维护者在 [#11859 的决定](https://github.com/metasequoiaime/msime/issues/11859#issuecomment-7215080948) 里定了：可选设置、默认关闭；按住约 400 ms 触发，只认左 Option，右 Option 不变；短按或与其他键、修饰键组合时照常传给应用；面板真正打开后才吞掉松开事件；安全输入期间不生效；有组字时先上屏再打开面板。

## Decision

共享偏好新增 `keybindings.symbol_panel_left_option_hold`（`crates/client-core/src/preferences.rs` 的 `KeybindingPreferences`），出厂和缺字段都读成关闭，目前只有 macOS 宿主经 `MSIMEAppearancePreferences.symbolPanelLeftOptionHold` 读它。

手势判定是纯状态机 `platforms/macos/src/core/LeftOptionHold.h`（`MSIMELeftOptionHold`），不读时钟、不碰界面，宿主喂给它三样东西：

- `observe(事件类型, keyCode, modifierFlags, 时间戳, enabled)`：左 Option（keyCode 58）单独按下时开始计时，按下事件本身从不吞；计时期间任何 keyDown、keyUp 或别的修饰键变化（Shift、Control、Command、Fn、右 Option）都把这次按住判为组合键；带着别的修饰键按下从一开始就是组合键。只有面板由这次按住打开之后，左 Option 的松开返回「吞掉」。`enabled` 为假（开关关、安全输入中）时整个状态作废，事件全部交给应用。
- `due(now, 会话修饰键状态)`：到期时再用 `CGEventSourceFlagsState` 确认左 Option 还单独按着，松开事件丢失或别的修饰键的按下没送到时不会误开。
- `didOpen(成功与否)`：面板没打开时，这次按住的松开照常交给应用。

`MSIMEInputController` 在 `handleKeyEvent:` 里、焦点接管之后、语音按住快捷键之前调用 `observeLeftOptionHold:`，按事件时间戳加 400 ms 安排一次性计时器；计时期间挂本地和全局鼠标按下、滚轮监视器，Option+点按、拖动会取消这次按住。失焦、换客户端、`deactivateServer:` 和 `dealloc` 都重置。

到期后宿主调用 `presentEmojiPanelCommittingComposition:YES`：先确认面板有可输入的前台客户端（`sharedTextToolApplication`，与菜单打开表情面板同一条件），再 `MSIME_FINISH_COMPOSITION` 上屏组字，最后走菜单「水杉表情面板…」的同一路径打开共享的「表情与符号」面板；Swift 桥不在时退回系统字符检视器。`showSharedTextTool:` 因此改为返回是否开始打开，供手势判断要不要吞掉松开。

右 Option 的「按住说话」（`VoiceHoldShortcut.h`）只处理 keyCode 61，与本手势互不相交。

界面按 ARCHITECTURE.md 的「macOS 的 AppKit 设置窗口冻结为回退」：冻结的 AppKit 回退窗口和 `packages/ui` 都不加控件，开关随 macOS 原生设置迁移接入；字段在 `scripts/test-preferences-field-parity.py` 的 `RUST_ONLY` 里登记了原因。迁移前要用就直接编辑偏好文档打开。

## Alternatives considered

- **任意 Option 长按都打开面板**：实现更简单，但右 Option 是语音按住说话的默认键，也是很多布局的字符层，超出报告者要的左 Option，放弃。
- **按下就吞掉左 Option，超时不触发再补发**：可以保证面板打开后应用完全看不到这次 Option，但补发的修饰键会打乱应用对 Option 字符层和快捷键的判断，短按也会延迟，放弃；改为按下照常透传、只吞打开后的松开。
- **复用语音按住手势 `MSIMEVoiceHoldShortcut`**：已经处理了侧别位和丢失松开，但它没有时长阈值，且按住与录音生命周期绑在一起，混进去会让两套设置互相牵连，放弃；侧别位补全和会话状态轮询的做法照搬了过来。
- **直接打开面板的「符号」页**：更贴近「标点符号」的诉求，但要给共享面板路由和 React 面板加入口，迁移期 `packages/ui` 只修缺陷，放弃；先打开整个表情与符号面板。

## Consequences

默认设置下行为与改动前一致。打开开关后，按住左 Option 约 400 ms 会先上屏组字再打开表情与符号面板；面板按它自己的规则选初始页，不会直接跳到「符号」页。Option 字符输入、Option 快捷键和 Option+鼠标操作在 400 ms 内完成时不受影响；按住左 Option 超过 400 ms 再点按鼠标的操作（例如先按住 Option 再移鼠标去点），面板会先打开，这是阈值手势本身的代价，所以默认关闭。按下事件已经交给应用，面板打开后只吞得到送回输入法的那次松开；面板激活后松开送到面板进程时由系统处理。

验证：`platforms/macos/tests/input/LeftOptionHoldTest.mm`（`left-option-hold` CTest）覆盖到期打开与吞松开、短按、面板未打开、按住期间按键或加修饰键、带修饰键按下、右 Option、只有通用位的事件、大写锁定、重复快照、丢失松开、鼠标取消、打开后松开丢失的下一次按下、开关关闭与安全输入、reset；`ShortcutTest.mm` 的 `TestLeftOptionHoldOpensSymbolPanel` 走真实 `handleEvent:` 路由覆盖默认关闭、先上屏再开面板并吞松开、短按、Option+字母、Command+Option、右 Option、安全输入和到期时左 Option 已抬起；`crates/client-core/tests/symbol_panel_left_option_hold.rs` 覆盖默认值和持久化往返。没有做真实编辑器里的签名构建验收，macOS 的设置界面入口留给原生设置迁移。
