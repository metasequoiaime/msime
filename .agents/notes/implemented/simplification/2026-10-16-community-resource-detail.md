# Agent Note: 社区资源详情与动作独立复用

Status: implemented

## Problem

词库与回复模板的详情是整页图库里的私有函数，持有加载、收藏、导入、评分、举报、下架和编辑后刷新。其他共享入口无法单独展示详情，动作生命周期的验证必须经过列表导航。

## Decision

`use-community-resource-detail.ts` 持有原有状态与动作，客户端只要求详情动作；`community-resource-detail.tsx` 持有界面、能力显示条件和编辑器接线，客户端另外要求发布能力。页面通过新组件展示详情，公共包入口导出组件与 props。保留各操作顺序、逐步 owner 与 mounted 检查、同步防重、错误格式、提示和发布刷新闭包，不引入额外取消语义。

## 旧笔记归属

按 `ResourceDetail`、资源详情、社区资源状态和 `community-resource` 检索 proposed、implemented、rejected：

- [资源模型与添加](../../implemented/simplification/2026-10-16-community-resource-actions.md)部分重叠：继续持有安装和添加的共享操作，本片仅迁出调用方状态，互链。
- [资源编辑与发布](../../implemented/simplification/2026-10-16-community-resource-publication.md)部分重叠：继续拥有编辑器和提交；本片持有详情里的打开、关闭与发布后刷新，互链。
- [作品模型](../../implemented/simplification/2026-10-16-community-models.md)和[候选皮肤发布](../../implemented/simplification/2026-10-16-candidate-skin-publication.md)拥有不同业务边界，无关，保留。
- iOS 页面可见性、Windows 社区和触屏功能页笔记持有原生宿主或导航语义，无关，保留。文件锁释放和 MCP 过期确认拥有各自 owner 规则，与本片无关。没有完全吸收或过时提案。

## Alternatives considered

- 只移出完整详情组件：可以直接复用界面且接线较少，但无界面调用和生命周期测试仍依赖整份 JSX，因此同时分离状态 hook。
- 直接共用 `addCommunityReply` 执行详情里的收藏与添加：已存在且可以删掉连续步骤，但详情在每一步等待后检查 owner，避免切换后继续下一步；通用添加操作没有这个职责，保留详情原有步骤和逐步守卫。

## Verification

迁移前 7 文件 29 项基线通过；迁移后原页面、独立状态与源码门禁 8 文件 49 项通过。独立公共组件测试验证不提供列表能力时的导入、返回、可选操作显示和发布后刷新。完整社区套件 85 文件 402 项通过；新增测试的举报理由类型由错误的英文值改为既有契约“其他”后，2 文件 22 项复跑通过，桌面类型检查与生产构建通过。HarmonyOS 类型检查、改动文件格式/lint、notes 与 `git diff --check` 通过。六个源码门禁迁到实际持有者且保留断言，独立审查复核无剩余问题。状态体、发布后刷新闭包、JSX 及页面剩余正文机械比对一致。

`bash scripts/verify-local.sh --quick` 通过，包括仓库契约、Rust 工作区、Android 目标和共享 Apple 桥接编译。Linux Docker、Windows MinGW 与 HarmonyOS 原生打包环境缺失，相关阶段跳过；其他原生阶段按范围跳过。

## Consequences

详情和状态可脱离图库使用，页面仅控制选择与返回历史；界面与 hook 多一层动作接线，需要原页面测试和机械比对确认。状态初始化与客户端、资源 ID owner 规则原样保留，不扩展重新初始化策略。详情每一步等待后的守卫继续存在，旧请求仍只丢弃过期界面结果，不新增取消宿主操作的语义。

本片不改变宿主读写、权限、Android 资源或协议，不把共享前端验证称为设备验收。
