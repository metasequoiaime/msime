# Agent Note: 专用英文切换的组字清理与同值保留

Status: implemented

## Problem

`Runtime::set_dedicated_english` 让引擎在中英文标志真正改变时清空读音，但 runtime 自己的 `phrase_prefix` 和 `phrase_selections` 不随之清空。启用 `phrase_preedit` 的宿主选过半个词后切换专用英文，已选词段仍让会话处于组字中；后续英文上屏可能带出旧中文词段，退格也可能恢复旧读音。IBus 与 Fcitx5 启用词段暂存，并从快捷键直接调用这个接口。

引擎外层 `Session::set_dedicated_english` 原本在检查模式前无条件取消九宫格组字，而内层 `InputSession` 已经保护同值调用。重复同步当前模式因此会清掉中文九键、14 键和英文 T9 的输入及候选，与 runtime 保留组词的规则不一致；原有 runtime setter 单测使用替身，没有覆盖真实引擎的这层取消。

## Decision

只有模式标志实际变化且引擎设置成功时，runtime 同步丢弃暂存前缀和撤回记录，并为消失的暂存词段推进组字事件戳，然后刷新视图。重复设置同一模式保留当前组词与撤回能力；引擎 setter 报错时保留原有组词。清理发生在刷新前，避免引擎已经切换而刷新失败时遗留旧词段。

引擎 `Session` 在取消九宫格组字前检查当前 `dedicated_english`，同值直接返回，让全键盘和网格共用同一幂等规则。实际切换仍先取消网格、清空普通组字，再同步英文 T9 模式；`set_key_grid` 继续负责网格变更时的同步。

## Alternatives considered

- **切换时先上屏已选词段。** 保留用户已经选择的文字，与失焦送出暂存词段的规则一致；但专用英文切换的现有契约是 `SetEnglishInputMode` 加 `ClearState`，会取消整段组字，且接口只返回视图，没有可交付的提交文本。只有用户显式提交或宿主先提交的路径才应送出它。
- **在各宿主切换前补一次 Cancel。** 能直接复用取消规则，Windows 的专用英文快捷键已有类似处理；但 IBus、Fcitx5 和其他调用者都要各自记住，公共 setter 仍会留下跨模式状态。由拥有暂存段的 runtime 收尾，使公共接口自身遵守清空组字的契约。
- **只在 runtime 跳过同值 setter。** 能避免宿主经 runtime 重复同步模式时丢字；但直接调用公开 Rust `Session` 的用户仍会丢掉九宫格组字。把同值检查放在引擎取消入口，让引擎自己的公开接口也满足契约，runtime 继续正常刷新视图。

## Verification

### 网格同值设置

- 使用真实引擎核对中文九键、14 键的完整快照和后续选词，以及英文 T9 的快照和选词；英文 T9 同时覆盖两个方向的实际模式切换仍取消组字。真实 runtime 另核对中文九键、14 键的输入、拼写条、候选和选词提交。
- 修复前，两项 engine 回归分别在中文 `64`、英文 `435` 被清空处失败；runtime 回归也在 `64` 变空处失败。修复后三项回归通过。
- `cargo test -p msime-engine -p msime-input-runtime --locked` 中 engine 2233 项单测、31 项 golden 通过，37 项按既有配置忽略；runtime 因本次命令把 `TMPDIR` 指到工作树的较长路径，导致 socket 测试报 `path must be shorter than SUN_LEN`。使用默认临时目录重新执行 `cargo test -p msime-input-runtime --locked`，310 项全部通过。
- `cargo clippy -p msime-engine -p msime-input-runtime --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`pnpm run verify-notes` 和 `git diff --check` 通过。
- 独立只读审查检查模式赋值入口、网格同步、实际翻转和真实引擎回归，未发现阻断或 Important 问题。
- `bash scripts/verify-local.sh --quick` 通过，覆盖 Windows 可移植测试、仓库契约与笔记门禁、Rust 工作区、Android Rust 目标与 Java 宿主、两种 WASM 配置、macOS 原生宿主及共享 Apple 桥接层构建。Linux 宿主与外壳、Windows 交叉构建和 HarmonyOS 真打包因本机缺少前置环境跳过；Tauri 外壳因缺少约定位置的原生 bundle 跳过。没有做设备或系统输入框验收。

### runtime 暂存词段清理

- 真实 C ABI 会话以合成词库部分选词后切换专用英文，无论剩余读音是否删空，暂存词段与读音都消失，生命周期正确结束且没有提交事件；旧候选不能再选择，退格不能恢复旧中文读音，后续英文原文上屏不夹带旧词段。
- 重复设置同一模式的保护用例保留暂存前缀与读音，之后仍能撤回部分选词。
- runtime 单测在 setter 生效后注入快照失败，确认返回错误且暂存词段与撤回记录均已清理。
- 旧实现中，模式翻转用例因残留 `phrase_prefix = 你` 失败，刷新失败回归也因旧词段未清理失败；同模式保护用例通过。修复后三项回归全部通过。
- `cargo test -p msime-host-api -p msime-input-runtime --locked` 通过：host-api 643 项单测、15 项集成测试，runtime 296 项单测；host-api 另有 2 项按现有配置忽略。
- `cargo clippy -p msime-host-api -p msime-input-runtime --all-targets -- -D warnings`、`cargo build -p msime-host-api --locked` 和 `cargo fmt --all -- --check` 通过。
- `bash scripts/verify-local.sh --quick` 通过，包括 165 项 Windows 可移植宿主测试、Rust 工作区、Android 目标、两种 WASM 配置、Android Java 宿主、macOS 原生宿主与共享 Apple 桥接层构建，以及仓库契约、workflow 和笔记门禁。Windows 交叉构建、Linux 宿主与外壳、HarmonyOS 真打包因本机缺少相应前置环境跳过；Tauri 外壳因未构建其所需位置的原生 bundle 跳过。
- `pnpm run verify-notes` 和 `git diff --check` 通过。
- 独立审查核对清理顺序、重复设置的保护、仅剩暂存词段的生命周期、Linux 调用链和真实 C ABI 回归，未发现可证实的问题。

## Consequences

专用英文切换后的视图、组字生命周期和后续上屏都不再带有上个模式的暂存词段。代价是切换动作现在也丢弃已经选择但尚未上屏的部分，符合这条入口既有的取消契约；需要保留它的宿主仍应先显式上屏。清理不发生在引擎 setter 报错前，无变化的 setter 保留组词。普通中英切换由宿主先上屏的策略和失焦送出暂存段的策略照旧。未做系统输入框验收。

## 历史笔记检索

按 `set_dedicated_english`、专用英文、切换、暂存、`phrase_prefix` 和 `phrase_preedit` 检索活跃笔记。[桌面生命周期](../../implemented/architecture/2026-10-11-desktop-lifecycle-core.md)持有专用英文切换丢弃组字与旁路事件规则，[Windows 托盘模式](../../implemented/feature/2026-10-10-windows-tray-menu-mac-parity.md)持有托盘在组字中拒绝切换的约束；两者继续保留，本篇补 runtime 暂存状态的收尾。[触屏就地组字](../../implemented/feature/2026-10-15-touch-composition-in-field.md)、英文标点、英文前缀借用和日文读音笔记不持有这次状态清理。`docs/windows-parity.md` 的“切换语言是否清空组字”明确已有 `SetEnglishInputMode` 加 `ClearState` 契约，未覆盖 runtime 的已选词段。

相关能力 setter 的无变化调用规则见[词段暂存幂等性](../../implemented/bug-fix/2026-10-16-phrase-preedit-idempotence.md)。
