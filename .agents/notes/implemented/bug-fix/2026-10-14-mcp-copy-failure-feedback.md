# Agent Note: MCP 复制失败提示与请求顺序

Status: implemented

## Problem

MCP 设置页的复制操作只处理成功。宿主剪贴板不可用时，同步抛错和异步拒绝均没有可见反馈，错误还会成为未处理异常。同一宿主可以连续提交复制，较早请求的成功也能在最新请求失败后显示「已复制」。

## Decision

复制操作在组件内用 `try/catch` 消费同步和异步失败，只显示固定提示「复制失败，请重试。」，不展示宿主错误详情。每次复制清除旧错误和成功按钮标识，用独立的 `useAsyncGeneration(status, install, copyText)` 递增请求代次；成功和失败仅在组件仍挂载且请求仍属当前代次时更新反馈。宿主回调更换时清除旧反馈。成功提示继续复用现有 1600 毫秒公共计时逻辑。

## Related decisions

已检索活跃笔记。以下记录部分重叠，保留各自职责：

- [MCP 复制提示生命周期](../../implemented/bug-fix/2026-10-12-mcp-copy-feedback-lifetime.md)规定成功计时、重复复制和卸载清理，本篇补充失败和请求完成顺序。
- [MCP 旧安装请求确认](../../implemented/bug-fix/2026-10-14-mcp-stale-entry-confirmation.md)规定安装请求的确认生命周期，复制不进入安装 runner。
- [UI 提示共用状态](../../implemented/simplification/2026-10-14-ui-timed-feedback.md)持有公共计时契约，本篇不扩展其接口。

## Alternatives considered

只添加拒绝回调改动最小，可以消费异步失败。但它不能捕获同步抛错，也不能阻止旧成功覆盖最新失败，因此使用 `async` 与 `try/catch`，并独立隔离复制请求代次。

把复制放进安装操作的公共 runner，可以复用忙碌和错误处理。但复制与写入互不依赖，runner 会使复制影响安装的忙碌状态，还需扩大通用错误格式接口。复制仍由组件管理，复用已有代次 hook。

## Consequences

同步和异步复制失败都给出固定、可重试的错误提示，旧宿主及交叠请求的迟到结果不能覆盖最近一次反馈。代次守卫只控制 UI 反馈，不取消已经提交给宿主的剪贴板写入。错误状态独立于安装结果，新增复制和宿主更换会清除旧反馈。成功提示原有计时器可能在失败后继续到期，但按钮标识已清除，不会再显示旧成功；后续成功仍会重新计时。

## Verification

`apps/desktop/tests/settings/mcp-copy-feedback.test.tsx` 的原始七项回归在旧实现中有三项失败、四个未处理错误；修复后十一项全部通过且没有未处理错误。新增用例覆盖成功后失败、旧失败晚于新成功、更换宿主清除成功及失败反馈。与 `mcp-connect.test.tsx`、`mcp-connect-generation-reuse.test.ts`、`mcp-connect-mounted-reuse.test.ts` 联跑，四文件共 35 项通过。`pnpm lint`、`pnpm build`、文件格式检查、`pnpm run verify-notes`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 通过。quick 的 Linux 外壳、Windows 原生构建与鸿蒙真打包因缺少工具链或预备资源跳过，其余平台按改动范围跳过。验证层为 jsdom 与前端构建，不代表原生设备剪贴板验收。
