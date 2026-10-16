# Agent Note: 社区资源编辑与发布独立复用

Status: implemented

## Problem

词库与回复模板的新发布和版本更新共用一个私有 `ResourceEditor`，状态、校验、词条编辑和异步提交嵌在整页图库中。其他共享入口无法单独调用表单或复用发布状态。

## Decision

`use-community-resource-publication.ts` 持有原有状态和动作，客户端只要求 `publish`；`community-resource-editor.tsx` 持有控件、文案与表单事件。页面委托新组件，包入口导出组件及 props。保留 ID、版本号、授权差异、Unicode 码点长度校验、同步提交锁、客户端 owner 和成功回调顺序，不改变现有词条编辑语义。

## 旧笔记归属

检索 proposed、implemented、rejected 的资源编辑、发布、`ResourceEditor` 和 `community-resource`：

- [资源模型与添加](../../implemented/simplification/2026-10-16-community-resource-actions.md)部分重叠：继续持有模型、安装和添加操作，本篇持有编辑和发布状态，互链。
- [作品模型](../../implemented/simplification/2026-10-16-community-models.md)持有其他作品的数据边界，无关，不改。
- [候选皮肤发布](../../implemented/simplification/2026-10-16-candidate-skin-publication.md)部分重叠：沿用状态与表单分离，资源没有目录预检与打包，不统一业务状态机。
- [发布建议字段](../../implemented/simplification/2026-10-12-community-publication-suggestions.md)、Windows 社区资源和云账号、触屏功能页笔记持有字段或宿主入口，无关，保留。没有完全吸收或失效的提案。

## Alternatives considered

- 只移出表单组件：能直接复用完整编辑器，改动较少，但状态仍依赖整个表单，独立 hook 使发布和词条编辑可以单独使用和验证。
- 合并到皮肤与插件的发布 hook：元数据和异步动作形似，但资源使用内容与版本号，其他作品使用目录和打包预检。共用现有校验与异步工具，保持业务边界。

## Verification

迁移前 3 文件 22 项基线通过，迁移后独立 hook 与原页面 4 文件 34 项通过。机械比对确认状态体除表单事件拆分外逐字一致，JSX 仅 setter、词条动作与关闭回调委托变化。独立组件从公共包入口渲染，只提供 `publish`，验证授权提交与关闭；三个源码门禁迁到实际持有者，保留原断言。

完整社区套件 83 文件 380 项通过。桌面类型检查与生产构建、HarmonyOS 类型检查、格式、lint、notes 与 `git diff --check` 通过。独立审查发现一个校验源码门禁仍读旧页面，修正到实际 hook 后保留四条原断言，复核无剩余问题。

`bash scripts/verify-local.sh --quick` 通过，含仓库契约、Rust 工作区、Android 目标和共享 Apple 桥接编译。Linux Docker、Windows MinGW 与 HarmonyOS 原生打包环境缺失，相关阶段跳过；其他原生阶段按范围跳过。

## Consequences

编辑器和状态可独立复用，图库及[资源详情](../../implemented/simplification/2026-10-16-community-resource-detail.md)各自控制打开、关闭与刷新；多出一层 props 与回调接线，需要页面测试和机械比对守住。初始资源仍在挂载时读取，调用方切换编辑对象或类型时应重建编辑器实例；本片不新增重新初始化、取消宿主请求或词条编辑语义。客户端替换和卸载时的旧回包守卫沿用共享异步工具。

该片不改变宿主 I/O、权限、Android 资源或协议，前端验证不表示设备验收。
