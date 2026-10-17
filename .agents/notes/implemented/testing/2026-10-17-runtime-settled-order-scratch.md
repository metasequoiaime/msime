# Agent Note: 复用落定重排的插件座位顺序缓冲

Status: implemented

## Problem

`Runtime::rerank_settled` 在有脚本插件操作需要旁路重放时，先用 `seat_order()` 复制一份当前座位到临时 `Vec<usize>`，重排并重放后又调用一次 `seat_order()` 生成另一份向量，只为比较插件重放是否改变了座位。落定重排可能在用户停顿期间重复调用，这两次最多候选数的堆分配不参与模型评分或候选呈现。

## Decision

在 `Runtime` 中增加跨次复用的 `settled_order_scratch`。落定重排开始时把当前座位顺序写入该缓冲，重放后直接用 `seat_order_matches` 与当前引擎下标比较；没有插件操作时不触碰缓冲。`seat_order()` 保留原有拥有型测试接口，但改为经同一写入辅助函数生成结果。

候选排列、插件身份、引擎选择下标、代次推进和 `rerank_settled` 返回值保持不变；只替换比较所需的临时向量所有权。

## Existing decisions

已检索运行时插件排列、落定重排、在线排列缓冲和候选身份相关笔记。现有[插件排列工作缓冲](2026-10-15-plugin-transform-arrangement-buffer.md)只复用排列阶段的布尔、自由座位和尾部索引，[候选身份出现次数缓冲](2026-10-15-plugin-transform-occurrence-buffer.md)只处理重复候选定位；本项处理 `rerank_settled` 外层比较向量，三者生命周期互不替代。未发现已覆盖该比较向量的笔记。

## Alternatives considered

- **每次继续调用 `seat_order()`**：实现最简单，但每次落定重排在旁路路径固定产生两次短向量分配。
- **只用顺序变更计数判断移动**：可以省去向量，但无法区分插件先重排后恢复原顺序的情况，会改变现有最终顺序比较语义。
- **用哈希代替完整比较**：无需缓冲，但哈希碰撞会把未变化误判成变化；复用完整顺序保留确定性比较。

## Verification

旧实现的新增分配回归在第二次热落定重排中观测到 2 次分配；接入复用缓冲后降为 0 次。新增测试 `settled_rerank_reuses_the_replayed_seat_order_buffer` 同时预热模型和插件状态后断言第二次调用零分配。

候选变换测试 25 项通过，包含重复候选、隐藏行、锁定席位、整句改字旁路重放和落定重排只整理隐藏行的行为回归。完整 `msime-input-runtime` 319 项、`msime-engine` 2321 项、golden 31 项通过；两 crate 全目标 Clippy（`-D warnings`）、Rustfmt、`verify-notes` 和 `git diff --check` 通过。Android `aarch64-linux-android` 宿主检查也通过且无新增 warning。`bash scripts/verify-local.sh --quick` 的编译、契约和绝大多数平台阶段通过，唯一失败是已知 macOS 主机差异 `platforms/windows/tests/input/candidate_translation_commit.cpp`，脚本报告“disabling AI candidates left the displayed row”，与基线相同。

## Consequences

有插件操作的重复落定重排复用一份最多候选数的 `usize` 缓冲，减少两次临时向量申请和释放；首次使用仍按候选数量扩容一次。没有插件操作、移动端无插件变换和模型评分路径的行为与存储保持原样。
