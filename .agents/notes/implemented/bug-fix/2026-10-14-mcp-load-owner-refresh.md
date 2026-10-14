# Agent Note: MCP 宿主更换后重新加载状态

Status: implemented

## Problem

MCP 状态请求的 `refreshGeneration` 以 `status`、`install`、`copyText` 为所有者，任一回调更换都会使旧请求失效。初始加载 effect 却只依赖由 `status` 决定的 `refresh`。只更换安装或复制能力时，正在加载的结果被丢弃，而新加载没有发起，页面永远没有命令和配置。

## Decision

加载 effect 同时监听 `install` 和 `copyText`。请求所有者更换时，旧代次失效后立即发起新的状态读取；现有代次检查继续丢弃旧成功和旧失败。保留安装、复制各自的 runner 与代次，不改变宿主接口。

## Related decisions

已检索 `McpConnectSection`、`refreshGeneration` 和 `useAsyncGeneration` 的活跃笔记：

- [MCP 旧安装确认](../../implemented/bug-fix/2026-10-14-mcp-stale-entry-confirmation.md)与[复制失败反馈](../../implemented/bug-fix/2026-10-14-mcp-copy-failure-feedback.md)部分重叠，分别持有旧宿主动作隔离决定；本篇补充读取状态的重新同步，两篇保留。
- [AI 回复模板代次](../../implemented/simplification/2026-10-12-ai-reply-generation.md)和[外部主题代次](../../implemented/simplification/2026-10-12-external-theme-generation.md)提供已有所有者失效与 effect 重新加载的参考，不持有 MCP 读取决定。
- MCP 配置路径、复制提示计时与其他平台笔记无关，保留。

## Alternatives considered

让 `refreshGeneration` 只以 `status` 为所有者，可避免能力回调更换时多读一次状态，也不会丢弃原请求。但组件已经把三个回调视为宿主边界，缩小边界会允许旧宿主结果跨越能力切换，因此保留现有隔离范围并补齐重新同步。

把 `install` 和 `copyText` 加进 `refresh` 的 `useCallback` 依赖，同样能驱动加载 effect。但它们不参与读取函数体，真正需要同步的是加载 effect；直接补齐该 effect 的依赖即可，不扩大刷新函数的身份变化范围。

## Consequences

只更换安装或复制能力也会重新读取状态，初始加载不会因为丢弃旧结果而永久留空。旧成功和旧失败仍被现有代次守卫丢弃。宿主安装或复制能力每次更换会额外读取一次状态，调用方应保持稳定的能力回调身份；普通页面状态更新不改变这些回调时不会重新读取。

## Verification

`apps/desktop/tests/settings/mcp-connect.test.tsx` 新增四项行为回归，保持 `status` 身份不变，只更换 `install` 或 `copyText`，分别让旧读取成功或失败。旧实现四项全部失败，命令始终不出现；修复后四个 MCP 文件共 39 项测试通过，没有未处理错误。回归断言新命令内容和恰好两次读取，也验证组件状态更新不会反复发起读取。`pnpm lint`、`pnpm build`、改动文件格式检查、`pnpm run verify-notes`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 通过。quick 的 Rust 工作区、Android target 和共享 Apple bridge 编译通过；Linux 外壳缺 Docker、Windows 原生构建缺配置/MinGW、鸿蒙真打包缺工具链或预备资源而跳过，其他平台按范围跳过。验证使用合成宿主回调和 jsdom，不代表原生设备验收。
