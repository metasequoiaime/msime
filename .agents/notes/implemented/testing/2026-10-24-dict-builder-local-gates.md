# Agent Note: 词库构建器进入本地完整验证

Status: implemented

## Problem

词库生成器是工作区成员，但不在默认成员中；完整本地验证又显式枚举测试与 Clippy 包，两个列表都没有它。工作区编译无法替代词库构建器的真实测试，严格警告也不会作为门禁执行。

## Decision

将 `msime-dict-builder` 加入现有完整测试和全目标严格 Clippy 循环，复用已有的构建失败检测、测试失败名称归集和硬失败规则。保持 `--quick` 仅编译的约定；验证覆盖不依赖每位维护者手动补跑工具包。

历史检索：[引擎 CI 笔记](../../implemented/testing/2026-10-10-engine-tests-in-ci.md)持有 `--quick` 与完整验证的分工，仍然成立；词库读取与生成模块笔记持有业务实现，与本次枚举决定无重叠；脚本运行时和插件打包工具沿用同一循环，不改变其验证边界。 [参考中继的 Clippy 覆盖](2026-10-24-clipboard-relay-clippy.md)另行补齐同一显式循环中的中继包；本篇继续持有词库构建器的测试与检查边界。

## Alternatives considered

- **只将词库构建器加入工作区默认成员**：裸 `cargo test` 能自动覆盖它，命令也较短。但本地完整验证逐包调用 `cargo test`，默认成员不会改变这份显式列表，Clippy 枚举同样不会增加，因此不能补上实际入口。
- **把整个工作区交给一个测试和 Clippy 命令**：以后新增包不容易遗漏。但桌面包、平台桥和目标专用代码的前置条件不同；现有分包失败归集和已知失败规则也依赖包名。本次沿用现有机制，只补词库构建器。

## Testing

- `cargo test --locked -p msime-dict-builder --no-fail-fast`：183 项二进制单测与 11 项真实 CLI 集成测试通过，0 失败；另 1 项需外部 `MSIME_DICTIONARY` 检出的测试按原规则忽略。
- `cargo clippy --locked -p msime-dict-builder --all-targets -- -D warnings`、笔记检查、Shell 语法与 diff 检查通过。
- 当前固定基线完整 `bash scripts/verify-local.sh --quick` 通过（2338 秒），验证期间脚本和笔记摘要保持不变。包括契约、Rust、Android/JVM、wasm、Linux 桌面外壳与原生宿主、macOS 和共享 Apple bridge；Linux ctest 为 110 项通过、0 失败、1 项原规则跳过。MinGW 与 Harmony HAP 缺少既定输入，按原规则跳过。
- 初次 quick 因上游合并残留导致的公共头编译失败而中断，记录保留；上游随后独立修正了冲突残留和 Windows 提取夹具，分支同步后重新完成上述整套验证。没有修改失败基线、测试断言或时限。

## Consequences

完整模式增加词库构建器编译和测试的时间，覆盖不再依赖维护者手动补跑。既有测试使用合成输入；本次不触发正式词库生成或发布。
