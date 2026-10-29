# Agent Note: 词条文件导入在宿主提交前检查操作归属

Status: implemented

## Problem

共享 React 词库页的 `importPhrases` 只在导入完成后检查 `isCurrent()`。文件读取或 SCEL 转换期间替换客户端、离开页面，旧操作仍会向原宿主提交导入；没有批量能力的宿主在首条写入等待时失效，还会继续写剩余条目。结果提示虽然被过滤，持久化副作用已经发生。

七项受控文件读取、转换与首条写入回归在现有实现均失败，分别观察到过时批量导入、旧 SCEL 转换和后续逐条写入。

## Decision

复用 `runPhraseAction` 的现有 `isCurrent()`，在格式识别后、整份 SCEL 读取后、文本准备完成后以及每条兼容写入前检查。将嵌套的 SCEL 读取与宿主转换拆成两个边界。归属失效直接返回，已经提交的宿主请求仍按原语义完成，不做回滚。查询条件不属于操作代次，修改搜索不取消用户已请求的导入。

## Existing notes audit

- [词库公共契约](../../implemented/simplification/2026-10-16-dictionary-contracts.md)：只管理类型归属，本次不改变能力或字段，保留。
- [细胞词库导入](../../implemented/feature/2026-10-13-scel-import.md)：管理识别、转换和上限，部分重叠，保留全部格式行为。
- [双拼文件操作归属](../../implemented/simplification/2026-10-14-shuangpin-custom-editor-generation.md)：同一守卫模式的其他消费者，保留，本次复用检查方式。
- [词书操作归属](../../implemented/bug-fix/2026-10-15-vocabulary-action-owner.md)：独立词书入口，按客户端和文件代次处理，本次不改其代码。

## Alternatives considered

只在最终导入前加一次检查，改动最少且可阻止批量写入；但仍会在失效后请求旧 SCEL 宿主转换，也不能阻止逐条导入的后续写入，因此在每个后续副作用边界检查。

## Verification

七项生命周期回归在旧实现全部失败，原有 31 项通过；修复后词库管理与领域测试共 353 项通过，覆盖真实文件解码、格式与导入边界。当前客户端的显式 SCEL 格式与魔数识别另有正常导入回归，管理 hook 合计 40 项通过。桌面与鸿蒙生产构建（包含类型检查）通过。变更范围 lint、格式、笔记与差异检查通过。

本地 `MSIME_BUILD_JOBS=4 CARGO_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 通过，含契约、Rust 工作区、Android Rust 目标与共享 Apple 桥编译；原生平台阶段按范围或工具条件跳过，不作为设备验收。独立审查无阻塞项。

## Consequences

失效的文件准备不再发起旧宿主操作，逐条兼容导入停止尚未提交的剩余条目。代价是每个后续副作用边界都需要检查现有操作归属，未来增加异步准备步骤时仍须守住它。

已经发出的宿主请求不能由 UI 取消。逐条兼容路径可能已写入首条，失效仅停止尚未提交的剩余条目，不承诺整个文件事务回滚。本次只调整共享页面生命周期，不修改文件读取实现、存储实现或 Android/iOS 发布包。共享前端验证不代表原生窗口验收。
