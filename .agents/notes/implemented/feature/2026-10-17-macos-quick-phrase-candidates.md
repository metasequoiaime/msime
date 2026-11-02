# Agent Note: macOS 普通候选接入快捷短语

Status: implemented

## Problem

macOS 的快捷短语目前只能在按下 `Shift+K` 后进入 K 模式。快捷短语编码无法直接参与普通拼音候选，用户必须记住额外的前置键。

## Decision

macOS 会话声明一项临时宿主能力。普通全拼、双拼和五笔组字查询在保留原有候选顺序的基础上追加编码匹配的快捷短语；其他平台和普通会话保持现状。Linux 的两个宿主后来也请求这项能力，见 [Linux 账号云功能对齐](2026-10-20-linux-account-features-and-input-parity.md)。快捷短语行继续使用 `CandidateSource::QuickPhrase`，选中后直接提交文本。`Shift+K` 手动模式保留，供需要独立查看短语表的用户使用。

## Alternatives considered

- **把每个平台的 `k` 都改成快捷短语入口**：实现局部，但会抢走普通拼音以 `k` 开头的输入，且不满足“参与候选”的语义。
- **只在普通拼音无候选时查询快捷短语**：能减少候选冲突，但有效编码可能被大量普通候选挤出第一页，用户仍会觉得快捷短语没有生效。
- **修改共享偏好让所有触屏和桌面都直接混入**：行为更统一，但 issue 只针对 macOS，会改变 Android、iOS、HarmonyOS 和 Windows 的既有入口契约。

## Intended behavior

- macOS 普通组字对匹配编码显示快捷短语，短语行可点选、按数字选择并直接上屏。
- 普通拼音候选、快捷短语去重和候选上限保持稳定。
- 其他平台默认不追加快捷短语。
- 设置页能说明 macOS 可直接输入编码，且保留 `Shift+K` 入口。

## Risks

普通编码同时命中拼音和快捷短语时会增加候选数量；追加顺序必须避免改变已有普通候选的排序和个人学习路径。

## Verification

- `cargo test -p msime-engine --lib`：2256 passed，40 ignored。
- `cargo test -p msime-client-core --lib`：396 passed，2 ignored。
- `cargo test -p msime-host-api --lib`：785 passed，2 ignored。
- `pnpm --dir apps/desktop exec vitest run tests/settings/local-mode-options.test.ts`：4 passed。
- macOS 原生宿主在 `target/macos-isolated` 构建成功；CTest 150 项中 149 项通过，`shortcut` 单独重跑通过。`http-voice-controller` 因现有测试夹具缺少 `updatePreferencesSnapshot:error:` 方法异常退出，与本改动无关。
- `cargo check -p msime-host-api --locked` 和 `cargo fmt --all -- --check` 通过。

## Consequences

这一版的 host-api 解析了 `HostOptions.quick_phrase_candidates`，却在 `into_engine_options` 里把它写死成 `false`，所以 macOS 会话里实际没有生效；[Linux 账号云功能对齐](2026-10-20-linux-account-features-and-input-parity.md) 改为按请求转交，并加了经 C ABI 建会话的测试。同一篇里还把普通组字改成只认整个编码、受设置页「快捷短语」开关控制，并让五笔四码唯一不计入追加的短语。

macOS 会话通过临时 `HostOptions` 能力开启快捷短语候选，运行时配置文件和其他宿主的默认行为不变。普通候选仍先保持原顺序，快捷短语追加在末尾并按文本去重；`Shift+K` 独立入口继续可用。设置页会针对 macOS 说明两种入口。普通候选和快捷短语共用候选上限时，追加项可能排在首屏之后，这是保持既有普通候选排序的取舍。这个取舍已被 [快捷短语混输默认落在首页末位](2026-10-25-quick-phrase-first-page-default.md) 取代：#9702 里小鹤双拼 `zj` 有 83 个普通候选，短语落在第 14 页；现在没有学习记录的短语落在首页最后几格，普通候选不满一页时仍接在它们之后。

快捷短语的 `${cursor}` 展开与上屏后缀契约见[光标定位笔记](../../implemented/feature/2026-10-31-quick-phrase-cursor.md)；不改变本文原有决定。
