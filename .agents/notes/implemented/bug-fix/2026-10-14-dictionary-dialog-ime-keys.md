# Agent Note: 词库编辑弹窗让组词回车和 Esc 留在输入法里

Status: implemented

## Problem

鸿蒙手机的共享词库页用 `FormDialog` 承载新建词库和添加词条。对话框把输入框的任何回车当成保存、任何 Esc 当成关闭，确认候选会保存未完成的字段，取消候选会关掉弹窗并丢掉草稿。

## Decision

在共用对话框的键盘入口检查 `nativeEvent.isComposing` 和 `keyCode === 229`，组合事件不保存、不关闭，也不取消默认行为。独立回车和 Esc 保留现有保存与关闭功能。

新建词库与添加词条分别由 `packages/ui/src/dictionary/dictionary-edit-dialogs.tsx` 的 `DictionaryNameDialog`、`DictionaryWordDialog` 承载；两者共用私有 `FormDialog` 与 `DialogField`。字段草稿、校验、规范化和提交中状态归对话框，宿主请求、成功后关闭与页面导航仍由调用方通过回调处理。组件不依赖设置表单上下文，包入口不增加公开符号。

## Alternatives considered

- 换成原生 `<form>` 可让浏览器管理提交，但共享设置页自身已经是表单，嵌套表单会使 WebView 重新提交整个页面；继续使用 `div` 和明确的快捷键入口。
- 只判断 `isComposing` 更简洁，但组词结束事件可能早于最后一次键盘事件，需要 `keyCode=229` 保护结束边界，依据 [MDN 键盘事件说明](https://developer.mozilla.org/en-US/docs/Web/API/Element/keydown_event)。
- 对话框继续内嵌词库设置页可以少一个模块，但新建词库、内置词库与命名词库添加词条的入口无法脱离宿主组合；把两种表单及其共用骨架移到词库目录，继续复用现有输入与样式组件，不让通用弹层承担领域校验或宿主调用。

## Testing

新建词库和添加词条各自覆盖回车、Esc 的两类组合标记。旧实现上 8 个用例全部失败，分别确认提前调用保存接口或关闭弹窗；修复后不调用存储接口，弹窗和字段保留，独立普通快捷键继续工作。词库页、词条和词库动作三个测试文件共 25 项通过；前端类型检查与构建、改动文件 lint 和格式检查通过。

`pnpm run verify-notes`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 通过。quick 的 Rust 工作区、Android 目标和 Apple 桥接编译通过；Linux Docker、Windows MinGW 及 HarmonyOS 原生工具链未配置，相关阶段跳过。

对话框抽取前，词库页、弹层样式和词库纯校验三文件 50 项通过。独立对话框测试先因缺少目标模块失败，接入后四文件 54 项通过；新增覆盖独立嵌入表单、回调更新时保留草稿、字段校验与规范化，以及两种表单提交期间阻止重复保存和关闭、完成后恢复交互。原有页面用例继续覆盖两类组词标记的回车与 Esc。四个组件声明排除导出、改名及空白后与基点一致，弹层源码扫描能从新路径解析既有厚玻璃样式。桌面类型检查与构建、Harmony 类型检查、变更文件 lint、格式、笔记和差异空白检查通过，独立审查未发现问题。本地 `bash scripts/verify-local.sh --quick` 通过，编译了 Rust workspace、Android Rust 目标、Linux 设置外壳和共享 Apple bridge；主机桌面 Rust 包缺少 macOS bundle、Harmony 原生 staging 和 Windows 工具链未准备，对应阶段跳过，其余原生阶段按范围跳过。本片未做原生装机验收。

## Consequences

用户先确认或取消组词，再用独立快捷键保存或关闭，防止丢掉编辑草稿或提前保存。本次只调整事件入口，不改存储协议和词库校验规则。验证范围是共享设置组件，没有做原生宿主设备验收。

对话框可由不同词库界面直接组合，代价是调用方继续承担保存错误处理、关闭和成功后的刷新；对话框不自行操作词库存储、不重置父页面状态。两种编辑表单仍不用 `<form>`，保留设置外层表单、忙碌状态和草稿生命周期。

## 旧笔记归属

检索 `harmony-phone-dictionary`、`FormDialog`、新建词库和添加词条：社区内容并入功能页的笔记持有图库导航和外层表单边界，搜狗词库导入笔记持有导入协议，两者都不持有编辑弹窗的组合按键契约。保留原有不嵌套表单的约束。
