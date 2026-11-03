# Agent Note: iOS 按键按下即反馈

Status: implemented

## Problem

#10768：iPhone SE 3（iOS 17.7，0.53.0）开着振动打字，按下去明显觉得有延迟，系统键盘则是按下就有反应；快速连按时觉得「同时按下去两个键」。

原因是 iOS 键盘的按键音和振动都在松手时才发出。键盘按键是 `UIButton(primaryAction:)`，动作挂在 `.primaryActionTriggered`，也就是松手时在 `KeyboardKeyButton.touchesEnded` 里同步执行；按键音和振动都在动作里的 `playInputClick` 中发出。按下时挂的 `prepareKeyFeedback` 只预热振动器，不振。退格也一样：按下只起连删计时器，松手时 `finishBackspacePress` 才删字、出声。用户要等抬手才感觉到那一下，所以觉得按下去慢；快速连按时两次振动挤在两次抬手上，和按下的键对不上。

[开了按键振动后打字不跟手](2026-10-27-mobile-haptics-off-ui-thread.md)那一次在 iOS 上只减少了按键路径上读振动偏好的开销，没有改振动的时机，所以 0.53.0 照样能复现。Android 的同一问题是 #10757，单独修。

## Decision

- 键盘按键在按下的那一刻反馈。造按键的两处 `makeKey(…primaryAction:)` 和 `makeIconKey` 在 `.touchDown` 上挂 `keyPressFeedback(_:)`，它播按键音、振动，并在按键上记下这次按下已经反馈过（`KeyboardKeyButton.markPressFedBack`）。空格、退格、回车按自己的声音类别出声（`KeyboardKeyButton.pressSoundClass`，造这几个键时设好）。
- 松手不重复。`KeyboardKeyButton.touchesEnded` 整段放在 `KeyboardKeyButton.releasing(self)` 里执行：这次按下反馈过时，范围里的 `playInputClick` 不出声，并取走标记。点按、角标、九键滑动输入数字都在 `touchesEnded` 里同步上屏，都在这个范围内。手势接管了按键触摸、在手势结束时才上屏的两处也包进同一个范围：假名滑动（`JapaneseNineKeyView` 的 `KanaFlickGesture` 结束时）和 18 键横滑（`handleEighteenKeySwipe` 的 `.ended`）。
- 标记跟着按键的这一次按下，不用全局计数器。范围只在同步执行期间有效，结束就还原。没有经过按下的激活（`sendActions`、读屏激活）和候选、面板、工具栏的按钮，在范围外、或范围里的标记为假，照常在动作里出声。被手势取消的触摸不清标记，因为假名滑动和 18 键横滑在取消之后才结束；下一次按下会重新记，所以留下的标记只可能被同一个键的松手取走。键离开窗口时清掉标记。
- `prepareKeyFeedback` 不改，`decorateKey` 也不挂新的反馈。候选词块、`closePredictionButton`、`hanjaButton` 都挂着 `prepareKeyFeedback`；换皮肤时 `applyKeyboardSkin` 又会对所有带底色的按钮调一次 `decorateKey`。把反馈放进这两处，候选栏一开始滑动就会振。
- 在滚得动的容器里的键（`KeyboardKeyButton.sitsInScrollableContent`：祖先里有开着滚动、内容比可见区域大的 `UIScrollView`），例如九键左列的自定义符号表和九键面板的拼音栏，按下可能是在开始滚动，所以仍由松手时的动作反馈，滑一下列表不振。放得下、滚不动的列按下即反馈。
- 长按生效时照旧各有一下反馈：弹出九键或 14 键的长按菜单（包括九键反方向滑动弹出）、长按空格开语音、长按 123 开符号面板。它们原来就是这一次触摸唯一的反馈，现在按下时多了一下。按住退格连删时，每删一下也照旧响一下。
- 滑行输入的起笔按下现在会有一次反馈，和系统键盘一样，整笔不再有别的反馈。[iOS 键盘的滑行输入宿主](../feature/2026-10-07-ios-glide-typing-host.md)里「滑行不另加声音或振动」那条的依据（按键反馈在抬手的 primary action 里）不再成立，已同步改成现在的事实。
- 出厂默认不动：振动开关、档位和按键音都不改。

## Alternatives considered

- **让 `prepareKeyFeedback` 直接振**：改动最小，所有已经挂着它的按钮都会按下即振。但候选词块和候选栏的几个按钮也挂着它，候选栏一开始横滑就振、长按菜单也振；换皮肤时 `decorateKey` 还会把它挂到更多按钮上。
- **全局计数器**：按下时加一，`playInputClick` 见到计数就减一并跳过。不用改按键类。但手指滑出按键后松手不执行动作，计数留下来，会吞掉下一次候选点击的反馈；多指交替时也分不清是哪个键的按下。
- **给 `playInputClick` 加参数，由每个调用方传入触发它的按键**：最明确。但它有五十多处调用，多数在 `handleCharacter`、`handleSymbol` 这类既被按键、又被候选和面板调用的函数里，调用方并不知道自己是不是被按键触发的；按键类自己知道「我正在松手」，在那里划出范围更少出错。
- **按下时只振、按键音仍在松手时播**：改动更小。但系统键盘的声音和振动都在按下时，分开反而更怪。

## Consequences

- **收益**：开着振动或按键音时，按下即有反馈，和系统键盘一致；一次按键仍只响一下、振一下。候选、面板、读屏激活的反馈不变。
- **代价**：原来动作里不出声的几种按键现在按下也会响一下，例如手写板有笔迹时按退格撤销一笔、`toggleInputMode` 不记住选择的分支、Shift 不出声的分支，以及只在 `.allTouchEvents` 上处理的地球键。这些都和系统键盘一样。滑出按键后松手不输入时，按下那一下照样振过。
- **未验证**：本机和 CI 都没有在真机上确认。2026-10-27 那篇已经指出 `impactOccurred` 在主线程上占多久没测过；现在它挪到按下时，排在按下高亮绘制之前。真机批次要分别量开、关振动时从按下到高亮的延迟，并用 os_signpost 或 Instruments 量 `impactOccurred` 的主线程耗时。如果开着振动仍明显更慢，就另立 issue 处理这部分开销。
- **三端**：Android 的同一问题走 #10757。HarmonyOS 的 `KeyboardSession.tapFeedback` 也是在按键动作里调用的，触发时机需要另立 issue 核对。

## Verification

- `platforms/ios/KeyboardTests/input/KeyPressFeedbackTests.swift`（`MSIMEKeyboardTests`）：一次按下只让一次松手静音，松手取走标记；没有按下的激活、不是按键的按钮、嵌套范围都照常出声；滚得动的判定；26 键的字母、Shift、空格、退格、回车都挂了 `keyPressFeedback:`，空格、退格、回车的声音类别正确。
- 本次只在 Mac Studio 上用 `rbuild` 编译了测试目标（`build-for-testing`），没有在模拟器上运行，因为当时设备和模拟器被另一个工作流占用。真机核对清单写在 PR 的「验证」一节。
