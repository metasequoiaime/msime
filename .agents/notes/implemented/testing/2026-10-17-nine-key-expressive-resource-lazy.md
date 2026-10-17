# Agent Note: 九键符号混排按需解析表情词库路径

Status: implemented

## Problem

九键混排刷新在符号开关单独打开时仍无条件构造 `OTHER_DICTIONARY` 的资源路径。符号查询使用内置符号目录，不读取这个数据库；路径对象因此是每次有效混排查询都多出的临时工作。

## Decision

只有 `emoji_candidates` 或 `kaomoji_candidates` 打开时才解析 `OTHER_DICTIONARY` 路径。两个数据库查询分支继续共享同一个路径；仅符号分支不构造路径，符号结果、读法边界、去重、顺序和上屏数字保持不变。

## Historical audit

已检索九键、表情混排、`OTHER_DICTIONARY`、资源路径和分配复用相关的 `proposed`、`implemented`、`rejected` 笔记。现有九键左列追加、读法行缓冲、简拼读音容量及表情候选行为笔记分别覆盖相邻优化或功能契约；没有完全吸收本次“符号独立时不解析外部数据库路径”的决定。

## Alternatives considered

- 始终构造路径：代码最简单，但符号单独启用时每次刷新都做无用路径工作。
- 把符号查询也迁移到 `OTHER_DICTIONARY`：可以共用查询入口，但会改变符号目录的来源和失败语义；本片只收窄资源访问条件。
- 为每种开关拆成三套查询函数：能消除分支，但扩大重复代码和维护面；保留一个可选路径即可表达契约。

## Verification

旧实现的符号独立路径分配计数为 1152；按需路径实现为 1148，新增回归测试以不超过 1150 次为门槛，并确认读法路径非空。九键模块 135 项测试全部通过，完整 engine/runtime 测试为引擎 2253、golden 31、runtime 318 通过（引擎 40 项既有忽略）。Clippy（`-D warnings`）、Rustfmt、笔记检查和 `git diff --check` 通过。`verify-local.sh --quick` 的仓库级检查、Rust workspace、WASM、Android JNI 契约、macOS 配置和 Apple bridge 通过；Windows 测试、Android Java 与 macOS Swift 阶段因宿主磁盘仅剩约 129 MiB 报 `No space left on device`，其余未修改平台阶段按环境跳过，不能把本次 quick 记为全绿。

## Consequences

符号单独启用时不再构造未使用的资源路径；emoji 或颜文字启用时仍按原路径解析数据库，资源缺失仍只返回空候选。收益是一个小的热路径分配削减，不能外推为九键 16 ms 端到端预算已达成；后续若增加新的混排目录开关，应明确它是否需要 `OTHER_DICTIONARY`。
