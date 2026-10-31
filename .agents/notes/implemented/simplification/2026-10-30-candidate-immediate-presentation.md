# Agent Note: 所有平台的候选立即呈现

Status: implemented

## Problem

候选栏的按压、展开、显隐与高亮过渡在各宿主中独立实现，用户要求先取消 iOS，再取消其他平台的同类动画。只处理移动端箭头会留下桌面皮肤的淡入、逐行浮现、退场与选中弹簧，候选仍要等待过渡才能看清。

## Decision

iOS、Android、鸿蒙、macOS、Windows、Linux 和网页候选栏直接显示当前状态。取消候选词块按压过渡、展开箭头旋转以及候选进场、退场、逐行揭示和高亮动画。保留按下态、候选选择和长按菜单；输入在原来的时机提交。皮肤的图片帧动画、装饰图待机动效及用户单独选择的打字特效保持独立。

共享皮肤清单继续接受原有动画字段，保证旧包能读取和同步，宿主忽略候选界面过渡。网页 `resolveSkin` 继续解析兼容数据，候选 DOM 不再挂载动画和延迟隐藏定时器。macOS 展开和收起移除 `NSViewAnimation` 及旧位置采集，原窗口与候选按钮直接设置最终布局。

## Historical audit

[iOS 立即反馈](../../implemented/bug-fix/2026-10-19-ios-nine-key-immediate-press-feedback.md)的候选范围扩展到其他宿主，九键规则继续成立。[皮肤 v2 动效契约](../../implemented/architecture/2026-10-13-candidate-skin-v2-motion-contract.md)与 [macOS 声明式动效](../../implemented/feature/2026-10-16-macos-candidate-declared-motion.md)的候选呈现决定由本篇取代，共享格式保留。[Linux 原生候选默认](../../implemented/architecture/2026-10-21-linux-x11-native-candidate-default.md)部分重叠，其布局、指针和定位规则不变。[装饰图待机与 APNG](../../implemented/feature/2026-10-16-skin-decoration-idle-motion-and-apng.md)独立于候选界面过渡，继续成立。[macOS 自适应数量](../feature/2026-10-25-macos-adaptive-candidate-count.md)部分重叠，其展开窗口过渡由本篇取代，分页、尺寸与滚动规则继续成立。未发现需要拒绝的过时提案。

## Alternatives considered

- **只取消内置动画，保留皮肤显式声明的过渡**：保留皮肤作者的呈现选择，但安装带过渡的皮肤后候选仍会淡入、退场或弹跳，不能满足用户对所有平台取消候选动画的要求。
- **删除共享清单的动画字段**：可以缩小格式，但旧皮肤和同步数据会失去兼容性，超出本次呈现改动，因此只在宿主执行层取消。

## Verification

- iOS 键盘扩展模拟器 Debug 构建与 Swift 语法检查通过。
- Android Java/API/JNI 检查、316 项 JVM 冒烟和 256 份设备测试源码编译通过。HarmonyOS 逻辑套件与先安装模块后的实际 ArkTS/HAP 构建通过。
- macOS 完整原生构建通过；候选面板、行布局、用户几何、自适应展开、显式皮肤、高亮、阴影、快捷键及展开滚动回归通过。展开/收起反复切换时同步到最终尺寸，两种系统动态效果设置都覆盖；AppKit 的窗口坐标允许一像素取整。现有装饰待机和 APNG 生命周期测试保留。
- Windows 候选窗口头文件与实际消费方通过 MinGW 交叉编译语法检查。Linux 原生画布严格警告构建与绘制回归在 macOS 上通过，显式动画字段不会启用候选过渡。
- 网页 28 项皮肤单元测试、JavaScript 语法检查及 Chrome 浏览器回归通过；用 `getAnimations` 检查没有候选过渡，组字结束同步隐藏。
- 远端 `verify-local.sh --quick`、本机 `pnpm run verify-notes` 与 `git diff --check` 通过。quick 中缺少 Docker、MinGW、Node/pnpm 或鸿蒙暂存产物的阶段按提示跳过，相关可执行检查如上单独覆盖。

未安装或发布，未在移动真机、Windows 系统输入入口或 Linux 真实 X11/Wayland 编辑器中验收；macOS 原生测试宿主与合成会话不能代替已安装输入法的真实编辑器验收。

## Consequences

依赖皮肤候选过渡的测试需要改成即时状态断言，不能删掉候选选择和动态资源的验证。失去过渡的平滑效果属于本次用户要求；不将取消过渡解释为引擎查询提速。
