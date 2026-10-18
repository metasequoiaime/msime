# Agent Note: 面板宿主能力移入公共模块

Status: implemented

## Problem

`usePanelHost`、`useHostPlatform` 和 `panelSurfaceTheme` 同时服务设置外壳、屏幕键盘和其他桌面面板，但实现放在 `desktop-keyboard.tsx`。设置入口因此要从键盘组件文件借用宿主能力逻辑，新增面板时也容易继续把无关的键盘实现当作依赖边界。

## Decision

面板宿主能力契约放在 `apps/desktop/src/input/panel-host.ts`。该模块只负责从设置客户端或 Tauri 外壳取得宿主能力、管理异步响应的挂载生命周期，并根据 `surface_themes_follow` 投影面板主题；平台面板的布局、偏好读取和业务动作仍由各自调用方拥有。

`desktop-keyboard.tsx` 继续转导出原有三个符号和两个类型，保持已有内部导入路径可用；新的设置入口直接依赖 `panel-host.ts`，不再依赖屏幕键盘模块。面板窗口在能力尚未返回时继续使用 `resolved: false` 的安全默认，Tauri 不可用或查询失败时继续视为已解析且没有宿主。

## Alternatives considered

- **继续从 `desktop-keyboard.tsx` 导入**：改动最少，但设置入口与键盘文件保持反向依赖，其他面板复用时仍没有清晰的宿主能力边界，因此不采用。
- **把宿主能力逻辑复制到 `main.tsx` 和每个面板**：可以避免跨文件导入，但会复制 Tauri 查询、错误回退和主题跟随规则，后续平台能力字段容易漂移，因此不采用。
- **把逻辑下沉到 `packages/ui`**：共享 React 设置包无法直接依赖 Tauri `invoke`，而面板宿主能力是桌面外壳协议，不属于跨平台 UI 契约，因此不采用。

## Consequences

桌面设置入口和屏幕键盘现在共享一个明确的宿主能力模块，新增面板可以直接复用同一生命周期与主题投影。旧的 `desktop-keyboard` 导出仍保留，减少内部迁移风险；代价是该文件暂时有一层兼容转导出，待所有调用方迁移后仍可单独清理。没有改变 `host_capabilities` 命令、主题跟随规则、默认间距或任何原生资源和权限行为。

验证覆盖模块契约测试、桌面类型检查、桌面生产构建、相关键盘测试、格式检查、笔记校验和 quick 门禁。
