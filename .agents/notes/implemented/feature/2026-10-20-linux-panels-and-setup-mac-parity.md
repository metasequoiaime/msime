# Agent Note: Linux 共享面板与首次配置的交互对齐 macOS

Status: implemented

## Problem

Linux 的设置应用和共享面板（Tauri/React）在行为、结构和内容上与 macOS 有一串差别：表情面板「输入到原应用」或「粘贴」之后不关闭；面板一律放在输入窗口下方，屏幕键盘在最大化窗口下被挤到屏幕边缘；剪贴板面板没有「发到云剪贴板」、看不到采集状态、长文本只有桌面主题的原生 tooltip、删除按钮常驻；云剪贴板条目不显示「已置顶 · 设备」；独立的本地剪贴板窗口顶着表情面板的页头和分类；首次配置页是一个没有标题栏、关不掉、拖不动的 1000×780 无边框窗口，进度只有滚动的原始输出，结论要用户自己读日志；没有后台更新提醒；词库过期时只让用户开终端跑命令；帮助页、插件库、屏幕键盘标题和诊断日志入口也都与 macOS 不同。用户的决定是：Linux 保留 GNOME/Adwaita 的视觉，行为、结构、内容和交互向 macOS 看齐。

## Decision

- 插入后关闭：`EmojiPanelClient.closeAfterInsert` 由 `DesktopEmojiPanel` 只在 Linux 的独立窗口上打开。「输入到原应用」或「粘贴」成功后由页面先把最近使用写进存储，再请宿主关窗；复制不关闭。不放在 Rust 的 `send_text`/`paste_clipboard_text` 里关窗，因为窗口在命令返回前就没了，页面收不到成功回调，最近使用会丢。
- 落点：`panel_input::linux_panel_placement` 让屏幕键盘贴工作区底部居中、表情和本地剪贴板居中，其余面板仍在输入窗口下方。X11 用目标窗口找显示器（`panel_anchor` 返回窗口中线上的锚点，`visible_panel_position` 换算到那块显示器的工作区），Sway 直接用 workspace 矩形；拿不到目标矩形时居中类面板按窗口当前或主显示器摆放。GNOME/KDE Wayland 不让客户端定位，交给合成器。
- 剪贴板面板：右键（菜单键、Shift+F10）菜单「发到云剪贴板」，经 `cloud_clipboard_request` 的 `add`，只在 Linux 和 macOS 的 React 面板提供；`clipboard_capture_status` 报告设置应用里采集线程最近一轮的状态（macOS 原生状态行的四种，加 Linux 的「读不到系统剪贴板」：只在本会话该用的 wl-paste、xclip、xsel 一个都没装时才考虑；装了却没读到文本是剪贴板为空或只有图片、文件，这些工具此时同样以非零状态退出，仍算正常采集。设置程序自己读不到时先问 `<package>-clipboard.service` 是否在运行：真正的采集在这个用户服务里，它用随包的 `msime-linux-clipboard-watch-x11`，不需要 xclip、xsel，默认的 Ubuntu GNOME 上就是这样，服务在跑就算正常采集。服务没在跑时按会话分三种说法：有 X11 显示提示 xclip 或 xsel，只有 Wayland 且没装 wl-paste 提示 wl-clipboard，其余（GNOME Wayland 没有 XWayland、没有显示）装什么都没用，`unreadable_clipboard_status`），变化时发 `clipboard-history-changed`；悬停或键盘聚焦片刻显示最多 12 行的预览气泡（触屏和点击聚焦不弹）；× 删除按钮只在悬停或聚焦这一行时出现；没有悬停的设备（`(hover: none)`，Android 设置应用的表情与剪贴板页也显示这个共享面板）不用 28px 的 ×，仍用行旁带文字的「删除」，免得误触；删除提示改为「已删除历史记录（不会清空系统剪贴板）」；临时消息改为浮在内容上方的胶囊提示「已复制  X」，与 macOS 原生表情面板的尺寸、配色和 24 个 UTF-16 单元截断一致。独立的本地剪贴板窗口（`standaloneClipboard`）页头叫「剪贴板」，不显示分类导航，Esc 直接关窗（没有表情首页可回）。预览气泡和右键菜单只在列表内容变化时收起：面板每 400 毫秒轮询一次，每次都给出新数组，按引用比较会让它们一出现就消失。顺带修了表情面板根元素在浅色主题下仍是深色底的问题（`panelSurface` 的浅色变体是 `group-data`，只作用于子元素）。
- 首次配置：页面结构照 macOS 安装窗口（小窗口、logo 卡片、主按钮、百分比进度条、结果卡片），配色用 GNOME token，自带可拖动标题栏和关闭按钮。`msime-linux-setup` 在 `MSIME_LINUX_SETUP_REPORT=1` 时额外打印 `msime-setup-stage:`、`msime-setup-result:` 和 `msime-setup-note:` 三种机读行，`linux_setup.rs` 把阶段转成 `linux-setup-stage` 事件、把结果放进 `LinuxSetupStatus.registration`、把提示（GNOME Wayland 的 GTK_IM_MODULE、玲珑与 IBus、Fcitx5 托盘）放进 `LinuxSetupStatus.notes`，不显示成输出。结果卡片 `LinuxInputMethodNotice` 对应 macOS 的 `InputSourceStartupNotice`，配置成功时提示也列在卡片上，原始输出收起后它们不会被藏住。设置前端在各 Linux 版本之间共用，页面、结果卡片和帮助卡片里的产品名、Fcitx5 条目名、IBus 输入源名和 `<package>-setup` 命令都按 `HostCapabilities.edition` 取（`linuxEditionNames`），不写死 full 的。窗口由宿主在启动时变成 500×640（`shape_setup_window`），离开时 `leave_linux_setup_window` 只在确实变过时才换回。
- 词库过期：`msime-linux-first-run-guide --reason dictionary-outdated` 在引导目录留下 `dictionary-outdated` 标记，通知指向设置里的「立即更新」（支持动作时点通知即打开设置）；设置窗口看到标记就以同一个页面的更新模式打开，「立即更新」运行 `msime-linux-setup --update --download`。`--update` 成功（切换了或本来就是当前版本）时脚本自己删掉两处标记，终端里运行也一样。已在运行的设置进程被叫到前台（`settings-route`，包括空路由）时重看一次 `linux_setup_status`，点通知的「打开设置」也能进到更新页；用户在本进程里点过「稍后」就不再打断。
- 后台更新检查：打开设置窗口时（启动或被叫到前台；只为面板启动的设置进程不检查）若距上次成功检查满一天、距上次尝试满六小时，就在后台线程用同一个 `check_for_update` 问一次，记录写在状态目录的 `update-check.json`，连不上也记下尝试时间，断网时不会每次打开都重试；构建时或运行时的 `MSIME_DISABLE_UPDATE_CHECK` 关掉它，PRIVACY.md「资源与更新下载」写明了这个请求；新版本只提醒一次，通知的应用名和图标按本安装包的版本取；找到的版本一并记进 `update-check.json`，`pending_update_check` 从这份记录读出它（仍高于运行的版本时），发通知的进程退出以后关于页也能直接显示。通知的「查看」只在发通知的进程还在时有效。只提醒、不安装，检查不在 Fcitx5 插件或 IBus 宿主里做，开发构建不做。
- 其余：帮助页给 Linux 一组 `linuxHelpCards`（Linux 的键名和排查步骤）和「打开官网」；插件页在 Linux 也用可筛选的插件库；屏幕键盘标题显示与 macOS 相同的操作说明，Win 键标作 Super，Linux 的两种投递错误码各有说明；手写面板字体栈在 Apple 字体之后补 Cantarell 和思源/Noto 中文字体；诊断日志在 Linux 上经 `org.freedesktop.FileManager1.ShowItems` 选中文件，退回 `xdg-open` 打开目录。

## Alternatives considered

- **在 Rust 的提交命令里关窗**：改动只在宿主一侧，macOS 也是在 `submit` 里 `destroy`。但窗口在命令返回前就关掉，页面的成功回调永远收不到，Linux 的面板进程会反复打开同一个窗口，最近使用就再也记不上；改由页面在成功后关窗。
- **保留跟随输入窗口的落点，只在 Wayland 上退到底部居中**：跟随落点离输入处近，粘贴依赖的目标也已记住。但 macOS 的表情面板由系统居中、屏幕键盘贴底，用户要求向 macOS 看齐；而 Wayland 上客户端本来就定不了位置，退到底部居中在那里不起作用。手写、语音等面板保持原样，不在这次范围里。
- **设置应用自己判断词库是否过期**：不依赖引导脚本的标记。但唯一可靠的判断是按词库锁逐个校验 170 MB 的文件，设置窗口每次打开都做一遍代价太高，宿主启动时本来已经做了这次校验，复用它的结论即可。
- **systemd 用户计时器做后台更新检查**：设置应用没开时也能检查。但要新增计时器单元、改所有发行版的打包和首次配置，且仍需要一个无界面的检查入口；用户打开设置窗口时检查、按天限流已足够及时。面板启动不检查：输入法按需拉起面板，用户没有打开设置，不该因此联网。
- **把检查频率和开关放进共享偏好**：与 macOS Sparkle 的自动检查开关对应。但这是跨平台的偏好格式改动，要同时改各宿主的默认值和设置页，和这次并行的其他改动冲突面大，先只做默认开启、每天一次的检查。
- **在 Tauri 里桥接 portal 的 color-scheme**：gap 分析担心 WebKitGTK 不跟随 GNOME 的深色设置。tao 0.37 在启用 `dbus` 特性（tauri 默认启用，Cargo.lock 里 tao 依赖 `dbus`）时已经在建窗时读取 `org.freedesktop.appearance color-scheme` 并订阅 `SettingChanged`，据此设置 `gtk-application-prefer-dark-theme`，所以不另写一份。

## Consequences

- macOS 的 React 面板随之得到胶囊提示、预览气泡、× 删除、新的删除提示和「发到云剪贴板」，与 macOS 原生兜底一致；Windows 的 Tauri 面板得到前四项，没有「发到云剪贴板」，也不在插入后关窗。
- 首次配置页依赖脚本的机读行；旧版脚本没有它们时进度条只走到起步阶段，结果卡片按两种框架给出手动步骤，配置本身不受影响。
- `dictionary-outdated` 标记放在本版本自己的目录里（full 是 `msime-client`，五笔版是 `msime-client-wubi`，引导脚本按版本改写成同一个），并排安装的两个版本互不影响；标记在会话目录下随登录清空；用户在终端里运行 `--update` 成功后标记同样被删掉。宿主之后发现词库已是新的时不会主动删标记，靠的是更新命令这一处。
- 后台检查只在用户打开设置窗口时发生，从不打开设置的用户收不到提醒。

## Verification

- Rust：落点、锚点、采集状态、FileManager1 参数、诊断日志目标、配置机读行、过期标记、更新参数和后台检查的单测在 Debian 桌面检查容器（`Dockerfile.desktop-check`）里随 `cargo check -p msime-desktop --all-targets` 与 `cargo test -p msime-desktop --lib` 运行。
- Python：`first_run_guide.py` 覆盖新的通知文字和标记，`setup_update.py` 覆盖 `MSIME_LINUX_SETUP_REPORT=1` 时的阶段顺序。
- React：表情面板、剪贴板记录、云剪贴板历史、首次配置页、帮助页、插件库、屏幕键盘、诊断日志和关于页预填的 vitest 用例。
- 在 Playwright 的 Chromium 里用合成数据截过首次配置（就绪、进行中、三种结果）、词库更新页和独立剪贴板窗口（悬停预览、右键菜单、胶囊提示）的深浅两套图；WebKitGTK 与它的排版可能有细微差别。
- 没有在真实 GNOME/KDE Wayland 或 X11 桌面上看过落点、通知动作和 portal 深色跟随。
