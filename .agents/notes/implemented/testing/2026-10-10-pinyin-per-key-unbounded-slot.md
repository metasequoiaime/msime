# Agent Note: 拼音按键结果槽避免无界限额预留

Status: implemented

## Problem

`query_exact_keys_per_key` 对多音节键从批量查询收到首行时，直接按 `per_key_limit` 创建结果槽。底层 `query_capacity` 已把 `i32::MAX` 及以上限额视为无界并自然读取实际行，但结果槽仍会为 `usize::MAX` 申请不可表示容量，真实命中会触发 `capacity overflow`。单音节路径由 `rows` 直接接管，未触发同一槽位预留；纠错与滑行权重调用会到达该多音节分支，但当前仓库调用方分别使用有限的每键上限和 `1`；本片修复公共入口的无界限额契约，不宣称常用有限按键路径减少分配。

## Decision

有限 SQL 限额继续按原 `per_key_limit` 为首次结果槽预留，保持已有容量和分配预算。`query_capacity` 判定为无界的限额不预留提示容量，首次行写入空 `Vec`，由实际行数自然增长；每键限额判断、表序、行字段、稳定权重顺序、同值行和返回所有权不变。不增加 SQL 查询，不改变底层批量页。

## 既有笔记审计

[结果槽复用已有键](2026-10-10-pinyin-result-key-reuse.md)部分重叠于首次槽预留，互链保留其独立重复键收益。[借用键规划](2026-10-10-pinyin-borrowed-key-plan.md)、[借用分组键](2026-10-10-pinyin-borrowed-key-groups.md)、[空结果表预留](2026-10-10-pinyin-empty-key-map-storage.md)和[键去重](2026-10-08-pinyin-exact-key-dedup.md)属于规划与 Map 层，有限槽保持不变，因此保留其冻结正文与预算。[聚合首个页复用](2026-10-10-pinyin-aggregate-page-reuse.md)属于精确切分和跨度入口，不改本片每键结果 Map。无相同决定、完全吸收或过时提案。

## Alternatives considered

- 统一取消所有结果槽预留：可以减少所有大提示，但会改变常用有限限额的容量与分配预算。
- 把无界限额截断为固定槽数：会丢失真实词库中超过槽数的行，破坏每键限额语义。
- 只在调用方把 `usize::MAX` 改小：公共查询 API 仍会在其他调用方溢出，也会把无界读取错误地变成隐式截断。

## Verification

真实 SQLite 多音节命中以 `usize::MAX` 返回全部行，不 panic；`i32::MAX` 同样不申请提示容量。有限限额仍返回原字段、顺序、每键行数和容量；重复/非法键、空结果、缺表、跨表和大权重保持不变。新增测试覆盖单行、1026 行密集页、多键、空/非法批次、真实 SQLite 首次 step 错误和释放输入后的结果；旧生产在无界真实命中上先行触发 `capacity overflow`，修复后全部通过。

新增回归 8 项通过；同步至固定提交 `ea593d0965da9c74261a054166fa4f52fe7a06b4` 后，完整 engine 单测 1625 项通过、13 项忽略；golden 31 项通过；engine 全目标 clippy、Rust 格式与差异空白检查通过。有限限额矩阵与固定基线的完整函数比较结果字段、行序、结果 Map 与每槽容量、分配次数，均保持相等；无界单行冷测量记录 Rust 分配 34 次、峰值 1804 B、保留 449 B、最小净值 0，仅作为本机 debug 证据，不推断发布性能。

Linux 首轮 quick 的原生宿主容器因内存不足失败，第三方 `glam` 的 rustc 收到 SIGKILL，容器标记 `OOMKilled=true`。沿用 `target/linux-build-gate` 的既有产物，本地将 Cargo 并发限制为 1、容器 CMake 并发限制为 2；不修改门禁脚本或跳过阶段。同目录低并发完整复跑 `bash scripts/verify-local.sh --quick` 退出 0：Rust workspace、Android Rust 目标与宿主 Java 编译/冒烟、Linux 共享外壳检查、Linux 原生宿主构建及 77 项 ctest、五笔版构建与各版本安装契约、macOS 原生宿主和 Apple bridge 编译通过。wasm 缺少目标或 LLVM 工具链，HarmonyOS 缺少构建输入，Windows 缺少 MinGW，相关阶段按门禁规则跳过；macOS 主机上的 `msime-desktop` 因尚未构建它要求的 app bundle 跳过。未做设备装机、系统输入法验收或 macOS ctest，不据编译结果宣称这些覆盖。

## Consequences

无界有限行的结果槽可能经历自然扩容，分配次数可能增加；这是避免不可表示预留的必要代价。与[结果槽复用已有键](../../implemented/testing/2026-10-10-pinyin-result-key-reuse.md)部分重叠于同一 `Vec`，本片只改变无界容量提示，有限槽容量和其重复键收益保持不变。底层批量读取和 SQL 限额不变；不据此宣称 SQLite C 堆或真实宿主延迟收益。
