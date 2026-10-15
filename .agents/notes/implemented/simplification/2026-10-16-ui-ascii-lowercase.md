# Agent Note: 共享 UI 的 ASCII 小写转换

Status: implemented

## Problem

应用例外的标识规范化与去重在 `app-input-mode-rules.ts` 定义 `asciiLowercase`；词库编码前缀匹配在 `dictionary-errors.ts` 另定义局部 `fold`。两份实现都只转换 `A` 到 `Z`，其他字符原样保留。整串 `toLowerCase()` 会改变非 ASCII 标识和词库查询的匹配范围。

## Decision

`asciiLowercase` 位于现有 `core/text.ts`，两个调用方直接导入。路径、引号和首尾空白的处理仍由应用规则负责；拼音的空格与撇号处理、非拼音词条的键首尾空白处理仍由词库匹配负责。公共方法只转换 ASCII 大写，不扩展包入口 API。

历史检索中，[Windows 输入模式](../feature/2026-10-10-windows-input-mode-parity.md)持有应用规则只折叠 ASCII 的约束，本片复用其约束并互链；Windows 社区设置笔记持有原生社区页的字素与字节数规则，保持独立。引擎的 ASCII 借用和字节比较笔记属于 Rust 查询与分配策略，与此 TypeScript 方法无关。

## Alternatives considered

- **保留两个局部方法。** 每个调用方都能就地看懂转换规则，方法很短；但相同的 ASCII 边界仍要维护两次，后续修改可能只改一端。
- **用整串 `toLowerCase()`。** 标准库调用更直接，也能覆盖更多文字的大小写；但本机应用规则与 Rust 使用的 ASCII 判重不再一致，词库查询也会扩大匹配范围，所以保留原有转换方式。

## Verification

公共方法的五项测试先因缺失导出失败，迁移前的十一项词库测试通过；实现与迁移后，公共文本、词库前缀、应用规则和词库管理共五个文件、42 项测试通过。测试覆盖完整 ASCII 大写范围、相邻符号、空字符串、非 ASCII 保留、拼音分隔符、前缀、键首尾空白，以及非 ASCII 应用标识仍可区分。桌面类型检查与构建、Harmony 类型检查、修改范围的 lint、格式和差异检查通过；笔记与 quick 门禁覆盖提交前的仓库约束。

## Consequences

两处完全相同的小写转换由同一方法维护，各自的规范化顺序仍由原调用方负责。代价是词库匹配增加对已有公共文本模块的引用。公共方法名称中的 `ascii` 是约束，后续消费者需要完整 Unicode 转换时不能扩展此方法的语义。
