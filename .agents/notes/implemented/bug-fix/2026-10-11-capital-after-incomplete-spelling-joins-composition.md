# Agent Note: 拼写不完整时组字中的大写字母并入组字

Status: implemented

## Problem

#7106：全拼、关掉辅助码时，打 `mac` 再按 Shift+O，macOS 上得到「马车O」——用户没选过的首选被上屏了。用户想要的是 `macOS` 留在组字里，回车按原样上屏英文。

原因在引擎 `InputSession::handle_character` 的接受闸门：组字中的大写字母只有在「方案是全拼或双拼、并且对应的辅助码开关打开」时才收下，开关关着就返回 `unhandled`。macOS 宿主收到 `unhandled` 而组字不为空时，按 Apple 的做法先 `MSIME_FINISH_COMPOSITION` 上屏首选，再让应用插入字母。光标插入路径 `insert_at_caret` 有同一个闸门。

这个闸门把「开关开着」当成了「这个字母是辅助码」。开关开着时，`mac` 后的 O 根本凑不成辅助码（`detect_active_helpcode_length` 要求去掉尾部大写后是完整拼音），照样过闸门，由方案保留大小写追加，回车上屏 `macO`。也就是说开关的实际作用不止「是否按辅助码筛选」，还决定了凑不成辅助码的大写字母是并入组字还是让首选上屏。

## Decision

辅助码开关只管「会被读成辅助码的那个大写字母」。

- 新增 `InputSession::accepts_composing_capital(raw_with_cases)`（`crates/engine/src/session/composition.rs`），入参是收下这个字母之后的带大小写原文。辅助码开着时一律收下，行为不变。关着时：
  - 全拼：`detect_active_helpcode_length(小写原文, 原文) == 0` 才收下。
  - 双拼：从 `active_shuangpin_helpcode_length` 抽出不看开关的检测体 `shuangpin_helpcode_length`（原函数先看开关再调用它，行为不变），结果为 0 才收下。
  - 其他方案不收，与原来一样。
- `handle_character` 用 `原文 + 新字母` 调它；`insert_at_caret` 用插入后的文本调它，光标在组字中间时与行末键入同一条界线。
- 会被读成辅助码的字母（完整拼音后的位置：`ni`+H、`nihao`+O、`woyong`+M），开关关着时仍照参考引擎和 PR #347 交还宿主，上屏加插入。其余大写字母（前面的拼写不完整：`mac`、`iphon`）和开关开着时一样作为字面字母并入组字，回车上屏 `macOS`。

宿主一律不改：macOS、Windows（TSF 和 Server 都经 `msime_client_character`）、Linux IBus 和 fcitx5 都由引擎的 `handled` 决定。golden 不变：7 个辅助码关闭场景（`qp_helpcode_disabled_uppercase` 和 6 个 `bridge_helpcode_*_off`）都是 `ni`+H，仍在辅助码位置；`session/tests.rs` 和 `host/tests.rs` 里的 `ni`+H 探针同样不变。

网页引擎 `crates/engine-wasm/src/host.rs` 不在范围内：它在辅助码关闭时自己 `commit_plain` 上屏原码再打出字母，结果本来就是英文，不是首选；而且 [网页引擎双拼辅助码](../feature/2026-10-08-web-engine-shuangpin-helpcode.md) 承诺过辅助码关着时帧不变。没有组字时的 Shift+字母由 [Shift+字母进入快捷模式总开关](../feature/2026-10-11-shift-letter-entry-switch.md) 决定，与本篇不重叠。

## Alternatives considered

- **关掉辅助码时组字中的大写字母一律放行** — 最简单，一行条件，开关开不开对「并入组字」完全一致。但全拼方案层不看开关地剥掉活动辅助码（`quanpin/scheme.rs` 的 `apply_segmentation` 注释原文是 "stripped whatever the request's helpcode switch says"，选词完成判断和选词后推进组字、在线请求的缓存键也都不看开关）。放行完整拼音后的大写字母，`nihao`+O+空格会上屏「你好」，O 被静默吞掉，比原 bug 更糟。它还会让「中文后接首字母大写的英文词」退化：`woyong`+Mac 原来得到「我用Mac」，放行后整串变成组字 `woyongMac`。它还要改 7 个 golden 场景。不采用。
- **宿主层收到 `unhandled` 时不上屏** — 只改 macOS 一处就能消除症状。但规则会变成每个宿主各一份，Android、Windows、Linux 的表现继续各不相同，违背 ARCHITECTURE.md「行为规则只在引擎一份」。不采用。
- **让全拼剥离辅助码时看开关，再一律放行** — 这是最彻底的修法，也能关掉下面说的剩余窗口。但 `apply_segmentation` 在 `build_request` 里运行，早于 `apply_request_options` 把开关写进请求（`ime/mod.rs`），要把开关一路传进方案层、选词层和在线请求层，改动面大，还会改变辅助码关闭时现有场景的切分结果。超出一个 bug 修复的范围，不放进这次。
- **新增「组字中大写字母上屏 / 并入」偏好** — 现有偏好里没有这类开关，`local_modes.shift_letter_entry` 只管没有组字时。把一个缺陷修复做成用户要理解的选项不值得。不采用。

## Consequences

- **收益**：拼写不完整时按 Shift+字母不再让首选静默上屏，回车上屏带大小写的原文（`macOS`），与辅助码开着时的现有行为一致；不会丢字，因为并入的字母本来就不会被检测成活动辅助码。Android 原来在 `mac`+O 时吞掉字母（`ChineseHelpcodePolicy` 不看开关、忽略返回值），现在引擎收下，也一起修好。Windows 段落恢复按原顺序逐字节重放原文，新闸门只看前缀，重放时每个大写字母看到的前缀与当初相同，结果一致。
- **与参考引擎的偏离**：拼写不完整、辅助码关闭时，组字中的大写字母不再 `unhandled`。现有 golden 没有覆盖这种情况。
- **剩余的丢字窗口**：组字里已有按字面并入的大写字母时，光标编辑可能让它前面变成完整拼音。例如 `xiaozO`，把光标移到 `z` 后插入 `i`，得到 `xiaoziO`，这时全拼会不看开关地把 O 剥掉，空格选词后 O 丢失；Backspace、DeleteForward 删掉中间字母也可能造成同样的情况。这需要光标编辑带大写字母的组字，概率很低。真有人报，就做上面第三个备选：让剥离看开关。
- **并入的大写字母按小写参与查询**：`macOS` 按空格得到的是把它读成拼音的中文候选，与辅助码开着时的现有行为相同；回车始终上屏原文。不做微软拼音「吗cOS」那种中英混排候选。
- **Caps Lock**：大写锁定时，拼写不完整的组字里的大写字母也会并入，不再先上屏首选。Windows TSF 对 Caps Lock 的注释是组字中吞掉，方向一致；macOS 的 `MSIMECapsLockFreshUppercaseBypass` 只在没有组字时生效，不受影响。
- **双拼同一条界线**：双拼原文不是英文拼写，并入的大写字母按双拼键解码，但双拼的剥离看开关，不会丢字。
- **同类宿主缺陷不在本篇**：Android 在辅助码关闭、完整拼音后按 Shift+字母时，`MSIMEInputService` 忽略引擎的 `unhandled`，把字母吞掉。这是读代码得出的，没有上设备验证，需要另开 issue 在设备上确认后修宿主。

## Verification

`crates/engine/src/session/tests.rs` 的三条用例在旧闸门上都在 `handled` 断言处失败，修复后通过：

- `capital_after_incomplete_spelling_joins_composition_when_helpcode_is_off`：全拼、辅助码关，`mac`+O+S 组字为 `macOS`、不上屏，CommitRaw 上屏 `macOS`；`nihao`+O、`woyong`+M 仍 `unhandled`；没有组字时 O 仍 `unhandled`；辅助码开时 `mac`+O 和 `nihao`+O 都收下。
- `capital_inserted_at_the_caret_follows_the_same_helpcode_boundary`：`mac` 光标左移一位插入 X，收下，CommitRaw 上屏 `maXc`。
- `shuangpin_capital_joins_only_where_it_cannot_read_as_a_helpcode`：小鹤双拼、辅助码关，`ni`+H 仍 `unhandled`，`m`+A 收下并上屏 `mA`；辅助码开时 `ni`+H 收下。

`cargo test -p msime-engine`（含 golden，场景无变化）、`cargo test -p msime-input-runtime -p msime-host-api`、`cargo clippy -p msime-engine --all-targets -- -D warnings`、`cargo fmt --check` 均通过。
