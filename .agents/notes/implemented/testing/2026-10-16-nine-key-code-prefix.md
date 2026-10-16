# Agent Note: 九键候选前缀校验不物化中间组码

Status: implemented

## Problem

固定 `c2d2d0c78c` 的九键二十秒、1 ms 采样中，候选刷新本体、分配释放和候选复制仍在实际按键调用树。`NineKeySession::refresh` 为每条词典候选调用 `grid.encode(matched)`，但编码字符串只用于非空判定、与输入的双向前缀判定，以及写回输入前缀的长度。大量随后被去重或过滤的行也承担这次分配。采样有观察扰动，不作为延迟预算成绩。

## Decision

增加私有 `matching_code_prefix_len`，返回与输入相容时的覆盖字节数。九键、十四键沿用 `encode` 的小写 ASCII 字节过滤和键盘表映射，逐字节比较；发现冲突立即失败，输入已被完整覆盖时立即返回，读音没有任何可编码字母时仍失败。九键双拼继续调用原完整编码器，保留非法音节使整个编码失效的语义。刷新只用返回长度写回原有 `candidate.pinyin` 缓冲。

## Historical audit

检索活跃笔记中的 `reading_code_len`、`grid.encode`、编码前缀和组码前缀。[双拼模糊候选重标](../../implemented/testing/2026-10-15-shuangpin-fuzzy-relabel-borrow.md)部分相关但不重叠：它处理 26 键双拼编码写回，本片处理触屏网格的组码相容判定。[九键查询去重](../../implemented/testing/2026-10-08-nine-key-query-dedup.md)、[九键候选去重](../../implemented/testing/2026-10-08-nine-key-candidate-dedup.md)继续约束查询和排序，发生在不同位置。[中间路径后继索引](../../implemented/testing/2026-10-16-nine-key-path-chain.md)只影响切分路径。模型 token 编码、五笔反查、emoji 目录查询均无关；没有吸收或过时提案。

同仓库 `KeyGrid::reading_code_len` 已使用相同的 ASCII 字节过滤；[Rust str::bytes](https://doc.rust-lang.org/std/primitive.str.html#method.bytes)按 UTF-8 字节遍历，不作大小写或 Unicode 规范化。本片保持 `encode` 原本只接收小写 ASCII 的规则。

## Alternatives considered

- 跨候选复用一个编码字符串：不改变编码器、容易复用，但仍要完整扫描、写入随后丢弃的编码；相容判定只需要共同前缀。
- 九键双拼也改成流式短路：可以进一步减少分配，但后续非法音节会使原完整编码失效，短路前必须另做全读音校验；本片保留原编码器。
- 为所有读音缓存组码：重复读音可共享，但增加常驻存储和缓存寿命，直接比较无需新状态。

## Verification

### 行为与分配

先把原完整编码表达式移入生产 helper 并接入刷新，再运行回归：九键 `ni'hao` 仍有一次中间编码申请，零分配断言真实失败；另外两项通过。替换成字节比较后，三项测试全部通过。行为对照冻结 `c2d2d0c78c` 的编码、非空、双向前缀和覆盖长度规则，覆盖五种网格、全部完整音节、声母、空串、分隔符、大小写、Unicode、非法尾段和确定随机输入。九键、十四键申请为零；三个双拼方案申请不增加。

### 完整判定配对预算

release 每形状 40 对、交替先后，旧臂校准至少 10 ms，计入编码构造、判定和析构。正形状预算 `< 0.90`，空读音控制 `< 1.10`，双拼控制 `< 1.05`；运行前写定，所有形状收集后统一断言。40 形状全部通过，测试用时 44.40 秒。运行前后没有观察到并行编译或其他评测进程。下表是每形状新／旧耗时比的配对中位数：

| 形状 | 九键 | 十四键 | Migala2 | Migala3 | Gushi2Low |
| --- | ---: | ---: | ---: | ---: | ---: |
| full | 0.2364 | 0.2272 | 1.0023 | 1.0032 | 1.0018 |
| typed_prefix | 0.0265 | 0.0265 | 1.0001 | 1.0017 | 1.0002 |
| code_prefix | 0.1318 | 0.1089 | 1.0162 | 1.0158 | 1.0157 |
| reject_first | 0.0206 | 0.0206 | 1.0044 | 1.0027 | 1.0000 |
| reject_last | 0.2203 | 0.2201 | 1.0057 | 1.0069 | 1.0062 |
| ignored_bytes | 0.2657 | 0.2638 | 1.0062 | 1.0060 | 1.0060 |
| empty_reading | 0.8509 | 0.8455 | 1.0131 | 1.0132 | 1.0136 |
| empty_digits | 0.0207 | 0.0207 | 1.0030 | 1.0029 | 1.0014 |

### 集成与质量

- 引擎含句子模型：2166 passed / 33 ignored；golden 31 passed；运行时 294 passed。
- `cargo clippy -p msime-engine -p msime-input-runtime --all-targets --locked -- -D warnings` 通过；release `convert_eval` 与 `rerank_latency` 构建通过。
- 全拼 60 例、九键 81 例的完整质量 JSON 与上一片完全相同，包括来源桶和失败样例。
- `cargo fmt --all --check`、`pnpm run verify-notes`、`git diff --check` 与本地 quick 通过。quick 覆盖 Rust 工作区、Android 目标与 Java 宿主、WASM、macOS 原生宿主、共享 Apple bridge；Windows 本机套件 151 项通过。Linux 容器检查、Windows 完整交叉构建、Harmony ArkTS 构建因工具链或暂存资源缺失跳过，不作为设备验收。首次 quick 仅因根依赖符号链接触发 pnpm 工作区依赖不完整而失败；移除链接并在本 worktree `pnpm install --frozen-lockfile --offline` 后，设置页检查及完整 quick 重跑通过。
- 独立审查没有阻塞问题；审查覆盖空编码、共同前缀覆盖长度、双拼非法尾段、刷新写回及计时公平性。

### 固定二进制逐键延迟

旧臂源码为 `c2d2d0c78cf6434a67c89eb6ca3af676ea43060c`。旧二进制 SHA-256 `6bc1fa614008d7265c2082c3b10b2b9155c2f958feee129b68071ad3c2dd9f50`，新二进制 `a126af850cfcd9d4fefa6d485e76492899fd72674d0f37eb5132bbadd5b5f21e`。句子模型 SHA-256 `86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1`。

所有构建、测试结束后串行旧／新／新／旧，每轮 81 例、1791 键、5 预热；各轮退出 1 均为明确的 16 ms 预算失败，未出现资源或执行错误。运行中检查未观察到其他编译或评测进程。

| 轮次 | 引擎 p95 / ms | 模型 p95 / ms | 超预算键 |
| --- | ---: | ---: | ---: |
| 旧 1 | 6.85 | 17.09 | 111 |
| 新 1 | 6.77 | 17.39 | 115 |
| 新 2 | 6.51 | 16.79 | 105 |
| 旧 2 | 7.37 | 17.33 | 128 |

新旧模型 p95 区间重叠，四轮不足以证明端到端加速，也不能据此排除小幅回退。局部判定预算通过只证明这条校验路径更省；九键 16 ms 仍不达标。

## Consequences

九键、十四键候选相容判定不再拥有临时编码，冲突或输入已覆盖时可以提前结束。新增私有 helper 和对照测试，不改变公开编码器、候选顺序、缓存、依赖或内存安全边界。双拼完整校验保留原成本。

短路必须保持空编码与双拼非法尾段语义。返回长度只用于已有 ASCII 组码输入切片。局部收益不能推断端到端改善；模型矩阵计算仍是主要热点，九键 16 ms 目标保持 active。
