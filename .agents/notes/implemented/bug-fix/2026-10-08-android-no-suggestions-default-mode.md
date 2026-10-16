# Agent Note: Android 只要求不给建议的输入框不再强制进英文

Status: implemented

## Problem

#5998：Android 0.3.1 每次点开输入框都是英文，设置页的「默认中英文」改成中文也没用。

`EditorPolicy.prefersLatin` 把带 `TYPE_TEXT_FLAG_NO_SUGGESTIONS` 的文本框和网址、邮箱、密码框归为一类。`onStartInput` 先按 `default_ime_mode` 和 `InputModeStore` 的记忆算出模式，再交给 `KeyboardInputContext.englishOverride`；`prefersLatin` 为真时，每个新文档都被覆盖成英文。用户在框里手动切回中文，下一次进框又被覆盖，于是默认值和按应用记忆对这类输入框都不起作用。

这个标志在 Android 上很常见：React Native 的 `autoCorrect={false}`、Flutter 的 `enableSuggestions: false`、Chrome 地址栏和不少网页输入框都带着它，覆盖到的是大量聊天框和搜索框，不是少数验证码框。0.3.1 之前这类输入框干脆不走引擎；9abfa29f7 让它们走引擎，组字恢复了，但同时沿用了 `prefersLatin` 里这一条，于是变成「能打中文，但每次都得先切」。#5933 里 Via 浏览器「默认英文的输入框」也是这一类。

2026-10-16 的完整 Native Platform CI（run `38854961285`）在英文纠错的无建议输入框验收失败。门控已经禁止纠错，但换框时 `onStartInput` 又把 `runtime-options.json` 的旧 `default_ime_mode`、`ime_mode_scope` 应用了一遍：实时偏好为全局记忆、旧副本为按应用记忆时，已记住的英文被错误的应用记忆覆盖，原始拼写在空格处按中文候选上屏。本机设备复现原始七个字母在边界变成三字文本；这与显式大小写纠错算法无关。

## Decision

- `prefersLatin` 只认 `TYPE_TEXT_VARIATION_URI`、两种邮箱 variation 和三种密码 variation，不再看 `TYPE_TEXT_FLAG_NO_SUGGESTIONS`。只带这个标志的输入框按 `default_ime_mode` 和 `ime_mode_scope` 的记忆进框。
- Chrome 地址栏的 `inputType` 是 `0x80011`（URI 加无建议），仍因 URI 进英文，这次不动。
- 输入模式的默认值与记忆范围一旦读到实时偏好，就保持实时值；换输入框再次应用旧运行时副本不能覆盖它们。首次还没读到实时值时允许副本兜底；下一次真正的实时更新仍然生效。模式本身继续经 `InputModeStore` 与 `KeyboardInputContext` 按既有规则解析。
- `tests/core/EditorSmoke.java` 断言只带无建议标志的文本框不进英文、`0x80011` 仍进英文；旧实现下这条断言失败。

## Alternatives considered

- **每次进框在主线程同步读取模式偏好** — 能直接使用存储里的最新值，避免任何副本判断；但会把存储调用带进系统焦点回调，并与既有工作线程的偏好读取重复。本次只维护已读实时值的优先级，不新增文件读写，冷启动未读实时值时仍有原副本兜底。

- **在英文纠错验收里每次换框都点英文键** — 能让测试直接进入待测纠错模式，隔离模式记忆的影响；但产品已经按全局记住英文，普通无建议输入框仍应保留该选择。补点会掩盖真实的模式回退，保留模式与原文断言，修复偏好来源优先级。

- **保留这条规则，只让框里的手动切换写进记忆并压过覆盖** — 对真想要英文的无建议字段（比如手填代码、序列号的框）仍然起手英文，改动也只在模式记忆这一层。但用户在每个应用、每种这样的输入框里都得先切一次，「默认中英文」依旧管不到它们；而且这会把「字段要求的临时英文」和「用户的选择」混进同一份记忆，README 明确说临时覆盖不写入记忆。
- **把无建议字段改回不走引擎** — 9abfa29f7 之前就是这样，结果是 Chrome 地址栏和这些聊天框只能打英文字母、切不到中文，比现在更糟。
- **改用 `IME_FLAG_FORCE_ASCII` 判断** — 这是 Android 上与 iOS `.asciiCapable` 对应、真正表示「要 ASCII」的信号，可以补进 `prefersLatin`。但它在 `imeOptions` 里，要改 `prefersLatin` 的签名和调用处，而且修这个 issue 并不需要它，这次不做。

## Consequences

- 实时模式偏好的来源标记跟随服务对象存活，结束某个输入会话不会清掉它；新服务首次读取仍按原有副本与异步实时读取顺序运行。已读到实时值的会话切换更稳定，不增加主线程存储调用。

- **收益**：聊天框、搜索框和网页输入框按用户设置的默认模式和按应用记忆进框，「默认中英文」在 Android 上对这些框重新生效。
- **代价**：只带无建议标志、实际想要英文的输入框现在按默认模式（通常是中文）进框，需要用户自己切；切过之后按 `ime_mode_scope` 记住，同一应用的其他输入框也会跟着用这个模式。
- **未改**：HarmonyOS 的 `EditorPolicy.prefersLatin` 也把 `noSuggestions` 算进去，但那边的 `noSuggestions` 只对应 `PATTERN_ONE_TIME_CODE`，范围本来就窄，不受这个问题影响。URI 字段（包括 Chrome 地址栏）是否也该改为按默认模式进框，是另一个问题，这次不动。
- **覆盖边界**：最初 #5998 仅有 JVM 冒烟。此次追加的是专用 API 35 AVD 的真实编辑器验收，不代表已在 Chrome、Via 或其他真实应用与真机上验证。

## Verification

- 完整设备 CI 的表情与组合表情套件已通过，失败位于后续 `EnglishCorrectionDeviceSmoke` 的 `correction-no-suggestions`。本地原宿主复现同一失败；在合成编辑器中追加边界前断言确认原始拼写完整，边界后文本长度变成三字，未误判为 `language` 的纠错结果。
- API 35 的旧宿主在补充的模式断言处失败；从本分支重新编译 Java 宿主后，同一套英文纠错与编辑器门控验收完整通过。原生库与词库暂存沿用已合入 `develop` 的 `508c8d9cce9a98f60e400d36c52849de36145f20` 构建产物，本轮没有重建 Rust 原生库。
- 英文纠错设备验收补充换框后仍显示英文模式、边界前保留原始拼写的断言；网址、邮箱、密码的禁用纠错覆盖继续保留。
- 与[移动端英文纠错](../feature/2026-10-16-mobile-english-correction.md)部分重叠：继续由共享算法提供纠错，宿主模式与编辑器门控在设备上验收；不改变大小写算法。
