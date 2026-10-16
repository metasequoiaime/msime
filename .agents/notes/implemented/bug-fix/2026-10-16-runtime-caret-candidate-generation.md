# Agent Note: 直接移动组字光标使旧候选身份失效

Status: implemented

## Problem

公开 Rust 接口 `Runtime::set_caret` 会让引擎重新解码光标之前的读音，但没有推进候选 `generation`。输入合成 `nihao` 后，首位由整句 `你好` 变为前缀 `你`，原首位的 ID 仍能通过校验并选中新的前缀候选；从前缀恢复末尾也有同样的问题。C ABI 的光标命令经 `dispatch` 推进代次，本问题限定于直接调用 Rust 接口的使用方。

## Decision

`set_caret` 在改变引擎之前调用既有的 `advance()`，与派发光标命令、在线候选更新的身份规则一致。即使刷新失败，旧代次也失效；代次耗尽时在改变引擎之前返回 `IdentityExhausted`。不增加身份字段或改变引擎解码规则。

## Alternatives considered

- **刷新后只在候选变化时推进代次。** 可以让没有改变前缀的重复设置继续使用原 ID；但候选与光标变化已经发生，再遇到代次耗尽就无法发布安全的新身份，需要回滚引擎状态。事前推进沿用现有旁路更新规则。
- **依赖 C ABI 的命令路由。** 原生宿主的现有移动命令已经安全，无需改变代码；但公开 Rust 接口仍可独立调用，接口自身也必须保护它发布的候选 ID。

## Consequences

移动光标后，旧候选 ID 和旧代次的释义回包失效，调用方使用设置后的新视图。代次耗尽时在修改引擎之前拒绝操作。重复设置同一光标也会消耗一代身份，与派发命令的规则一致。此接口不用于当前 C ABI 光标命令，不改变原生宿主的按键路径。

## Verification

两项真实引擎回归在旧实现中失败：保存的整句 `你好` 的 ID 在移动光标后提交了 `你`；耗尽代次后设置光标仍然成功。修复后 `cargo test -p msime-input-runtime --locked moving_the_caret_` 两项通过。前缀与末尾双向操作拒绝原 ID，新视图的候选正常提交；代次耗尽时底层引擎完整快照不变。测试输入、词库均为合成值。未做系统输入框或真机验收。

在 macOS 上，`cargo test -p msime-host-api -p msime-input-runtime --locked` 共 958 项通过：host-api 644 项单测与 15 项集成测试、runtime 299 项单测，另有 2 项既有忽略。`cargo clippy -p msime-host-api -p msime-input-runtime --all-targets --locked -- -D warnings`、`cargo fmt --all -- --check`、`pnpm run verify-notes` 和 `git diff --check` 通过。独立只读审查未发现可证实的问题。

`cargo build -p msime-host-api --locked` 与 `bash scripts/verify-local.sh --quick` 通过。quick 覆盖 155 项 Windows 可移植宿主测试、Rust 工作区、Android 目标、两种 WASM 配置、Android Java 宿主、macOS 原生宿主与共享 Apple 桥接层构建，以及契约、workflow、笔记检查。另有 2 项 Windows 测试因宿主差异排除、66 项需要 Windows 构建、1 项不是测试。Windows 交叉构建、Linux 宿主与外壳、HarmonyOS 真打包缺少前置环境而跳过，Tauri 外壳因规定位置没有原生 bundle 而跳过。

## Related notes

检索 `set_caret`、候选身份和候选代次的活跃笔记：

- [非法固定候选位置不推进代次](../../implemented/bug-fix/2026-10-09-invalid-fixed-position-generation.md)：部分重叠，保留其拒绝非法参数不变更状态的决定；本篇处理有效光标操作必须使旧身份失效。
- [脚本候选变换](../../implemented/architecture/2026-10-12-script-plugins-modify-commit-and-candidates.md)：部分重叠，`set_caret` 继续只重放候选操作，不运行脚本。
- [在线候选更新保留释义](../../implemented/bug-fix/2026-10-11-online-candidate-gloss-retention.md)：部分重叠，沿用其变更候选前推进代次、旧 ID 被拒绝的边界，不更改释义保留策略。

其余命中属于原生候选窗口、宿主异步请求或展示事件，与直接光标入口无关；没有完全吸收或需要否决的旧提案。


## Editable range

[本地模式入口标记](2026-10-17-local-caret-entry-marker.md)补齐同一直接 Rust 光标接口的引擎下界：带标记模式不能把光标设在入口之前。runtime 仍按本篇规则提前推进代次，候选身份协议不变。
