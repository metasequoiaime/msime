# Agent Note: 全拼热行缓存直接构造最终候选

Status: implemented

## Problem

`query_database` 与 `longer_phrase_candidates` 命中原始行缓存后先 `Vec<DictRow>::clone`，再消费整页构造拥有型候选，重复申请临时行向量。单字母分支先把完整键放入两个读法字段，再替换显示读法，丢弃一次完整键副本。`expand_initial_candidates` 也先复制完整读法再替换为单字母，每个成功展开行丢弃一份完整键。模糊候选的共享前缀热页也先克隆完整 `Vec<DictRow>`，随后只消费一次。已有合成预算中普通热命中为 5 次、自动切分为 6 次、长词两行页为 8 次申请。

## Decision

用共用的私有映射消费 `IntoIterator<Item=DictRow>`。热缓存用 `rows.iter().cloned()` 惰性提供行，只复制最终候选需要的字段，不建立完整的中间向量；冷页仍沿用原缓存页克隆和拥有型迭代。单字母分支直接以原匹配码构造 `pinyin`，完整键直接交给 `canonical_pinyin`，不经过被替换字段。`expand_initial_candidates` 同样直接消费 SQLite 页，以单字母构造最终显示读法。原 `QuanpinDictionary::query_initial` 包装只由展开调用，没有从 crate 导出；优化后移除该闲置内部入口。`PinyinDatabase::query_initial`、九键使用的简拼查询与 `initial_items` 的完整读法语义保持原样。模糊行缓存辅助函数直接向结果追加候选，热页传入惰性克隆迭代，冷页仍克隆缓存后消费原 SQLite 页；首行取得拥有型匹配码（已有拥有值直接接管，借用前缀按[延迟拥有](2026-10-14-quanpin-fuzzy-typed-prefix-borrow.md)复制），其余行复制该读法，保留原容量与路径预算。最终候选继续拥有全部字符串；普通切分、长词与初始字母保持原键、顺序和默认元数据。

## Existing decisions

按行缓存、`query_database`、`longer_phrase_candidates`、补全键检索活跃树。没有同一行投影决定。[词网格缓存借用键](../../implemented/testing/2026-10-10-pinyin-lattice-span-key-borrow.md)、[模糊缓存借用键](../../implemented/testing/2026-10-11-fuzzy-row-cache-key-borrow.md)只持有另一组缓存的键与失效边界，本篇不改变它们。[长词按页预留](../../implemented/testing/2026-10-10-pinyin-longer-rows-reserve.md)、[聚合页复用](../../implemented/testing/2026-10-10-pinyin-aggregate-page-reuse.md)部分重叠于冷页容量，保留各自 SQL 页决定，不吸收或归档。[会话行缓冲](../../implemented/testing/2026-10-08-session-candidate-buffer-reuse.md)处理更外层重复展示行，与本篇从原始行到输出候选的拥有边界不同。首座优先级与长词每页门槛笔记继续持有排序规则，其余插件与日文/注音借用笔记无关。

首字母行查找的两字节临时键由[局部键借用](2026-10-14-quanpin-initial-row-key-stack.md)处理，本篇继续持有行迭代和候选字段的所有权策略。

模糊前缀的无消费者读法由[延迟拥有](2026-10-14-quanpin-fuzzy-typed-prefix-borrow.md)处理，本篇保留现有行索引、容量与投影边界。

字典入口的输入音节副本由[借用输入音节](2026-10-14-fuzzy-input-segments-borrow.md)处理；本篇冻结完整查询与历史测量保持，次数预算在计量外单独补偿非空输入音节的复制节省。

## Alternatives considered

- 冷页直接转移进缓存、再借用生成输出：还可以省去冷缓存行向量副本；不选，因为 SQLite 页可能按有限上限预留，原缓存克隆只有真实行数容量，直接接管会把大提示带进 512 槽常驻缓存。
- 模糊行辅助函数返回借用页：可以让投影留在调用方；不选，因为冷页还要同时写入缓存并保留原容量策略，借用返回会引入额外缓存查找或冷页字段复制，直接追加能在现有借用范围内保持同一套投影。
- 返回候选视图或共享引用：可以避免所有字段克隆；不选，因为会跨越词典、会话和公开输出所有权，候选需要独立修改、学习和在失效后继续拥有内容。
- 只保留热页克隆：所有路径的映射代码最短；不选，因为中间页只用于随后转换，流式迭代有同样的顺序与拥有语义。

## Consequences

非空热页省去一次临时行向量申请；单字母每行再省去一次完整键副本。最终候选仍拥有独立字段，普通切分、长词、首字母顺序与元数据保持等价，缓存仍按 FIFO 读取和原策略失效。冷页克隆与最终候选字段、容器分配仍存在，不把预留的 SQL 页容量转进常驻缓存。

惰性迭代保持准确长度提示，返回与缓存的字符串、向量容量与旧实现相同。借用只持续本次映射，不增加会话状态、依赖、公开 API，也不改变 SQL 查询、文件系统、权限或 Android 资源。

## Verification

### 模糊共享前缀行投影

固定 `db0b15772189fc9fe7ea7afadd2f3d64e9d89cc6` 的完整模糊候选与行缓存函数为对照，只改名、转接旧入口并移除注释。两个申请预算在旧生产代码上真实失败：两个热前缀页的完整查询为 41 次，目标 39 次；不同输入共享一个热前缀为 151 次，目标 150 次。冷页、整条答案缓存、字段/顺序/容量、空页与独立所有权对照在旧代码通过。

改后三个新增测试全部通过，独立复核再次通过，无阻断或重要问题。两热页查询在每表 1/8/24/64/129 行夹具上的申请次数分别为 41→39、126→124、318→316、798→796、1,566→1,564；129 行夹具的单页仍受 128 行上限约束。真实不同输入共享前缀为 151→150，完整冷查询与整条答案缓存命中的申请次数均未增加。既有单非空热页预算从 35 收紧到 34，定向复跑通过。完整输出和缓存容量保持等价，最终候选与冷页副本仍拥有字符串。未测量本项 CPU 时间、峰值内存或设备输入延迟。

- `cargo test -p msime-engine --locked`：1,998 项单测通过、15 项忽略；31 项 golden 通过，文档测试 0 项。
- `cargo clippy -p msime-engine --locked --all-targets -- -D warnings`：通过。
- `cargo fmt --all -- --check`、`git diff --check`、`pnpm run verify-notes`：通过，852 篇活跃笔记与 2 篇归档封印完成校验。
- `CARGO_BUILD_JOBS=6 bash scripts/verify-local.sh --quick`：退出 0。仓库契约、actionlint、Rust workspace、Android/wasm target、Android Java、macOS 原生宿主与共享 Apple bridge 编译通过。Linux 外壳/原生因无 Docker，Windows 本机/交叉与 pipe-only 因无工具链，HarmonyOS 真打包因缺少 hvigor/产物/依赖而跳过；macOS workspace 的 `msime-desktop` 因原生资源 bundle 未预先生成跳过。未运行真实宿主输入或设备验收。

### 首字母展开

固定 `db43f05f0f1c067f65122376ebc0fc7dcf2376e9` 的完整旧展开及旧首字母包装作为测试基线，仅改名、转接旧入口并移除注释。新增测试覆盖 25/32/128 行真实展开的字段、顺序、容量和逐行分配预算，混合来源候选的插入位置与全部元数据，行缓存/系列槽、结果修改和清空后的独立所有权，空码/多字母/缺词/24 行以内等拒绝边界，以及简拼查询保持完整读法。

分配预算在旧生产的 25 行展开上真实失败（267 次，行投影目标 242 次）。每个成功展开行恰好省去一次完整键复制；最终候选、行缓存与系列槽的拥有型副本继续存在。[解析缓存按需投影](2026-10-14-quanpin-resolution-cache-projection.md)在同一热展开另省两次字符串复制，当前定向复跑的完整展开为 25 行 267→240 次、32 行 337→303 次、128 行 1,299→1,169 次，预算为每行省一次加解析省两次。既有 `caret_prefix_expands_its_own_initial_list` 的 30 行展开精确预算经行投影从 353 收紧到 323，解析投影再收紧到 321，定向复跑通过。三个行投影测试与两处预算调整均通过独立复核，无阻断或重要问题。未测量此次完整展开的堆峰值或发布构建延迟，不把先前纯投影计时算作本次整条展开的改善。

- `cargo test -p msime-engine --locked`：1,995 项单测通过、15 项忽略；31 项 golden 通过，文档测试 0 项。
- `cargo clippy -p msime-engine --locked --all-targets -- -D warnings`：通过。
- `cargo fmt --all -- --check`、`git diff --check`、`pnpm run verify-notes`：通过，851 篇活跃笔记与 2 篇归档封印完成校验。
- `CARGO_BUILD_JOBS=6 bash scripts/verify-local.sh --quick`：退出 0。仓库契约、actionlint、Rust workspace、Android target、wasm target、Android Java、macOS 原生配置/编译和 shared Apple bridge 编译通过。macOS workspace 的 `msime-desktop` 因原生资源 bundle 未预先生成而跳过；Linux 外壳/原生因无 Docker，Windows 本机/交叉与 pipe-only 因无相应工具链，HarmonyOS 真打包因缺少 hvigor/产物/依赖而跳过。未运行真实宿主输入或设备验收。

### 热行缓存投影

三个既有热查询预算在旧实现上实际失败，改后分别为 4、5、7 次申请。新增固定旧函数与当前真实 SQLite 查询的对照覆盖冷/热、缺词/表、首字母 24 行上限、别名、不完整/无效/空输入、同权及负/宽权重，比较全部字段、顺序、返回与缓存容量，并检查 FIFO 年龄、清空与独立结果生命周期。四项新增测试中三项在旧实现失败，改后全部通过；独立复核再次运行四项测试通过，无阻断或重要问题。

线程本地计数器测量预热真实查询的 Rust 逻辑请求字节，最小净值均为 0，返回逻辑存储完全相同：

| 输入/行数 | 申请次数（旧→新） | 峰值字节（旧→新） | 返回字节 |
| --- | --- | --- | --- |
| 首字母 `n` / 24 | 99→74 | 5,040→3,671 | 3,669 |
| 显式 `ni'hao` / 35 | 107→106 | 7,642→5,682 | 5,682 |
| 自动切分 / 35 | 108→107 | 7,648→5,688 | 5,682 |

独立 `rustc -O` 程序机械抽取生产字段、构造函数与映射代码，使用合成读法及 Unicode 词语，先验证输出和容量，再测 15 轮交替配对线程 CPU 时间（包含输出析构）。普通页测 0/1/2/8/24/32/128/1024/4096 行，首字母只测 0/1/2/8/24 行。非空普通页旧/新 CPU 配对比中位数为 1.028–1.273；非空首字母为 1.400–1.562，24 行分别为 1.046 和 1.400。该测量排除缓存查找、SQL、拼接键、失效与完整输入路径；堆测量排除 SQLite C 堆和 RSS。这些数据只说明投影阶段的成本，不代表真实宿主输入延迟，计时不作测试门禁。

- `cargo test -p msime-engine --locked quanpin::dictionary -- --nocapture`：64 项通过。
- `cargo test -p msime-engine --locked`：1,987 项单测通过、15 项忽略；31 项 golden 通过，文档测试 0 项。
- `cargo clippy -p msime-engine --locked --all-targets -- -D warnings`：通过。
- `cargo fmt --all -- --check`、`git diff --check`、`pnpm run verify-notes`：通过，849 篇活跃笔记与 2 篇归档封印完成校验。
- `CARGO_BUILD_JOBS=6 bash scripts/verify-local.sh --quick`：退出 0。仓库契约、actionlint、Rust workspace、Android target、wasm target、Android Java、macOS 原生配置/编译和 shared Apple bridge 编译通过。macOS workspace 的 `msime-desktop` 因原生资源 bundle 未预先生成而跳过；Linux 外壳/原生因无 Docker，Windows 本机/交叉与 pipe-only 因无相应工具链，HarmonyOS 真打包因缺少 hvigor/产物/依赖而跳过。该门禁没有运行真实宿主输入链路，也不声称设备验收。

外层九键系列答案由[读法筛选后复制](2026-10-16-nine-key-filter-before-clone.md)减少随后拒绝行的字段副本；本篇继续持有原始行到候选的映射和冷缓存容量决定。
