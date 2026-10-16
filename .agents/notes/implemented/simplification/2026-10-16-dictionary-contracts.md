# Agent Note: 词库宿主与展示复用公共契约

Status: implemented

## Problem

`DictionaryClient`、导入响应、失败条目与清单定义在 React 设置入口；清单卡片、失败提示和导入摘要又重复定义相同字段。五笔查码响应定义在 JSX 组件中，词库客户端因此反向依赖展示模块。

## Decision

在 `dictionary/dictionary-contracts.ts` 统一词库客户端、响应与五笔查码类型，包入口和查码组件保留旧导出。内部消费者直接引用公共契约；清单卡片、失败提示与导入摘要复用领域类型，摘要继续允许省略 `failed`。保留所有参数、可选方法、空值与数组可变性，不改变宿主操作和异步生命周期。

## Historical audit

检索活跃笔记的词库契约、`DictionaryClient`、`DictionaryManifest`、`DictionaryImportResult`、`WubiCodeLookup`、`ImportSummary` 和导入提示：

- [发现词库请求代次](../../implemented/simplification/2026-10-12-dictionary-discovery-generation.md)管理异步归属，本片只迁移类型，保留该决定与其测试。
- [五笔反查](../../implemented/feature/2026-10-11-wubi-reverse-lookup.md)管理查码协议与展示行为，本片迁移 TypeScript 类型归属，保留方法和响应结构；两者部分重叠，互链。
- [细胞词库导入](../../implemented/feature/2026-10-13-scel-import.md)管理导入协议与摘要语义，保留，字段仅由公共响应投影。
- [面板宿主契约](../../implemented/architecture/2026-10-15-panel-client-contracts.md)、[服务商凭据契约](../../implemented/simplification/2026-10-16-provider-credential-contracts.md)同样收敛依赖方向，但拥有不同契约，保留。
- 其他命中是平台功能、设备验证或引擎内部约束，均不拥有这组 TypeScript 类型的归属；没有完全吸收或过时的提案。

## Alternatives considered

- **保留入口与展示中的结构声明。** 无需增加文件，结构类型现在也兼容；但字段要分别维护，纯逻辑依旧依赖入口，采用统一契约。
- **仅迁移入口类型，查码响应继续留在组件中。** 改动较少，但公共契约仍反向依赖 JSX；一起迁移查码的三个类型，原组件转导出以兼容旧路径。
- **所有导入展示都要求完整响应。** 只需一个类型，约束更强；但现有摘要和社区客户端允许只有应用条数，收紧会改变契约，摘要保留可选失败计数，社区最小接口保持原样。

## Verification

七个迁出的声明去除注释和空白后与原文完全一致。类型测试在公共模块不存在时由实际 `tsc --noEmit` 报错，迁移后桌面与鸿蒙的实际 `tsc --noEmit` 通过，确认新旧路径、展示投影与最小宿主能力兼容。词库管理、鸿蒙词库页、清单、导入、导出、失败恢复与查码等 11 个文件、116 项测试通过；桌面生产构建、lint、格式、笔记检查与 `git diff --check` 通过。lint 保留三条既有的未使用导入警告。`bash scripts/verify-local.sh --quick` 通过，覆盖契约检查、Rust 工作区、Android 交叉检查、共享 Apple 桥与鸿蒙设置打包；macOS 原生构建按范围跳过，Windows 与 Linux 原生检查按工具条件跳过，均不计为设备验收。

## Consequences

共享消费者不再为词库类型依赖设置入口或查码组件，减少三组重复字段。代价是旧路径需要兼容转导出；`DictionaryFailureNotice` 使用类型别名，旧清单组件转导出公共清单类型。导入摘要继续接受仅有 `applied` 的结果，完整宿主响应仍要求 `failed`。社区客户端只要求应用条数的最小接口保留。

类型擦除可能掩盖错误，因此验证使用实际 TypeScript 编译器。本片不改变文件读写、原生资源或设备行为，前端门禁不构成原生设备验收。
