# Agent Note: Fcitx5 使用 classicui 九宫格绘制 canvas 面板

Status: implemented

## Problem

Linux 的公共候选目录只携带配色与旧顶部装饰，canvas 包虽然通过共享层扫描，Fcitx5 仍只画纯色回退。classicui 已有九宫格背景，但源切片和显示边宽共用同一个 Margin，不能直接映射 `stretch_dip` 与 `width_dip` 不同的皮肤。

## Decision

沿用 [Linux 的原生面板归属](../architecture/2026-10-13-cross-platform-candidate-rendering-boundaries.md)，不另开候选浮窗。在含 canvas 包的公共目录顶层发布绝对 `directory`，旧条目保持不变；Fcitx5 只在主题输入变化后通过现有 `msime_client_skin_package` 读取共享层校验的包。IBus 继续按原来的条目解析，不启用贴图配色。

把静态 PNG／APNG 首帧的源九宫格预先规范化成 classicui 所需的边宽，生成有界的 1x／@2x PNG，并映射内容与外留白。资源解码成功后才通过 `skins_directory` 与 `canvas_panel_drawn` 请求同一个共享配色解析器。无法读取或解码时保持可读回退；写盘失败保留旧主题并重试。继续尊重第三方主题所有权、原子写入与候选身份。

这是一段原生绘制增量，动画、其他图片格式、自由挂件、其他 Fcitx UI 与 IBus 的实际绘制仍待补齐，不能据此宣布全平台兼容完成。

资源热更新后请求真实窗口重绘，以及横排／竖排 GTK 像素验收，见[资源刷新与窗口验收](../bug-fix/2026-10-19-linux-canvas-gtk-acceptance.md)。本篇的配置测试不替代该显示层证据。

## Alternatives considered

- **直接复制源图并使用 `slice_dip` 作为 Margin**：成本低且 classicui 原生支持，但源切片与显示边宽不同时会改变皮肤边框比例；规范化每个切片后再交给 classicui 拉伸。
- **在公共条目里加入完整 canvas 清单**：不用另外调用包 API，但旧解析器严格拒绝未知字段，且会复制已有共享清单协议。顶层只发布资源根目录，复用已存在的 API。
- **普通 Cairo／Pango 顶层浮窗**：可以自行播放动画和任意层级挂件，但不能获得所有 Wayland 合成器上的输入法 popup 定位与指针路由保证，维持现有面板所有权边界。

## Historical audit

- [跨平台清单兼容](../feature/2026-10-19-windows-skin-schema-parity.md)：共享扫描和可读回退仍有效，本篇补 Fcitx5 的实际静态贴图通路，不更改其他宿主的颜色协商。
- [候选渲染边界](../architecture/2026-10-13-cross-platform-candidate-rendering-boundaries.md)：面板、定位与指针仍归桌面 UI，本篇只扩展已有 classicui 主题映射，不翻转窗口所有权决定。
- [旧顶部装饰几何](../bug-fix/2026-10-07-fcitx5-overlay-geometry.md)：非 canvas 路径原样保留；canvas 面板不叠加品牌图与旧装饰带，避免污染九宫格图案。
- [原生宿主生命周期](../architecture/2026-10-13-candidate-skin-v2-native-host-lifecycle.md)：静态首帧不构成动画播放，仍保留 Linux 动画与 compositor 验收缺口。

## Verification

固定容器内的完整 Linux 门禁通过：92 项原生测试通过，包含真实 classicui 与共享包／配色 API 的加载、明暗切换、文字取色、资源更新、坏图回退与修复、第三方主题所有权。全拼与五笔插件均完成构建。

独立 Cairo 像素测试覆盖源切片与显示边宽不同、透明中心、1x／@2x、内容／外留白、坏图与尺寸预算、符号链接／硬链接拒绝、写盘失败保留旧主题。已有 Fcitx5 契约、特殊文件读取测试通过。固定上游提交 `dd018f79858314ed542a47c528608752b13bd261` 的夜樱、绿叶资源完成生成贴图目测；这不是实际候选窗口截图。

最新 `develop` 基线上的共享服务单测 1199 项通过、1 项忽略；`cargo clippy --locked -p msime-client-services --all-targets -- -D warnings` 通过。提交前 `bash scripts/verify-local.sh --quick` 通过，覆盖本机可运行的 Windows 策略测试 170 项、Rust workspace、Android Rust／Java、WebAssembly、Linux 设置壳与原生宿主、macOS 原生宿主和共享 Apple bridge；完整 Windows 交叉构建及 HarmonyOS 打包因远端工具链不全而跳过。真实桌面与 compositor 验收仍未完成。

## Consequences

Fcitx5 classicui 获得可维护的静态 canvas 面板通路，旧目录条目和其他宿主的可读回退不变。图片先准备完成再原子更新主题，失败保留旧配置；文件戳缓存避免每拍扫描或解码。

classicui 只支持整数边距；非整数 DIP 会向上取整。规范化贴图占用额外内存，但源图与输出尺寸、文件字节数都有上限。APNG 只显示默认帧；GIF／WebP／JPEG 面板、动画、自由挂件、行间距、IBus 及其他 Fcitx UI 的绘制仍待补齐。实际桌面、Wayland 客户端面板和其他 UI 的验收不能由生成 PNG 的像素测试代替。
