# Agent Note: 只出单字

Status: implemented

## Problem

内测用户在 Android 九键上打 `94748`（想要「字」）时，候选前几页全是 紫苏属、自输入、字数去、字数诉 这类词和整句，单字排在 22 页里很靠后，翻不到。用户要一个开关：候选只列单个汉字，逐字挑。

## Decision

- 共享偏好 `single_character_only`（`crates/client-core/src/preferences.rs`，缺省 `false`）。`host-api` 在 `apply_pending` 和建会话时把它抄进 `EngineOptions::single_character_only`，再由 `host::options::session_options` 交给 `SessionOptions::single_character_only`。改它和其他候选偏好一样，等组字结束才重建 Engine。
- 过滤在引擎里，规则是 `text::is_han_phrase`：候选含汉字、且不止一个字符就去掉（`你好`、`T恤`、整句）。单个汉字留下，不含汉字的英文、emoji 和颜文字也留下，它们由各自的混输开关管。
- 只对 `SchemeType::filters_to_single_characters` 为真的方案生效：全拼、双拼、五笔、粤拼。这几个方案选一个候选就上屏，剩下的拼写接着组字，所以逐字挑能把整句拼完。注音的候选列表是在组字里改一段字，不上屏，过滤它会让改字只剩单字；笔画本来只出单字。日文、韩文、越南文、藏文不受影响。
- 全键盘：在 `InputSession::mixed_from` 里过滤，放在 `apply_candidate_positions` 之后，因为输入只有一个字母时固定位置会补回列表里没有的词。学习排名用的 `ranking_candidates` 走同一个函数，显示的列表和排名列表一起过滤，序号对得上。
- 九键：在 `NineKeySession::refresh` 查词典的循环里、`push_ranked` 之前跳过，不在排完序后再删。九键列表上限是 `CANDIDATE_LIMIT`（128），等排完序再删的话，名额先被词组占满，单字会被截掉。
- 界面：Android 设置「打字 › 中文」（`TypingPage`，表项 `InputFeatureToggle.SINGLE_CHARACTER_ONLY`）；共享设置页「输入 › 候选与联想」（`SingleCharacterOnlySection`），桌面宿主和 iOS 用这一页。iOS 键盘扩展把它列进 `MetasequoiaInputSessionBridge.appEditedKeys`，已经打开的键盘在空闲时就能用上新值。

## Alternatives considered

- **在 input-runtime 或宿主里过滤显示出来的列表**：不用改引擎，但选择是按序号回到引擎的，显示层删掉几行以后，序号得在运行时和引擎之间来回换算，学习排名也会对着没删过的列表。ARCHITECTURE.md 也规定候选的取舍归引擎。
- **只去掉含两个及以上汉字的候选**：`T恤`、`卡拉OK` 之外，词典里汉字和字母混排的词都会留下来。用户要的是「只看单个字」，所以混排的词也去掉。
- **在固定位置之前过滤**：固定位置会补回列表里没有的词，过滤要放在它后面，才不会被补回的词组绕过。
- **随账号同步（`settings_sync.rs` 的 `input.single_character_only`）**：同步字段表在服务端也要登记，服务端这次没改，所以先只存本机偏好。

## Consequences

- **收益**：用户报的那个输入在出货词典上从 `艺术 字数 一书 以求…` 变成 `一 子 自 以 已 意…`，选「一」后剩下的 `748` 接着出 `书 数 求 球…`。全拼 `zishu` 同样先出 `子 自 字 资 紫…`，选一个字后 `shu` 出 `书 数 术…`。
- **代价**：五笔只有词组的编码（如 `ukuy`）开着开关时没有候选。五笔四码唯一自动上屏看的是过滤后的列表，同一个编码下只剩一个单字时会直接上屏，这是五笔单字模式的通常行为。
- **未覆盖**：没有随账号同步。HarmonyOS 和 macOS 原生设置页没有单独的开关行，这两个平台的引擎能读到共享偏好，但用户在宿主自己的设置里改不了它。键盘工具栏上没有快捷切换。

## Verification

- 引擎：`cargo test -p msime-engine --lib`（新增 `session::tests::single_character_only_offers_one_character_at_a_time`、`nine_key::tests::single_character_only_keeps_one_character_readings`，以及 `text`、`host` 映射用例）。另用出货资源跑过一次性的探针：全拼 `zishu`、`woaibeijingtiananmen`、`shi` 和九键 `94748`，结果见上。
- host-api：`cargo test -p msime-host-api --lib single_character_only`。
- 共享设置页：`apps/desktop` 下 `pnpm exec vitest run tests/settings`、`pnpm run typecheck`。
- Android：`ANDROID_SDK_ROOT=… bash platforms/android/check-host.sh`（含 `InputFeatureToggleSmoke` 和 `scripts/test-android-preference-keys.py`）。没有在真机或模拟器上看过。
