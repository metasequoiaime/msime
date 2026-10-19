# Agent Note: iOS App 明确声明 iPad 设备族

Status: implemented

## Problem

设置宿主已经实现了 iPad regular width 的 `NavigationSplitView` 和平板键盘控件，但生成的 App `Info.plist` 只有 `UIDeviceFamily = 1`。iPad 因此只能以 iPhone 兼容模式运行，平板设置分栏与平板专属控件无法进入；XcodeGen 默认值随版本变化，不能把注释里的默认行为当作契约。

## Decision

在 `platforms/ios/project.yml` 和已提交的 Xcode 工程配置中显式设置 `TARGETED_DEVICE_FAMILY = "1,2"`，App 与键盘扩展的 Debug、Release 配置保持一致。测试在 iPad Air 13 英寸的 regular width 上验证分栏设置；iPad mini 竖屏为 compact width，按设计继续使用手机设置栈。

## Alternatives considered

- 只在测试命令行传 `TARGETED_DEVICE_FAMILY=1,2`：能让本机验收进入平板，但发布工程仍可能生成 iPhone-only 包，不能修复产品配置。
- 重写 `TabletSettingsView` 让所有 iPad 宽度都强制显示分栏：会破坏 iPad mini 竖屏和窄窗口应使用手机布局的既有契约，因此保留 size class 判断。

## Verification

- `UIDeviceFamily` 在新模拟器 App 包中包含 1、2。
- iPad Air 13 英寸模拟器 `OnboardingUITests/testTabletSettingsSidebarSelectsPagesAndShowsTabletControls` 通过 9.197 秒；截图已人工查看，左侧设置栏、右侧键盘布局预览以及 `数字行与 Tab 键`、`横屏分离式键盘` 控件均显示。
- iPad mini 竖屏手工启动显示手机宽度设置页，符合 compact width 回退规则。

## Consequences

发布包会正式声明 iPhone 与 iPad，iPad regular width 的页面终于可达；测试需要区分 compact 与 regular width，不能只按设备型号判断。键盘扩展也同步声明设备族，避免宿主与扩展的可安装范围不一致。
