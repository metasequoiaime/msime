# Agent Note: 专用英文模式按标点输出半角英文标点

Status: implemented

## Problem

在手机键盘的英文模式（引擎的专用英文模式，`dedicated_english`）下按 `.`，上屏的是中文句号「。」：打 `si` 再按 `.` 得到 `site。`。逗号、问号同理。

「固定标点」（`punctuation_lock`）的默认值是「跟随中英文状态」，用户切到英文就是要英文标点。引擎处理字符键时也已经这样做了：`handle_character` 在专用英文模式下吞掉所有非字母键，注释写明是为了不让英文模式往宿主漏中文标点。

可标点键走的是 `handle_punctuation`。那里「不用中文标点的方案（韩语）写半角标点」这条规则特意用 `!self.dedicated_english` 把专用英文排除在外，于是专用英文落到中文标点那一段，跟着中文标点开关译成了「。」。

字符入口在光标编辑时也绕过了专用英文的非字母保护：`dispatch_character` 先走 `insert_at_caret`，拒绝分支返回未处理。runtime 随后把数字当候选键，例如合成词 `synthetic` 中左移光标后按字符键 `1` 会提前提交整词；字符入口的标点同样可能被转交标点回退并结束单词。

Android（`MSIMEInputService.punctuation`）和鸿蒙（`KeyboardSession.punctuation`）在英文模式下都把标点交给引擎的 `punctuationWithContext`，所以问题出在引擎，两个平台是同一个。iOS 的会话桥也有 `msime_client_punctuation_with_context`，它在英文模式下是否同样调用，没有逐个调用点核对过。

## Decision

`handle_punctuation` 在专用英文模式下走半角那一段：
- 有组字时结束组字，接上英文标点，一次提交；
- 没有组字时返回未处理，由宿主插入按键本身。

例外是「固定标点」锁成「始终使用中文标点」（lock 1）：此时照中文标点开关走，那是用户明确要的。「始终使用英文标点」（lock 2）本来就是半角。中文模式不受影响。

宿主侧不用改：引擎不处理时宿主自己插入按键（Android 的 `commitText`、鸿蒙 `punctuation()` 的字面插入）；「重复标点转中文」的启用条件已经排除了专用英文模式，所以英文下连按两次 `.` 不会再被换成中文。

`insert_at_caret` 对专用英文不接受的字符返回已处理、无提交，与词尾和空闲时的字符入口一致；普通字母仍插到光标处。显式 `punctuation`、选词与命令入口继续各自处理标点、空格选择和提交，不能把字符路由的拒绝误读成选词或结束单词。其他模式的光标拒绝策略保持各自既有契约。

## Alternatives considered

- **由宿主在英文模式下不把标点交给引擎，自己插入 ASCII。** 最强的理由是改动只在宿主，引擎行为不变。不用它，是因为三个手机宿主都要各改一遍，而且组字中按标点时，还得由宿主先让引擎结束组字再插入，顺序容易写错；引擎里本来就有一段同样语义的代码（韩语），只是被错误地排除了专用英文。
- **在英文模式切换时，由宿主把 `chinese_punctuation` 关掉。** 这就是桌面端「跟随中英文状态」的做法，Windows 和 macOS 按应用切换标点状态。不用它，是因为这会改写用户的偏好或者需要另一份运行时状态，而专用英文是引擎自己的模式，引擎知道自己处在其中，在引擎里判定最直接。
- **只在 runtime 阻止专用英文数字选词。** 能直接挡住提前上屏，但字符入口的标点仍会进入回退，而且直接调用引擎的使用方仍收到错误的未处理状态；由共享引擎保持字符入口契约更完整。

## Consequences

- **收益**：手机英文模式下的标点与「跟随中英文状态」的含义一致，不再出现 `site。`；字符键和标点键两条路对英文模式的处理一致。
- **代价**：用户如果在英文模式下想要中文标点，需要把「固定标点」设成「始终使用中文标点」。以前不用设置就能得到中文标点，这是一次行为变化。
- **光标编辑**：专用英文字符入口的非字母不再因光标在中间而选词、结束单词或透传；调用方要插入标点或选择候选时继续使用相应的显式入口。光标与候选保留，字母插入正常。

## Verification

- `dedicated_english_caret_rejects_nonletters_without_releasing_the_key` 与 `dedicated_english_caret_does_not_turn_rejected_keys_into_candidate_picks` 使用真实引擎和合成输入。旧实现分别因数字返回未处理、字符键 `1` 提交整词 `synthetic` 失败；回归覆盖非字母被消费且不提交、光标与候选不变、合法字母插入和随后显式选词。
- 光标编辑修复在 macOS 上运行 `cargo test -p msime-engine -p msime-host-api -p msime-input-runtime --locked`：整合 `develop` 的 `9a1331b14` 后 3,217 项通过，39 项按既有设置忽略，包含全部 31 项 golden 基准；未做系统输入框或设备验收。
- 光标编辑修复通过 `cargo clippy -p msime-engine -p msime-host-api -p msime-input-runtime --all-targets --locked -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check` 和 `pnpm run verify-notes`。
- 光标编辑修复通过 `bash scripts/verify-local.sh --quick`：包含 Rust workspace、Android target、wasm target、Android 宿主 Java、macOS 原生宿主及共享 Apple 桥接层；Linux 因缺 Docker、Windows 因缺 MinGW、HarmonyOS 真打包因缺完整前置环境跳过，桌面 Tauri 检查因缺其声明的产物跳过。
- `crates/engine/src/session/tests.rs` 的 `dedicated_english_writes_ascii_punctuation_unless_locked_to_chinese` 覆盖：组字中、空闲、lock 1、lock 2、回到中文模式。
- `cargo test -p msime-engine -p msime-input-runtime -p msime-host-api` 全部通过。
- 鸿蒙模拟器上的验证见 PR。
