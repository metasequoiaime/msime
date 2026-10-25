# Agent Note: 单按 Caps Lock 切换中英文

Status: implemented

## Problem

#6807：用户想要 Apple 自带输入法那样的手感，轻点 Caps Lock 切换中英文，长按才是大写锁定。此前三个桌面宿主都只把 Caps Lock 当作状态（工具栏和语言栏的大写指示、「大写锁定时使用英文标点」、大写锁定时新起的大写字母交给应用），来源 Windows 产品也一样。macOS 系统的「使用大写锁定键切换 ABC」能给出同样的手感，但切到的是系统的 ABC 输入源，不是本输入法自己的英文模式，切回来还会从 `default_ime_mode` 重新开始。

难点不在判定，在撤回：Caps Lock 的锁定状态由系统在按下时就改掉了，输入法吃掉按键也拦不住。轻点之后必须把大写锁定撤回，否则切到英文却留着大写锁定，比什么都不做更糟。

## Decision

- 共享偏好 `keybindings.switch_language_caps_lock`，serde 缺省和 `Default` 都是关。`HostCapabilities::caps_lock_language_switch` 只在三个桌面宿主上为真，共享设置页「快捷键 › 输入模式切换」在声明了它的宿主上多一行开关。它和切换键下拉框互不排斥：Caps Lock 是另加的一个键，打开它不关掉 Shift。
- 判定规则写在 `shared/input/CapsLockTap.h`，三个宿主共用：开关打开、按下前大写锁定是关的、按下时没有别的键或修饰键、按着期间没有按别的键、按下到松开不超过 `kCapsLockTapThresholdMs`（300 毫秒）才算轻点。大写锁定开着时按 Caps Lock 是关掉它，照常处理，不切换。单按 Shift、Control 的 500 毫秒窗口不变；Caps Lock 取更短的 300 毫秒，因为长按它是在要大写锁定，那一侧应该早点认定。
- 认定为轻点后，宿主先撤回大写锁定，撤回成功才切换；切换走单按 Shift 的同一条路，组字中先上屏原文。撤不回时不切换，大写锁定照常生效。
- macOS：IMK 不保证交付实体松手事件。控制器在大写锁定从关变开时布防，并暂不把这次的大写锁定报告给会话和工具栏；每 10 毫秒从 `CGEventSourceFlagsState(kCGEventSourceStateHIDSystemState)` 的 `NX_ALPHASHIFT_STATELESS_MASK` 查实体键是否还按着；若松手状态事件先到，就使用原来的按下时刻完成判定。键码 0x39 的查询会随逻辑锁定保持为真，不能用于计时；这项决定的翻转及实机缺口见[实体按住状态](../bug-fix/2026-10-21-macos-caps-lock-physical-flags.md)。松开且在分界内就用 IOKit 的 `IOHIDSetModifierLockState(kIOHIDCapsLockState, false)` 撤回；按住超过分界、按着时打字或撤不回，就把大写锁定补报给会话。失焦、换会话等外部重置时按实际锁定状态报告。
- macOS 的轮询定时器注册到主运行循环的 `NSRunLoopCommonModes`：`scheduledTimerWithTimeInterval` 只注册默认模式，事件跟踪或模态面板期间不会执行；等默认模式恢复时，真实时钟可能已经超过 300 毫秒，轻点就被当作长按。普通、事件跟踪和模态三种模式均需执行相同的轻点、长按判定（Refs #8961）。此修复覆盖运行模式造成的漏判，尚不能据此认定报告人的物理按键链路已完整修复。
- macOS 轻点撤回后，窗口服务器可能在紧邻的事件中暂时保留旧的 `CapsLock` 修饰位；控制器在下一次真实按下或收到关闭状态前忽略这份旧位，并把英文模式下受其影响的 ASCII 字母还原为小写，避免工具栏回到 `A` 或连续输入大写。
- Windows：TIP 在 `OnTestKeyDown` 的 Caps Lock 首次按下时布防（`GetKeyState` 这时还是按下前的状态），`_TrackModifierHotkeyArming` 遇到别的键按下时取消；`OnTestKeyUp` 和 `OnKeyUp` 认定后用 `SendInput` 再按一次 Caps Lock 撤回，合成事件带 `CAPS_LOCK_SENDINPUT_EXTRA_INFO`（`0x4D535054`），按键处理和裸 Shift 钩子都放行，然后把切换排进延迟按键队列。松开事件不吃掉。系统设置成只有 Shift 能关掉大写锁定（`KLF_SHIFTLOCK`）时不布防。合成的那一下被 TIP 当作自注入放行，TIP 撤回成功后自己把 `Global::CapsLockEnabled` 记成关；Server 的低级钩子（`MaintenanceHotkey.cpp`）在跳过注入按键之前计入 Caps Lock，因为它记的锁定状态是每按下一次翻转一次，漏掉撤回的那一下，报告给工具栏和语言栏的大写锁定就从此反着，直到 Server 重启。
- Linux：IBus 与 Fcitx5 按 `Caps_Lock` 符号认键，排除被 xkb 选项改成 `Caps_Lock` 的 Shift 键；按下事件带的是按下前的锁定状态，松开时认定。撤回用 libX11 的 `XkbLockModifiers`（`platforms/linux/src/system/CapsLockRelease.cpp`，X11 宏不流进宿主代码），只在 X11 会话里可用；`WAYLAND_DISPLAY` 有值或 `XDG_SESSION_TYPE=wayland` 时不布防。Fcitx5 不把 `Caps_Lock` 算作修饰键，组字时按下它会照普通按键先把首选上屏，所以布防了的按下直接交给应用、不往下走，组字留到松开时由切换上屏原文；IBus 宿主本来就把 Caps Lock 当修饰键放过。
- 原生设置：macOS 设置窗口「按键」页、Windows 原生设置窗口「输入模式切换」组各加一行；Linux 的设置窗口就是共享设置页。同一个开关写在三处设置界面，是[设置界面按平台归属](../architecture/2026-10-15-settings-ui-ownership-by-platform.md)记录的代价之一；macOS 的 AppKit 设置窗口按那篇冻结为回退。

Windows 的 `0x4D535053` 留给[面板文字注入](2026-10-10-windows-clipboard-privacy-panel-statistics-host-diagnostics.md)：面板标记只跳过直通统计，不能进入 `IsSelfGeneratedSendInputExtraInfo` 的整键放行集合。Caps Lock 使用相邻的 `0x4D535054`，避免同值时把面板字符错误地按自注入放行；保留面板已有值也让设置应用与 TIP 的标记契约保持兼容。

### 权限

`IOHIDSetModifierLockState` 经 `IOServiceOpen(kIOHIDSystemClass, kIOHIDParamConnectType)` 调用。在 macOS 27（Darwin 27.0）上用一个独立签名、经 `open` 启动的探针程序实测：`IOHIDCheckAccess` 对输入监控和事件发送都是「未询问」，`AXIsProcessTrusted` 为假，照样能把大写锁定打开再关上，前后不弹授权框，授权状态也不变。所以输入法不需要为它申请输入监控或辅助功能。Caps Lock 的实体状态通过 `CGEventSourceFlagsState` 只读 HID 系统的 stateless 标志，不创建监听器或请求输入权限。`ModifierTap.h` 仍用键码查询校验其他按键是否按住，不用它判定 Caps Lock 的实体松手。

## Alternatives considered

- **更换面板文字标记，保留 Caps Lock 的原值**：同样可以分开两个用途，但面板标记由 Rust 设置应用和 C++ TIP、Server 共享，更新期间不同进程版本可能混用；只更换 TIP 内部使用的 Caps Lock 标记，改动面更小。

- **只在文档里引导用户打开系统的「使用大写锁定键切换 ABC」**：零代码，系统在 HID 层分辨轻点和长按，手感最准，Apple 键盘那种「很快的一下不打开大写锁定」的延迟也由系统处理。但它切换的是输入源而不是本输入法的英文模式，切回来从 `default_ime_mode` 开始、按应用记忆的状态丢失，Windows 和 Linux 也没有对应的系统设置。维护者决定三个桌面都在输入法内做。
- **macOS 上不撤回，直接把 Caps Lock 当作英文模式开关（Rime/Squirrel 的做法）**：不碰 IOKit，也不用轮询。但大写锁定一直开着，英文模式下打出来全是大写，工具栏和系统指示灯也说是大写锁定，与「不留着大写锁定」的要求相反。
- **按下时立即认定、靠下一次 FlagsChanged 判断长按**：macOS 没有 Caps Lock 的松开事件，下一次 FlagsChanged 是用户再按一次，判断不了按住多久。只能轮询按键状态。
- **Linux 在 Wayland 下也切换，留着大写锁定**：至少在 GNOME、KDE 默认的 Wayland 会话里功能「有反应」。但切到英文而大写锁定还开着，中文模式下大写锁定的字母本来就交给应用，用户得到的是全大写的英文；撤不回就不切换是三个平台一致的规则。

## Consequences

- **收益**：三个桌面宿主同一条规则、同一个阈值，切换的是本输入法自己的中英文模式，按应用记忆照常；缺省关，现有用户无变化。
- **代价与已知上限**：
  - 大写锁定会先亮一下再被撤回：macOS 和 Linux 的指示灯、Windows 语言栏的 Caps 图标在轻点时短暂显示大写锁定。macOS 宿主在认定之前不把大写锁定报告给会话，所以[大写锁定时使用英文标点](2026-10-09-caps-lock-ascii-punctuation.md)不会在轻点期间生效；长按认定后照常报告，两个开关可以同时打开。
  - Apple 键盘对很快的一下 Caps Lock 本身不打开大写锁定，系统也就不报事件，这样的一下不切换，需要稍微按实一点。没有在外接非 Apple 键盘上比较过。
  - macOS 打开了系统的「使用大写锁定键切换 ABC」时，轻点先被系统拿去切输入源；设置页说明里提示关掉它。
  - Windows 上不把裸修饰键松开交给 `ITfKeyEventSink` 的宿主（Word、mintty）可能同样收不到 Caps Lock 的松开，那里轻点照常是大写锁定；裸 Shift 的低级钩子没有扩展到 Caps Lock。
  - Windows 打开了「要关闭 Caps Lock：按 SHIFT 键」（`HKCU\Keyboard Layout` 的 `Attributes` 带 `KLF_SHIFTLOCK`）时不布防，轻点照常是大写锁定。那个设置下再按一次 Caps Lock 关不掉大写锁定，注入的那一下也一样，而 `SendInput` 只报告事件进了输入队列，照样返回成功，TIP 会以为撤回了就切到英文、留着大写锁定。改为注入一下 Shift 来撤回能保住功能，但那是一次真实的裸 Shift，应用和别的输入法都会看到；读设置只在开关打开、按下 Caps Lock 时发生。没有在实机上验证这个设置对注入按键的效果，依据是注入按键和真实按键走同一条处理路径。Server 记的大写锁定在这个设置下本来就不准（按 Caps Lock 关不掉、按 Shift 关掉都没有计入），与本功能无关，未改。
  - Linux 只在 X11 会话里有效，构建时没找到 libX11 也不可用。
  - `switch_language_caps_lock` 在 `KeybindingPreferences` 里，结构带 `deny_unknown_fields`，早于它的设置应用或 `msime-mcp` 会拒读写过这个字段的文档；正式包里输入法和设置应用版本一致，和以往加键的处理相同。没有加进账号设置同步（同步只覆盖 Android 那组键，快捷键本来就不在其中）。

## Verification

- `client-core`：`caps_lock_language_switch_defaults_off_and_persists_an_explicit_enable` 覆盖默认关、旧文档缺键、保存读回且不动其他切换键；`host_surface` 断言只有三个桌面宿主声明能力。
- 共享设置页：`input-mode-shortcuts-section.test.tsx` 覆盖没有能力时不显示、缺省关、打开只写这一个键、macOS 与 Linux 的限制说明。
- macOS CTest `shortcut` 的 `TestCapsLockTap` 用合成的时钟、按键状态和撤回函数驱动真实控制器：轻点切换且不报告大写锁定、撤回后的陈旧修饰位不使英文变大写、撤回后系统的「关」不再切换、长按与两次轮询之间的慢松开都保留大写锁定、大写锁定开着时按下只是关掉、按着打字和组合键不切换、撤不回不切换、组字中先上屏原文、开关关掉时照常。原生设置的开关读写也在其中。
- `shortcut-test --caps-lock` 单独运行上述用例，并实际推进默认、事件跟踪和模态运行模式，确认真实定时器调用控制器完成轻点撤回与长按保留；只有时钟、原生 flags 状态查询和撤回函数是合成值，生产的物理按住判定仍参与执行，不改动测试机器的真实 Caps Lock。新增运行循环用例在旧实现的事件跟踪模式下失败，直接手动调用轮询的旧用例无法发现这一缺口。[Apple 的定时器文档](https://developer.apple.com/documentation/Foundation/Timer/scheduledTimer%28withTimeInterval%3Arepeats%3Ablock%3A%29)明确默认注册模式。
- Windows：`crates/host-windows/tests/send_input_marker.rs` 保证面板标记与 C++ 一致且不出现在 TIP 自注入集合中，防止 Caps Lock 再次与面板碰撞。`msime-tsf-caps-lock-tap` 逐条钉住共享判定规则，并检查 TSF 接线（按下前状态、两个松开入口、先撤回后切换、撤回后记成关、`KLF_SHIFTLOCK` 时不布防、合成事件标记、Server 钩子在跳过注入按键之前计入 Caps Lock、偏好键与缺省值），本机 clang 运行通过；`windows-maintenance-hotkeys` 覆盖 `CapsLockTracker`：真实的一下加注入的一下之后报告关，之后的长按照常；`KeyEventSink.cpp` 用 MinGW 做过语法检查。WinUI 3 的原生设置窗口只能交给 Windows CI 编译。
- Linux：`linux-caps-lock-tap` 覆盖判定规则和 X11/Wayland 会话判断。宿主层面 `CapsLockRelease.h` 有一个只给测试用的替换入口，`ibus-engine-smoke --caps-lock` 和 `fcitx5-native-test --caps-lock`（ctest 里的 `ibus-caps-lock-tap`、`fcitx5-caps-lock-tap`，需要已校验的词库）从真实的按键处理路径驱动：缺省关时不切换、偏好热重载打开与关闭、轻点切到英文并从英文透传切回、长按、大写锁定开着时按下、按着打字、和 Shift 一起按、撤不回不切换、组字中上屏原文；Fcitx5 还覆盖组字中长按不上屏。在 `build-container.sh` 的镜像里编译两个宿主、跑 ctest，并对着本机已校验的词库手动跑了这两个模式和两个宿主的其他模式。
- 没有在任何平台上实机按 Caps Lock 验证：macOS 只验证了 IOKit 调用在无授权进程里可用，Windows 与 Linux 没有实机。

[可编辑焦点限制](../feature/2026-10-24-macos-editable-shift.md) 只约束可选的单按 Shift，不改变本文的快捷键或状态归属规则。
