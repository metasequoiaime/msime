# Agent Note: 设置皮肤页社区区域独立复用

Status: implemented

## Problem

`useSettingsCommunityViews` 已持有皮肤分类和图库覆盖状态，`SettingsShell` 仍内联触屏分类栏、触屏皮肤图库和桌面候选皮肤图库的接线，状态消费与整页布局交织。

## Decision

公开的 `SettingsSkinCommunitySection` 与属性类型消费既有模型和社区视图 hook 的字段投影。区域保留两套标签及图库的原条件、属性、外观、历史页面与回调；不创建分类状态，不接管本地皮肤表单。外壳继续调用原 hook，并把同一组标签及覆盖结果交给区域和表单，社区图库仍在表单之外。

## Historical audit

- [社区视图状态](../../implemented/simplification/2026-10-17-settings-community-views.md)部分重叠，继续持有分类及覆盖判定，本篇持有皮肤页的渲染接线。
- [本地皮肤统一选择](../../implemented/simplification/2026-10-18-local-skins-unified.md)与[手机皮肤顶部分类](../../implemented/feature/2026-10-16-mobile-skin-page-top-tabs.md)部分重叠，保留本地预览、草稿、社区获取与分类生命周期，仅更新共享渲染位置。
- [设置页组件](../../implemented/simplification/2026-10-19-settings-page-component-module.md)部分重叠，继续持有布局和功能页组合。
- 社区根页、图库公共搜索外观和插件社区笔记分别持有其他组合或控件，与本片不重叠；没有完全吸收的笔记或失效提案。
- 按组件名、`phoneSkinCommunityShown` 与皮肤图库检索活跃笔记，没有既有同名组件。

## Alternatives considered

- 将社区视图 hook 移进新组件：接口更少；但本地表单也消费同一组标签和覆盖结果，内移会产生第二份状态或回传状态的额外机制，因此继续由外壳调用一次。
- 让普通皮肤页直接渲染社区图库：功能聚合更紧；但图库自带搜索与发布表单，普通皮肤页位于偏好表单中，会产生嵌套表单，因此区域仍由外壳在表单之外组合。

## Verification

桌面分类键盘操作、候选图库与本地返回保持；触屏图库只在社区分类挂载，离开页面后保留外层分类并重置图库搜索；服务缺失时不显示社区分类。抽取前入口与视图状态基线 2 文件 10 项通过；新增 4 项真实图库行为用例先因组件缺失失败，实现后连同基线共 3 文件 14 项通过。完整设置与皮肤回归 7 文件 490 项通过。

桌面类型检查与生产构建、HarmonyOS 类型检查、lint、改动文件格式、笔记树/格式/归档封印和 `git diff --check` 通过；lint 只有 4 条既有未使用声明警告。结构对照确认两套分类栏及两类图库的条件、属性、面板、历史和回调一致；视图 hook 的输入、调用次数与顺序不变，外壳其余函数及四项组件接线一致；原 hook 与公共导出未变。组件及 170 个传递模块无运行时入口循环。独立审查无阻塞项或重要问题，并独立复跑 4 项组件测试。

`bash scripts/verify-local.sh --quick` 通过，覆盖契约、Rust 工作区、Android Rust 目标、Linux 桌面外壳、HarmonyOS 设置打包与共享 Apple 桥。HarmonyOS ArkTS 真编译缺工程依赖，Windows 原生编译缺本机工具链，按脚本跳过。自动化回归不代表原生宿主装机验收。

## Consequences

皮肤页的社区区域可独立组合，外壳只负责提供模型、视图和动作；代价是新增公开 props，需要继续传入与本地表单相同的一组视图对象，保持标签 id、选中项和覆盖结果一致。组件没有自己的分类 hook，不改变图库位置、皮肤获取或应用流程，不修改文件读写、权限、原生资源和协议。
