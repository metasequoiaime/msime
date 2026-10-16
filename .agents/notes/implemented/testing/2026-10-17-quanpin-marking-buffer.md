# Agent Note: 全拼主切分标记复用已有字符串

Status: implemented

## Problem

`query_exact_projected` 每次从解析结果复制 `segmentation`，再替换 `pinyin_segmentation`，丢掉前一次查询的缓冲。九键逐键会查询多条切分，系列热命中也重复申请同样的标记字符串。标记内容需要独立拥有，但容量不需要每次重新申请。

## Decision

在同步解析投影里对现有 `pinyin_segmentation` 调用 `clone_from`。只复用这一个字符串，替代标记数组、槽键、候选及失效顺序不变。容量不足仍扩容，空输入仍提前返回并保留原标记状态。缓冲生命周期与词典一致。使用 Rust 标准库的 [`String::clone_from`](https://doc.rust-lang.org/std/string/struct.String.html#method.clone_from) 及其[开源实现](https://doc.rust-lang.org/src/alloc/string.rs.html)，不引入依赖、公开接口或新的缓存。

## Existing decisions

检索活跃树中的 `pinyin_segmentation`、标记缓冲、切分缓冲和 `resolution_cache`：[解析缓存投影](../../implemented/testing/2026-10-14-quanpin-resolution-cache-projection.md)持有同步借用和失效顺序，本项部分重叠，继续保留其独立标记所有权，仅复用主标记容量；[大小写输出缓冲](../../implemented/testing/2026-10-15-pinyin-case-byte-comparison.md)处理另一个请求字段，不持有词典标记。其他路径/键规划缓冲无关，没有需要吸收或撤回的提案。

## Alternatives considered

- 继续复制后替换：容量总是等于本次长度，不保留历史峰值；但每条热切分都申请再释放标记字符串。
- 同时复用替代切分数组：纠错时可再减少分配；但扩大本次切片与容量管理范围，单独处理主切分即可验证一个独立收益。
- 借用解析缓存里的切分：免去复制；但标记状态需要在同步投影结束后持有，后续可变缓存操作不能继续借用它。

## Verification

真实查询红测试在旧生产失败：热单音节两侧均为 7 次申请，要求保留容量的一侧为 6 次。实现后 18 组对照全部严格少一次，分别覆盖六种读法的系列热命中、系列未命中和解析冷重建；包括普通单双音节、别名、两档纠错、删首字母纠错与空候选。热单音节 7→6、双音节 10→9、两档纠错 9→8、删首字母 12→11；解析冷重建的删首字母 191→190。此处对照只在测量前丢弃主标记缓冲，其余状态完全相同，用于隔离容量复用。历史测试中的完整冻结查询仍提供行为与累计分配对照。

新增增长和空输入用例检查从容量不足的单音节扩大到三音节、空输入不修改既有标记且零分配、修改返回候选不污染缓存及词典释放后输出仍拥有数据。历史真实 SQLite 对照继续覆盖 FIFO、个人模型和数据库版本失效、纠错与候选拥有性。五个历史状态断言仅将主标记容量相等改为不小于旧容量；候选容器、字段和缓存容量检查不变。16 个历史查询预算按明确预热条件另计本片一次节省，序列测量的两次查询另计两次；6 个会话预算各精确收紧一次：忽略按键 23→22、末尾输入 126→125、末尾退格 15→14、设置/移动光标 22→21、光标处输入 192→191。所有冻结旧方法正文保持不变。

`cargo test -p msime-engine --locked`：2,233 项通过、37 项忽略，31 项 golden 通过；`cargo clippy -p msime-engine --locked --all-targets -- -D warnings` 通过。Rustfmt、`git diff --check` 与 `pnpm run verify-notes` 通过（1,175 篇活跃笔记、4 篇归档及封印）。独立审查无阻塞项，并另行复跑新增两项测试通过；确认所有冻结 `legacy_*` 正文未修改。

本地 `CARGO_BUILD_JOBS=6 bash scripts/verify-local.sh --quick` 退出 0：155 项可移植 Windows 原生测试通过（2 项因宿主差异排除），仓库契约、actionlint、Rust workspace、Android/wasm、Android Java 宿主、鸿蒙共享设置 bundle、macOS 原生宿主和共享 Apple bridge 构建通过。Linux 因无 Docker、Windows 原生交叉及 pipe 配置因无 MinGW、鸿蒙真打包因缺少 hvigor/资源/原生库/依赖而跳过；`msime-desktop` 的 macOS Rust 外壳因未准备应用资源 bundle 未纳入 workspace 检查。没有真实宿主输入或设备验收。

## Consequences

容量够用时每次查询少申请和释放一个主标记字符串，查询结果和候选容量不变。词典保留主切分历史峰值容量，首次申请及扩容遵循 `clone_from` 的标准增长规则；不保证内部标记容量等于本次长度。输入上限仍由既有会话管理。本片没有测量 CPU、进程内存或真实宿主输入延迟，不宣称达到九键 16 ms 目标。
