# Agent Note: 触屏键盘工具栏偏好与设置分组独立于几何控件

Status: implemented

## Problem

`TouchKeyboardGeometrySection` 同时持有几何、布局、手势和工具栏设置，工具栏八项偏好类型及缺省合并也定义在 JSX 中。总偏好因此依赖几何展示组件，独立使用工具栏分组时还要带入尺寸和手势的属性。

已检索 `TouchToolbarPreferences`、`defaultTouchToolbar`、`touch_toolbar`、工具栏按钮与顶部语音入口。[几何偏好](../../implemented/simplification/2026-10-14-touch-keyboard-geometry-preferences.md) 持有尺寸和间距规则，[鸿蒙键盘页布局](../../implemented/feature/2026-10-11-harmony-keyboard-page-layout.md) 持有布局与尺寸分组，[Windows 原生屏幕键盘页](../../implemented/feature/2026-10-11-windows-settings-screen-keyboard.md) 持有原生同步与重置规则，三者与本片相邻但归属不同，均沿用。[Android 工具栏启动状态](../../implemented/bug-fix/2026-10-08-android-toolbar-startup-state.md) 和其他命中处理原生菜单、首帧或宿主入口，不改其决定。

## Decision

纯模块 `touch-keyboard-toolbar-preferences.ts` 持有原工具栏类型、语音入口类型和按原对象展开顺序计算有效按钮偏好的函数；默认对象保持内部所有。`TouchKeyboardToolbarSection` 只负责工具栏组，几何组件组合它并保留原属性及类型重导出，总偏好直接依赖纯模块。不增加包入口导出。

## Alternatives considered

- **只移动类型和缺省对象**：改动最少，总偏好可以摆脱 JSX 依赖。但几何组件仍持有完整的工具栏 UI，独立使用还需提供无关尺寸属性，因此同时拆出有独立职责的设置分组。
- **让工具栏模块负责重置**：入口更集中。但恢复默认同时删除几何、语音和按钮偏好，属于整页操作，移入工具栏会改变原有责任与范围，因此保留既有重置 hook 和父组件说明。

## Consequences

- 八项字段、缺省值、按钮顺序与中文文案不变；显式 false 覆盖缺省，合并不修改输入，编辑一项保留其他按钮选择。
- 语音入口按开始输入、最近结果、隐藏三种宿主语义显示；隐藏语音且不支持按钮配置时没有空分组，只有按钮时正常显示。
- 几何组件属性继承独立工具栏属性，避免两份契约漂移；旧组件类型和包入口 API 兼容。恢复默认仍覆盖整页既有范围，尺寸、布局与手势保持。
- 增加两个职责明确的内部模块，不增加包入口导出。默认对象仍是模块私有，解析每次返回新对象；选项序列供组件与契约测试共用。

## Verification

几何组件基线和回归覆盖两种尺寸布局、原工具栏事件、语音文案与隐藏条件、布局与手势。独立工具栏测试覆盖不提供几何属性时渲染、仅按钮组、编辑保留其他选择、仅语音和空组消失。纯规则覆盖默认值、字段顺序、部分偏好、显式 false 与输入不可变；整页回归核对工具栏保存、语音和几何保存及手机重置。类型探针将旧几何属性与现接口逐字段对照，核对原类型导出并拒绝新增包入口 API；另跑桌面构建、Harmony 类型检查、34 组字段契约、格式、lint、笔记和 quick。
