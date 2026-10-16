# Agent Note: 全拼替代合并直接消费热主候选行

Status: implemented

## Problem

`merge_alternatives` 在已有替代词时再次取得主切分候选。热行缓存整页克隆成临时 `Vec<WordItem>`，随后全部移入合并结果，临时页没有后续消费者。

## Decision

合并核心接收精确长度的拥有行迭代器。原有公开拥有页入口继续移交向量；词典热命中传入缓存切片的逐行克隆迭代器，冷路径保持原查询、缓存副本及拥有页移交。使用 `Cow` 区分两种所有权，主查询仍先于替代行字段构造，避免提前扩大临时字段的存活范围。提升阈值只取迭代器首行，重复词检测在主行移入合并向量后进行，不额外克隆字段。

## Historical audit

[前缀行流](../../implemented/testing/2026-10-16-quanpin-prefix-row-stream.md)部分重叠，处理较短前缀的临时页；[提升标记](../../implemented/testing/2026-10-15-quanpin-alternative-promotion-marker.md)部分重叠，保留唯一词地址和重复词回退；[小页去重](../../implemented/testing/2026-10-07-series-alternative-dedup.md)部分重叠，去重门槛不变。检索 `primary_full`、`merge_alternatives` 和替代切分，没有相同方案。架构仍在纯 Rust 引擎内。[标准库接口](https://doc.rust-lang.org/std/iter/trait.ExactSizeIterator.html)与 [Cloned 实现](https://doc.rust-lang.org/src/core/iter/adapters/cloned.rs.html)确认切片克隆迭代器长度可预知。

## Alternatives considered

- 从系列结果取回主候选：能够连字段深复制也省掉；不选，因为完整组已混有长词、整句和同词不同元数据，按索引抽取不成立，按值匹配还会增加扫描和所有权契约。
- 保留整页克隆：入口最简单；不选，因为可用已有借用缓存与精确长度迭代器消除临时页，不改变最终拥有字段。

## Verification

新增预算先在旧生产实现失败。专项非空主候选冷路径 46→46、热路径 22→21；空主候选冷路径 27→27、热路径 15→15。独立冻结旧词典合并、排序及去重；40 组宽度、同词、极值权重形状对照所有字段、顺序和容量，同时检查拥有页入口、无替代词零消费、缓存字段及 FIFO 年龄。最终版本完整测试通过 2178 项引擎测试、31 项 golden、294 项 runtime 测试，33 项既有忽略；全目标 Clippy、release 构建与笔记校验通过。全拼 60 例及九键 81 例完整质量 JSON 与前缀行流版本逐项相同。quick 门禁通过，包括 Android/WASM/Java、macOS 原生宿主和 Apple bridge 编译。Linux 容器、完整 Windows 交叉及 HarmonyOS 真打包因缺前置环境跳过。延迟采样启动前检测到其他工作树的构建，预检退出 3，未启动计时；本片没有新的有效延迟样本。

## Consequences

字段仍须深复制，不能声称消除了主候选克隆或达到 16 ms。重复检测的时机调整必须保留首条替代行去重后回退语义；无替代词仍立即原样返回。
