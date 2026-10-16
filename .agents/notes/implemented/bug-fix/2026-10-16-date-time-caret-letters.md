# Agent Note: 日期时间模式允许在光标处补回关键词字母

Status: implemented

## Problem

`T` 日期时间模式在末尾接受小写关键词，也支持移动光标和删除中间字母，但光标插入路径把这个模式的所有字符都拒绝。输入 `Trq` 后移到 `r` 前、删除 `r`，无法再补回它来恢复日期候选；公历、时间、星期和农历关键词都受同一条入口规则影响。

## Decision

日期时间模式与快捷短语共用光标插入的小写字母判断，与现有末尾输入的字符集一致。关键词识别、注入时钟、候选格式和选择上屏仍由现有日期时间查询完成。

## Alternatives considered

- **关闭日期时间模式的光标移动和中间删除。** 可以消除“能删不能补”的不对称，且不必修改插入分支；但用户只能整段重输才能改错，而本地模式已经提供逐字编辑，应恢复其合法字符插入。
- **在光标处接受所有大小写字母。** 与部分临时语言模式统一更简单，但日期关键词和末尾入口只接受小写，扩大字符集会引入无法查询的关键字；复用小写规则保持两条路径一致。

## Verification

关键词恢复回归在旧实现中因 `Trq` 删除 `r` 后补回按键被拒绝而失败；修复后两项回归通过。固定时钟的真实 Session 回归覆盖公历、时间、星期、农历的十二个别名：从合法关键词中删除一个非末尾字母，在光标处补回后恢复预编辑、光标和完整生成候选，首选能上屏并退出模式。大写、撇号、数字和分号在光标处仍交还宿主且不改组合。

`cargo test -p msime-engine` 通过 2219 项单测和 31 项 golden，36 项单测按现有配置忽略；`cargo clippy -p msime-engine --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`pnpm run verify-notes` 和 `git diff --check` 通过。独立只读审查未发现问题。

预先构建共享宿主库后，`bash scripts/verify-local.sh --quick` 通过，实际执行 165 项 Windows 宿主测试，并通过契约、actionlint、Rust 工作区、Android 目标、wasm、Android 宿主 Java、macOS 原生宿主及共享 Apple 桥构建。Windows 测试另有 2 项因宿主差异排除、56 项需要 Windows 构建；Linux 缺少 Docker，Windows 缺少交叉工具链，HarmonyOS 缺少暂存依赖，macOS Tauri 外壳缺少其资源产物，相关阶段跳过。

## Consequences

本篇原先保留的“光标拒绝字符后组字不变”由[光标拒绝键的提交与透传](2026-10-16-local-caret-rejected-keys.md)部分取代；以下合法插入语法、字符集和容量边界继续成立，拒绝后的提交与退出遵循新规则。

日期时间关键词可以像其他本地模式一样逐字修正，合法小写字母不会因输入位置不同被拒绝。光标坐标包含入口 `T`，插入保留前缀并将光标推进一个字符；大写、撇号、数字和分号的插入拒绝规则、末尾非法键的原文提交与透传、日期时间格式均维持既有契约。未进行系统输入框或真机验收。

## 历史笔记检索

检索活跃笔记的日期时间、DateTime、date_time、T 模式和光标。[农历关键词](../../implemented/feature/2026-10-09-lunar-date-keywords.md)持有关键词及生成候选规则；[本地模式拒绝键透传](../../implemented/bug-fix/2026-10-15-local-mode-rejected-key-passthrough.md)持有末尾非法键的提交和透传行为；它们均继续保留，本次不翻转其决定。[表达式光标输入](../../implemented/bug-fix/2026-10-16-expression-caret-unit-letter.md)与[翻译光标分词](../../implemented/bug-fix/2026-10-16-command-caret-separator.md)覆盖相邻模式，部分重叠但无日期关键词的插入判断。行内日期候选、展示时间格式化和快捷键笔记不持有本次入口规则。
