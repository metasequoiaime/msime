# Agent Note: Gboard v1 文本词库复用共享导入链路

Status: implemented

## Problem

#8822 的兼容 Gboard 文本是编码、词、语言三列。现有 TSV 导入把第三列当成整数词频，导致 `zh-CN` 行全部失败；用户需要另找转换工具。

## Decision

共享解析器增加内部 Gboard v1 格式。文本入口在整份文件上识别 v1 文件头，或第一条非注释、非空行的三列及明确语言标签；命名词库的 `txt` 与桌面、iOS 的标准 TSV 入口复用识别。先判格式再解析，不标记为 `swapped` 列序翻转；各行仍受编码、词长、控制字符和数量约束。第三列仅作语言元数据，权重一律 10000。分批入口把识别结果固定为 `gboard`，避免后续批次失去文件头。

无头识别接受两字母语言以及带连字符的两或三字母语言标签，后续子标签为 1–8 位 ASCII 字母数字。单独三字母标签仅在有 v1 文件头时作为元数据接受，避免把 `bad`、`NaN` 等坏词频当作格式提示。空语言的无头文件保持已有 Windows TSV 的默认权重语义。不猜 v2 和 ZIP；遇到声明的其他版本不按 v1 自动识别。

核对 Google Mozc 实际默认分支 `master` 的固定提交 [`0136c35782536cb43d710575c311cd4185982738`](https://github.com/google/mozc/blob/0136c35782536cb43d710575c311cd4185982738/src/client/user_dictionary/user_dictionary_importer.cc)：`GBOARD_V1` 的第三列是 locale，文件头用于识别 v1。这里只研究格式，不引入上游实现或词条。

## Existing notes audit

- [Rime 百分比权重](../../implemented/bug-fix/2026-10-18-rime-percentage-weight.md)：部分重叠；Rime 百分比和标准 TSV 整数词频保持原规则，Gboard 是独立格式识别。
- [细胞词库导入](../../implemented/feature/2026-10-13-scel-import.md)：部分重叠；共用后续导入与默认权重，不改二进制转换。
- [失败排序](../../implemented/bug-fix/2026-10-12-dictionary-import-failure-order.md)：部分重叠；保留文件行号和最早五条失败。
- 其余检索命中是键盘、剪贴板、触控功能，与词库文件格式无关。没有完全吸收或过时提案。

## Alternatives considered

- 直接放宽 Windows TSV 第三列：改动最小，也无需判别整份文件。但会混淆词频与语言，分批时难以保证统一语义；使用独立格式承载元数据规则。
- 所有平台新增 Gboard 下拉项：用户可以明确选择格式，也能消除无头三字母标签的歧义。但当前文本入口已经自动识别列序；新增多个原生入口会让同一份 txt 再要求用户选格式。复用现有入口，显式 `gboard` 仅用于共享层固定批次格式。

## Verification

- 词典相关测试 105 项通过，1 项依赖外部样本的既有测试跳过；覆盖无头 `zh-CN`、BOM/CRLF/v1 头、空语言和两字母语言，以及默认权重。
- 回归覆盖非法编码、过长词、列数错误仍逐行失败，行号与数量限制保持。
- 集合回归确认词条送达个人词库队列，来源保持既有 `txt` 规范化后的 `standard`；桌面分批测试 19 项通过，包含 2500 行 Gboard 文件。Host API 另确认连续拼音规范化为完整音节并保留引擎拒绝行号。
- 远端运行词典与分批测试、clippy 和 quick，本机运行 rustfmt 与笔记校验，避免在不足 50 GiB 的磁盘上新建构建树。远端 PATH 将 `/opt/homebrew/bin` 放在前面，使用 Python 3.14；系统 Python 3.9 不能运行仓库的部分契约检查。验证临时文件放在构建盘的 `target/tmp`，避免把空间紧张的系统 `/var/folders` 写满；`PYTHON_CPU_COUNT=4` 限制宿主 C++ 检查的并行编译数。

## Consequences

用户无需转换即可从既有文本入口导入 Gboard v1，且不新增持久化格式、宿主控件或依赖。代价是无头文件第一条数据行无法指示语言时不推测整个文件。语言元数据不存储也不用于筛选，用户仍选择目标词库种类；编码为空的 Gboard 自学习词无法导入。v2、ZIP 和恢复 locale 不在此次范围内。
