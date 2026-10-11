# Agent Note: 指定词库的导出只读取一页

Status: implemented

## Problem

通用 Host API 的 `export` 按 1000 条反复读取混合日志页，再在内存筛选词库并保留从开头到 `offset + limit` 的全部匹配项。大偏移重复打开数据库、重复扫描前缀，且持有的词条随偏移增长。其他词库仍有行时，即使指定词库已导完，也可能返回 `has_more: true`。

## Decision

引擎提供指定 `PersonalDictionaryKind` 的导出入口，在 SQL 中用 `dictionary = ?5` 筛选后应用 `LIMIT/OFFSET`，复用现有主键 `(dictionary,key,value)`，每次请求只打开一次只读连接并读取一页。通用 Host API 直接序列化该页。混合列表与快照等既有调用保留原入口，不改变分页契约。拼音导出仍包含学习权重并排除单字；无中文主词库的版本仍只允许英文行。路径验证、锁、错误映射和线程连接释放保持现有机制。

## 历史笔记审计

按个人词库、词库分页、分页导出和词库导出检索活跃笔记，命中六篇：`2026-10-09-harmony-ime-shared-sandbox` 管共享目录，`2026-10-11-wubi-reverse-lookup` 管查码，`2026-10-11-android-shuangpin-hint-profile-hint` 管提示键盘，均无关；`2026-10-08-android-local-backup`、`2026-10-09-android-backup-learning-state`、`2026-10-10-android-backup-habits-and-verified-restore` 管快照与恢复，也不持有本次按种类分页决定。没有可就地更新的归属。

## Alternatives considered

只限制 Host API 的累积向量到当前窗口，改动最少且能控制内存。但仍重复打开数据库并扫描混合词库的前缀，不能消除根因，因此将筛选下推到引擎。用可选参数和 `(?5 IS NULL OR dictionary = ?5)` 统一 SQL 可省一条查询文本，但直接等值条件更明确地利用主键前缀，因此保留混合与指定种类两条静态 SQL，共用读行与验证流程。

## Verification

Host API 合成日志回归在旧实现上失败（英文最后一页错误报告 `has_more: true`），指定词库查询后通过。该测试还覆盖排序、非零偏移、两种文本列序、空页和输入上限。引擎测试覆盖全部五种词库、学习权重、单字排除、无中文词库限制及只读错误路径。`cargo test -p msime-engine --lib user_dictionary::personal --locked` 的 8 项与 `cargo test -p msime-host-api --lib --locked` 的 472 项全部通过。新增 Host API 导出回归在 API 28 和 API 35 专用 Android AVD 上分别通过；API 28 与 API 35 的 client-core 整套单测均为 1268 项通过、10 项忽略、7 项硬链接用例按设备脚本过滤。独立代码评审无阻断项。严格 Clippy 检出既有字体测试中的常量断言，将其改为 Windows 上对实际返回值的断言，保留字体集合必须存在的验证意图。

## 合成 SQL 对照

同一 SQLite 文件含 2000 条英文、2000 条拼音和 60000 条快捷短语，导出快捷短语 `offset=50000, limit=1000`，三次中位数：旧循环 55 次查询、4.0217 秒、Python `tracemalloc` 峰值 13052830 字节；指定词库单查询 1 次、0.0280 秒、188122 字节；SQLite 虚拟机约从 14190000 步降为 465000 步。`EXPLAIN QUERY PLAN` 显示主键索引的 `SEARCH ... (dictionary=?)`。这是模拟两种分页算法的 SQL 层对照，Python 对象内存不是 Rust 分配测量，也不是完整宿主耗时。

## Consequences

收益是一次请求只打开一次数据库、只持有当前页词条，且末页状态不受其他词库影响。代价是两条静态 SQL 的共同谓词需要同步维护。`OFFSET` 仍须扫描指定词库的前缀，但只扫描一次；此次不引入游标接口或新索引，也不修改落盘格式。Android 个人词库队列有独立导出实现，此次只改变通用 Engine 日志导出。

## 最终验证补充

- 合并前 `bash scripts/verify-local.sh --quick` 重跑返回 0：Linux 原生宿主 80/80 CTest 通过，Windows native run 145 项通过、2 项按主机差异排除，macOS CMake 与 shared Apple bridge 通过；Harmony settings bundle、契约检查和 Android host Java 通过。首次 quick 的 Linux CTest 受并发机器负载出现超时，热缓存重跑后 80 项全部通过。
- JDK 21 下 Android Release APK 构建成功，大小 122143194 字节，16 KB ELF 对齐、依赖白名单、ABI 和签名校验通过。API 28 核心原生 smoke 通过（安装、资源准备、拼音上屏和退格）；API 35 冷启动前的核心、候选面板和工具栏测试通过。完整 API 35 套件在表情最近项重启测试中失败，独立重跑又遭遇 UiAutomation 启动状态错误；冷启动重试在系统窗口切换时 `uiautomator dump` 卡住，因此没有把完整套件记为通过。
- 合成 SQL 对照仍只代表 SQLite 查询层：指定词库查询利用主键索引，避免混合分页循环和累积匹配向量。
