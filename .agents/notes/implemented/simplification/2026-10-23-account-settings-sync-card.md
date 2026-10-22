# Agent Note: 账号设置同步卡片与状态独立

Status: implemented

## Problem

`account-page.tsx` 内嵌 `SettingsSyncCard` 的展示、快照和字段表、确认选择、忙碌与异步动作。首次读取和手动刷新各自重复 `schema/load` 并行读取及代次检查，账号页面承担同步卡片的内部实现。

## Decision

卡片独立在 `account/settings-sync-card.tsx`，内部 `useSettingsSync` 持有状态、确认选择、初始化及三个手动动作。继续组合 `useAccountAction(client, report, report, userId)`，首次读取与刷新共用一个读取方法，保留初始化可越过旧渲染忙碌快照的 `allowBusy` 入口。账号页原有两个调用点直接复用卡片，公共包导出和客户端契约不变。

## Historical audit

已按 `SettingsSyncCard`、设置同步卡片、`settings-sync` 检索 proposed、implemented 和 rejected。[同步命令桥接](../../implemented/simplification/2026-10-14-settings-sync-bridge.md)持有宿主命令映射和专属文案，[账号模型](../../implemented/simplification/2026-10-16-account-model.md)持有客户端及快照类型，均部分重叠且保留。[Windows 原生账号页](../../implemented/feature/2026-10-11-windows-settings-account-page.md)和 [iOS 页面请求代次](../../implemented/simplification/2026-10-13-ios-visible-request-generation.md)持有原生实现，与共享 React 生命周期无关。没有现有笔记拥有这张 React 卡片的状态与展示边界。

## Alternatives considered

仅移动整张卡片可缩短账号页且改动较少，但异步状态仍与 JSX 混在一起，重复读取仍要维护两处；拆分内部 hook 并共用读取方法，使用已有生命周期工具。

让 `useAccountAction` 接管同步快照和确认状态，可减少独立 hook，但会把专属于同步的字段与动作写进所有账号界面共用的操作层；该层继续只持有通用忙碌和代次守卫。

## Verification

迁移前 88 项真实行为与账号操作测试通过；迁移后含抽取边界检查的四文件共 89 项通过。新增回归覆盖字段表等待、空快照禁用应用、取消确认、刷新失败保留快照与后续成功恢复、普通及 StrictMode 下替换同步客户端隔离旧读取，以及旧上传迟到成功和失败不结束新上传。抽取边界检查在旧页面上因缺少独立卡片接入失败，迁移后通过。

桌面生产构建、鸿蒙类型检查、改动文件 lint 与格式检查、笔记检查、差异检查及本地 quick 门禁通过。独立审查确认 JSX、确认生命周期、闭包、代次和忙碌语义保持，重跑相同四文件 89 项通过。quick 的原生 Windows 构建与鸿蒙真打包因工具链或 staged 输入缺失而跳过，其余原生阶段按范围跳过；没有把这些跳过项记成设备验收。

## Consequences

账号页不再持有同步卡片实现，首次读取与手动刷新共用字段表和快照的并行读取及代次检查。增加两个内部模块以分离状态与展示；读取时机、刷新失败保留已有快照、原有确认选择的生命周期、文案、按钮状态、上传及应用参数均保持原语义。

hook 继续通过已有账号操作层串行执行动作，初始化保留 `allowBusy`，客户端与 `userId` 仍是独立的代次拥有者。共享 React/jsdom 测试和构建不代表原生设备或真实账号同步验收。
