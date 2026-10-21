# Agent Note: 向官网提供 TestFlight 安装次数汇总

Status: implemented

## Problem

官网只能读取 GitHub 资产下载次数，无法展示 iOS 的 TestFlight 安装次数。Apple 授权只存在主仓库的 iOS 发布环境，复制到公开站点会扩大凭据部署范围。

## Decision

独立统计 workflow 复用现有 App Store Connect 授权，每半小时分页读取当前应用的 iOS 有效构建及各构建 `betaBuildUsages`。只将安装次数合计、构建数、指标覆盖日期和生成时间通过既有官网 dispatch 凭据发送到官网，协议名为 `testflight-stats-v1`。官网独立快照与 GitHub 合计分开维护；无需触发发布或读取测试者明细。

分页失败、指标缺失或无法确认统计口径时整次失败，保留官网旧快照，不发布部分计数或伪造零值。请求与分页链接只允许预期的 Apple API 路径；不记录令牌、密钥、构建原始响应或测试者资料。

## Alternatives considered

把 Apple 密钥配置到官网能减少跨仓事件，但当前主仓库已有授权与 dispatch 凭据，复制私钥增加维护范围；因此复用主仓库授权，只把汇总数字交给官网。

按测试者聚合能得到用户维度的指标，但会读取身份标识，且测试者指标接口不提供构建安装次数。采用不含测试者明细的构建指标，不将安装次数称为独立人数或全部历史安装量。

## Verification

6 项合成测试验证 ES256 JWT、完整分页、重复构建去重、合法零值、缺失指标与失败分页拒绝发布、跨域分页拦截和仅发送公开汇总。workflow 的 actionlint、`bash scripts/run-checks.sh` 契约门禁与 `pnpm run verify-notes` 通过；缺少工具链的既有契约按门禁规则跳过。

本机可用磁盘约 12 GiB，低于仓库要求的 50 GB，不创建新的 Rust 构建树，因此未运行会无条件编译 workspace 的 `verify-local.sh --quick`。本次没有修改 Rust、原生宿主或发布链。首次真实执行仍须在 workflow 合入默认分支后验证现有密钥的指标权限和官网接收链；合成测试不是 Apple 授权或实数验证。

## Consequences

Apple 返回的覆盖区间不保证完整历史；构建安装次数合计不等于独立用户数。GitHub cron 可能延迟，需保留手动同步入口。两仓按生产者先合并的顺序发布，官网仅在有效数据到达后显示数字。

## Related notes audit

检索活跃笔记，nightly 的 TestFlight 外部组门槛与原生安装包分发记录涉及发布和安装方式，与只读统计的权限、指标及传输契约不同，原地保留。本项不修改发布工作流，也不改变测试组策略。
