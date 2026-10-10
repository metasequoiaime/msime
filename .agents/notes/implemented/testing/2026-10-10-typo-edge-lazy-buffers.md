# Agent Note: 纠错边收集缓冲按需预留

Status: implemented

## Problem

`collect_typo_edges` 在查询全部命中跨度缓存时，仍为未命中键列表预留 `planned.len()` 个字符串槽；所有缓存结果均为空时，又为最终空纠错边列表预留 `planned.len() * TYPO_ROWS_PER_KEY` 个槽。这两块存储没有承载任何元素，却随按键重复申请。

## Decision

两个向量从 `Vec::new` 开始，在首个未命中键和首组非空行写入前，分别调用原来的 `Vec::with_capacity`。非空结果保留原始容量提示，字段、顺序、罚分、查询批次、FIFO 和行克隆行为保持一致；空边结果明确返回零容量。使用完整冻结 helper 对照验证两项分配减少及冷查询峰值。

## Alternatives considered

- 保留立即预留：循环最直接，但全命中和空结果仍持有无用缓冲。
- 依赖 `push` / `extend` 自然扩容：空列表不分配，但有结果时容量提示和扩容次数改变。首次写入前沿用原预留避免扩大命中路径的改动。
- 两次扫描预先统计未命中键和非空行：可以按实际数量分配，但增加缓存查找次数，并改变非空结果容量策略；现有上限预留只需延后一次。

## Verification

完整旧 helper 固定于 `ffb35a03751e1c2fdf139f13b142f83b0558f75b`，除函数名外正文相同，两侧共用当前未改的规划和数据库 helper。旧生产代码上全热空结果测试真实失败（两侧均 92 次分配，期望少两次）；新实现通过五项回归。非空测试覆盖 1/4/5 行、仅最后键有结果及个人接受记录，比较完整字段、罚分位模式与容量。真数据库测试覆盖冷查、部分命中、容量 2/3/512 的缓存及两轮淘汰；边界覆盖段数 0/1/2/6/64/65、关闭数据库、关闭纠错及无完整字词覆盖。

arm64 debug 下六段合成 `zhuang` 的测量如下，单位为 Rust 请求字节：

| 场景 | 分配次数：旧→新 | 峰值：旧→新 | 返回存储：旧→新 |
| --- | --- | --- | --- |
| 全热空结果 | 92→90 | 11817→5021 | 6400→0 |
| 新建数据库与缓存、空结果 | 200→199 | 30790→24971 | 6400→0 |
| 新建数据库与缓存、仅最后键两行 | 213→213 | 31358→31358 | 6456→6456 |

冷测量在区间内新建并析构数据库、缓存，只返回边；个人资料、输入和不可变变体表在区间外准备，预热旧 helper 的临时数据库、缓存和结果先析构。两侧最低有符号净字节均为零；这里的冷指新建数据库与缓存，不代表冷文件页缓存。

`cargo test -p msime-engine --lib` 通过 1581 项（13 项忽略），`cargo test -p msime-engine --test golden` 通过 31 项，`cargo clippy -p msime-engine --all-targets -- -D warnings` 通过。`cargo fmt --all --check`、`git diff --check` 与 `pnpm run verify-notes` 通过。完整 `MSIME_BUILD_JOBS=2 CARGO_BUILD_JOBS=6 bash scripts/verify-local.sh --quick` 重跑 exit 0，覆盖 103 项本机可运行的 Windows 原生测试、仓库契约和笔记、workspace、Android target 和 Java、macOS 原生宿主与共享 Apple bridge。独立审查无阻断问题。

首轮 quick 在磁盘余量不足 300 MiB 时报告 macOS 链接失败，过滤后的输出没有保留链接器具体原因。只移除本任务可重建的前端依赖和已完成原生测试产物的非运行期符号后，同一构建目录的完整 CMake 构建 exit 0；`skin-preview` 测试执行通过，再完整重跑 quick 通过，没有修改代码或绕过门禁。

Linux 桌面与原生阶段因 Docker 不可用跳过，本次没有 Linux 构建证据；macOS 共享桌面包尚未构建，Wasm/MinGW 工具链、HarmonyOS 真打包输入及设置页依赖缺失，相应阶段跳过。Android 共享桌面库仍报告 11 条已有警告。没有设备或系统输入法框架验收，也不沿用上一片的平台通过记录。

## Historical audit

[变体规划缓冲](2026-10-08-typo-plan-variant-buffer.md) 只管 `plan_keys` 的变体向量，[空查询映射](2026-10-10-pinyin-empty-key-map-storage.md) 只管数据库返回映射，均与本次两个收集缓冲无关；本次冻结 helper 与新实现共用这些已合入的优化，不重复归因。[纠错 beam 按需预留](2026-10-09-autocorrect-lazy-beams.md) 是相同预留手法用于其他对象的参考。历史检索未找到这两个缓冲已有的决策归属，无需修改这些笔记。

## Consequences

全命中路径省去未命中键缓冲，空边结果省去输出缓冲且容量为零；有元素时继续使用原容量提示，不改变查询批次、FIFO、行克隆和排序语义。首次写入前增加空向量判断，尚无真实按键计时；Rust 请求字节不包含 SQLite C 堆、分配器暂存或 RSS，不能由这些结果声称端到端加速。大提示及查询预算保持原样。
