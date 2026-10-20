# Agent Note: Linux 上 MCP 写入皮肤后重新发布皮肤目录

Status: implemented

## Problem

Linux 的 IBus 和 Fcitx5 宿主自己不扫皮肤目录，只从运行时选项文档（`runtime-options.json`）的 `candidate_skin_catalog` 知道装了哪些皮肤、每款画什么配色、装饰带多大、卡片圆角多少。这份目录只有设置应用在写：保存偏好时（`sync_runtime_options`）和重扫皮肤时（`rescan_skin_catalog` → `publish_candidate_skin_catalog`），而设置应用装、删、替换皮肤之后都会重扫。

MCP 服务的 `create_candidate_skin` 和 `delete_candidate_skin` 只写皮肤目录，从不发布。于是在 Linux 上经 MCP 新装的皮肤在两个宿主的主题菜单里不出现、选上也画不出来；同名替换（`replace: true`，包括替换正在用的那款）之后，IBus 的配色和 Fcitx5 非 canvas 包的配色、尺寸一直是旧包的，直到用户打开设置页触发一次重扫。只有 Fcitx5 的装饰图和 canvas 包不受影响，它们按文件戳刷新。

MCP 的 `update_preferences` 在 Linux 上把偏好写进同一份文档，但原样保留旧的皮肤目录。目录放不下全部皮肤（最多 32 款、16 KiB 预算内）时，设置应用的保存按新选择重新裁剪、优先留下选中的那款，MCP 的保存不会，选了一款被裁掉的皮肤就画不出来。

这是 [macOS 与 HarmonyOS 发现被同名替换的皮肤包](2026-10-20-macos-replaced-skin-package-refresh.md) 列为后续的 Linux 那一项。

## Decision

把设置应用里算文档的那段挪进 `msime-client-services` 的 `skin::linux_runtime_options`，设置应用和 MCP 共用：

- `LINUX_RUNTIME_OPTIONS_LIMIT`（16 KiB）、`LINUX_RUNTIME_OPTIONS_CATALOG_BUDGET`（再留 1 KiB 给升级刷新）和 `TooLarge`。
- `runtime_options_bytes`：超过上限就拒绝。
- `runtime_options_with_skin_catalog`：按 `host_candidate_catalog` 生成目录，放不下时从末尾丢包、选中的两个槽位最后丢，连空目录都放不下时整个键不写。
- `republish_skin_catalog`：只换目录，新目录与文档里已有的一样时返回 `None`，调用方不写文件。

文件读写留在两边：设置应用照旧用 `shared::atomic_file` 和进程内的文档缓存，MCP 用 `msime_client_core::file_lock` 相对已打开目录读写（新模块 `crates/mcp-server/src/runtime_options.rs`，偏好的发布也从 `preferences.rs` 挪到这里）。

MCP 一侧：

- `skins::create` 和 `skins::delete` 多一个 `linux_host_options: Option<&Path>`，写成功后调用 `runtime_options::republish_skin_catalog`。服务器传 `Config::linux_host_options()`，只在 Linux 上是运行时选项的路径，其他系统是 `None`。发布与设置应用一样尽力而为：皮肤已经写好，发布失败不让调用失败，只在 stderr 记一行。被拒绝的写入不碰文档。
- `update_preferences` 在 Linux 上改用 `runtime_options_with_skin_catalog`，偏好和按新选择裁剪的皮肤目录一起发布，与 `sync_runtime_options` 相同。发布失败时照旧把偏好回滚。

## 同名替换不需要另加过时信号

macOS 要另记包的文件身份，是因为它直接读包、而同名替换不改选择也不改偏好的 revision。Linux 宿主读的是目录条目，条目里正是非 canvas 包在 Linux 上画的全部输入：配色（`candidate.light`、`candidate.dark`）、装饰带的尺寸、装饰图的路径和对齐、`corner_radius_dip`。逐个宿主核对过比较的东西：

- IBus：`ibus_main.cpp` 监视文档，内容与上次不同才调 `msime_ibus_configure`，后者在文档变了时递增 `configuration_generation`，下一次画候选窗时重新取色（`theme_in_mode` 每次读 `configured` 里的目录）。
- Fcitx5：每 250 ms 重读文档（`refreshProviderSockets`），`applyCandidatePanelTheme` 比较的输入里有 `candidate_theme_request`，它带着所画包在目录里的原样条目；另外比较装饰图的文件戳（`fcitx_overlay_stamp`：路径、大小、修改时间）和 canvas 包的 `skin.toml` 与素材的文件戳。

所以替换后条目变了，文档就变，两个宿主都会重新应用；条目没变，说明宿主从目录画的东西都没变，「没变就不写」不会漏掉什么，装饰图和 canvas 素材的原地变化由 Fcitx5 的文件戳发现，IBus 不画图。`republish_skin_catalog` 的测试同时断言了这两种情形。

## 其他平台

- macOS：输入法经 `msime_client_skin_catalog` 直接读皮肤目录，同名替换由 #8882 的文件身份检查发现，运行时选项里没有皮肤目录。
- Windows：服务器每 500 ms 用 `SkinResourceRevision` 对两个槽位的包算文件指纹。
- 设置应用也只在 Linux 上发布目录（`#[cfg(target_os = "linux")]`），所以 `Config::linux_host_options()` 在别的系统上是 `None`，不给不读它的宿主写文件。

## Alternatives considered

- **MCP 自己再写一份发布逻辑**：不碰设置应用，改动只在一个 crate。但 16 KiB 上限、给升级刷新留的 1 KiB、选中皮肤最后丢的裁剪顺序和「没变就不写」都是两个宿主的读取契约，两份实现迟早漂移；MCP 已经抄过一次偏好发布，`LINUX_RUNTIME_OPTIONS_LIMIT` 的注释只能写「见设置应用」。
- **连文件读写一起挪进共享层**：两边只调一个函数。但设置应用的读取带进程内文档缓存、2 MiB 读取上限和 `NotFound` 当作尚未安装，MCP 的读写相对已打开目录、防路径上的目录被换成符号链接，两套原语和错误类型各有测试；硬合成一个要么丢掉一边的保证，要么塞进一组参数。共享的是规则，不是 I/O。
- **只修 `create_candidate_skin` 和 `delete_candidate_skin`，`update_preferences` 保持只换偏好**：改动更小。但设置应用的保存发布目录正是为了「选了就画得出」，MCP 的保存不重新裁剪，同一类问题就留在选择那一步；用共享函数补上只多一行。
- **同名替换时另加文件身份之类的过时信号**：照 macOS 的做法。上一节核对过，条目覆盖了 Linux 非 canvas 包画的全部内容，图片和 canvas 已有文件戳，再加一道只会让宿主多 `stat` 而不会多发现什么。
- **发布失败时让 MCP 调用失败**：代理能知道输入法还没看到新皮肤。但皮肤已经写进目录，报失败会让代理以为没装上而重试或换 id；设置应用的重扫同样尽力而为，下一次重扫或保存会补上。

## Consequences

- **收益**：在 Linux 上经 MCP 新装、替换或删除皮肤后，IBus 和 Fcitx5 在下一次读文档时（IBus 文件监视，Fcitx5 最多 250 ms）就用新目录，主题菜单、配色、装饰带尺寸和圆角都跟上，不用打开设置页；经 MCP 选皮肤时目录按新选择裁剪。设置应用和 MCP 只剩一份算文档的代码。
- **代价**：`update_preferences` 在 Linux 上每次保存都扫一遍皮肤目录，与设置应用的保存相同。MCP 和设置应用仍可能同时改这份文档，各自读改写之间没有跨进程锁，后写的一方保留自己读到的其余键，与之前偏好发布的情形相同。
- **已知上限**：`publish_candidate_skin`（`license_assets`）改清单不发布目录，因为授权不在目录条目里，宿主没有可更新的东西。目录发布失败只记在 MCP 的 stderr 里，代理看不到。

## Verification

- `crates/client-services/src/skin/linux_runtime_options/tests.rs`：`a_republish_writes_only_when_what_the_hosts_draw_changed`（首次发布、没变不写、按安装器的暂存加 rename 同名替换成新配色后条目变、内容相同的再次替换不写、删除后发布空目录）；`the_skin_catalog_stays_within_what_the_hosts_read`（40 款皮肤时裁剪进预算并留下两个槽位选中的包，放不下目录时不写键，去掉目录仍超限时拒绝），后者从设置应用的 Linux 测试挪来，现在在所有平台上跑。
- `crates/mcp-server/src/skins.rs`：`every_skin_write_republishes_the_catalog_the_linux_hosts_read`，临时状态目录加临时运行时选项文档，不涉及真实的 IBus 或 Fcitx5：安装发布新条目，同名替换后条目里是新配色，被拒绝的写入和不给文档时文件字节不变，删除后条目消失、其余键保留。把发布调用短路后这个测试失败。
- `crates/mcp-server/src/runtime_options.rs`：目录没变时文件的字节和修改时间都不变；偏好随所选皮肤的目录一起发布；原有的偏好发布测试（屏幕键盘照片不进副本、超限拒绝、符号链接的父目录和文档被拒绝）随函数挪来。`crates/mcp-server/src/config.rs`：`linux_host_options` 只在 Linux 上给出路径。
- `crates/mcp-server/src/preferences.rs` 的 `an_unreadable_runtime_options_document_does_not_leave_preferences_saved` 只在 Linux 上编译，CI 里没有地方在 Linux 上跑 `msime-mcp-server` 的测试，它自提交 `fc03bc2235`（`fix(mcp): 发布失败时回滚偏好修改`）起一直是红的：回滚本身也是一次按修订号的保存，revision 前进两次，而测试拿整份视图（含 revision）和修改前比较。改成断言 revision 前进两次、其余内容回到修改之前。
- 在 Mac Studio 的 Linux 容器（`Dockerfile.desktop-check`，非 root 用户）里跑了 `msime-client-services` 的上述测试、`msime-mcp-server` 的全部单元测试，以及 `msime-desktop`、`msime-mcp-server`、`msime-client-services` 的 clippy 和设置应用 Linux 侧的 `runtime_options_*`、`skin_rescan_*` 测试。以 root 运行时 `publishing_runtime_options_rejects_a_symlinked_parent` 和 `argument_files_reject_a_symlinked_parent` 两个符号链接测试在 develop 上同样失败（root 绕过了属主检查），与本次无关。没有在真实 IBus 或 Fcitx5 会话里验收（证据第 2 级）。

## Existing notes audit

按 `candidate_skin_catalog`、`publish_candidate_skin_catalog`、`rescan_skin_catalog`、`runtime_options_with_skin_catalog`、重扫检索：

- [macOS 与 HarmonyOS 发现被同名替换的皮肤包](2026-10-20-macos-replaced-skin-package-refresh.md)：其他平台一节的 Linux 一项由本篇补上，就地改了那一条和 Consequences 里的后续列表。
- [MCP 候选皮肤管理](../feature/2026-10-19-mcp-candidate-skin-management.md)：「已知上限」里 Linux 要等设置页重扫的事实不再成立，就地改成指向本篇。
- [MCP 社区皮肤发布](../feature/2026-10-20-mcp-community-skin-publish.md)：`add_license` 那条「与 `create_candidate_skin` 相同」不再成立，就地改正。
- [浅色深色槽位](../feature/2026-10-09-candidate-skin-light-dark-slots.md)：「Tauri 发布目录时两个槽位的皮肤都保留在上限之内」改成设置应用和 MCP 经共享模块发布。
- [候选皮肤编辑器](../feature/2026-10-19-candidate-skin-editor.md)：只描述设置应用装好后重扫，仍然成立，不动。
- [Linux canvas 图片格式](../feature/2026-10-20-linux-canvas-image-formats.md)：命中的是上游测试文件名，无关。
