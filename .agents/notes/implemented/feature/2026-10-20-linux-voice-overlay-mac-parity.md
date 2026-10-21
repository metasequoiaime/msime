# Agent Note: Linux 语音浮层对齐 macOS 的布局、动画与交互

Status: implemented

## Problem

Linux 的语音浮层和 macOS 差得很远。X11 与 Wayland 两个后端各画各的：不论什么状态都是固定 420×132 的方块，X11 是不透明窗口加 X 服务器边框、misc-fixed 点阵字体，Wayland 是 12 px 宽的实心边框、`Sans 12`；波形是从左到右递减的静态柱子，只在量化电平变化时重画，IBus 要等电平变化、Fcitx5 要等 250 ms 轮询，录音停了柱子也停在最后的高度。取消、完成按钮从录音一开始就显示，压在波形两端；IBus 识别中点完成什么也不做，Fcitx5 识别中再点完成会再发一次结束请求，被拒后整次录音被取消。转写只截前 72 个字符（X11）或按字节截 240 字节（Wayland，可能切坏 UTF-8），看不到最新的文字。按钮点击只在浮层重画时读取，静音或识别期间点了要等下一次变化。失败提示只停 1.2 秒，Esc 收不掉。三处状态文案（X11、Wayland、IBus 辅助文字、Fcitx5 辅助文字）各说各的。Wayland 浮层的底边距其实传进了 `right` 参数，没有 buffer scale，在 HiDPI 屏上被放大发虚。

## Decision

浮层的几何、计时、波形运动和文案放在纯头文件 `platforms/linux/src/overlay/WaveOverlayLayout.h`，数值与规则照 `platforms/macos/src/voice/VoiceWaveOverlay.mm`：四种布局（紧凑 78×32、处理中 112×40、按钮 142×40、转写 420×112，优先级按钮 > 标签 > 转写），圆角规则，12 根细胶囊的多谐波电平模型，波形几何，16 ms 帧间隔，6 秒失败时长，以及“最新三行”转写的二分搜索（由调用方给出可截断位置和量行函数）。

`WaveOverlayModel` 从状态推出呈现，不再由宿主直接写 `actions_visible`：`actions_visible()` 只在空格锁定且仍在录音、或识别/润色进行中时为真，失败时为假；`label_visible()` 在识别/润色和失败时为真；`animates()` 在没有标签时为真。模型保存原始电平 `input_level`，`advance(seconds)` 每帧推进柱高。转写窗口从 160 个码点放宽到 2048，丢掉更早文字时记 `transcript_truncated`，预览一律以“…”开头。`set_transcript` 在校验 UTF-8 的同时丢掉 NUL 和换行、制表以外的 C0 控制字符：语音服务的文字是外部输入，带 NUL 时 GLib、IBus 和 Pango 的校验都会在它那里截断，而字节长度照算；画家按首字节数字符，与 `pango_get_log_attrs` 写入的条目数一致。

X11 与 Wayland 共用 `WaveOverlayPainter.h`（Cairo + Pango）。画家按逻辑像素工作，测量与绘制共用一个关闭度量 hinting 的 Pango 上下文，保证任何缩放下换行一致；转写的截断位置取 Pango 的 `is_cursor_position`，不会切开字形簇。X11 后端改用 cairo-xlib：有合成管理器时用 32 位 ARGB visual 画半透明圆角，没有时面板不透明、用 XFixes 的 ShapeBounding 区域裁圆角，合成管理器启停后下次显示重建窗口；离屏 group 绘制后一次拷贝，避免 16 ms 一帧时闪烁；处理 Expose。Wayland 后端按布局 `set_size` 并在拿到 configure 后再挂新尺寸的缓冲；按所在输出的 `wl_output.scale` 画整数倍缓冲并 `set_buffer_scale`；底边距 10 px、exclusive zone 0，与 X11 的“工作区底部之上 10 px”同义；隐藏后再次显示先重新协商尺寸，符合 layer-shell 对重新映射的要求。

两个宿主在浮层显示时各起一个 16 ms 定时器推进波形（IBus 用 GLib，Fcitx5 用事件循环并在每帧读取邮箱里的最新电平），并在自己的主循环上监听浮层显示连接的 fd（`WaveOverlaySurface::event_fd()` / `dispatch_events()`），点击随到随处理。辅助文字回退不动画、不监听。识别中点完成只收起浮层（`wave_overlay_dismissed`），结果照常上屏；Fcitx5 的 `stopVoice` 在已进入识别后直接返回，不再重复发结束请求。点击是在浮层自己的 `update()` 或 `dispatch_events()` 里报告的，处理它可能停掉正在回调的帧定时器或 fd 监听，连同保存回调的闭包一起销毁；Fcitx5 的帧回调因此在调用 `update()` 之前把 `this` 复制到栈上，之后不再读捕获。失败提示 6 秒，Esc、切换焦点或下一次录音提前收起，Esc 不吞键；浮层上的失败提示在继续打字时不收起（macOS 和 Fcitx5 本来如此，IBus 以前每次重画候选都收掉它），只显示在辅助栏时把那一行让给候选。画得出浮层时失败提示只在浮层上，不再往辅助栏另写一份：IBus 由 `FallbackSurface` 在画不出时退到辅助文字，Fcitx5 只在画不出时写辅助栏，否则浮层收起后那份副本会一直留在候选窗里。IBus 的 `State::close()` 也收起浮层并撤掉帧定时器和 fd 监听，`guarded()` 兜底和词典维护释放会话之后不再经过 `render()`，浮层不会留在屏幕上空转。空格锁定在 Fcitx5 上立即刷新浮层。

状态文案统一为 macOS 的写法：“正在录音…”“识别中...”“处理中...”；锁定提示只在辅助文字里出现。两个框架的辅助文字回退都用 `WaveOverlayFeedback.h` 生成同一段文字，Fcitx5 不再加“语音：”前缀。

颜色不照搬 macOS 的固定紫/白配色：按产品决定，浮层继续取候选窗解析出的主题（内置主题是 Adwaita 风格配色，皮肤用自己的颜色），只把角色对齐——面板 0.90 不透明、1 px 描边、按钮是面板色向文字色偏 12% 的中性圆、符号和波形都用文字色（macOS 用同一个前景色画柱子和文字）；去掉 Wayland 独有的锁定黄、处理橙。

两个后端都需要 `pangocairo`（X11 另需 `cairo-xlib`）才编译；缺少时不编译该后端，语音状态走辅助文字。X11 的这两项单独检查（`X11_OVERLAY_DRAW`，浮层代码按 `MSIME_LINUX_HAS_X11_OVERLAY` 编译），不并进 `X11_SURFACE`：后者定义的 `MSIME_LINUX_HAS_X11_SURFACE` 还决定 X11 的 Caps Lock 撤回，只该依赖 Xlib。Debian 10 的 legacy 构建镜像因此补装 `libpango1.0-dev`，其余发行版包定义本来就依赖 cairo 和 pango。

## Alternatives considered

- **把几何和波形模型抽到 `shared/voice/` 让 macOS 也引用同一份头文件** — 最强理由是两边永不漂移，`shared/voice/InputLevel.h` 已有先例。没有这样做，是因为这会改动 macOS 的 `VoiceWaveOverlay.mm` 并需要在本 PR 里完整编译、回归 macOS 宿主，而本次改动只针对 Linux；Linux 头文件里注明了数值来源，macOS 那边改了要同步。
- **X11 保留核心 Xlib 绘制作为没有 Cairo/Pango 时的退路** — 能让极简构建环境仍有原生浮层。没有保留，因为那等于维护两套画法，正是这次要消除的漂移来源；所有发行版包和 legacy 镜像都有 cairo 与 pango，缺少时退回辅助文字仍可用。
- **Wayland 用 `wp_fractional_scale_v1` 画分数缩放** — 在 1.25、1.5 倍屏上最清晰。仓库里没有该协议的 XML，需要新增协议代码；整数 buffer scale 由 compositor 缩小显示，已经解决了“放大发虚”的问题，留待需要时再加。
- **波形用 macOS 的固定前景色（浅色下的紫色、深色下的白色）** — 与 macOS 像素一致。产品决定 Linux 保留主题配色、只对齐几何与交互，所以取的是主题自己的文字色，角色与 macOS 相同。
- **波形保留主题强调色（Adwaita 的蓝）** — 第一版这样做，理由是强调色最醒目。改掉了：macOS 的柱子和文字是同一个前景色，文字色本身就在主题配色里，用它并不违背保留 Linux 配色的决定，用强调色反而让“颜色角色对齐 macOS”这句话不成立。
- **缺少 ASR token、本地模型时在开始录音前提示（macOS 的 `MissingToken`、`MissingLocalModel`）** — 能更早告诉用户该去设置哪里。Linux 宿主不接触凭据（只转发非敏感选项给语音服务），也看不到本地模型是否就绪，做这项检查需要语音服务协议新增错误类别，超出本次范围，没有做。

## Consequences

- **收益**：普通按住录音只出现一个小圆角条；按钮只在锁定和识别时出现，不再压住波形；识别中点完成有效且不会误取消；转写显示最新三行；点击立即响应；HiDPI Wayland 清晰；IBus 与 Fcitx5 的辅助文字一致。
- **代价与已知上限**：X11 与 Wayland 浮层现在必须有 pangocairo 才编译；浮层显示期间每 16 ms 重画一次（几十个逻辑像素宽的小面板，Wayland 在两块缓冲都被占用时跳帧）；X11 没有合成管理器时圆角是 1 位形状，边缘没有抗锯齿；Wayland 输出仍由 compositor 选择，宿主拿不到光标所在的输出；屏幕阅读器仍只能读到辅助文字和 IBus 属性，原生浮层没有 AT-SPI 接口。macOS 的几何或电平模型若改动，需要同步 `WaveOverlayLayout.h`。

## Verification

- `platforms/linux/tests/runtime/wave_overlay_model.cpp`：布局优先级与尺寸、圆角、按钮规则与命中、波形在静音时为点、说话时中间高于两边且有界、停止后回落、转写窗口与“…”、NUL 等控制字符被丢掉、辅助文字的电平格、锁定提示与失败文案。
- `platforms/linux/tests/runtime/wave_overlay_painter.cpp`（`linux-wave-overlay-painter` 测试，需 pangocairo）：用真实 Pango 量出的四种呈现尺寸、失败胶囊加宽、最新三行与组合字符不被切开、圆角外全透明、面板 0.90 不透明、无 alpha 时不透明、皮肤配色且波形用文字色、带 NUL 的转写照常截断绘制；设置 `MSIME_TEST_CANVAS_ARTIFACTS` 时把每种呈现存成 PNG。
- `platforms/linux/fcitx5/tests/native.cpp`：开始录音不显示按钮、辅助文字与 IBus 同文、锁定提示、失败提示无按钮且 Esc 可收起；一个在 `update()` 里报告点击的假浮层点取消，`update()` 返回前录音结束、帧定时器和监听都撤掉（测试不跑事件循环：sd-event 的循环退出后不再接受新的事件源，帧回调本身由合约测试钉住）；识别中再次 `stopVoice` 不发请求也不取消，点完成只收起浮层；画出来的浮层显示失败提示时辅助栏保持空白，Esc 收起后也一样。
- `platforms/linux/tests/core/fcitx5_contract.py` 钉住 Fcitx5 的帧定时器、fd 监听、收起与统一文字，并禁止宿主再写 `actions_visible = true`；还钉住帧回调在 `update()` 之后只经栈上的 `self` 访问状态、IBus 的 `render()` 保留浮层上的失败提示、`X11_SURFACE` 不含 cairo 与 pango。
- 运行：`bash platforms/linux/build-container.sh`。
