# Agent Note: 复用账号移动子页的历史状态动作

Status: implemented

## Problem

账号页仍内联维护移动子页的历史状态：首次读取当前记录、打开时写入 `accountSubpage`、接收 `popstate`、关闭时回退，以及退出登录时清理本地子页。它与已有的[移动设置子视图历史状态动作](2026-10-17-mobile-settings-subview.md)生命周期相同，但此前因账号页需要从已存在的历史记录初始化而被排除，导致公共 Hook 仍有一套重复调用方。

## Decision

`useMobileSettingsSubview` 增加只在首次挂载读取的 `initialValue`，由调用方根据当前历史记录计算初始子页。账号页用该 Hook 管理 `profile` 和 `download` 两个 `accountSubpage` 值，继续由账号页解码 `msimeSettings` 和领域字段；退出登录时使用 Hook 的本地重置动作，再按原契约回退历史记录。

## Alternatives considered

- **继续在账号页保留独立的历史状态实现。** 可以避免扩展 Hook 的参数，但同一组打开、返回和清理动作会继续分叉，后续修复只覆盖部分移动子页。
- **让 Hook 自己读取并解析所有历史路由。** 可以省掉调用方的初始值计算，但会把账号字段、页面字段和领域校验耦合进通用设置路由，破坏调用方负责解码的边界。
- **把账号的历史状态改成全局设置导航状态。** 可以统一入口，但会改变账号页已有的 `accountSubpage` 历史契约和返回顺序，迁移风险高于复用现有 Hook。

## Consequences

- 账号移动页打开、系统返回、无匹配记录时的本地关闭和退出登录清理继续保持原行为。
- 通用 Hook 可在首次渲染时承接已经存在的子视图值，避免每个调用方重复写一段初始化 effect；初始值仍由调用方按领域路由计算。
- 账号页的 `accountSubpage` 字段和 `msimeSettings` 标记不变；桌面账号编辑仍走原来的内嵌对话框。
- Hook 的初始值只读取一次，后续历史变化仍通过 `popstate` 解码；调用方如果需要响应客户端或页面变化，仍需自行重置。
