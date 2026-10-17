# Agent Note: 九键读音列表复用刷新缓冲

Status: implemented

## Problem

九键刷新开始时会清空会话的 `spellings`，随后让 `SpellingTable::spellings_for` 新建一个读音向量，最后用它替换旧字段。旧向量的容量在每次按键后都被丢弃，热刷新反复为同一类左列读音申请新的槽位。

## Decision

增加向目标缓冲写入的 `spellings_for_into`，先清空调用方的 `Vec<String>`，按原有覆盖、频度、字典序排序后把读音字符串追加到该缓冲。九键刷新直接复用 `self.spellings`，再从同一缓冲复制未完成路径需要的读音；保留拥有型 `spellings_for` 包装供独立表测试使用。读音顺序、筛选、路径扩展、左列选项和撤销记录均不变。

## Existing decisions

已检索九键读音、拼写路径和缓冲相关笔记。[九键左列选项直接追加](2026-10-17-nine-key-key-choices-append.md)只消除追加选项的临时外层向量，本片复用的是主读音列表；[九键路径延后物化](2026-10-16-nine-key-path-chain.md)和[种子尾段按上限物化](2026-10-17-nine-key-seed-tail-materialization.md)处理切分路径，不拥有本字段。三篇职责不同，继续保留。

## Alternatives considered

- **继续返回新向量**：实现最小，但每次九键刷新都放弃会话已有容量。
- **只在刷新后把新向量的容量转移回字段**：无法复用旧字段中仍保留的容量，而且会让旧向量和新向量同时存在到替换点。
- **在 `SpellingTable` 中保存可变共享缓冲**：可以减少调用方参数，但把一次会话的临时状态放进不可变键盘表，扩大生命周期和借用边界；目标缓冲参数更直接。

## Verification

旧实现上的 `refresh_reuses_spelling_vector_storage` 在固定合成输入 `64426` 的第二次刷新中指针不同而失败；新实现指针和容量均保持不变。冻结九键投影刷新在同一场景的分配对照由旧 1708、新 666 调整为复用主读音缓冲后少 2 次，回归断言同步更新。九键模块 140 项、完整引擎库 2260 项（40 项既有忽略）、golden 31 项和 runtime 318 项通过；Clippy、格式、`git diff --check` 与 `pnpm --config.verify-deps-before-run=false run verify-notes` 通过。

`TMPDIR=/Volumes/Workspace/tmp/msime CARGO_BUILD_JOBS=4 MSIME_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 通过：Rust workspace、Android target、WASM、Android Java、macOS、共享 Apple bridge 和 pipe-only 配置（按环境跳过）检查完成。

## Consequences

九键热刷新保留读音字符串槽位，减少主读音列表的重复向量申请；空结果仍清空已有缓冲，不增加常驻状态。独立的 `spellings_for` 测试入口仍返回拥有型向量。这里声明的是 Rust 容器容量复用，不推导词典查询、RSS 或端到端设备延迟收益。

