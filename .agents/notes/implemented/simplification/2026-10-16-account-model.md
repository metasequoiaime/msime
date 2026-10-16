# Agent Note: 账号数据与客户端模型独立于页面

Status: implemented

## Problem

账号用户、服务商、验证挑战、资料、设置同步、应用图标、客户端及社区目标共 12 个类型定义在 `account-page.tsx`。头像、昵称、登录 hook、设置宿主与社区导航为取得类型而反向引用账号页面，数据和宿主操作契约没有独立来源。

## Decision

将这 12 个声明原样迁入 `account/account-model.ts`。模块不导入 React、页面或宿主实现。账号页面和内部消费者直接引用公共模型，账号页面保留全部原有类型重导出，包入口直接从公共模型重导出原有集合。字段、方法形式、可选性、空值、数组可变性、字面量联合及参数返回值保持原样。

## Historical audit

检索 `AccountClient`、`AppIconClient`、`AccountUser`、`AccountCommunityDestination`、账号模型、契约及头像缓存相关的活跃笔记，没有已有记录持有这 12 个声明的归属。

- [设置同步桥接](../../implemented/simplification/2026-10-14-settings-sync-bridge.md)持有宿主命令转发；[设置宿主契约](../../implemented/simplification/2026-10-16-settings-client-contracts.md)组合账号与图标客户端。本片只改变它们引用类型的来源。
- [Google 登录等待状态](../../implemented/bug-fix/2026-10-14-google-login-client-waiting.md)持有客户端更换与取消边界；[面板焦点恢复](../../implemented/bug-fix/2026-10-14-account-sheet-restore-focus.md)与[组词期间 Escape](../../implemented/bug-fix/2026-10-14-account-sheet-ime-escape.md)持有面板行为，全部保留并复用回归测试。
- 原生账号、同步、头像文件读取和后端认证笔记分别持有平台实现、持久化与安全边界，不被这次 TypeScript 类型迁移取代。

## Alternatives considered

- **按用户、图标和同步分别创建文件。** 领域文件更小，但当前客户端互相组合、页面与宿主也一起使用这些类型；先建立完整且无依赖的账号模型边界，避免给无独立消费需求的类型增加跳转。
- **仅让纯逻辑消费者直接引用新模型。** 改动更少，但相同内部类型仍保留两套引用来源，页面依赖继续扩散；内部统一直引，旧公开路径只用于兼容。
- **从客户端方法返回值派生 DTO。** 可以减少部分声明，但会改变现有独立 DTO 的归属和推断形式；本片保持原声明，避免混入接口重新设计。

## Verification

12 个声明去除注释及空白后与原页面逐字一致。临时 TypeScript 探针先因公共模块不存在而失败，迁移后真实编译器核对独立旧基线、新模型、旧页面和包入口完全一致，覆盖可选登录、取消头像选择、同步参数与数组可变性。账号页面及 14 个内部模块除迁出声明、类型导入与重导出外一致，运行时导入导出的符号与顺序保持不变。设置按钮门禁仍识别 42 个回调，偏好字段门禁仍覆盖 35 组类型。账号与设置导航等 41 个文件、206 项既有测试通过；桌面生产构建、鸿蒙类型检查、lint、格式、笔记及差异检查通过。lint 仅有三条既有未使用导入警告。`bash scripts/verify-local.sh --quick` 通过；原生宿主阶段按范围或本机工具条件跳过，没有计为设备验收。

## Consequences

头像、昵称、登录 hook、设置宿主和导航直接依赖同一份账号模型。纯类型迁移会被 Vitest 擦除，必须用实际编译器检查兼容性。保留旧路径增加一层类型重导出；认证、令牌隔离、头像缓存、同步持久化和页面状态仍由现有实现持有。本片没有文件 I/O、原生资源或设备行为变化。
