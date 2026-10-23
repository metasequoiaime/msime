# Agent Note: 键盘皮肤库编辑状态独立复用

Status: implemented

## Problem

`TouchKeyboardSkinEditor` 同时持有设计编辑和皮肤库的加载、命名、重命名、更新确认、删除及提示状态。其他共享入口若要呈现同一组皮肤库操作，只能依赖整个编辑器，容易重复异步互斥与过期结果守卫。

## Decision

公开 `useTouchKeyboardSkinLibrary` 及状态类型，注入既有 `CustomSkinLibraryClient`。hook 持有保存列表、提示、名称草稿和确认目标，复用一个 `useAsyncActionRunner` 串行加载与变更；客户端身份仍界定请求归属。名称输入继续按 32 个字素限制并在提交时去掉首尾空白，实际容量和数据校验继续由既有服务负责。

命名提交接收当前设计，成功返回 `created` 或 `renamed`；失败、忙碌、无客户端与过期结果不返回成功。编辑器据此切换到「我的」，只有新建成功调用 `onUse`。更新确认接收确认时的当前设计，删除只发送目标 id。保留已有列表、对话框和提示在客户端变更期间的显示语义，仅隔离旧请求的后续更新。

## Existing notes audit

已检索 proposed、implemented 和 rejected。没有同范围归属或过时提案需要吸收。

- [编辑器平台控件](../../implemented/feature/2026-10-12-skin-editor-platform-controls.md)持有布局、预览和可访问名称，部分重叠，继续保留。
- [抽卡模块](../../implemented/simplification/2026-10-14-ai-skin-generation-module.md)与[抽卡状态](../../implemented/simplification/2026-10-17-ai-skin-generation-state.md)持有生成、保存提案与发布会话；编辑器库操作不发起抽卡，不合并。
- [社区皮肤库](../../implemented/simplification/2026-10-12-community-skin-library-generation.md)持有社区卡片的获取和即时恢复，部分重叠但操作语义不同，继续保留。
- 原生皮肤库的目录句柄、锁、格式和云同步笔记无关；本片不改底层文件操作或原生宿主。

## Alternatives considered

- 只抽出加载和 `mutate` 包装：迁移最小，能复用忙碌和错误转换；但名称草稿、确认目标和提交分支仍留在 JSX，独立入口还要重写完整流程。采用完整领域 hook。
- 复用 `useCommunitySkinLibrary`：它已维护本机列表和过期读取；但还依赖社区下载、试用恢复与偏好应用，并不拥有命名和确认，套用会引入无关服务与生命周期。

## Verification

迁移前原编辑器和生命周期契约 3 个文件共 5 项通过。新公共 hook 的 14 项测试先因入口未实现而失败；抽取后与原组件、契约合计 4 个文件共 19 项通过，覆盖完整 CRUD、名称字素限制与空白、取消、错误重试、同步互斥、客户端更替、卸载、实例隔离和 StrictMode。

设置整页的皮肤库与编辑器集成定向运行 3 项通过；桌面生产构建（含类型检查）、鸿蒙类型检查、全仓 lint、变更代码格式与 diff 校验通过。lint 仅保留未改动文件的两条既有警告，生产构建保留既有大包提示。独立只读审查无阻断或重要问题，另行复跑上述 4 个文件共 19 项通过。

提交前 `scripts/verify-local.sh --quick` 通过，包含 Rust 工作区、Android 目标、Linux 桌面外壳和共享 Apple bridge 检查；按范围或环境跳过的阶段见脚本输出。

验证范围是共享 React 状态与交互，不代表原生宿主装机或真实输入法验收。

## Consequences

已经发出的服务变更无法撤销，但旧结果不能改变新客户端的 UI 或继续触发使用设计。设计编辑与历史由[公共设计编辑状态](2026-10-24-touch-skin-editor-state.md)持有；编辑器继续拥有分类、布局和 `onUse`，皮肤库 hook 不引入文件格式、存储或协议。

公共入口可独立组合库操作与自定义布局，编辑器减少约百行状态与操作编排。代价是新增领域 hook 与公开表单类型；设计、分类和应用回调需要调用方接线。底层服务继续拥有格式、容量、锁与存储校验。
