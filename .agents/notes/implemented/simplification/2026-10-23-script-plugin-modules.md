# Agent Note: 脚本插件模型与授权对话框独立维护

Status: implemented

## Problem

`plugin-script-view.tsx` 同时持有能力规则、授权状态、运行顺序、指令状态说明、授权对话框和详情页。列表与 `usePluginsSection` 读取纯规则时需要导入详情视图，模型与 UI 的依赖方向不清楚；单独复用授权对话框也绑定了整页实现。

## Decision

`plugin-script-model.ts` 集中已有纯函数和静态说明，只依赖插件类型；列表、插件库和状态 hook 直接引用模型。`script-grant-dialog.tsx` 持有公开的 `ScriptGrantDialog` 与属性类型，`use-script-grant-dialog.ts` 管理授权选择、忙碌和失败恢复；DOM 焦点、标题关联、键盘和遮罩事件仍在对话框组件。`plugin-script-view.tsx` 只组合详情，并保留原模块的公开重导出；包入口现有 API 不变。

不改变能力顺序、依赖及互斥规则、先取启用的前 8 项再核对哈希、包更新后只提交当前清单仍申请的能力、确认失败保持对话框、初始焦点在取消和关闭后回到打开控件等契约。对话框确认成功后仍由调用方关闭。

## 旧笔记审计

精准检索活跃笔记中的 `plugin-script-view`、`ScriptGrantDialog`、`runningScriptIds`、`scriptMarker`、`toggleCapability` 和 `scriptGrantState`。[脚本授权页](../../implemented/architecture/2026-10-12-script-grants-settings-page-without-keychain.md)持有授权和安全文案的语义，与本片部分重叠，补充模块归属并互链；[插件设置状态与导航](../../implemented/simplification/2026-10-23-plugins-section-state.md)持有目录、授权写入和页面代次，本片只更新它所调用规则的实现位置，原决定继续有效。[脚本指令行](../../implemented/architecture/2026-10-13-script-plugin-command-rows.md)和 [Windows 授权页](../../implemented/architecture/2026-10-12-windows-script-grants-page.md)部分重叠，仅同步文案与规则的来源模块，Windows 模型头文件的对应来源注释也同步。运行时与官方包笔记归属宿主或业务规则，无模块边界重叠；没有过时提案或需要吸收的旧笔记。

## Alternatives considered

- **只增加一个文件重导出原视图的函数**：迁移最少、接口自然兼容，但模型仍依赖完整视图，无法实现无 React 的独立验证，因此不采用。
- **只移动对话框 JSX，继续把授权状态留在详情组件**：可减少文件长度，但对话框需要从父级接收内部选择、忙碌和错误，独立复用的接口更大。授权状态属于对话框，独立 hook 保留原生命周期。
- **把静态文案与纯规则再分成更多小模块**：可进一步缩小导入范围，但它们都围绕同一份脚本展示模型，拆散后增加交叉依赖；先保持一个无 React 的模型边界。

## Verification

- 迁移前 4 个文件、101 项行为测试通过；源码边界检查先因模型尚未拆分而失败，迁移后 10 个文件、110 项测试全部通过。
- Node 环境直接验证模型的八项顺序与哈希筛选；真实对话框覆盖确认进行中禁止取消和失败后可重试。模型与基点逐字核对，仅排序函数增加导出；详情实现保持不变。
- 独立审查未发现阻断问题，另跑 8 个定向文件、105 项测试全部通过。
- 桌面类型检查与构建、Harmony 类型检查、11 个变更 TS/TSX 文件的 lint 与格式检查、笔记校验和 `git diff --check` 通过。

- Linux 容器首次检查因默认 DNS 无法解析依赖站点而失败；指定主机 DNS 后，同一源码的 `cargo check -p msime-desktop --locked --all-targets` 通过，随后完整本地 quick 重跑通过。Windows 本机 164 项通过，Rust 工作区、Android 目标、Linux 外壳和共享 Apple 桥接编译通过；缺少 MinGW 与 Harmony 打包环境的阶段按脚本跳过，未进行原生宿主装机验收。

## Risks

包在授权对话框打开后变更时，不能用新 `initial` 覆盖正在编辑的选择，只把提交和互斥判断限制在当前清单。模型函数的名称、返回值与文案保持原样，重导出防止内部路径与公开包入口的旧调用方失效。只改共享 React 模块，不涉及文件读写、原生资源或设备验收。

## Consequences

列表和业务状态不再加载脚本详情组件以取得纯规则，模型可在无 DOM 的 Node 环境独立验证。授权组件保留原 props 和 DOM，状态 hook 不接管宿主写入、页面代次、对话框关闭或焦点恢复；原包入口与模块重导出兼容。新增模块只表达已有职责，不增加行为、权限或持久化格式。
