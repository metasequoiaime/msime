# Agent Note: Linux 候选主题字段类型错误时降级

Status: implemented

## Problem

IBus 与 Fcitx5 共用的 `candidate_dark_theme`、`surface_dark_theme` 和 `candidate_layout_id` 已对非对象偏好文档做回退，但对象中的 `candidate_theme`、`theme`、`toolbar_theme` 或 `candidate_layout` 如果是数字、数组或 null，仍会经 `nlohmann::json::value()` 抛出类型异常。手工编辑或损坏的 `runtime-options.json` 可因此中断候选主题和浮层外观的解析。

## Decision

三个入口改用 `find()` 和 `is_string()` 检查字段。错误类型与字段缺失同义：候选主题跟随全局模式，全局模式跟随系统明暗，浮层自身模式跟随全局模式，候选布局取纵向。合法字符串和未知字符串原有的判定保持不变。

## Alternatives considered

- 在调用点捕获 `json::type_error`：IBus 和 Fcitx5 有多个调用点，容易漏掉；此处是两个宿主共用的解析边界。
- 在读取运行配置时拒绝整份文档：仅一个外观字段损坏时会让输入法配置整体不可用，超出该字段的影响范围。

## Consequences

单个错误类型的外观字段不再中断这些 helper；外观使用对应缺省值。修复范围仅限候选主题、浮层模式与候选布局解析，不改变其它配置字段的校验策略。

## Verification

`candidate_palette.cpp` 新增的错误类型用例在修复前以 `json.exception.type_error.302` 失败；修复后定向编译运行通过。`bash platforms/linux/build-container.sh` 退出 0，JUnit 报告有 83 项测试、0 失败、0 跳过，并编译了 IBus/Fcitx5 原生宿主。`bash scripts/verify-local.sh --quick` 退出 0，其中 Linux 原生宿主再次通过 83 项测试。
