# Agent Note: 设置插件页社区区域独立复用

Status: implemented

## Problem

插件来源状态和图库业务已经各有 owner，`SettingsShell` 仍内联来源标签、社区图库与本地目录、登录动作的接线，外壳布局和功能区域混在一起。

## Decision

公开 `SettingsPluginCommunitySection` 与属性类型，接收现有模型、社区覆盖结果和受控插件分类。保留原 JSX 条件、属性、位置与分类语义；外壳继续调用一次社区视图 hook，偏好表单复用相同覆盖结果，图库仍在表单之外。

## Historical audit

- [社区视图状态](../../implemented/simplification/2026-10-17-settings-community-views.md)和[外壳临时状态](../../implemented/simplification/2026-10-17-settings-shell-state-hook.md)部分重叠，继续持有覆盖判定与分类生命周期，本篇只持有区域接线。
- [插件图库](../../implemented/simplification/2026-10-17-community-plugin-gallery.md)与[插件详情](../../implemented/simplification/2026-10-17-community-plugin-detail.md)持有业务及详情，与本片区域组合不重叠。
- 皮肤社区区域、社区根页、图库公共模型、发布预检与平台宿主笔记分别持有其他页面、业务或宿主边界，不受此片影响。按组件名、插件社区和图库检索活跃笔记，无完全吸收的决定或失效提案。

## Alternatives considered

- 将插件分类状态和覆盖 hook 内移到区域：调用属性更少；但外壳偏好表单也消费覆盖结果，内移会复制状态或增加回传机制，因此沿用外壳唯一 owner。
- 在普通插件设置页直接组合图库：功能更集中；但普通页位于偏好表单中，图库拥有搜索与发布表单，因此继续在外壳表单外组合。

## Verification

抽取前设置社区视图与插件图库基线 2 文件 34 项通过；新增 3 项真实图库行为用例先因组件缺失失败，实现后连同基线共 3 文件 37 项通过。覆盖受控来源切换、图库卸载与搜索重置、离开页面保留外层分类，以及服务缺失时隐藏区域。完整设置与插件回归 4 文件 432 项通过。

桌面类型检查与生产构建、HarmonyOS 类型检查、lint 通过；lint 只有 4 条既有未使用声明警告。AST 对照确认来源标签、图库挂载条件、属性与回调一致，外壳其余逻辑、视图 hook 调用和表单位置保持，原导出不变，仅新增组件及属性类型。组件及 110 个传递模块无运行时入口循环。独立审查无阻塞项或重要问题，独立复跑 3 项组件测试通过。

`bash scripts/verify-local.sh --quick` 通过，覆盖契约、Rust 工作区、Android Rust 目标、Linux 桌面外壳、HarmonyOS 设置打包与共享 Apple 桥。HarmonyOS ArkTS 真编译缺工程依赖，Windows 原生编译缺本机工具链，按脚本跳过。格式、笔记树/格式/归档封印及 `git diff --check` 通过。

## Consequences

插件社区区域可独立组合，外壳只提供模型、视图和动作。代价是增加公开 props，调用方须传入与偏好表单相同的覆盖结果；外壳仍是分类状态的唯一 owner。若今后要将完整插件页独立挂载，需要同时解决偏好表单覆盖契约。此片不改图库业务、文件读写、权限或原生资源，自动化验证不等于宿主装机验收。
