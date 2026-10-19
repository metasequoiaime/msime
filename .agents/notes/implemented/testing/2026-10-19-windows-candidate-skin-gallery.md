# Agent Note: Windows 候选皮肤截图工具与 CI 产物

Status: implemented

## Problem

Windows 候选窗这几天接连画上了皮肤的版式、外框、阴影、列表翻页箭头、收紧宽度和卷轴展开（[版式等键](../feature/2026-10-19-windows-candidate-skin-layout-keys.md)、[卷轴展开](../feature/2026-10-19-windows-candidate-skin-expand-panel.md)），两篇笔记都写着同一个缺口：没有在真实 Windows 上看过。维护者手边没有 Windows 机器，本机的 MinGW 只能做语法检查，Wine 的 DirectComposition 是桩、画面不可信。Direct2D 的逐角圆角、三角箭头、阴影档位、展开网格这些观感问题，单测里的尺寸断言抓不到，只能看图。需要一条不依赖 Windows 机器、每次改动都能拿到真实渲染结果的路。

## Decision

`platforms/windows/tests/tools/candidate_skin_gallery/main.cpp` 编出 `msime-candidate-skin-gallery.exe`：在自己的进程里建 Server 的 `CandidateWindow`，读取器返回一页固定的假候选，把皮肤根目录里每款皮肤和不带皮肤的基线，按横排、竖排 × 浅色、深色各画一张，写了 `expand_panel` 的皮肤横排再各画一张展开状态，从屏幕 DC 截下候选窗的窗口矩形，经 WIC 写成 PNG。

### 皮肤怎么套用

照 `server_main.cpp` 的首次套用，不另写一套：`msime_client_skin_catalog` 扫目录，对每张截图用 `candidate_theme_request` 拼出和 Server 相同的主题请求（全局主题 `custom`、`custom_theme.candidate_skin` 指名这款皮肤，等于在设置里选了它），经 `msime_client_resolve_theme` 解析；`theme.candidate_skin` 不是这款皮肤时说明包没有声明这个排列或明暗，Server 在这里也不画，跳过并记一行。配色是 `apply_candidate_skin_extra_slots(candidate_theme_palette(...))`，其余资源是 `candidate_skin_assets`，经 Server 用的同一组 setter（`set_palette`、`set_skin_min_width`、`set_skin_decoration`、`set_skin_background`、`set_skin_corner_radius`、`set_skin_layout`、`set_skin_font_family`、`set_skin_animation`）交给候选窗。字体、字号、缩放、不透明度和首行设置取共享默认偏好（`msime_client_default_preferences`），经 `candidate_font_settings`、`candidate_window_style`、`candidate_layout_settings` 投影，和新装的用户看到的一样。每张截图新建一个候选窗，前一张的状态不会带到下一张。

### 假候选与展开

假候选页是一个 Engine view 形状的 JSON，经 Server 同一个投影 `candidate_presentation_from_view` 变成快照：拼音 `shuru`，九个候选各带一行英文释义，第一页、共三页。展开状态由新加的 `CandidateWindow::open_expansion` 打开：它和点「∨」走同一条 `toggle_expansion`，展开通知照常发出，这里的通知处理扮演 Server，让之后的快照带上 `expansion_open` 和一份 45 项的完整候选列表。这是 `CandidateWindow` 唯一的改动，Server 不调用它。

### 截图与空白判断

工具把进程设成每显示器 DPI 感知，用一块纯色底（浅色 `#E6E6E6`、深色 `#202020`）盖住主显示器工作区，鼠标挪到右下角，候选窗锚在工作区左上部。等到候选窗出过帧（`Rendered` 回调）、动效时长加 300 毫秒过去、两次 `DwmFlush` 之后，用 `BitBlt(SRCCOPY | CAPTUREBLT)` 从屏幕 DC 截窗口矩形：屏幕 DC 读的是 DWM 合成后的桌面，DirectComposition 交换链和分层窗口回退都在里面。和底色不同的像素不到 0.5% 算空白。另有两项自检，专抓卡片画出窗口被裁的情况：`CandidateWindow::painted_card()`（上一帧实际画出的卡片矩形）要落在客户区里、和 `card_on_screen()`（按窗口尺寸推出的卡片）每边相差不超过 2 像素；截图最外一圈要是底色，任一边和底色不同的像素超过 10% 就算卡片画到了窗口边上（`edge_bleed`）。第一次在 CI 上跑就是靠看图发现了这类问题，见 [按窗口客户区排版](../bug-fix/2026-10-19-windows-card-size-from-client-area.md)。皮肤目录有加载失败的文件夹、主题解析失败、没出帧、候选窗失败、截图空白、卡片被裁、PNG 写不出，或一款皮肤一张也没画时，以 1 退出；每一行日志写明皮肤、排列、明暗、窗口与卡片的像素尺寸、帧数和不同像素的比例，空白的那张也写出来，CI 失败只看日志和产物就能定位。

要系统材质（`backdrop`）的皮肤换成彩色竖条纹底（`backdrop_stripe_color`）：纯色底上看不出模糊，条纹在亚克力卡片后面糊成一片混色。这些截图的空白和最外一圈自检改为和候选窗出现之前截下的同一块屏幕比（`crop` 加按参考图比较的 `capture_verdict` / `edge_bleed`），日志逐张写明卡片背后实际垫了什么（`CandidateWindow::painted_backdrop()`），开头记下 Windows 版本、透明效果和高对比度开关。材质画不画出来不算失败：runner 关着透明效果时，日志会写明是不透明回退。见 [Windows 候选窗按皮肤垫系统材质](../feature/2026-10-19-windows-candidate-backdrop-material.md)。

文件名、假候选页、后续候选、空白和卡片被裁的判断放在不碰 Win32 的 `CandidateSkinGallery.h`，由 CTest `windows-candidate-skin-gallery-plan` 覆盖。工具本身要交互桌面，不是 CTest，名字也不以 `windows-` 开头，`run-tests-wine.sh` 不会把它当测试在 Wine 里跑。

### 在 CI 里怎么跑

`ci-platforms.yml` 的 `windows` job（MinGW 交叉构建，本来就编译 `CandidateWindow.cpp`）随整棵树编出工具，把它、`msime_host_api.dll` 和三个 MinGW 运行时 DLL 用 `strip --strip-debug` 去掉调试信息，上传为 `windows-candidate-skin-gallery-tool`。新的 `windows-skin-gallery` job（Windows candidate skin gallery）`needs` 它，在 `windows-2025` runner 上对 `crates/client-services/tests/fixtures/candidate-skins` 运行，截图上传为 `windows-candidate-skin-gallery`，失败时也上传。fixture 在 `crates/` 下，改它们会经共享路径触发 Windows 作业。

## Alternatives considered

- **放进 Windows native settings job，用 MSVC 编译** —— 那个 job 已经有交互桌面，也已经在截设置窗口的图，加一步最省一个 runner。不采用：它只用 MSBuild 编 WinUI 设置工程，`CandidateWindow` 所在的 `msime-windows-server` 要整个 CMake 工程配置成功，而 Windows 侧的 CMake 依赖 vcpkg 的 curl、fmt、utfcpp，那个 job 一个都没有，要先在 runner 上从源码编 curl。更要紧的是 MSVC 的 `/W4 /WX` 本机检查不了，新写的代码第一次被 MSVC 编译就是在 CI 上；交叉构建用的正是本机能做语法检查的 MinGW，`CandidateWindow.cpp` 也已经天天在那里编过。代价是多一个 Windows runner 和一次产物传递，交叉构建四分钟左右，截图作业仍比设置作业先结束。
- **在交叉构建的容器里用 Wine 加 xvfb 跑截图** —— 不用 Windows runner，`run-tests-wine.sh` 已有这套环境。不采用：Wine 的 DirectComposition 是桩，候选窗会退回分层窗口，Direct2D 和 DirectWrite 也是 Wine 自己的实现，截出来的不是用户看到的画面，正是这个工具要排除的不确定性。
- **用 Windows.Graphics.Capture 或 `PrintWindow` 截窗口** —— 只截这一个窗口，不受上面压着什么的影响。不采用：`PrintWindow` 拿不到 DirectComposition 交换链的内容；Windows.Graphics.Capture 要 WinRT 投影和 D3D 互操作，MinGW 交叉构建里的 WinRT 头文件没有核实过。屏幕 DC 截的是合成后的桌面，用纯色底和置顶窗口就能保证截到的只有候选窗。
- **用 UI Automation 点「∨」打开展开，不加 `open_expansion`** —— 不改产品代码。不采用：读屏提供者跑在候选窗的 UI 线程上，同一线程里再当 UIA 客户端调它会卡死；模拟鼠标点击又要知道按钮的位置，只能复制 `CandidateExpandPanel.h` 的几何或去读私有成员。一个只转调 `toggle_expansion` 的公开方法改动最小，行为与点按钮相同。
- **截图前关掉皮肤动效** —— 不用等动画放完。不采用：`set_skin_animation` 也是 Server 套用皮肤的一步，跳过就不是同一条路；进入动画放完以后的帧本来就是静止的，等它的时长加 300 毫秒即可。

## Consequences

收益：每个触发 Windows 作业的 PR 都带一组 Windows 真实渲染的候选窗截图，皮肤的版式、外框、阴影、翻页箭头、展开网格和配色都能直接看图核对；皮肤目录里有加载失败的包、或某款皮肤在 Windows 上画不出来时，CI 直接失败。套用路径与 Server 相同，截到的就是用户会看到的样子。

代价：
- 多一个 Windows runner 作业（约几分钟），而且排在交叉构建之后。
- `CandidateWindow` 多了两个 Server 不用的公开方法：`open_expansion` 和只读的 `painted_card`。
- 截的是新装默认偏好下的样子；用户改过字体、缩放、不透明度或取色器时的组合不在其中。工作区小于卡片时卡片会被钳住，截图仍然有效但和大屏上的位置不同。
- 卷轴展开只截打开后停在顶部的一帧，滚动、悬停和按下的底色不截。
- 截图只判断不是空白、卡片没画出窗口，不比对基准图；其余画错了要靠人看产物。

验证：`CandidateSkinGallery.h` 的 `windows-candidate-skin-gallery-plan` 在 Mac Studio 上用 `c++ -std=c++17 -Wall -Wextra -Werror` 编译并通过（繁简转换用原样返回的替身链接）；`main.cpp`、`gallery_plan.cpp` 和改过的 `CandidateWindow.cpp` 用本机 `x86_64-w64-mingw32-g++` 与 `i686-w64-mingw32-g++ -fsyntax-only -Wall -Wextra -Werror` 检查无错误无警告；`ci-platforms.yml` 过了 `actionlint`。链接、在 Windows runner 上真正截到非空白的图、上传产物，只有 PR 上的 CI 能证明。
