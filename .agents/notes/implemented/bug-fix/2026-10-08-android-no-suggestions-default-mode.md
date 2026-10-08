# Agent Note: Android 只要求不给建议的输入框不再强制进英文

Status: implemented

## Problem

#5998：Android 0.3.1 每次点开输入框都是英文，设置页的「默认中英文」改成中文也没用。

`EditorPolicy.prefersLatin` 把带 `TYPE_TEXT_FLAG_NO_SUGGESTIONS` 的文本框和网址、邮箱、密码框归为一类。`onStartInput` 先按 `default_ime_mode` 和 `InputModeStore` 的记忆算出模式，再交给 `KeyboardInputContext.englishOverride`；`prefersLatin` 为真时，每个新文档都被覆盖成英文。用户在框里手动切回中文，下一次进框又被覆盖，于是默认值和按应用记忆对这类输入框都不起作用。

这个标志在 Android 上很常见：React Native 的 `autoCorrect={false}`、Flutter 的 `enableSuggestions: false`、Chrome 地址栏和不少网页输入框都带着它，覆盖到的是大量聊天框和搜索框，不是少数验证码框。0.3.1 之前这类输入框干脆不走引擎；9abfa29f7 让它们走引擎，组字恢复了，但同时沿用了 `prefersLatin` 里这一条，于是变成「能打中文，但每次都得先切」。#5933 里 Via 浏览器「默认英文的输入框」也是这一类。

## Decision

- `prefersLatin` 只认 `TYPE_TEXT_VARIATION_URI`、两种邮箱 variation 和三种密码 variation，不再看 `TYPE_TEXT_FLAG_NO_SUGGESTIONS`。只带这个标志的输入框按 `default_ime_mode` 和 `ime_mode_scope` 的记忆进框。
- Chrome 地址栏的 `inputType` 是 `0x80011`（URI 加无建议），仍因 URI 进英文，这次不动。
- `tests/core/EditorSmoke.java` 断言只带无建议标志的文本框不进英文、`0x80011` 仍进英文；旧实现下这条断言失败。

## Alternatives considered

- **保留这条规则，只让框里的手动切换写进记忆并压过覆盖** — 对真想要英文的无建议字段（比如手填代码、序列号的框）仍然起手英文，改动也只在模式记忆这一层。但用户在每个应用、每种这样的输入框里都得先切一次，「默认中英文」依旧管不到它们；而且这会把「字段要求的临时英文」和「用户的选择」混进同一份记忆，README 明确说临时覆盖不写入记忆。
- **把无建议字段改回不走引擎** — 9abfa29f7 之前就是这样，结果是 Chrome 地址栏和这些聊天框只能打英文字母、切不到中文，比现在更糟。
- **改用 `IME_FLAG_FORCE_ASCII` 判断** — 这是 Android 上与 iOS `.asciiCapable` 对应、真正表示「要 ASCII」的信号，可以补进 `prefersLatin`。但它在 `imeOptions` 里，要改 `prefersLatin` 的签名和调用处，而且修这个 issue 并不需要它，这次不做。

## Consequences

- **收益**：聊天框、搜索框和网页输入框按用户设置的默认模式和按应用记忆进框，「默认中英文」在 Android 上对这些框重新生效。
- **代价**：只带无建议标志、实际想要英文的输入框现在按默认模式（通常是中文）进框，需要用户自己切；切过之后按 `ime_mode_scope` 记住，同一应用的其他输入框也会跟着用这个模式。
- **未改**：HarmonyOS 的 `EditorPolicy.prefersLatin` 也把 `noSuggestions` 算进去，但那边的 `noSuggestions` 只对应 `PATTERN_ONE_TIME_CODE`，范围本来就窄，不受这个问题影响。URI 字段（包括 Chrome 地址栏）是否也该改为按默认模式进框，是另一个问题，这次不动。
- **未覆盖**：只有 `check-host.sh` 的 JVM 冒烟，没有在真机或 AVD 上对具体应用复现验证。
