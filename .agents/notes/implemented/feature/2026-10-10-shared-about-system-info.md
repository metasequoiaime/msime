# Agent Note: 共享「关于」页的系统信息与一键复制

Status: implemented

## Problem

#6644：反馈问题时没法一键复制环境信息，用户要自己去查系统版本、架构、桌面环境、输入法框架。Android 已经有原生的「设备信息」组（[Android 设备信息与一键复制](2026-10-08-android-device-info-copy.md)），但 macOS、Linux 设置应用用的共享「关于」页（`packages/ui/src/settings/pages/about-page.tsx`）只有版本行。共享页能拿到的环境信息也很少：`HostCapabilities` 里只有 `os_version`（只有 macOS 和 HarmonyOS 填）和 `arch`；Linux 上 `os_version` 为空，「帮助与反馈」附带的诊断文本只能退回 User-Agent。

## Decision

- 共享「关于」页（HarmonyOS 之外的分支）在「版本与更新」和「许可与隐私」之间新增「系统信息」组，逐项列出应用版本（非 full 版本附版本名）、系统、内核、桌面环境、输入法框架、设备型号、处理器架构和输入方案，组末「复制系统信息」把同样的内容按「名称：值」每行一项放进剪贴板。宿主有 `copyText` 才画复制按钮。列表与文本由 `packages/ui/src/settings/system-info-section.tsx` 的纯函数 `systemInfoEntries()` / `systemInfoText()` 生成。
- 宿主报告不了的项直接省略，不写「未知」。这与 Android 原生页不同：共享页面对六个宿主，多数字段只有 Linux 填，满屏「未知」只会让其他平台看起来坏了。
- `HostCapabilities` 新增四个运行时可选字段 `kernel_version`、`desktop_session`、`input_method_framework`、`device_model`，与 `os_version` 一样不属于 `for_platform` 的平台假设，由宿主在之后按实际机器填写，缺省不序列化，旧文档照常解析。解析与清洗放在 `crates/client-core/src/host_surface/environment.rs`，只接受短的、不含控制字符的文本，读到别的就不报告。
- Linux 设置应用（`apps/desktop/src-tauri/src/lib.rs` 的 `fill_linux_environment`）填：`os_version` 取 `/etc/os-release`（或 `/usr/lib/os-release`）的 `PRETTY_NAME`；内核取 `/proc/sys/kernel/osrelease`；桌面环境取 `XDG_CURRENT_DESKTOP` 与 `XDG_SESSION_TYPE`；设备型号取 DMI 的 `sys_vendor` 与 `product_name`；输入法框架取运行中的宿主写的 `candidate-panel.json` 的 `host`（`ibus` → `IBus`，`fcitx5` → `Fcitx5`）。全是小文件和环境变量，不起进程。
- 所有字段只描述机器、会话和输入法设置，不含账号、输入内容或路径；只在用户点「复制」时进剪贴板，什么都不上传。

## 各平台接入情况

- 已接入：Linux（共享页 + 上面的全部字段）、macOS（共享页，系统版本、架构、应用版本、输入方案）。
- 已有原生实现：Android（「关于 → 设备信息」，#5662）。
- 需跟进：Windows 的设置是原生 WinUI 3 `msime-client-settings.exe`，iOS 的「关于」是原生 SwiftUI，都不渲染共享页；HarmonyOS 走共享页里按设计稿单独绘制的分支，没有加这一组，是否加入要按设计决定。macOS 的设备型号（`hw.model`）需要 `sysctl`，这次没有读；输入法框架版本（IBus 1.5.x、Fcitx5 5.x）需要宿主在状态文件里多写一项或起进程查询，也没有做。

## Alternatives considered

- **输入法框架从 `GTK_IM_MODULE` / `QT_IM_MODULE` / `XMODIFIERS` 推断**：不依赖宿主在运行，设置应用自己就能读。但这些变量说的是桌面配置想用哪个框架，不是水杉实际挂在哪个框架上；GNOME Wayland 下 GTK 走 text-input 协议，`GTK_IM_MODULE` 往往为空。宿主已经在 `candidate-panel.json` 里写了 `host`，用它更准，代价是宿主没在运行时这一项为空。
- **单独加一个 Tauri 命令（`system_info`）只在打开「关于」时读**：不让 `host_capabilities` 每次多读几个文件。但这要在 `client` 接口上新开一条方法、六个宿主的 client 都要考虑它；而 `os_version`、`arch` 已经是同样性质、走 `HostCapabilities` 的字段，几个小文件的读取开销可以忽略。
- **直接复用「帮助与反馈」的 `supportDiagnostics` 文本**：已有、各平台都能生成。但它只有版本、系统和输入方案，而且缺系统版本时会附上 User-Agent，不适合作为「关于」里逐项展示的信息；两者的用途不同，这次只让它自动受益于 Linux 新填的 `os_version`。

## Consequences

收益：macOS 和 Linux 用户在「关于」里一次复制就能带齐排错信息；Linux 的「帮助与反馈」附带的诊断文本也从 User-Agent 变成真实的发行版名。

代价：「关于」页多一组、版本在「当前版本」和「系统信息」里各出现一次；`HostCapabilities` 多了四个只有 Linux 会填的字段。Windows、iOS 还要各自在原生页面里做一遍。

## Verification

- `cargo test -p msime-client-core --lib environment`：8 项通过（os-release 解析与转义、内核、桌面会话、框架、设备型号，以及读到异常内容时不报告）。
- `apps/desktop/tests/settings/system-info-section.test.tsx`：字段顺序与省略、复制文本格式、非 full 版本名、「关于」页里的分组与复制按钮、没有剪贴板时不画按钮。
- `apps/desktop` 的 `vitest run`、`tsc --noEmit`，`cargo clippy -p msime-client-core --all-targets -- -D warnings`，`bash scripts/run-checks.sh`；`msime-desktop` 的 Linux 分支经推送前门禁的容器 `cargo check` 编译。没有在真实 Linux 桌面上打开设置应用看过实际读到的值。
