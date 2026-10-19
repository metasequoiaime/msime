# Agent Note: Windows 候选窗按皮肤垫系统材质（亚克力与云母）

Status: implemented

## Problem

皮肤的 `[candidate_window] backdrop = "acrylic" | "mica"` 和每种明暗的 `backdrop_opacity`、`backdrop_luminosity`，共享 catalog 早就按 msime-windows `c0a9d371` 读出来了，Windows 宿主却只做了来源在「材质不可用」时的回退：把 `surface` 补成不透明（见 [Windows 候选窗绘制皮肤的版式、外框、阴影、翻页箭头与收紧宽度](2026-10-19-windows-candidate-skin-layout-keys.md) 里被暂缓的那一项）。为 msime-windows 写的亚克力、云母皮肤在这里只是一张不透明的卡片。暂缓的原因是候选窗经 `msimeui` 的 `DeviceResources` 画进 DirectComposition 交换链，没有 Windows.UI.Composition 的模糊背景，而且 MinGW 交叉构建里的 WinRT 头文件没核实过。

## Decision

皮肤要材质、系统也允许时，`DeviceResources` 进入「材质模式」，用 Windows.UI.Composition 的桌面窗口目标代替 DirectComposition 交换链；其余时候（不要材质的皮肤、没有皮肤、材质画不出来）候选窗还是原来的交换链，一个像素都不变。

### 合成结构

- 视觉树是一个容器，底下一层精灵视觉对象垫材质，上面一层精灵视觉对象显示合成器的绘制表面（`ICompositionDrawingSurface`，按像素原样、左上对齐）。
- 候选窗的 `paint()` 不改画法：`GetRenderTarget()` 在材质模式下交出的设备上下文画进一张离屏位图（`ID2D1Bitmap1`，和交换链一样按 64 像素分档、只增不减），`Present()` 开一次绘制表面的 `BeginDraw`，把离屏位图按像素拷进去。多一次整面的 GPU 拷贝，换来调用方的 `BeginDraw` / `EndDraw` / `Present` 一行不改。
- 材质的来源：亚克力是 `ICompositor3::CreateHostBackdropBrush`（窗口后面已经模糊好的画面，Windows 11 上要给窗口设 `DWMWA_USE_HOSTBACKDROPBRUSH`）；云母是 `ICompositorWithBlurredWallpaperBackdropBrush::TryCreateBlurredWallpaperBackdropBrush`（模糊的桌面壁纸，Windows 11 才有），建不起来时按亚克力画。
- 色调按 msime-windows 的配方、与 WinUI `AcrylicBrush` 同一张效果图：模糊来源 → 亮度层（Direct2D Blend 的 COLOR 模式，叠 `surface` 颜色、不透明度 `backdrop_luminosity`，没写时亚克力 0.85、云母 1）→ 色调（LUMINOSITY 模式，不透明度 `backdrop_opacity`，没写时取 `surface` 的 alpha）。合成器把 Color 与 Luminosity 两种混合对调了，所以和 WinUI 一样反着写。两个不透明度都是 0 时直接用模糊来源。
- 材质按卡片外框线的外沿裁成圆角矩形（`ICompositor5` 的圆角矩形几何加 `ICompositor6` 的几何裁剪），圆角是卡片圆角加半个线宽，直角卡片还是直角。位置跟着进出场动效的平移、缩放和打字抖动（`candidate_backdrop_rect`），在 `EndDraw` 之前交给 `DeviceResources::SetBackdrop`，和这一帧的内容在合成器同一次提交里上屏，卡片伸缩时材质不会错开一帧。
- 垫了材质时卡片不画底色（色调已经在合成层里），阴影只画在卡片外面（Direct2D 层用「整块减去卡片」的几何裁剪），免得透上来压暗毛玻璃，同 msime-windows 的 `SetShadowOutsideOnly`；外框、背景图、文字照画。

### 回退链

规则在不碰 Win32 的 `CandidateBackdrop.h`，先后照 msime-windows：

1. 皮肤没要材质：什么都不垫，底色照旧。
2. 包有九宫格面板（`border_image`，`SkinSummary` 的 `borderImage`，只有 canvas 皮肤有）：面板盖住整张卡片，写了 `backdrop` 也不垫材质，面板照常画（[canvas 九宫格](2026-10-19-windows-candidate-canvas.md) 让面板代替底色、外框和阴影），同 msime-windows 按「包写了面板」判断、不看这一帧画没画出来。没有面板的 canvas 皮肤照常垫，材质和阴影裁剪都对着面板矩形（卡片加内容内边距）。
3. 这个窗口建过材质合成而失败了（低于 Windows 10 1903、合成器或效果画刷建不起来、`combase.dll` / `CoreMessaging.dll` 里的函数取不到、绘制表面出了不是设备丢失的错、窗口已经退成分层窗口）：以后不再尝试，`surface` 补成不透明。
4. 系统「透明效果」关着（`HKCU\...\Themes\Personalize\EnableTransparency` 为 0，读不到按开着）：`surface` 补成不透明。
5. 高对比度（`SPI_GETHIGHCONTRAST`）：`surface` 补成不透明。msime-windows 没有这一条；系统自己的材质在高对比度下也都换成纯色，候选窗跟着系统走。

后两项每帧读一次（只对要材质的皮肤），切换开关时系统广播的 `WM_SETTINGCHANGE` 让候选窗重画，换上对应的画法。设备丢失照旧走 `DiscardTarget`，下一帧在新设备上重建材质。

### 与用户偏好的先后

材质、它的两个参数和阴影只裁在外面都是皮肤决定的，Windows 上没有对应的用户偏好。用户的候选窗不透明度原来淡化卡片底色、背景图和外框，现在材质代替了底色，所以整块材质（视觉对象的 `Opacity`）也乘上它，再乘进出场动效的淡入淡出，和原来的底色一样。

### 两种工具链一份代码

MinGW 的 `windows.ui.composition.h` 只有最早一版的接口，没有 `ICompositor3/5/6`、壁纸模糊画刷、桌面窗口互操作、绘制表面互操作，也没有 `wrl/implements.h`；MSVC 的 Windows SDK 都有。`msimeui/src/CompositionInterop.h` 按 Windows SDK 元数据（与 windows-rs 0.62 从同一份元数据生成的 vtable 同序）自己声明用到的 ABI 接口和 IID，不包含任何 WinRT 头文件；`RoActivateInstance`、`RoGetActivationFactory`、`WindowsCreateStringReference` 从 `combase.dll`，`CreateDispatcherQueueController` 从 `CoreMessaging.dll` 运行时取，不加链接库，取不到就走回退。效果图里的节点是手写的 COM 对象（`IGraphicsEffect` + `IGraphicsEffectSource` + `IGraphicsEffectD2D1Interop`）。CI 上 MinGW 编出的截图工具画的，就是 MSVC 发行包画的那一份代码。

## Alternatives considered

- **DWM 的系统背景（`DWMWA_SYSTEMBACKDROP_TYPE` = `DWMSBT_TRANSIENTWINDOW` / `DWMSBT_MAINWINDOW`）**：最强的理由是几行代码、系统自己维护材质的配方和回退。不采用：只有 Windows 11 22H2 起才有，Windows 10 上什么都没有；材质铺满整个窗口，而这个窗口四周是给阴影留的透明边距，裁不成卡片的圆角矩形；`backdrop_opacity`、`backdrop_luminosity` 也没有地方交给它。
- **未公开的 `SetWindowCompositionAttribute`（`ACCENT_ENABLE_ACRYLICBLURBEHIND`）**：Windows 10 1803 起就有亚克力，同样不用 WinRT。不采用：没有文档，材质同样铺满整个窗口，只能用窗口区域裁剪，而窗口区域会把阴影边距一起裁掉；没有云母；行为在不同版本上变过。
- **保留交换链，用 `ICompositorInterop::CreateCompositionSurfaceForSwapChain` 把交换链放进 Windows.UI.Composition 的视觉树，材质垫在下面**：不用离屏位图那次拷贝。不采用：交换链的 `Present` 和合成器的提交各走各的，卡片变宽时材质的形状和内容会错开一帧，msime-windows 试过这条路，注释里写明了这个问题；候选窗每个按键都可能变宽。
- **让 `paint()` 直接画进绘制表面的 `BeginDraw` 上下文（msime-windows 的做法）**：省掉离屏位图。不采用：绘制表面交出的上下文画进一张共享图集，要带着偏移和裁剪，候选窗 `paint()` 里设变换、推图层的地方都要跟着改；这块代码正被别的工作改动（九宫格绘制），改调用方的画法会让两边难以合并。一次几百 KB 的 GPU 拷贝在打字频率下可以忽略。
- **只在 MSVC 构建里打开材质，MinGW 用 SDK 头文件缺失时编空实现**：不用自己声明 ABI 接口。不采用：CI 的截图工具是 MinGW 编的，这样它永远看不到材质，发行包里的那份代码在合并前就没有任何运行验证；两种工具链也会画出不同的东西。

## Consequences

收益：亚克力、云母皮肤在 Windows 10 1903 以上画出系统材质，参数、圆角、阴影的取舍与 msime-windows 一致；画不出来的每一种情况都退回来源的不透明底色，而且候选窗不会因此失败。不要材质的皮肤和没有皮肤时，候选窗走的还是原来的交换链。

代价：
- `msimeui` 多了一组手写的 ABI 声明，vtable 次序错了只会在运行时出错（读错方法），编译器查不出来；改它时要对照 Windows SDK 元数据。
- 材质模式每帧多一次整面拷贝，打开材质的皮肤第一次显示时要多建一个合成器和效果画刷。
- 一个窗口建材质失败一次，这次运行就一直是不透明底色，直到候选窗重建（Server 重启）。
- 云母取样的是桌面壁纸，截图工具的条纹底只验证得了亚克力。

验证：`windows-candidate-backdrop`（取舍与回退链、缺省参数、夹取、矩形随动效变换、日志文字、清单读取）、`windows-candidate-skin-layout`、`windows-candidate-skin` 和 `windows-candidate-skin-gallery-plan`（新增条纹底、裁剪、按截图前屏幕比较）在 Mac Studio 上用 `c++ -std=c++17 -Wall -Wextra -Werror` 编译并通过；`DeviceResources.cpp`、`CandidateWindow.cpp`、截图工具和其余用 `DeviceResources` 的窗口用本机 `x86_64-w64-mingw32-g++ -fsyntax-only -Wall -Wextra -Werror` 检查无错误无警告；`rbuild cargo test --locked -p msime-client-services skin::` 与 `rbuild cargo test --locked -p msime-mcp-server` 通过。CI 的「Windows candidate skin gallery」在条纹底上截 `verify-color-syntax`（亚克力）和新加的 `verify-backdrop-mica`（云母），日志逐张写明实际垫了什么：`windows-2025` runner（build 26100，透明效果开）上八张都是真材质，亚克力把卡片后面的条纹糊成带色调的一片、卡片外面的条纹仍然清楚，云母取的是 runner 的壁纸、建得起壁纸模糊画刷；圆角、外框和只在外面的阴影都对，其余皮肤的截图和自检照旧通过。MinGW 12（Debian trixie，CI 用的那一版）的头文件没有 `E_BOUNDS`，第一次推送因此编不过，改成本地常量。MSVC 编译只在发版流水线（`Build-Client.ps1`）上跑，PR 上没有；代码里没有用到 Windows SDK 独有的头文件。没有在真实的 Windows 10 和带壁纸的 Windows 11 桌面上目测过。
