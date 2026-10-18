# Agent Note: 复用基础账号宿主命令客户端

Status: implemented

## Problem

桌面入口和 Android/iOS 的 Tauri 宿主组装各自实现同一组基础账号命令。状态、登录方式、验证码申请与登录、资料、昵称、退出、注销和清理失效会话的命令名与参数相同，接口变化需要同步两份映射。

## Decision

在 `apps/desktop/src/account/account-command-client.ts` 提取 `createAccountCommandClient(invoke)`，返回基础 `AccountClient`。桌面入口和移动宿主组装共用它，命令、参数、返回值与拒绝结果保持原样。Google 登录、头像、QQ 关联、iOS Apple 登录与设置同步继续在已有宿主组装位置提供。

历史笔记审计：[设置同步宿主桥接](../../implemented/simplification/2026-10-14-settings-sync-bridge.md)拥有另一组命令和桌面文案，独立保留；[Windows 账号 API](../../implemented/architecture/2026-10-11-windows-settings-account-host-api.md)和[宿主注入会话存储](../../proposed/architecture/2026-10-16-host-injected-session-storage.md)持有原生会话、存储与网络协议，独立保留。本次沿用[设置界面平台归属](../../implemented/architecture/2026-10-15-settings-ui-ownership-by-platform.md)，只共享现有 Tauri 前端适配，不改变原生产品设置界面的归属。

## Alternatives considered

- **保留两份显式映射**：每个平台文件都能直接看到整组命令，定位简单。但这九个方法没有平台差异，继续复制会使参数与返回契约在后续修改中分叉，因此让两处调用同一工厂，平台能力仍由原有组装层补充。

## Consequences

两处基础账号映射共用一份实现；宿主保持原来的客户端创建时机及平台能力。后续基础命令变化只需修改一处，同时依赖两处宿主继续引用这个公共入口。

## Verification

逐项对照两份原有映射，命令名、参数名、参数值、返回值与拒绝传播一致。抽取前后同一组既有移动宿主、桌面宿主服务和账号页回归均为 6 文件、93 项通过；独立审查无阻塞项，另跑移动宿主、同步复用和账号页 3 文件、83 项通过。桌面类型检查与构建、lint、格式、笔记和本地 quick 门禁通过。前端回归不代表真实账号服务登录或原生宿主装机验证。
