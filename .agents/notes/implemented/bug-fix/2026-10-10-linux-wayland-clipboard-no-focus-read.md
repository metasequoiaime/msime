# Agent Note: 没有 data-control 的 Wayland 合成器上不用 wl-paste 读剪贴板

Status: implemented

## Problem

#6514：Ubuntu 24.04 GNOME 46 Wayland 上开启剪贴板历史后，gnome-shell 每秒刷一条 `meta_window_set_stack_position_no_sync: assertion 'window->stack_position >= 0' failed`，前台应用的输入焦点每 10 秒抖动十几次，拼音预编辑被反复打断，中文打不出来；关掉剪贴板历史立即恢复。

根因是 Mutter 不提供 `wlr-data-control`（也没有 `ext-data-control`）。在这种合成器上 `wl-paste --watch` 立即以非零状态退出，而一次性的 `wl-paste` 读取只能临时建一个窗口、拿到键盘焦点后才读得到剪贴板。每读一次，前台应用就失去一次焦点，输入法的输入上下文随之 focus out，预编辑被清掉；这个一闪而过的窗口就是那条断言的来源。

两处会在这种合成器上反复调用 `wl-paste`：

- `msime-linux-clipboard-monitor`（systemd 用户服务）：`--watch` 失败后回退到轮询，轮询先调 `wl-paste`。最初每 0.75 秒一次；#6618 把它节流到每 5 秒一次，频率降了，但每 5 秒仍抢一次焦点，长句照样会被打断。
- Tauri 设置进程的 `start_linux_clipboard_monitor`：开着设置窗口时每 0.75 秒先 `wl-paste --list-types` 再 `wl-paste`，一次两抢。

## Decision

只要合成器不提供 data-control，就不用 `wl-paste` 读剪贴板，改经 XWayland 的 X11 剪贴板。Mutter 会把 Wayland 剪贴板同步到 XWayland 的 CLIPBOARD（新的复制会让它重新成为 X 选择的所有者），X11 客户端经 `XConvertSelection` 读取不涉及焦点，XFixes 也能收到所有权变化。

- 监视器的 `clipboard_text()` 只走 X11 读取（原生 `msime-linux-clipboard-watch-x11 --read`、`xclip`、`xsel`），从不调用 `wl-paste`。Wayland 上能用的事件源只有 `wl-paste --watch`，它把数据经标准输入交给 `--capture-event`，本来就用不到轮询读取；而需要轮询的 Wayland 会话恰恰是 `--watch` 起不来、`wl-paste` 会抢焦点的那种。
- `wl-paste --watch` 以非零状态退出时，监视器记下这一点，立即在同一会话里改起 `msime-linux-clipboard-watch-x11`（XFixes 监听），并在 stderr（即 journal）写一行原因；关闭再开启剪贴板历史之前不再尝试 `wl-paste --watch`。没有 `DISPLAY` 时暂停采集，因为此时没有不抢焦点的读法。#6618 加的 5 秒节流随之删除：轮询不再碰 `wl-paste`，节流保护的东西不存在了。
- Tauri 侧的 `linux_clipboard_text()` 在 Wayland 上先用 `wayland_paste_is_background()` 判断：进程内跑一次 `wl-paste --type text --watch true`，1 秒内退出就是不支持 data-control（没装 `wl-paste` 同样算），结果缓存在 `OnceLock` 里。不支持时跳过 `wl-paste`，直接走 `xclip`/`xsel`。`--watch` 只走 data-control，不建窗口，探测本身不抢焦点。

提供 data-control 的合成器（wlroots 系、KWin）和纯 X11 会话行为不变：监视器仍用 `wl-paste --watch` 或 XFixes 事件，Tauri 仍先用 `wl-paste`。

## Alternatives considered

- **保留 #6618 的 5 秒节流，或继续加大间隔**：改动最小，也确实把断言从每秒一条降到每 5 秒一条。但每次读取都抢一次焦点这件事没变，间隔多长都会落在用户打字的中间；而且间隔越长，剪贴板历史漏记越多。拿采集频率换输入法可用性，两头都不好。
- **只在检测到 `--watch` 失败后才把 `wl-paste` 从轮询里拿掉**：在提供 data-control、但 watcher 意外退出的 30 秒重试窗口里还能用 `wl-paste` 补读。可状态更多：读配置出错的异常路径会在 `--watch` 失败被观察到之前就进入轮询，GNOME 上那 30 秒照样每 0.75 秒抢一次焦点，#6618 正是为此在异常路径里也把 Wayland 当成失败。直接让轮询不碰 `wl-paste`，这个窗口就不存在了；付出的只是 data-control 合成器上 watcher 崩溃后 30 秒内的复制可能漏记。
- **按 `XDG_CURRENT_DESKTOP` 识别 GNOME**：不用起进程。但它说的是桌面而不是合成器的协议支持，GNOME 哪天支持 `ext-data-control` 就会误判，其他不支持 data-control 的合成器又漏掉。`wl-paste --watch` 是否立即退出正是我们关心的那个能力本身。
- **引入 Wayland 客户端库直接枚举全局对象**：判断最准，但为一个布尔值给 Tauri 进程加一套 Wayland 依赖，而 `wl-paste --watch` 的行为本身就是 wl-clipboard 文档化的契约。
- **GNOME Shell 扩展或 Mutter 私有 D-Bus 接口**：要求用户另装扩展，私有接口没有稳定性承诺，维护成本远高于走 XWayland。

## Consequences

GNOME Wayland 上开启剪贴板历史不再抢焦点、不再刷断言，采集改由 XFixes 事件驱动，比原先的轮询更及时，再次复制相同文本也能记下。代价是监视器会连上 XWayland：Xwayland 按需启动的会话里，它会让 Xwayland 一直运行。禁用了 Xwayland 的 GNOME 会话没有 `DISPLAY`，监视器暂停采集、Tauri 读取报不可用，而不是像以前那样抢焦点去读；这是有意的取舍，输入法可用优先于剪贴板历史。Tauri 进程在 data-control 合成器上第一次读剪贴板会多等最多 1 秒的探测。

写入（`wl-copy`）不在本次范围：它只在用户主动选中一条历史时发生，一次性抢一下焦点不构成持续干扰。

## Verification

- `platforms/linux/tests/clipboard/clipboard_wayland_watch.py`：`--watch` 以非零退出后 `wl-paste` 只被调用过那一次、日志里写明原因；有 `DISPLAY` 时改由合成的 XWayland 监听器触发捕获、`wl-paste` 不被用来读取；正常 watcher 的事件捕获不变。前两项在改动前的监视器上失败。
- `clipboard_x11_events.py`、`clipboard_x11_read.py` 在容器的 Xvfb 里覆盖纯 X11 路径不退化。
- `linux_process.rs` 的 `stays_running_distinguishes_an_early_exit` 覆盖探测的提前退出、起不来和持续运行三种情况。
- 没有在 GNOME Wayland 真机上验证；Mutter 把 Wayland 剪贴板同步到 XWayland 并更新 X 选择所有者，依据的是 Mutter 的 X11 选择桥接行为，需在真机上确认 XFixes 事件确实送达。
