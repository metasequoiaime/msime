# Agent Note: Linux canvas 面板复用共享图片解码

Status: implemented

## Problem

canvas 清单允许 PNG、APNG、GIF 和 WebP，Fcitx5 的面板却只调用 Cairo 的 PNG 解码器。同一个合法包在其他宿主显示贴图，在 Linux 上悄悄退回纯色。传统背景可用的 JPEG 不属于 canvas 面板或挂件的允许格式：固定上游 `dd018f79858314ed542a47c528608752b13bd261` 的 `test_candidate_skin_catalog_images.cpp` 明确断言 `bi-jpg`、`bi-light-jpg` 与 `dec-ext` 加载失败，本次保持这一规则。

## Decision

图片首帧解码归 `client-services::skin`，复用已有的 `image` 解码器和 WebP 预检。Host API 提供同步 RGBA 回调，输入只是有界内存，不读路径、不保存全局状态、不传 JSON 像素数组。回调里的借用字节只在调用期间有效；宿主转成 Cairo 原生字节序的预乘 ARGB，再沿用已有九宫格规范化、明暗槽位和成功后才启用贴图配色的路径。

保持 8 MiB 编码输入、2048 边长和 64 MiB 解码分配限制。PNG/APNG 保留默认图语义，GIF/WebP 使用第一幅合成画布；这不表示接通动画。读取仍走原有拒绝符号链接、硬链接与非普通文件的路径。

## Historical audit

- [Fcitx canvas 面板](../../implemented/feature/2026-10-19-linux-fcitx-canvas-panel.md)：部分重叠；新篇取代 PNG 独占的解码路径，几何和主题所有权仍归原篇。
- [跨平台绘制边界](../../implemented/architecture/2026-10-13-cross-platform-candidate-rendering-boundaries.md)：保留；不另建候选顶层窗口。
- [旧装饰几何](../../implemented/bug-fix/2026-10-07-fcitx5-overlay-geometry.md)：相关但不取代；本次复用已有 Rust 依赖，不给 C++ 引入新的图片库，旧装饰通路不变。
- [GTK 资源热更新验收](../../implemented/bug-fix/2026-10-19-linux-canvas-gtk-acceptance.md)：部分重叠；沿用真实窗口像素与焦点检查，扩展到 PNG、GIF、WebP 三类编码，不改变原有按输入变化请求重绘的决定。该篇已记录的两项快捷模式基线失败仍单独区分。

## Alternatives considered

- **引入 GdkPixbuf**：GTK 桌面普遍已有它，Cairo 转换也简单。但 WebP 依赖发行版额外安装 loader，支持范围会随系统变化，且重复维护已有的图片边界，因此不用。
- **返回 PNG 的 JSON 字节数组**：沿用已有字符串响应与 Cairo PNG 路径，宿主实现最少。但会额外压缩、序列化再解码整张图，内存开销大于同步像素回调，因此不用。

## Testing

共享层用例覆盖四种编码、透明度、APNG 默认图与动画的区别、WebP 内部帧几何、坏图、超限尺寸和输入；C ABI 验证空指针、回调拒绝及有效字节生命周期；Linux 像素测试保持九宫格与预乘透明度正确。真实 GTK/classicui 横竖排中检查 PNG、GIF 与 WebP 三类编码、明暗、损坏回退、修复、焦点与 `nihao` 选择 `你好`。专用验收镜像提供生成图片用的 Pillow，缺少 Pillow 的普通环境明确跳过扩展格式用例，PNG 用例仍可独立运行。

共享解码测试 5 项与 C ABI 测试 1 项通过，`cargo clippy --locked -p msime-client-services -p msime-host-api --all-targets -- -D warnings` 通过。JPEG 清单边界回归通过。相关原生几何、主题与真实 GTK 验收现为 16 项，全部执行通过，无跳过；其中 X11 与原生 Wayland 各有 PNG／GIF／WebP 横竖排六项窗口用例。Wayland 使用独立无头 Sway、关闭 Xwayland，实际虚拟键盘输入并截取合成器输出，覆盖输入后光标跟随与焦点保持。共保留 54 张明暗、移动、损坏、修复截图，已目测 Wayland GIF 横排深色与 WebP 竖排移动后的九宫格；镜像补充 Noto CJK，最新截图已显示中文，选词上屏仍独立按实际字符串断言。GTK 异步主题重载的有界后续重绘及夹具目录约束见[窗口验收笔记](../bug-fix/2026-10-19-linux-canvas-gtk-acceptance.md)。完整扩展套件的 `fcitx5-native-context` 与 `fcitx5-native-ai` 在已有 `Disabled expression mode must pass through` 基线断言处失败，不能报告整套通过。`verify-local.sh --quick` 最新 run `20261020-013823` 通过（5 分 38 秒），Windows 本机测试 171 项通过，包含仓库契约、Rust workspace、Android 目标与宿主 Java、WebAssembly、Linux 设置壳、Linux 原生 92 项、full／wubi 构建与安装隔离、macOS 原生目标和共享 Apple 桥接。构建机缺少 MinGW 与 HarmonyOS 原生前置，相关构建按脚本跳过；未安装前端依赖，HarmonyOS 设置打包检查跳过，不能把这些跳过当成平台运行验收。最终笔记校验、Rust 格式与差异检查通过。

## Consequences

合法包的静态面板不再因编码格式退回纯色，宿主无需依赖发行版的图片 loader。代价是增加一个有生命周期约束的 C 回调，Cairo 转换时短暂同时持有非预乘与预乘的画布。首次加载仍在宿主线程上解码，受输入与尺寸预算限制；每次偏好轮询不重复解码。动画、自由挂件、IBus 和其他 Fcitx UI 仍未接通，不能把这一步称作 Linux 完整视觉兼容。

X11 canvas 的窗口所有权由[原生 X11 画布](../architecture/2026-10-20-linux-native-canvas-x11.md)部分取代：有效 canvas 包、X11 输入上下文和水杉持有的 classicui 主题使用原生候选窗，挂件与独立行距由该路径绘制；其余上下文继续使用本篇的桌面面板路径。原生窗口的图片播放由[有界动画接口](../feature/2026-10-20-linux-canvas-animation.md)补充；本篇的单帧回退语义保持，Wayland 与其他 UI 的缺口仍保留。
