# Agent Note: 词条权重读取与插入借用键规划

Status: implemented

## Problem

`find_weight` 和 `insert_word` 为表名与简拼调用 `split_segments`，复制每个音节并创建向量，最终查询和插入仍使用调用方原键。词条学习路径因此为只读规划支付临时所有权分配。

## Decision

私有帮助函数只统计原键的撇号段数与首字节，用既有 `quanpin_table` 生成表名。插入借用原键切片提取每个非空段的首个 Unicode 字符，简拼沿用按段数预留的策略。保持空段计数、首段拒绝规则、非完整音节可达性、原键/词值、SQL、权重和错误顺序；不复用会校验完整音节的 `complete_key_table`。

## 既有笔记审计

[完整键借用规划](../../implemented/testing/2026-10-10-pinyin-borrowed-key-plan.md)部分重叠于消除拆分复制，但其完整音节过滤不适用于词条入口，保留并互链。[学习句子音节计数](../../implemented/testing/2026-10-08-learning-sentence-segment-count.md)作用于调用方上限判断，与数据库表名和简拼独立。[精确键去重](../../implemented/testing/2026-10-08-pinyin-exact-key-dedup.md)作用于查询批次，与单词条读写独立；没有完全吸收或过时提案。

## Alternatives considered

- 泛化公共 `split_segments` 与 `segments_to_jianpin` 可以复用借用迭代，但改变真正需要音节所有权的调用方及公开接口；两处词条入口没有这种需求。
- 复用 `complete_key_table` 可以共享已有表名规划，但会拒绝空段和非完整音节，改变现有读写行为。
- 复用可变拆分缓冲可以摊销容器容量，但仍复制音节，并把状态和嵌套借用引入原本无规划缓存的数据库对象。

## Verification

冻结 `f6fbf3ece1afb6853dd8a8a1a4007f8015cfacf8` 的完整旧词条方法，仅适配接收者、中文注释和格式，机械核对一致。两项真实红测试分别看到旧生产和冻结方法均为 6/7 次分配，少三次的预算失败；改后双音节权重读取 6→3 次、插入 7→4 次。

五项测试用真实 SQLite 对照完整原始存储行、三次重复插入、空键/首尾和连续空段、非法首字节、非完整音节、未发货的 `i` 初始表、Unicode 后段、7/8 与 1025 段长键、关闭数据库、缺表、只读写入、真实 SQLite 首步错误、NULL 与文本权重转换。规划保留原键、简拼、词值、权重和错误类别/文本，闭库错误仍先于键校验。

冷测量在区间外初始化线程随机状态，区间内打开数据库、查询、显式析构，仅返回 `Option<i64>`。arm64 debug 的命中、缺词和 Unicode 后段分别为 29→26 次分配；缺表和包含空段为 28→25 次，非法首字节为 25→22 次，空键保持 22 次。七组峰值均为 712 字节，返回存储和最低 signed 差值均为零；独立新进程的七行测量逐字一致。独立只读审查无阻断。

完整引擎 1600 项通过、13 项忽略，golden 31 项通过；最后仅将两处测试夹具改用数组，新模块五项及 Clippy `--all-targets -- -D warnings` 再次通过。格式、差异和笔记检查通过。

本地 `bash scripts/verify-local.sh --quick` 退出 0：Rust workspace、Android target/Java 宿主、Linux 公共组件、Linux 原生宿主及 77 项测试、macOS 原生宿主编译、共享 Apple bridge 和本机可运行的 103 项 Windows 测试通过。Wasm、HarmonyOS 真编译/设置 bundle、Windows 原生交叉构建与 pipe-only 因环境缺失跳过；macOS 桌面公共组件缺少打包资源而跳过。未作设备或系统输入法验收。

## Consequences

词条入口只按首字节与段数规划；完整音节校验仍归调用方。消除的临时分配为音节向量及每个非空段的副本，表名、简拼与 SQLite 仍按原策略拥有必要存储。借用扫描仍有遍历成本；分配测量不代表 SQLite C 堆、RSS 或宿主时延。遵循 [str::split 官方借用切片契约](https://doc.rust-lang.org/std/primitive.str.html#method.split) 与 [Rust 标准库实现](https://doc.rust-lang.org/src/core/str/iter.rs.html)，不新增依赖或不安全代码。
