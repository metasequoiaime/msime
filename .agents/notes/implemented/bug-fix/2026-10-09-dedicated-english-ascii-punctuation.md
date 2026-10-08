# Agent Note: 专用英文模式按标点输出半角英文标点

Status: implemented

## Problem

在手机键盘的英文模式（引擎的专用英文模式，`dedicated_english`）下按 `.`，上屏的是中文句号「。」：打 `si` 再按 `.` 得到 `site。`。逗号、问号同理。

「固定标点」（`punctuation_lock`）的默认值是「跟随中英文状态」，用户切到英文就是要英文标点。引擎处理字符键时也已经这样做了：`handle_character` 在专用英文模式下吞掉所有非字母键，注释写明是为了不让英文模式往宿主漏中文标点。

可标点键走的是 `handle_punctuation`。那里「不用中文标点的方案（韩语）写半角标点」这条规则特意用 `!self.dedicated_english` 把专用英文排除在外，于是专用英文落到中文标点那一段，跟着中文标点开关译成了「。」。

Android（`MSIMEInputService.punctuation`）和鸿蒙（`KeyboardSession.punctuation`）在英文模式下都把标点交给引擎的 `punctuationWithContext`，所以问题出在引擎，两个平台是同一个。iOS 的会话桥也有 `msime_client_punctuation_with_context`，它在英文模式下是否同样调用，没有逐个调用点核对过。

## Decision

`handle_punctuation` 在专用英文模式下走半角那一段：
- 有组字时结束组字，接上英文标点，一次提交；
- 没有组字时返回未处理，由宿主插入按键本身。

例外是「固定标点」锁成「始终使用中文标点」（lock 1）：此时照中文标点开关走，那是用户明确要的。「始终使用英文标点」（lock 2）本来就是半角。中文模式不受影响。

宿主侧不用改：引擎不处理时宿主自己插入按键（Android 的 `commitText`、鸿蒙 `punctuation()` 的字面插入）；「重复标点转中文」的启用条件已经排除了专用英文模式，所以英文下连按两次 `.` 不会再被换成中文。

## Alternatives considered

- **由宿主在英文模式下不把标点交给引擎，自己插入 ASCII。** 最强的理由是改动只在宿主，引擎行为不变。不用它，是因为三个手机宿主都要各改一遍，而且组字中按标点时，还得由宿主先让引擎结束组字再插入，顺序容易写错；引擎里本来就有一段同样语义的代码（韩语），只是被错误地排除了专用英文。
- **在英文模式切换时，由宿主把 `chinese_punctuation` 关掉。** 这就是桌面端「跟随中英文状态」的做法，Windows 和 macOS 按应用切换标点状态。不用它，是因为这会改写用户的偏好或者需要另一份运行时状态，而专用英文是引擎自己的模式，引擎知道自己处在其中，在引擎里判定最直接。

## Consequences

- **收益**：手机英文模式下的标点与「跟随中英文状态」的含义一致，不再出现 `site。`；字符键和标点键两条路对英文模式的处理一致。
- **代价**：用户如果在英文模式下想要中文标点，需要把「固定标点」设成「始终使用中文标点」。以前不用设置就能得到中文标点，这是一次行为变化。

## Verification

- `crates/engine/src/session/tests.rs` 的 `dedicated_english_writes_ascii_punctuation_unless_locked_to_chinese` 覆盖：组字中、空闲、lock 1、lock 2、回到中文模式。
- `cargo test -p msime-engine -p msime-input-runtime -p msime-host-api` 全部通过。
- 鸿蒙模拟器上的验证见 PR。
