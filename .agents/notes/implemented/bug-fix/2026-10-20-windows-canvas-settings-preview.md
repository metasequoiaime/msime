# Agent Note: Windows 原生设置与社区预览绘制 canvas

Status: implemented

## Problem

Windows 实际候选窗已支持 canvas，原生设置的外部皮肤卡片和社区「生成预览图」仍只看传统背景及装饰带。用户看不到九宫格和自由挂件，发布时还会把错误的纯色样例写回皮肤。

## Decision

两处共用一个离屏静态 Direct2D 预览，在扫描工作线程读取由共享 catalog 验证的图片字节，复用候选窗的九宫格、挂件锚点与分层、内容内边距、外留白和行距算法。图片解码复用真实候选窗的 `AnimatedImage::DecodeMemory`，GIF 首帧保留逻辑画布与偏移，不直接拉伸 WIC 子帧。先确认面板完整解码，成功后才使用 `canvas_panel_drawn` 配色；坏图用安全配色。卡片异步加载生成的 PNG，继续沿用已有扫描／解码代次围栏。社区导出采用该包支持的明暗与排列。

预览只展示该包，不带用户取色器。主题响应提供基础配色，catalog 的 `candidate`／`panelCandidate` 补入候选正文、预编辑文字、底带与分隔线这些静态样例实际绘制的细分槽位；面板坏图时整套回退，不残留贴图专用预编辑色。

共用绘制头带入 WRL 的全局 `Microsoft`／`Windows` 命名空间，设置文件的界面名称显式绑定到 `winrt` 投影，原生 `ComPtr` 使用 `::Microsoft::WRL`，避免只在 MSVC 的 WinUI 构建中暴露的命名空间歧义。

## Historical audit

- [原生皮肤卡片](../../implemented/feature/2026-10-11-windows-settings-custom-theme-skins.md)部分重叠：保留选择、槽位和异步扫描，本篇补绘制。
- [Windows canvas 候选窗](../../implemented/feature/2026-10-19-windows-candidate-canvas.md)部分重叠：复用纯几何，不另开候选窗口。
- [图片动画](../../implemented/feature/2026-10-19-windows-skin-image-animation.md)保留：设置与社区图片是静态样本，不接动画计时器。
- 其他平台的 canvas 记录保留，各自宿主绘制不被本篇取代；检索中其余预览笔记不负责 Windows canvas 绘制。

## Alternatives considered

- **复用真正候选窗创建再截图**：文字、材质和动画可完全一致，但设置扫描会产生窗口、设备和消息循环，与用户正在输入的候选宿主争资源；不采用，复用不依赖窗口的几何计算。
- **分别在 WinUI 和社区绘制九宫格**：卡片容易接动画，但两份代码会再次漂移；先共用有界离屏静态绘制，卡片明确只是样例。

## Verification

新增合成九色面板测试覆盖浅深色、横竖排、前后层挂件、外留白、行距、坏图安全配色与画布分配上限。MinGW `-std=c++17 -Wall -Wextra -Werror` 编译链接通过。32 位 MinGW 语法检查、既有图片解码器语法、MSBuild XML、设置导航契约、笔记与差异检查通过。真正 Windows 的像素测试与 WinUI 设置卡片浅深色截图由 PR CI 执行，结果以对应提交的作业为准；不把交叉编译计为运行验收。

本地 Wine 10 首轮像素测试失败，面板只有左上角色块被拉伸；已对照其固定 `wine-10.0` 的 `dlls/d2d1/brush.c:1345–1388`，图像画刷按整个 bitmap 计算坐标，没有使用 `source_rect`。微软 `ID2D1ImageBrush::SetSourceRectangle` 文档明确源矩形左上角映射画刷原点，当前代码与固定上游一致。保留全部像素断言，由 Windows runner 执行；不改产品去适配 Wine，不增加已知失败豁免。

Wine 批量运行器会自动枚举新 EXE，因此对这一套件显式输出带原因的 `SKIP`，不计作通过；`verify-local.sh` 的总门禁同时显示跳过数和原因。原生 Windows CI 直接执行同一 EXE，保留全部断言，任何非零退出都会使作业失败；没有运行时自动检测 Wine 并放行的路径。Wine 补齐图像画刷源矩形语义后移除运行器的环境跳过。

## Consequences

两个实际入口不再另画缺少 canvas 的纯色卡片，代价是扫描时顺序生成至多四张有界静态位图。整个预览限制 8192 边长、16 × 1024 × 1024 像素，社区输出保持 256 KiB 上限。离屏预览仍不是输入法真实窗口验收；WIC 缺少某种图片解码器时需要安全回退，不能强行采用贴图专用文字色。导出沿用 256 KiB 预算，静态图不表示动画已验收。
