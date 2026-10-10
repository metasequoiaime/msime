# Agent Note: Windows 设置文件导出导入、卸载默认保留数据、关于页版本名与第三方许可、本地语音模型目录选择

Status: implemented

## Problem

macOS 与 Windows 在「本机数据」这一组功能上不一致，审计（desktop-local 一组）列出：

- macOS 原生设置窗口的「更多」菜单能「导出设置…」「导入设置…」（`AppearancePreferences.mm` 的 `exportSettings:` / `importSettings:`），Windows 的设置窗口（WinUI 3 的 `msime-client-settings.exe`，不是共享 Tauri 页面）和共享层都没有，换电脑或重装只能手工重配。
- macOS 设置里卸载默认保留词库、学习记录和偏好，勾选后才一并删除（`uninstall-section.tsx`）。Windows 卸载程序在 `usPostUninstall` 无条件删掉它拥有的数据目录，没有询问也没有开关，winget、Scoop、Chocolatey 的静默卸载同样全删。
- Windows 设置窗口「关于」页把产品名写死成「水杉输入法」，五笔版、拼音版等打开看到的是 full 的名字，而窗口标题和侧栏早已按版本取。
- 安装器把 `THIRD_PARTY_NOTICES.txt` 装到安装目录，设置窗口和共享关于页都没有打开它的入口（共享页的 `LicenseRows` 只在 macOS 显示，`open_third_party_licenses` 只在 macOS 编译）。
- 共享语音页的「选择…」在 Windows 上调用 `pick_voice_model_path`，这个命令只在 macOS 注册，点下去必然失败；只能手填路径。
- 数据目录迁移：macOS 和 Linux 的共享设置能在应用内搬迁，Windows 设置窗口只显示路径和「打开」。

## Decision

### 设置文件

- 格式和换算在 client-core：`crates/client-core/src/settings_document.rs`。文件是 `{"format": "app.msime.client.preferences", "version": 1, "preferences": {...}}`，`preferences` 是共享偏好文档的 `preferences`，缩进写出。
- 只属于本机的部分（`LOCAL_SECTIONS`）不写出也不导入：`voice_input`、`ai_assistant`、`custom_translation`、`tencent_tmt`、`niutrans` 整组，`diagnostic_log`，`usage_reporting`，`clipboard_history`。剪贴板历史开关默认关闭，大多数文件里都是关的；它若跟着文件走，导入会把本机开着的历史关掉，而关掉就清空已存的历史（`clear_disabled_clipboard_history`），记录找不回来，所以与使用统计一样按本机的隐私选择保留。也因此导入不会改变剪贴板历史开关，`msime_client_import_settings` 不再像保存偏好那样在写回后清空历史，写回成功就是整个导入成功。所有服务凭据（`Preferences::credential_slots`）都在这几组里，所以文件里永远没有密钥。
- `fuzzy_pinyin.seeded` 随文件写出（只在为真时出现），导入时只会从假变真：本机种过、文件里标着种过，或文件里模糊音开着，都标成已种过。否则 `PreferencesStore::save`、Windows 设置窗口和共享设置页的首次打开种子会把文件里的规则（包括有意清空的规则）盖成全部规则，所以 `save` 也改为写入方自己标了 `seeded` 时不再种。早先的做法是导出时去掉这个标记、只在文件里模糊音开着时标记，在另一台电脑上挑好规则再关掉模糊音导出的文件，导入后第一次打开时规则就被盖掉。
- 导入时文件的 `preferences` 覆盖本机，`LOCAL_SECTIONS` 换回本机的值，再整份反序列化成 `Preferences`（`deny_unknown_fields`）并 `validate()`。版本比 1 新、出现不认识的字段、取值不合规都整份拒绝（`settings_document_unsupported`），不做部分应用。文件里的方案本版本不提供，或本版本只有一个方案时，保留本机的方案与 `last_chinese_scheme`；非 full 版本保留本机的 `touch_keyboard_schemes`。规则与 `edition::filter_downloaded_account_settings` 对账号下载的处理一致。
- macOS 原生窗口导出的 `app.msime.client.settings` 被认出来并单独说明（`settings_document_macos`），不当成「不是设置文件」。
- C ABI：`msime_client_export_settings(directory)` 读出已保存的偏好、返回文件全文；`msime_client_import_settings(directory, expected_revision, document)` 换算后按修订号比较并交换写回，冲突报 `settings_conflict`，失败时什么也不写；导入不碰剪贴板历史，所以写回之后没有还可能失败的步骤。
- Windows 设置窗口「维护与诊断」页新增「设置文件」组：「导出…」「导入…」用系统的 `IFileSaveDialog` / `IFileOpenDialog`（只列 `.json`，默认文件名 `<版本名>设置.json`）。读文件上限 1 MiB、写文件先写临时文件再改名、错误码换成中文说明都在 `platforms/windows/settings/SettingsDocumentFile.h`。导入以窗口读到的修订号写回，成功或冲突都重新读取设置。

### 卸载默认保留数据

- `msime_setup.iss` 的 `DecideUserDataRemoval` 在 `usUninstall` 一开始决定 `RemoveUserDataOnUninstall`：数据目录不归本安装器管，或带 `/KEEPDATA`，保留；带 `/REMOVEDATA` 删除；静默卸载不带开关时保留；交互卸载问一次，默认按钮是「否」。`usPostUninstall` 只在 `RemoveUserDataOnUninstall` 为真且 `OwnsDataDir` 时调用 `DeleteDataDir`，所有权与嵌套目录的规则不变。选了删除时还调 `DeleteUserProfileData`，删掉不在数据目录里的本用户数据：`%LOCALAPPDATA%\<用户目录>\account`（设置应用的登录会话含刷新令牌、本机匿名账号的密钥）、`%LOCALAPPDATA%\<Tauri 标识>`（Tauri 的 `app_local_data_dir`：旧版本的登录会话、词库快照暂存、WebView2 数据）和 `%APPDATA%\<Tauri 标识>`（Tauri 的 `app_data_dir`：共享语音页下载的本机语音识别模型，动辄几百 MB，漏删时一直留在漫游配置里同步）；用户目录里的使用统计不动，目录空了才删。两个目录名由 `edition_windows.py` 生成进 `editions.iss`（`MyEditionUserDataDir`、`MyEditionTauriIdentifier`）。对话框的「是」因此写明会退出本机登录的账号。对话框的「否」只对默认位置说「重新安装后自动接着用」；数据目录在自定义位置时改说重新安装要在「选择数据位置」重新选它或传 `/DATADIR=`，因为 `HKLM` 的 `DataDir` 照旧随卸载删掉（见下面的已知上限）。
- 包管理器全都静默卸载，所以都保留数据；它们的说明（Scoop notes、nuspec、winget InstallationNotes、`packaging/README.md`）改成保留并写明 `/REMOVEDATA`，也写明只有默认位置会被重新安装自动接上，自定义位置（Chocolatey 的 `/DataDir:`）要重新传或重新选。`install-smoke.ps1` 和 `coexistence-smoke.ps1` 卸载时带 `/REMOVEDATA`，继续核对删除路径和「只删自己的目录」。

### 关于页、第三方许可、语音模型目录

- 设置窗口「关于」页的产品名用 `MSIME_EDITION_DISPLAY_NAME`。
- 「许可与隐私」组新增「第三方组件许可」，打开 `server` 目录上一级的 `THIRD_PARTY_NOTICES.txt`（`third_party_notices_path`，与安装器的 `DestDir` 一致）。共享关于页的 `LicenseRows` 在 Windows 也显示，Tauri 的 `open_third_party_licenses` 有 Windows 分支，经 `msime_host_windows::open_file` 用关联程序打开同一个文件。
- Tauri 的 `pick_voice_model_path` 有 Windows 分支：`tauri-plugin-dialog` 的 `blocking_pick_folder`，返回绝对路径，页面照旧写进 `voice_input.asr_model_path`。

### 数据目录迁移

不在设置窗口里做。「数据目录」行说明要移到其他磁盘就用完整安装包重新安装，在「选择数据位置」一步选空文件夹。

## Alternatives considered

- **复用 Android 本地备份用的账号设置文档（`msime_client_account_settings_export` / `_apply`）** — 已经有版本过滤、取值校验和凭据剔除，Android 备份也在用；但那张映射是按 Android 的同步字段表写的（`settings_sync::export_android_settings`），候选窗口透明度、圆角、游戏兼容、Windows 的预编辑样式这些桌面设置都不在里面，导出再导入会悄悄丢掉一大半 Windows 设置。共享偏好文档本身就是各桌面宿主读写的那一份，去掉本机部分整份带走更完整。
- **与 macOS 原生窗口共用 `app.msime.client.settings` 格式** — 两个平台的文件就能互导；但 macOS 那份的内容是 Apple 云同步的 `platform.macos.*` 键值，只覆盖皮肤、候选排列、方案等一小部分，而且这个窗口在 macOS 上只是共享设置打不开时的后备。共用格式名而内容不同，两边都只能在解析时才发现读不懂，所以格式名分开，导入时认出 macOS 的文件单独说明。
- **导入时不碰服务配置，只保留密钥字段** — 用户换机时连同服务商、地址和模型一起带过去更省事；但文件里没有密钥，本机密钥配上文件里的另一个服务商或地址，页面上看不出两者已经不配套，比整组保留本机更糟，与「恢复默认设置」保留整组服务配置的理由相同。
- **在设置窗口里实现数据目录迁移（仿 macOS 的 `move_data_directory`）** — 体验最好；但 Windows 的状态根登记在 HKLM 的 `DataDir`，32 位与 64 位 TSF DLL、Server 都按它解析，改它需要管理员权限的辅助进程，搬迁时要停 Server 和看门狗、处理已加载 TSF 的进程，这些安装器的「选择数据位置」一步（`MigrateUserDataDir` 只复制、安装成功后 `FinishDataDirMove` 才删旧目录）都已经做对了。另写一套等于复制一份没有真机验证的搬迁逻辑，所以只在设置里指向安装器。
- **卸载默认仍删除，只加 `/KEEPDATA`** — 不改变包管理器的既有语义；但这正是要对齐的 macOS 行为的反面，误卸载或重装时丢掉词库和学习记录无法挽回，而保留多占的只是磁盘空间。
- **把「Windows 输入法未加入列表」的提示也做进共享设置** — 审计把它列为缺口；但 Windows 设置窗口早已有同样的提示条（`main.cpp` 的 `input_method_state` / `add_input_method` / `make_input_method_default`，窗口重新获得焦点时重新检查，见 `platforms/windows/settings/README.md`「默认输入法提示」），共享 Tauri 应用在 Windows 上只承载面板和几张跨平台服务页，不需要第二份。

## Consequences

- **收益**：Windows 用户不登录也能把设置带到另一台电脑或重装后的系统，文件里没有任何密钥；卸载、winget 和包管理器卸载不再丢词库和学习记录；非 full 版本的关于页显示自己的名字；第三方许可在两个设置入口都能打开；共享语音页的「选择…」在 Windows 上可用。
- **代价与已知上限**：设置文件是共享偏好文档的形状，与 macOS 原生窗口的文件互不相通；共享 Tauri 设置在任何平台都还没有导出导入按钮，Linux 要用时只需接上同一对 host-api 函数或 client-core 模块。卸载后数据目录留在磁盘上，用户要删得在交互卸载时选「是」或用 `/REMOVEDATA`；`HKLM` 的 `DataDir` 照旧在卸载时删掉，所以自定义位置的数据目录重新安装时要在「选择数据位置」里重新选它（带所有权标记的非空目录会被接受），默认位置自动接上。卸载开关依赖 Inno 卸载程序第二阶段收到原始命令行；`/KEEPDATA`、`/REMOVEDATA` 和卸载对话框只经过脚本的静态检查，WinUI 设置窗口的新按钮和文件对话框只能在 Windows 的 MSBuild 下编译，这次没有编译，也都没有在真机上点过。应用内搬迁数据目录仍未实现：要做时需要一个提权的辅助进程改 HKLM，并让安装器之外的进程也能安全停掉 TSF 与 Server。

## Verification

- `cargo test -p msime-client-core settings_document`：导出不含任何凭据和本机部分、往返保留共享设置与本机部分（含剪贴板历史开关）、手写进文件的本机部分被忽略、非设置文件与 macOS 文件分开报错、新版本或坏文档整份拒绝、本版本不提供的方案保留本机方案、导入的模糊音规则不被首次打开的种子覆盖（文件里模糊音开着，或关着但种过、挑过规则或全部清空）。
- `cargo test -p msime-host-api exported_settings_import`：冲突时不写、非设置文件报错、导入后本机密钥保留、本机开着的剪贴板历史和已存记录不受文件里默认关闭的开关影响。
- `platforms/windows/tests/ui/settings_document_file.cpp`（CTest `windows-settings-document-file`）：错误码说明、许可文件位置、写入替换与读取上限。
- `platforms/windows/installer/tests/lifecycle.ps1`：`usUninstall` 先决定是否删数据，`DecideUserDataRemoval` 默认保留、静默保留、`/REMOVEDATA` 删除、对话框默认「否」，`usPostUninstall` 只在选了删除时删，且连同本用户的 account 目录和本地、漫游两个 Tauri 目录一起删、不整删用户目录；自定义位置的数据目录在对话框里提示重新安装时要重新选。
- `apps/desktop/tests/settings/about-third-party-licenses.test.tsx`：Windows 关于页有「第三方组件许可」，Linux 没有；`voice-model-picker.test.tsx` 覆盖选择目录写回路径。
- `python3 scripts/test-windows-package-managers.py`。

相关：Android 的本地备份走账号设置文档，见 [Android 本地备份与恢复](2026-10-08-android-local-backup.md)。
