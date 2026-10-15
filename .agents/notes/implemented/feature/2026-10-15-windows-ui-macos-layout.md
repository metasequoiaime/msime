# Agent Note: Windows 界面采用 macOS 的分组与选中样式

Status: implemented

## Problem

Windows 设置窗口把每一项设置画成独立的高卡片，侧栏和页边距挤占正文；候选窗使用蓝色文字与竖线标记选中。macOS 使用紧凑的分组卡片和实心绿色选中底。两边功能接近，视觉层级却不同，用户要求 Windows 参考 macOS 改版。本篇接管[Windows 候选窗口补齐 macOS](2026-10-10-windows-candidate-window-mac-parity.md)中的默认外观，候选内容及交互约定继续沿用。

## Decision

Windows 保留 WinUI 3 设置宿主与 Direct2D 输入浮层，以 macOS 的 `SettingsLayout.h`、`NativeCandidateTokens` 为布局和默认颜色依据。设置以功能组为卡片，标签与控件分列，窄行把控件移到标签下方；搜索保留设置项检索。候选窗、工具栏和托盘共用绿色强调色与浅深色表面，用户主题及皮肤覆盖继续优先。

## Alternatives considered

- **让 Windows 承载共享 React 设置页** — 能直接共享页面实现，但替换原生宿主会影响已有账号、社区及文件对话框路径，超出视觉改版需要；保留原生宿主并调整构件。
- **只修改颜色** — 风险小，但独立大卡片、宽侧栏和密度仍旧不一致，不能满足布局改版。

## Consequences

统一基础构件会覆盖全部设置页，也要求检查特殊表单、搜索定位和窄窗口。字体与标题栏仍采用 Windows 原生能力。MinGW 不编译 WinUI 3，必须独立核实设置工程的编译与运行证据。

## Verification

`tests/ui/candidate_palette.cpp` 验证原生默认值、显式主题槽位优先、强调色推导和半透明选中底的前景色；`candidate_theme_reload.cpp` 验证主题重新加载后的实际颜色。共享前端运行 `typecheck`、构建、platform-tokens 和 candidate-preview-native-tokens 测试。`CI (platforms)` 的 `Windows native settings` 先构建真实 Rust host-api DLL，再使用发布用 WinUI 工程的完整 `Build` 流程，以 `/WX` 编译实际 `main.cpp` 并链接原生设置宿主。不能直接调用 `ClCompile` 或单独的投影生成目标：它们会跳过 Windows SDK 引用初始化或 WinRT 投影头生成。`tests/tools/settings_ui.ps1` 在独立合成状态目录中通过 UI Automation 遍历 18 页，保存浅深色、800／1000 宽窗口截图，关闭进程并清除合成状态；截图作为 CI 产物供视觉审查。

macOS 本机没有 Windows App SDK 运行环境，Windows runner 的结果和截图是窗口运行与视觉审查的证据；交叉构建及策略测试不能替代它。脚本覆盖导航与页面渲染，搜索后定位、字体输入框编辑及已登录账号／社区表单的写操作仍需对应功能的交互验收，不能由截图推出。
