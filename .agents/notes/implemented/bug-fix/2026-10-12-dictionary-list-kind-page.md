# Agent Note: 指定词库的列表分页先筛选

Status: implemented

## Problem

通用 Host API 的 `list` 在请求 `kind` 但没有 `query` 时，先从混合个人词库读取每页 500 条，再在 Host 侧筛选。词库按字典名排序时，前一个词库超过 `SCAN_BUDGET` 就会让后一个词库的第一页返回空 `entries` 并错误保留 `has_more: true`；扫描预算也让大词库的列表结果依赖其他词库的行数。

## Decision

引擎提供指定 `PersonalDictionaryKind` 的用户列表入口，在 SQLite 查询中用 `dictionary = ?` 先筛选，再按既有主键 `(dictionary, key, value)` 应用 `LIMIT/OFFSET`。Host API 只在 `prefix` 为空且带 `kind` 时走该入口；带代码前缀的列表继续使用原有语义，因为它需要在用户词库中扫描并做前缀匹配。列表查询保留拼音单字，不附加学习拼音权重；未筛选的列表和导出入口继续使用各自原有规则。导出的相同 SQL 下推决定见[指定词库的导出只读取一页](2026-10-11-dictionary-export-kind-page.md)。

## Alternatives considered

只把 Host 侧的扫描窗口限制为当前页，改动更小，但仍会重复打开数据库、扫描混合词库并受 `SCAN_BUDGET` 影响，不能修复跨词库截断。让 SQL 通过可选参数统一混合和指定词库查询可以减少一条查询文本，但 `dictionary = ?` 的静态条件更明确地使用主键前缀，因此保留两条静态 SQL 并共用绑定和行转换逻辑。

## Verification

新增合成 Host API 回归：20,001 条 English 后放 1 条 Wubi，旧实现返回空页和 `has_more: true`，修复后返回唯一 Wubi 条目且 `has_more: false`。`cargo test -p msime-engine --lib user_dictionary::personal --locked` 的 8 项、`cargo test -p msime-host-api --lib --locked` 的 512 项和严格 Clippy 全部通过；`cargo fmt --all -- --check` 与 `git diff --check` 通过。`bash platforms/linux/build-container.sh` 热缓存重跑后 Linux native CTest 80/80 通过；`bash scripts/verify-local.sh --quick` 的其余阶段通过，首次 Linux 容器运行有 2 个启动时序测试超时，热缓存重跑已通过。

## Consequences

指定词库且无前缀的列表只扫描目标词库的索引前缀，分页结果不再被其他词库或 20,000 行预算截断，并且每次请求只持有当前页。带前缀的查询仍承担原有有界扫描和筛选成本；两条静态 SQL 的共同谓词需要同步维护。没有修改个人词库落盘格式、排序规则、错误映射或 Android 队列存储实现。
