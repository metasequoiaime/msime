# Agent Note: 社区资源模型与添加操作脱离图库页面

Status: implemented

## Problem

社区词库详情和图库行分别判断命名词库能力、执行安装或按类型导入。分组导入已有局部函数，但选择路径仍重复。共享的回复模板添加操作、资源数据类型和副标题函数定义在大型 JSX 页面中，AI 回复模板、词库页与纯逻辑消费者因此反向依赖图库。

## Decision

`community-resource-model.ts` 持有资源类型、范围和副标题函数；`community-resource-actions.ts` 持有本机安装能力判断、词库安装与回复模板添加。详情和图库行共用安装操作，操作返回命名词库或导入条数的结构化结果，各入口继续生成原有提示。回复模板保留收藏、读取最新详情、可选存入键盘的顺序及原返回文字。旧页面与范围组件兼容转导出原有名字，内部消费者直接引用模型或操作。

保留类型分组顺序、TSV 列顺序、`quick` 到 `quick_phrase` 的映射、请求标识生成方式、宿主方法接收者和失败即停止的行为。编辑和发布状态由[资源发布](../../implemented/simplification/2026-10-16-community-resource-publication.md)独立持有。调用方继续拥有详情刷新、异步代次、忙碌状态、toast 和已添加标记，不扩大共享操作的生命周期职责。

## Historical audit

检索活跃笔记的 `installCommunityDictionary`、`importCommunityDictionary`、`addCommunityReply`、`CommunityResourceClient`、`CommunitySharedWord`、共享词库与回复模板添加：

- [触屏社区融入功能页](../../implemented/feature/2026-10-11-touch-community-merged-into-feature-pages.md)持有添加语义和入口归属；本篇只收敛实现与类型位置，部分重叠，互链。
- [AI 回复模板代次](../../implemented/simplification/2026-10-12-ai-reply-generation.md)与[词库发现代次](../../implemented/simplification/2026-10-12-dictionary-discovery-generation.md)持有调用方生命周期，保留。
- [社区资源宿主 bridge](../../implemented/simplification/2026-10-14-community-resource-bridge.md)持有命令转发与移动端能力开关，保留；本片不改变宿主协议。
- Android、iOS 的社区功能页笔记拥有各自原生导航和安装操作，与本片的共享 React 实现独立。其他命中属于平台功能或错误修复，保留；没有完全吸收或失效的提案。

## Alternatives considered

- **只把现有私有函数移动到新文件。** 能降低页面体积，也能让回复模板直接引用操作；但详情仍自己选择命名词库或逐条导入，保留两套业务分支，因此一起统一安装操作。
- **让公共安装函数直接返回统一提示文字。** 接口简单，列表现有函数也是如此；但详情与图库的提示现在不同，统一会改变界面，选择结构化结果让调用方保留原文。
- **顺便统一所有入口的请求代次。** 生命周期实现能进一步收敛，但详情、图库和功能页各有不同所有者与刷新顺序，迁移操作时不改它们，避免把独立行为变更混入本片。

## Verification

独立操作测试在空实现上 11 项全部失败，补齐实现后全部通过。实际 TypeScript 类型检查确认公共模型与原包入口、范围组件兼容；社区资源、鸿蒙图库、功能页图库、词库页、宿主 bridge、范围与生命周期等 11 个文件、98 项测试通过，包含新增命名词库详情提示和公共模型类型兼容检查。九个模型声明、副标题与分组导入函数去除注释、空白后与旧实现完全一致。桌面生产构建、鸿蒙类型检查、lint、格式、笔记检查与 `git diff --check` 通过；lint 保留三条既有未使用导入警告。`bash scripts/verify-local.sh --quick` 通过，含契约检查、Rust 工作区、Android 交叉检查、共享 Apple 桥与鸿蒙设置打包。macOS 与 Linux 原生宿主阶段按范围跳过，Linux 设置外壳和 Windows 原生阶段因缺少 Docker 或 MinGW 跳过；这些跳过不计为设备验收。

## Consequences

详情与图库只保留各自的提示和生命周期，安装路径由一个公共操作选择，AI 回复模板和词库页不再为数据模型、副标题或添加操作依赖整个图库页面。代价是原页面与范围组件需要兼容转导出。公共操作不吞掉失败、不自动回退其他路径；空导入返回应用零条，不调用宿主。回复操作只依赖 `save`、`detail` 和可选的 `storeReply`，保留现有完整客户端调用方式。

模型从 `community-report-model.ts` 直接引用社区举报类型，不依赖举报界面。类型擦除可能隐藏兼容问题，因此实际运行 `tsc`。本片不改原生文件访问、资源或协议，前端验证不作为设备验收。
