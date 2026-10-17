# Agent Note: Linux 设置窗口的 WebKit 渲染规避只在 NVIDIA 专有驱动下默认启用

Status: implemented

## Problem

issue #6847：Arch Linux、niri 26.04 Wayland 会话、Mesa 26.2、webkit2gtk-4.1 2.54.1 上，设置窗口打开后持续闪烁，从终端、应用菜单和 Fcitx5 的「水杉设置」入口打开都一样。报告者用 `WEBKIT_DISABLE_COMPOSITING_MODE=0`（同时设了 `GDK_BACKEND=wayland`、去掉 `WEBKIT_DISABLE_DMABUF_RENDERER`）启动后不再闪烁。

起因是设置启动器 `msime-linux-settings` 对所有机器默认导出的两个变量。`WEBKIT_DISABLE_COMPOSITING_MODE=1` 来自 #3273，规避 NVIDIA 加 Hyprland 下的 Wayland 显式同步协议错误（Error 71）和 X11 下的 GBM 分配失败；`WEBKIT_DISABLE_DMABUF_RENDERER=1` 来自 #5834，规避 NVIDIA 加 KWin 下窗口一启动就 Error 71 退出，见 [Linux 设置窗口默认禁用 WebKit DMA-BUF 渲染器](2026-10-08-linux-webkit-dmabuf-renderer.md)。两个变量各自都会让 WebKitGTK 退回非合成的 CPU 绘制（WebKit 的 `AcceleratedBackingStore::rendererBufferTransportMode()` 在 DMA-BUF 渲染器被禁用时一种传输方式都不报，硬件加速随之不可用）。这条路径在 niri 上闪烁，在 deepin 上让滚动每步整块重绘（#6403，见 [Linux 设置页去掉分组与皮肤卡片的模糊阴影](2026-10-09-linux-settings-scroll-blur-shadow.md)）。

报告者装的 0.10.1 和 0.11.0 只带第一个变量，所以他那组对照实际只隔离出了合成开关；develop 上第二个变量单独也会把窗口送回同一条路径，不处理它的话，下一个版本连 `WEBKIT_DISABLE_COMPOSITING_MODE=0` 这个临时办法都会失效。

## Decision

`platforms/linux/data/msime-linux-settings.in` 先看 NVIDIA 专有驱动的内核模块是否已加载：`/proc/driver/nvidia/version` 或 `/sys/module/nvidia` 任一存在就算加载了（nvidia-open 也是这两个路径，nouveau 不是）。

- 加载了：与以前一样，两个变量未设置或为空时默认导出 `1`。
- 没加载：两个变量都不设，WebKit 按自己的默认判断能否走合成与 DMA-BUF。

调用方显式给出的非空值（`0`、`1` 或别的）在两种情况下都原样传下去，用户可以用它排查上游问题，或在规避判断不准的机器上自己打开、关掉规避。空值按未设置处理：WebKit 只把 `0` 当作不禁用，空串和 `1` 一样会禁用，原样传下去会在其他显卡上悄悄关掉合成，所以非 NVIDIA 时启动器把空值去掉。

启动器不改 `GDK_BACKEND`，也不按会话类型或合成器判断。

### 为什么按内核模块判断

- 不看哪块显卡在渲染：#3273 的机器是 Intel 核显加 NVIDIA 独显，照样出错。模块加载与否在双显卡笔记本上也成立，比解析 `lspci` 或 EGL 厂商字符串简单，也不用在启动器里起 GPU 上下文。
- 不加会话类型条件：#3273 在 X11 下也见过 GBM 分配失败；而且 Fcitx5 可能早于图形会话启动，它拉起的启动器拿到的 `XDG_SESSION_TYPE` 未必与终端里的相同。内核模块的状态对所有入口都一样，终端、桌面菜单、IBus/Fcitx5 菜单、Omarchy 钩子拉起的窗口因此得到同一组默认值。
- 用两个路径而不是一个：`/sys/module` 在某些容器里没有挂载，`/proc/driver/nvidia` 是驱动自己注册的，两者任一存在就够。其他项目的同类规避也用这两个路径，例如 derekross/onyx 的 `needs_dmabuf_workaround()`。

### 入口

所有面向用户的入口本来就经过这个启动器：桌面文件的每条 `Exec=`、IBus 与 Fcitx5 宿主的 `MSIME_EDITION_SETTINGS_PROGRAM`、首次配置引导、Omarchy 钩子与菜单。`MSIME_CLIENT_SETTINGS_COMMAND` 仍按原样优先于默认命令，由用户自己的命令决定环境。设置外壳是单实例的，先启动的那个进程的环境一直生效到它退出，排查时要先退出旧进程。

Arch 的 AUR 包有一条发布滞后兼容路径：`linux-v0.11.0` 标签切出来时还没有 DMA-BUF 变量，而新版 Arch WebKitGTK 已会在 NVIDIA + Wayland 上触发同一个 Error 71。`platforms/linux/packaging/arch/msime/PKGBUILD` 在源码包的 `prepare()`、`msime-bin/PKGBUILD` 在安装二进制时，都只对缺少该变量的旧启动器插入 `WEBKIT_DISABLE_DMABUF_RENDERER=${WEBKIT_DISABLE_DMABUF_RENDERER:-1}`；带有当前 NVIDIA 判断的后续源码与 RPM 不重复修改。这样不必改动发布标签或把开发分支源码混进 AUR，下一次 Linux 发布带上完整启动器后这条兼容分支自动变成空操作。

有一条启动不经过启动器：升级后的交接。旧的设置进程在关窗后还会留十分钟，这期间升级再打开设置，单实例插件只把参数和工作目录转给旧进程，它的 `hand_over_to_replaced_program`（`apps/desktop/src-tauri/src/lib.rs`）发现自身程序已被替换，就直接以 `Command::new` 启动新的 `msime-linux-desktop`，新程序继承旧进程的环境。旧启动器无条件导出过两个 `1`，所以从旧版本升级上来的第一个新进程仍走非合成路径，并且它自己关窗后也会再留十分钟。交接这里不清掉继承来的 `WEBKIT_DISABLE_*`：进程无从分辨这两个值是旧启动器的默认还是用户显式设的，清掉会丢掉用户真正的覆盖；改为经同目录的 `msime-linux-settings` 重新拉起也没用，继承来的 `1` 在启动器看来就是显式值。这只影响升级那一次，进程链退出后就不再出现，所以只在 README 里写明升级后要先让旧的设置进程完全退出，并请报告者复测前照做。

### 测试

`platforms/linux/tests/core/settings_launcher_contract.py` 把两个探测路径换成临时目录里的路径，分别检查：都不存在时四个入口两个变量都未设置；只有 `version` 文件、只有 `module` 目录时都默认为 `1`；每种情况下空值取默认、`0` 和 `1` 原样传下去，`GDK_BACKEND` 不变。

## Alternatives considered

- 维持对所有机器无条件默认（#3273 和 #5834 当时的选择）。理由最强的一条：故障取决于驱动、GTK/WebKit、合成器和实际渲染路径的组合，按厂商判断不一定准，无条件默认最省事，也不会漏掉没见过的组合；设置页是静态表单，CPU 绘制的开销当时看感觉不到。没采用：这个代价已经不是感觉不到的了，非合成路径在 niri 上闪烁（#6847）、在 deepin 上滚动不跟手（#6403），两次报告都来自没有 NVIDIA 的机器；而规避所针对的两次故障都只在 NVIDIA 专有驱动上出现过。没被判断覆盖到的组合仍可由用户显式设变量，README 写了做法。
- 用 `WEBKIT_DMABUF_RENDERER_FORCE_SHM=1` 代替两个禁用变量，保留合成、只把缓冲改走共享内存。理由：可能一个设置同时保住 NVIDIA 能启动和其他机器上的合成与滚动。没采用：#5834 的崩溃发生在哪一步没有定位到，这个替换能否规避它只能在出问题的 NVIDIA 机器上验证；它在 niri 上是否也闪烁同样不知道。等有 NVIDIA 用户能复测时可以再评估，届时只需改启动器的 NVIDIA 分支。2026-10-11 有 NVIDIA 专有驱动（open 内核模块 615.71.09）加 Hyprland 0.56.2 的机器复测：两个禁用变量都设成 `0` 再加 `WEBKIT_DMABUF_RENDERER_FORCE_SHM=1` 时窗口起得来，只设两个 `0` 时同一台机器约 0.3 秒退出（见 Verification）。它是否真的走 GPU 合成仍未量过，评估时先确认这一点。
- 按合成器判断，例如在 niri 下不导出。理由：改动范围最小，只影响报告者的环境。没采用：合成器白名单或黑名单要随每个新合成器补一条，而问题出在规避本身对非 NVIDIA 机器没有必要，不在 niri。
- 加一个水杉自己的环境变量或配置项开关这条规避。理由：给用户一个不依赖 WebKit 变量名的开关。没采用：两个 WebKit 变量本身就是显式开关，启动器原样尊重它们；再加一层只会让「最终值由哪里决定」更难查。

## Consequences

- 没有 NVIDIA 专有驱动的机器回到 WebKit 的合成路径：niri 上的闪烁应当消失，滚动由合成器平移图层，#6403 量到的每步整块重绘也随之不再出现。代价是这些机器上 WebKit 的 GPU 路径重新暴露出来：其他项目报告过个别 AMD/Intel 或虚拟机组合上 DMA-BUF 渲染器空白、EGL 初始化失败，我们手上没有这些报告，遇到时用户可以在 `~/.config/environment.d/` 里设变量绕开，README 写了做法。
- NVIDIA 专有驱动的机器行为不变，仍走非合成路径，所以 Linux 设置页不放带模糊阴影的约束继续成立。
- 判断只看内核模块。装了驱动却完全不用 NVIDIA 渲染的双显卡机器仍会套用规避，这是有意偏向安全的一侧：它与以前的行为相同，不会变差。
- 从旧版本升级时，如果旧的设置进程还在，升级后的交接会把它环境里的两个 `1` 带进新程序（见上文「入口」），新默认值要等这条进程链完全退出才生效，最长是关窗后十分钟。用户和 #6847 的报告者复测前都要先确认 `pgrep -af msime-linux-desktop` 没有输出，README 写了做法；不确认就复测，会误以为修复没有效果。
- Arch 的 0.11.0 AUR 包在构建或安装时会补上 DMA-BUF 规避，因此该版本的设置窗口可以启动；补丁只增加 NVIDIA 旧启动器已经缺失的第二个规避，旧版本原本就默认关闭合成，非 NVIDIA 机器不会因此回到更快的渲染路径。新版本启动器已经自带按驱动判断，补丁不再写入。

## Verification

- `python3 platforms/linux/tests/core/settings_launcher_contract.py` 在 macOS 和 Linux 构建镜像（Debian bookworm，`/bin/sh` 是 dash）里都通过。把非 NVIDIA 分支里去掉空值的那两行换成空操作，测试在空值用例上失败（`WEBKIT_DISABLE_COMPOSITING_MODE` 为空串而不是未设置）；把探测结果固定为未加载，测试在 NVIDIA 用例上失败。
- `sh -n platforms/linux/data/msime-linux-settings.in`：通过。
- `python3 scripts/test-arch-gentoo-packaging.py`：通过；静态门禁同时检查两个 Arch `PKGBUILD` 都保留旧启动器的 DMA-BUF 兼容补丁。
- 没有在 niri 上验证闪烁消失，也没有在 NVIDIA 加 KWin 或 Hyprland 的机器上验证 Error 71 没有回来；手上没有这两种环境，需要 #6847、#3273 和 #5834 的报告者复测。
- 2026-10-11 在 NVIDIA 专有驱动（open 内核模块 615.71.09）加 Hyprland 0.56.2、webkit2gtk-4.1 2.54.1 的混合显卡笔记本（AMD 核显加 RTX 4070 Max-Q）上补上了 Hyprland 侧的复测，三组对照都在 develop 的 `platforms/linux/data/msime-linux-settings.in` 生成的启动器下运行（0.11.0 的启动器还没有 #5877），每次运行前等旧的设置进程退出：NVIDIA 分支默认时 `/proc/<pid>/environ` 里两个变量都是 `1`，进程存活、窗口 `mapped`、日志里没有 `Error 71`；显式设成两个 `0` 时约 0.3 秒后以 `Error 71 (协议错误) dispatching to Wayland display` 退出；两个 `0` 再加 `WEBKIT_DMABUF_RENDERER_FORCE_SHM=1` 时存活、窗口 `mapped`。KWin 侧仍没有环境。
