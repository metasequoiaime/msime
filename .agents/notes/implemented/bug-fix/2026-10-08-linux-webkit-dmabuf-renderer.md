# Agent Note: Linux 设置窗口默认禁用 WebKit DMA-BUF 渲染器

Status: implemented

## Problem

在 Arch Linux、KDE Plasma 6.7 Wayland、NVIDIA 驱动（nvidia-open 615，KWin 提供 `wp_linux_drm_syncobj_manager_v1`）、webkit2gtk-4.1 2.54 的环境里，设置窗口一启动就以退出码 1 结束，stderr 只有 `Gdk-Message: Error 71 (Protocol error) dispatching to Wayland display`，KWin 侧记一条 `error in client communication`（issue #5834）。首次配置页也是这个窗口，所以这类用户连首次配置都进不去。

启动器早先已默认导出 `WEBKIT_DISABLE_COMPOSITING_MODE=1`，在这个环境里不够。报告者做了对照：同一会话只多设一个 `WEBKIT_DISABLE_DMABUF_RENDERER=1` 窗口就能正常打开；放进没有 `/dev/dri` 的沙箱、让 Mesa 退回软件渲染时两种设置都不崩。触发条件是 WebKitGTK 的 DMA-BUF 渲染器走 GPU 路径，与具体面板或配置状态无关，对应上游 WebKit bug 280210。

## Decision

`platforms/linux/data/msime-linux-settings.in` 在 exec `msime-linux-desktop` 之前，与 `WEBKIT_DISABLE_COMPOSITING_MODE` 并排导出 `WEBKIT_DISABLE_DMABUF_RENDERER=${WEBKIT_DISABLE_DMABUF_RENDERER:-1}`：不论驱动和会话类型，默认都禁用；调用方显式设成 `0` 时原样保留，用于排查上游问题。

终端、桌面菜单、IBus/Fcitx5 菜单（`MSIME_EDITION_SETTINGS_PROGRAM`）、Omarchy 钩子与菜单都经这个启动器拉起窗口，改一处就覆盖全部入口。直接运行 `msime-linux-desktop` 不经过启动器，它不是面向用户的入口，这次不处理。

`settings_launcher_contract.py` 核对两个变量在各入口的默认值都是 `1`，空值也会回到默认值，`0` 和 `1` 原样传下去，`GDK_BACKEND` 不受影响。

## Alternatives considered

- 只在检测到 NVIDIA 加 Wayland 时导出（issue 建议用 `/sys/module/nvidia` 加 `XDG_SESSION_TYPE=wayland` 判断）：这样能让其他显卡保留 GPU 渲染。没采用的原因：设置窗口是静态表单页面，用不着 GPU 渲染，退回共享内存的开销感觉不到；上游这个 bug 也不只出现在 NVIDIA 上；而检测分支要随驱动和会话的组合变化，CI 里又没有对应的机器能跑。现有的合成开关也是无条件默认，两个变量保持同一种规则。
- 在 Rust 宿主的 `main` 里、GTK 初始化之前用 `std::env::set_var` 设置：这样直接运行 `msime-linux-desktop` 也能覆盖到。没采用的原因：合成开关已经放在启动器里，环境默认值分在两处会让排查时查不清最终值是哪里定的；而且启动器就是唯一面向用户的入口。

## Consequences

所有 Linux 用户的设置窗口都改用共享内存把画面交给 GTK，不再走 DMA-BUF。对设置页这种静态页面，用户感觉不到差别；代价是将来如果在这个窗口里放重绘密集的内容，就需要重新评估。

## Verification

- `python3 platforms/linux/tests/core/settings_launcher_contract.py`：通过。把启动器换回改动前的版本，测试在第一个入口就失败（`WEBKIT_DISABLE_DMABUF_RENDERER` 为空）。
- `sh -n platforms/linux/data/msime-linux-settings.in`：通过。
- 没有在 NVIDIA 加 KDE Wayland 的机器上验证过。修复是否有效，依据是报告者在同一台机器上做的对照实验。
