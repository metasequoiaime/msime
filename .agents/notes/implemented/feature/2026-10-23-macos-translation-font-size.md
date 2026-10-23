# Agent Note: macOS 候选译文字号独立可调

Status: implemented

## Problem

#9274 的平台字段写 Windows，正文明确写 macOS，附件用另一款输入法的「译文字号」作参考。macOS 当前在普通横排、竖排和展开候选的测量与绘制中固定使用 12pt；增加候选字号不会放大译文，用户无法单独调整阅读大小。

## Decision

共享 Rust 偏好提供可选的 `candidate_translation_font_size`，允许 12–32 点整数；缺省或 `null` 恢复 12pt，缺省不序列化。macOS 共享设置页在「候选窗口 → 字体与大小」提供「译文字号」，带默认选项；其他宿主保留偏好但暂不展示控件。原生回退设置窗口冻结，不新增控件。

macOS 原生偏好读取该值。所有候选页的宽高测量、译文预留高度与实际绘制共用同一字号，继续乘整窗缩放；译文下的读音也按同一字号绘制。字段省略或清空时清除已经载入的覆盖，非法值不替换现有合法值。候选内容没变时仍重新测量，既有的原地更新路径同步替换译文字体，不增加另一份字号缓存。

## Historical audit

[候选字号叶子契约](../../implemented/simplification/2026-10-15-candidate-preferences.md)持有 TypeScript 字段归属，本项沿用它；[读音独立行](../../implemented/bug-fix/2026-10-16-macos-candidate-pronunciation-own-line.md)持有多行测量，不改变其排版规则。[皮肤出厂布局](../../implemented/feature/2026-10-20-skin-factory-layout.md)、[Windows 默认几何](../../implemented/feature/2026-10-20-windows-candidate-mac-default-geometry.md)和 [Linux 自绘候选](../../implemented/architecture/2026-10-21-linux-x11-native-candidate-default.md)的默认字号仍成立，本项只为 macOS 增加显式覆盖。

## Alternatives considered

- **跟随候选字号按比例放大译文。** 不增加偏好和控件，主字体变大时译文一起变大；但用户无法只放大译文，且已有用户的候选布局会随升级变化。使用独立可选偏好，默认保持 12pt。
- **仅在绘制时换字体。** 改动少，但窗口仍按 12pt 测量，长译文和多行读音可能被裁切；测量、预留空间与绘制必须一并读取覆盖值。

## Verification

- UI 新用例先在原实现上因缺少「译文字号」失败；实现后相关三文件 11 项测试通过，覆盖数值与 `null` 补丁、其他宿主隐藏、载入不写偏好。TypeScript 检查、Vite 生产构建通过。
- 共享偏好字段一致性、交叉类型解析与 macOS 消费覆盖检查通过。
- `AppearancePreferences.mm`、`InputController.mm` 和两个原生测试源文件在现有 CMake 编译参数下通过 `clang++ -fsyntax-only`（`-Wall -Wextra -Werror`）；语法检查没有运行测试、链接原生宿主或安装输入法。
- 新增共享偏好缺省、边界和序列化往返测试通过；`cargo test -p msime-client-core --lib --locked` 的 506 项全部通过，`cargo clippy -p msime-client-core --all-targets --locked -- -D warnings` 通过。同一测试文件中两处已有的 `field_reassign_with_default` 警告改成结构体初始化，未改变测试数据和断言。
- 当前源码的 Host API 静态库已构建，重新链接的 `candidate-appearance-defaults` 原生测试通过，覆盖合法值、非法值、`null` 和省略字段。独立的 `shortcut-translation-font-size` 原生测试通过：普通候选覆盖横排／竖排、100%／150% 缩放和 12／21／32／默认字号共 16 组，并执行实际位图绘制；展开候选通过 ↓ 事件进入，覆盖 150% 下的 21／32／默认字号及行高。独立入口使这些断言不依赖完整 `shortcut` 的前序用例。
- 静态审查及独立字号测试复审均无阻断问题。`shortcut-translations` 首次在既有 `TestCustomTranslationIdleDelay` 的请求数量断言失败，同一二进制单独复跑通过（82.44 秒）；记录首次失败，不把复跑通过视为该延时用例稳定性的证明。
- 首轮 quick 中，Windows 宿主策略 164 项、鸿蒙设置页构建、笔记检查、Rust 工作区、Android 目标、Wasm 编译和 Clippy 通过；iOS 豆包传输检查首次因 120 秒构建超时失败，独立复跑 17 项通过。系统盘可用空间随后降至约 1GB，在 Android Java 阶段停止该轮门禁；将临时文件改放 Workspace 卷后重新运行，完整 quick 仍待补齐。

## Consequences

用户可独立放大译文，恢复默认后与旧版一致；译文变大后每屏容纳的候选会减少。代价是共享配置增加一个可选字段和 macOS 的读取接线。Windows 字段与正文冲突仍存在，本项按明确正文实现 macOS，不声明 Windows 同时完成。本机系统盘低于仓库 50GB 新构建树门槛，本次复用已经存在且空闲的标准 `target/` 缓存补验证，不复制其他工作树的源码；尚未安装原生宿主，不能据语法检查或独立偏好测试宣称装机验收完成。
