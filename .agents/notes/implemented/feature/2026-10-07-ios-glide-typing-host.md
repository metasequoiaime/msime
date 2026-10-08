# Agent Note: iOS 键盘的滑行输入宿主

Status: implemented

## Problem

#5347 要在全拼 26 键上支持滑行输入：手指不停地划过字母键，抬手后输入经过的拼音。Engine 和 C ABI（`msime_client_glide`）已经负责把一笔滑行解码成全拼字母，但「这一下是轻点还是滑行」只有宿主能判断，而 iOS 键盘的每个字母键是独立的 `UIButton`：手指从 Q 拖到 W 再抬起，UIKit 仍会在 Q 上触发 `touchUpInside`（手指拖出按钮不远时仍算在按钮里），所以不先把原来那个键的触摸收回来，滑行就会多打出第一个字母。

## Decision

- 开关「滑行输入」在 App 的「键盘」页新增的「手势」一节，存 App Group 的 `keyboard.gesture.glide`（`KeyboardLayoutPreference.glideTyping`），默认关；键盘每次出现时读一次。只在本机，不进共享文档，也不随设置同步。
- 判定与请求的纯逻辑在 `KeyboardExtension/Sources/input/GlideTyping.swift`，不依赖 UIKit：手指在另一个字母键上、且横向离开按下点至少 0.4 个键宽才开始滑行；请求的 `keys` 是 a..z 的键中心，`key_width`/`key_height` 取二十六个键里最小的宽和高，`points` 均匀抽到至多 1024 个、首尾保留，坐标一位小数、时间取整毫秒。
- `GlideTypingGestureRecognizer`（`KeyboardExtension/Sources/keyboard/GlideTypingGesture.swift`）挂在键区根视图 `KeyAreaStackView` 上，判定为滑行之前一直停在 `.possible`，`delaysTouchesBegan`/`delaysTouchesEnded` 都关掉，轻点的时序不变；判定成立时进入 `.began`，`cancelsTouchesInView` 让 UIKit 取消原来那个键的触摸，它不上屏、高亮和放大预览一起消失。采样用 `coalescedTouches`。
- 滑行开始前第二根手指落下，手势失败，两根手指照常打字；滑行进行中 `KeyAreaStackView.suppressesKeyHits` 让新落下的手指命中键区自己而不是键，手势忽略它们。手势不经抬手或系统取消就被重置时补一次取消，键区不会一直挡着手指。
- 只在开关打开、显示全拼 26 键字母层、中文模式、没有本地输入模式、Shift 没有把下一个字母变成辅助码时开始滑行。抬手后经 `MetasequoiaInputSessionBridge.glide` 交给 Engine，回应和轻点字母键一样渲染；`handled` 为 false 时整笔丢掉。系统取消时什么都不发送。
- 轨迹 `GlideTrailView` 是盖在键区上的 `CAShapeLayer`：键盘强调色、4pt、经过中点的二次曲线，抬手 0.2 秒淡出，被打断时直接清空。滑行不另加声音或振动：iOS 的按键反馈在抬手的 primary action 里，按下只预热振动器，滑行取消了那次 action，所以整笔没有额外反馈。

## Alternatives considered

- **在每个字母键上各挂一个 pan 手势** — 和空格键的光标拖动、假名键的 flick 同一种写法，手势只看自己那个键，互不干扰；但滑行一开始手指就离开了按下的键，要跨键追踪，而且「滑行中忽略其他手指」需要一个看得到整个键区的地方，26 个手势之间还得互相协调。
- **重写 `KeyAreaStackView` 的 `touchesBegan/Moved/Ended` 自己追踪** — 不用引入手势；但触摸命中的是键而不是键区，键区收不到这些事件，要截获就得在 `hitTest` 里把所有触摸都交给键区，再自己转发给键，轻点、高亮、放大预览和空格拖动都要重做一遍。
- **开关写进共享文档随设置同步** — Android 的手势设置走 `settings_sync.rs` 的本机设置清单；那份清单在 Rust 里，这次不改 crates，所以 iOS 先和横屏分离式键盘一样只存 App Group。

## Consequences

- **收益**：轻点路径不变（手势在判定前不延迟、不取消任何触摸）；判定规则和请求格式有不依赖 UIKit 的单元测试，键盘接线有在真实 `KeyboardViewController` 上的测试，会话测试经真实词库确认一笔 `nihao` 变成全拼组字、五笔下不处理。
- **代价与已知上限**：开关不随设置同步，也不在 Tauri 共享设置页里出现；要同步时在 `settings_sync.rs` 的本机设置清单加一项并改成写共享文档。判定只看「另一个字母键 + 0.4 键宽」，从键距起滑或在键的边缘来回抖动不会触发；真机上的手感（阈值、轨迹宽度）没有在设备上验收过。

## Verification

`platforms/ios/KeyboardTests/input/GlideTypingTests.swift`（`MSIMEKeyboardTests`，CI 的 `iOS Simulator` job 经 `run_ui_tests.sh` 运行）。本地：`xcodebuild -project build/ios/MSIMEClient.xcodeproj -scheme MSIMEClientTests -only-testing:MSIMEKeyboardTests/GlideTypingTests test-without-building`，需要先 `bash platforms/ios/build-native.sh simulator` 并暂存 `target/ios/EngineResources`。
