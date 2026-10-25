# Agent Note: Linux 首次配置后把设置交给新进程，换到输入法的状态目录

Status: implemented

## Problem

Ubuntu 24.04 上全新安装 nightly 0.13.0 实机验收：没有 `~/.config/msime-client/` 时打开设置，完成首次配置后点「进入设置」，把「更新通道」改成测试版，偏好和更新记录写进了 `~/.local/share/app.msime.linux/preferences.json`、`update-check.json`，输入法读的 `~/.config/msime-client/preferences.json` 没变；结束进程重开设置后通道回到稳定版。同一进程里还显示「此平台暂不支持粤拼、注音、笔画」，重开后正常。

原因在设置进程启动时（`apps/desktop/src-tauri/src/lib.rs` 的 `setup`）只定一次状态目录：`msime-linux-settings` 总会导出 `MSIME_CLIENT_HOST_OPTIONS=~/.config/msime-client/runtime-options.json`，首次运行时这份文件不存在，读不到 `preferences_directory`，目录退回 Tauri 的 `app_data_dir`。这个目录随即交给 `PreferencesStore`、剪贴板历史、打字统计、诊断日志、公告、背单词、皮肤、插件、自定义皮肤库、键盘皮肤试用、Linux 账号会话和 `BackgroundUpdateState`，进程里不再换。首次配置页经 `msime-linux-setup --state ~/.config/msime-client` 调 `msime-linux-prepare`，宿主库写出的 HostOptions 里 `preferences_directory` 就是这个状态目录（`crates/host-api/src/lib.rs` 的 `prepare_host_with_updates`），但已经建好的状态对象仍指着 `app_data_dir`。宿主能力那一半在前端：`DesktopSettings` 在首次配置前调一次 `host_capabilities`，那时 HostOptions 是空文档，语言词库方案全被去掉，结果一直缓存在 `settingsClient` 里。这是首次配置页出现以来就有的问题，早于更新通道。

## Decision

首次配置换了状态目录时，把设置交给一个新进程，本进程退出；新进程按准备好的 runtime-options.json 定目录、重新发现宿主能力。

- `LinuxStateDirectory`（`lib.rs`）在启动时定目录并记下依据：`MSIME_CLIENT_STATE_DIR`、启动时的 runtime-options 路径和退回用的 `app_data_dir`。`moved()` 现读这份 runtime-options，按它该用的目录与启动时定下的不同才返回真；目录由 `MSIME_CLIENT_STATE_DIR` 指定、文件读不了或内容不对时一律为假，后者是因为新进程启动时同样读不了它，交过去只会起不来。Linux 启动时的目录改由它求出，规则与原来的 `linux_runtime_state_directory` 相同，Windows、macOS、Android、iOS 的分支不动。
- 原来升级后交接用的 `hand_over_to_replaced_program` 改名 `hand_over_settings_process`，交接条件从「程序已被替换」扩成「程序已被替换，或 `LinuxStateDirectory::moved()`」；后者启动 `current_exe()`，前者照旧启动原路径上的新程序。隐藏窗口、等 `FLUSH_GRACE`、放开单实例 D-Bus 名、启动并退出的顺序不变。新进程起不来时清掉交接标记，留在本进程上把界面重新打开。
- 两个地方检查：收到下一次启动时（单实例回调，原有入口；这也覆盖在终端里跑 `msime-linux-setup` 时还留着一个首次运行的设置进程的情况），以及离开首次配置页时（`leave_linux_setup_window`，传本进程自己的参数和工作目录）。`leave_linux_setup_window` 改为返回 `bool`，`true` 表示已交给新进程；前端 `onComplete` 收到 `true` 就不再进入设置，免得在旧目录上加载偏好。启动时已配置好的那次调用和词库更新页的「稍后」目录都没变，返回 `false`，行为与原来相同。

## Alternatives considered

- **在同一进程里把所有状态对象换到新目录**：用户看不到窗口重开，最贴合「不用重开」的字面要求。但目录在启动时分给了十几个状态对象，其中 `PreferencesStore` 是 `msime-client-core` 的共享类型，被 40 多处命令和后台监视器以 `Arc` 持有，剪贴板监视器、偏好监视器、账号会话、资源包补齐也都在启动时拿走了目录；Tauri 的 `manage` 不能替换已登记的状态。要做就得给每个状态对象加一层可换的目录句柄并处理换目录瞬间的并发读写，前端还要重新发现宿主能力、重建 `settingsClient`。漏换任何一个对象就是同类 bug 换个地方再出现，改动面和风险都远大于交接。
- **首次运行时直接用 runtime-options 所在目录作状态目录**：配置后什么都不用换。但 `msime-linux-prepare` 只接受不存在的目录（安装器留下的匿名账号文件除外），设置进程在首次配置前任何一次写入（启动时的后台更新检查会写 `update-check.json`，账号、诊断日志、剪贴板也可能写）都会让这个目录被占用，首次配置报 `setup_directory_exists`，比现在的 bug 更糟。
- **用 Tauri 的 `AppHandle::request_restart`**：一行调用，经 `RunEvent::Exit` 退出，单实例插件在那里放开 D-Bus 名后再启动新进程（tauri 2.12 的 `process::restart`）。但它启动失败只记一行日志就照样退出，用户手里什么窗口都没有；也不会先隐藏窗口留出写完待保存编辑的时间。仓库已有的升级交接两件事都做了，而且与升级交接共用一个交接标记，两种原因同时出现时不会交两次。

## Consequences

- **收益**：首次配置完成后，偏好、后台更新记录、账号会话、剪贴板历史、统计、皮肤和插件都落在输入法读写的 `preferences_directory`，宿主能力按准备好的 HostOptions 重新计算，用户不用自己重开设置。在终端里完成首次配置后再打开设置，留着的旧进程也会交出去，不再沿用 `app_data_dir`。
- **代价与已知上限**：点「进入设置」后窗口消失约一秒（`FLUSH_GRACE`）再由新进程打开，新窗口回到设置窗口的默认大小和位置。首次配置前已写进 `app_data_dir` 的文件（后台检查的 `update-check.json` 等）留在原处，不迁移；首次配置前用户在设置里改过的偏好（首次配置页挡在前面，通常没有）也不迁移。若以后首次配置允许另选状态目录，`moved()` 仍然成立，因为它比较的是 runtime-options 里写的目录。

## Verification

- `apps/desktop/src-tauri/src/tests.rs` 的 `linux_first_run_state_directory_moves_once_setup_publishes_the_options` 模拟首次运行：没有 runtime-options 时目录是退回的数据目录、`moved()` 为假；写出带 `preferences_directory` 的文件后 `moved()` 为真；按新文件重新求出的目录是状态目录，不再判为换过。`linux_state_directory_stays_put_unless_prepared_options_name_another_directory` 钉住不该交接的情况。
- Linux 容器里 `cargo check -p msime-desktop --all-targets`、`cargo clippy -p msime-desktop --all-targets -- -D warnings` 和上面两个测试（见 `.agents/skills/launch-platform-host/SKILL.md` 的 Linux 一节）。交接本身（窗口隐藏、新进程拿到单实例名、新窗口打开设置）需要 Linux 图形会话实机验收，属于证据分级第 4 级。
