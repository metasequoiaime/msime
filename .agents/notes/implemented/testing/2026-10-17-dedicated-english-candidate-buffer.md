# Agent Note: 专用英文候选行缓冲复用

Status: implemented

## Problem

专用英文模式每次刷新前会丢弃整个 `Vec<WordItem>`，英文词典查询再为每一行重新创建 `WordItem` 及其 `pinyin`、`word` 字符串。连续输入英文前缀时，候选行的字段和容量大多可以继续使用，却重复产生行对象和字符串分配。

## Decision

英文词典新增 `query_prefix_into`，在已有候选缓冲中按查询顺序覆写可复用的 `WordItem`，清空并复用其字符串；结果变短时截断，查询失败或输入无效时清空，首次命中行仍按原 `limit` 精确预留。`query_prefix` 保留原有拥有型接口，只通过新接口实现。

词典读取使用借用的 `Cow<str>`：有效 UTF-8 的 SQLite 文本直接借用当前行，非法字节仍按原 `from_utf8_lossy` 规则拥有替换后的文本。专用英文会话把结果写回自己的候选缓冲，再暂时取出执行既有候选定位和未知词回退；空输入、未知词、排序、位置和错误语义保持不变。

## 历史笔记审计

已检索[专用英文前缀借用](2026-10-08-dedicated-english-prefix-borrow.md)和[英文前缀空页延后预留](2026-10-10-english-empty-prefix-storage.md)。前者只约束查询前缀的大小写所有权，后者只约束空页的首次容器预留；本篇新增的是跨词典与会话的候选行生命周期复用，三者互链但不重复决定。

## Alternatives considered

- **继续每次创建新候选向量**：实现最简单，但热刷新必然丢弃行和字符串容量，无法保留已经付出的存储。
- **只在会话层缓存查询结果**：可以跳过相同前缀的查询，但要处理词典变更、候选位置学习和未知词回退失效，缓存契约比当前复用输出缓冲更宽。
- **让词典返回借用行**：能减少文本复制，但 SQLite 行的生命周期不能越过查询，且会把候选所有权扩散到会话；在现有 `WordItem` 结构中覆写字段能保持调用方接口不变。

## Verification

新增专用英文热刷新回归：在旧的“新建查询结果”路径和复用路径上对照分配次数、候选字段、外层指针和容量；旧实现先因指针改变而失败，复用实现通过。专用英文回归和英文词典测试通过；完整 session 测试通过 322 项。引擎库测试通过 2287 项、忽略 40 项；`cargo fmt --all -- --check`、`git diff --check`、`cargo clippy -p msime-engine --all-targets -- -D warnings`、`pnpm run verify-notes` 和 `bash scripts/verify-local.sh --quick` 均通过。quick 门禁中 Windows 本机测试 157 项通过；HarmonyOS、Linux、Windows 交叉构建和 pipe-only 因本机缺少对应工具链跳过，macOS 配置/编译与共享 Apple bridge 通过。

## Consequences

专用英文连续刷新保留候选行及其字符串容量，减少热路径分配；冷调用的 `query_prefix` 仍返回独立拥有的向量，其他英文查询调用方无需迁移。每个结果行增加字段覆写和容量判断，复用行容量不足时仍会按需增长。候选查询本身仍访问 SQLite，复用缓冲不等同于前缀结果缓存，也不改变词典排序、查询错误或未知词的显示契约。

[九键英文前缀页直接追加](2026-10-23-nine-key-english-page-append.md) 消除分组查询的临时候选页；本篇原有的查询、复用及容量契约继续保留。
