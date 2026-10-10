# Agent Note: 检查更新移到 Rust，由各宿主调用

Status: implemented

## Problem

「检查更新」原本只写在共享 React 设置页里（`packages/ui/src/settings/use-update-check.ts`、`update-manifest.ts`、`release-assets.ts`）：网页自己 `fetch` GitHub 的发布列表，按平台标签前缀、版本 id 和架构挑出本版本的安装包和摘要。

Windows 的原生 WinUI 设置窗口用 C++ 写，用不了这段 TypeScript，「关于」页的「检查」只能用 `MSIME.exe --route=settings:about` 拉起共享应用，在那边检查。用户点一下检查更新就弹出另一个窗口。要在原生窗口里直接检查，要么在 C++ 里再写一份同样的挑选规则，要么把规则放到各宿主都能调用的地方。

## Decision

挑选规则和网络请求在 `crates/client-core/src/update_check.rs`，各宿主都调用它：

- `check_for_update(&UpdateCheckRequest)` 读 `https://api.github.com/repos/metasequoiaime/msime/releases?per_page=100`（10 秒超时、16 MiB 上限、`User-Agent: msime-client`），交给 `evaluate_update` 判断。`select_platform_release` 跳过草稿和预发布、去掉 `<platform>-` 前缀、只接受 `https://github.com/metasequoiaime/msime/releases/tag/…` 的发布页，按版本号取最新；Linux 和 Windows 上再按版本 id（和 Linux 的架构）挑出唯一的安装包与 GitHub 给的 `sha256:` 摘要，`signed` 为 `false`。规则逐条沿用原 TypeScript 实现，原来的选择测试移植成了 `update_check/tests.rs`。
- 结果序列化为 `{status:"available"|"current", update:{version:{display,parts}, release_url, installer_name, installer_sha256, signed}}` 或 `{status:"none"}`。
- 各宿主的接入：
  - Tauri 外壳（Windows、macOS、Linux 的设置应用）：命令 `update_check`，在阻塞线程池里调用 `check_for_update`。
  - 原生宿主：`msime_client_update_check`（`crates/host-api/src/ffi/reporting.rs`，请求 `{platform, current_version, edition?, arch?}`）。
  - Windows 原生设置窗口：在工作线程调用它，结果直接写在「关于」页那一行下方，有新版本时按钮变成「前往下载」。
  - 鸿蒙：NAPI `updateCheck` → `Settings.ets` 的桥接请求 `update_check`（平台由 ArkTS 固定为 `harmony`）→ 页面的 `checkUpdate`。
- 共享设置页通过可选的 `SettingsClient.checkUpdate(request)` 拿结果，自己不再联网；页面只保留展示：`fromHostUpdate` 再校验一次发布页地址、安装包名和摘要的形状，`describeInstallerTrust` 写未签名提示和校验命令，`mirrorDownloadUrl` 给国内镜像。宿主没有提供 `checkUpdate`（没有宿主的浏览器预览）时，「关于」页不显示检查更新按钮。
- 网页不再发出任何 HTTPS 请求，Tauri CSP 的 `connect-src` 只剩 `ipc:` 和 `http://ipc.localhost`。
- 原生设置窗口的当前版本是编译期宏 `MSIME_WINDOWS_VERSION`：`Build-Client.ps1` 用 `/p:MsimeVersion=<TargetVersion>` 传入，开发构建由工程读 `platforms/windows/version.txt`，和 CMake 给 Server 的值相同。
- `scripts/test-editions.py` 同时核对 `update_check.rs` 和 `update-manifest.ts` 推导 Windows 安装包名前缀的写法。

## Alternatives considered

- **在 Windows 原生窗口里用 C++ 再写一份挑选规则** — 只改 Windows，不动其他平台和共享页面，风险最小。但版本 id、架构、摘要格式和「两个候选就不给」这些规则会有两份，任何一边改了另一边都不会知道；`test-editions.py` 已经要靠抄写片段来防止安装包名推导走样，再多一份 C++ 只会更难对齐。
- **只给 Windows 原生加 Rust 导出，共享 React 页面保留自己的 TypeScript 实现** — 改动面小，网页不用接新的宿主方法。但这样规则仍是两份（Rust 和 TypeScript），只是把重复从 C++ 换成了 Rust；用户选择了一次把所有用到这段逻辑的宿主都切过来。
- **保留现状，只把「检查」做成在共享应用里静默检查、不显示窗口** — 不用动任何逻辑。但结果没有地方显示，用户还是得打开共享应用才能看到，问题没有解决。

## Consequences

- **收益**：「选哪个发布、哪个安装包」只有 Rust 一份规则，Windows 原生、Tauri 和鸿蒙得到同样的答案；Windows 点「检查」不再弹出共享应用；网页的联网面收窄到 IPC；原生设置窗口第一次知道并显示自己的版本号。
- **代价与已知上限**：
  - 没有宿主的浏览器预览不再能检查更新，原来读 `https://msime.app/update.json` 的那条路径随之删除。
  - Android、iOS 和 macOS 输入法本体的检查更新仍是各自独立的实现（`UpdateApi.java`、App Store 查询、Sparkle），不走这里，见 [Android 更新过滤非法发布标签](../bug-fix/2026-10-09-android-update-invalid-tag-selection.md)。哪天要统一它们，从这个模块扩展。
  - 网页原来的 10 秒 `AbortController` 超时改由 Rust 的请求超时承担；鸿蒙桥接自己还有 30 秒的兜底超时。
  - 原生 Windows 窗口只显示文字提示和摘要，不像共享页面那样给可复制的 `Get-FileHash` 命令和国内镜像链接。
  - Linux 的 Nix 包里为设置页 HTTPS 请求加的 `glib-networking`（`platforms/linux/nix/fcitx5.nix`）现在可能已经不需要了，这次没有改，需要在 Linux 上验证后再决定。

## Verification

- `cargo test -p msime-client-core --lib update_check`：12 项，包括逐条移植的 Linux 和 Windows 资产选择、摘要校验、架构筛选、版本排序、发布页地址校验，以及结果的序列化形状。
- `cargo test -p msime-host-api --lib update_check`：非法请求在联网之前就被拒绝。
- `cargo check -p msime-desktop`（Windows）。
- `apps/desktop`：`pnpm run typecheck` 通过；`vitest run` 中检查更新相关的 hook、展示逻辑、关于页和鸿蒙胶囊测试全部通过，全套只剩 3 项在 develop 上就失败的 Windows 换行符文本比对。
- `apps/harmony`：`pnpm run typecheck` 和 `pnpm --filter @msime/harmony build` 通过；`scripts/test-harmony-*.py` 中除了 `test-harmony-settings-bundle.py`（在 Windows 上因 `stage-settings.sh` 的 CRLF 无法运行）全部通过。ArkTS 没有用 `hvigorw` 编译过。
- `platforms/windows/tests/tools/build_client.ps1`：设置窗口在有 `TargetVersion` 时带 `/p:MsimeVersion`，没有时不带。
- `scripts/test-editions.py` 通过。
- Windows 11 实机：`msime_host_api.dll` 发布构建对真实 GitHub 返回 Windows full「none」（当时两个 Windows 发布都是预发布）、Linux full x86_64 选中 `msime-linux_0.11.0_amd64.deb` 并带摘要、Linux 五笔 aarch64 选中 `msime-linux-wubi_0.11.0_arm64.deb`、macOS 0.52.0。用这个库编出的 `msime-client-settings.exe`（`/p:MsimeVersion=0.0.1`）以 `--route=settings:about` 启动，UI 自动化点「检查」：显示「当前版本 v0.0.1」和「暂无可用发行版」，没有启动 `MSIME.exe`；临时把请求平台改成 `linux` 编译一次，显示「发现新版本 v0.11.0」、未签名提示，按钮变为「前往下载」。
