# Agent Note: Windows 原生设置应用的脚本插件授权页，Windows 的 release 构建运行脚本

Status: implemented

## Problem

[授权页](2026-10-12-script-grants-settings-page-without-keychain.md) 让 macOS 和 Linux 的 release 构建运行脚本插件：授权文件不签名，同一用户下的程序能伪造授权，这一残余风险写在共享设置页的授权页上。Windows 的设置是原生应用（platforms/windows/settings，Win32 + WinUI 3，不承载共享页），它的插件页经 host-api 的 `msime_client_plugins` 列目录、导入，而那个接口给 Android 和 HarmonyOS 用，一律去掉脚本插件、在 release 构建里拒绝导入脚本包，也没有读写授权的操作。Windows 用户没有地方授权、撤销、排序，也看不到残余风险，所以 Windows 的 release 构建一直不运行脚本。

要补的是三件事：host-api 给 Windows 设置应用一条能看到、导入、授权脚本插件的路，同时 Android 和 HarmonyOS 的调用照旧看不到脚本插件；原生设置应用画出和共享页一样的「脚本插件（实验）」分组、详情和授权对话框，写明 Windows 上的残余风险；然后放开 Windows release 构建的进程开关。

## Decision

**`msime_client_plugins` 的请求多两个可选字段，多三项操作。**

- `script_page: bool`，缺省为假：宿主有脚本插件的授权页。为真时 `catalog` 不经 `without_scripts`、列出脚本包和脚本文件夹的问题行，`import` 走 `plugins::import_from_settings`（接受脚本包）；为假时和以前逐字节一样：`catalog` 去掉脚本插件，`import` 走 `import_without_script_page`（release 构建拒绝脚本包，原因与移动端遇到 `script` 时相同）。Android、HarmonyOS 和各处的辅助码表导入不带它。字段由调用方声明，不按平台推断：Windows 设置应用里社区发布对话框和辅助码方案的目录请求也经这个接口，它们不带 `script_page`，看不到脚本包，社区发布本来也不收脚本包。
- `user_data: 绝对路径 | null`：输入法宿主选项里的同名目录，授权操作的回答带上它下面 `script-health.json` 里的停用记录；没有时停用记录为空，相对路径整个请求是 `invalid`。
- `script_grants`、`set_script_grant {id, hash, enabled, granted}`、`reorder_scripts {ids}`，只在桌面目标上编译（与 client-core 的 `script_grants` 同一个 cfg），移动端的请求反序列化失败、回 `invalid`。三项都回整份 `ScriptGrantsView`，失败码就是桌面外壳同名 Tauri 命令的那些（`plugin_script_changed`、`plugin_script_limit`、`plugin_script_memory`、`plugin_script_grant_invalid`、`plugin_script_missing`、`plugin_script_grants_format`）。三项不要求 `script_page`：只有桌面能调到它们，而写授权本身不让任何东西出现在没有授权页的宿主上。

**授权页读到的形状归 client-core。** `ScriptGrantsView`、`ScriptGrantView`、`ScriptHealthView`、健康文件的读取（`script_grants::health`，只认 `invalid`、`init`、`failed`、`slow` 四个类别）和 `script_grants::view` 从 Tauri 的 `desktop_plugins.rs` 移进 `plugins::script_grants`，健康文件名 `HEALTH_FILE` 也在那里，host-api 的输入法进程（`scripts::process`）写文件时用同一个常量。桌面外壳的三条命令和 `msime_client_plugins` 调同一个函数，回答的 JSON 一样。授权规则（依赖、互斥、哈希绑定、准入、`revision`）仍只在 `script_grants::set` 和 `reorder` 里。

**原生设置应用照共享页画。** 规则和文案逐条移进 `PluginPageModel.h`（不依赖 Windows 头文件）：能力的白话说明（`script_capabilities` 按 `Capability::ALL` 的顺序，现在 8 项，第 8 项「提供 / 指令」的文案和共享页一字不差，见 [插件指令行](2026-10-13-script-plugin-command-rows.md)）、钩子的白话名字、停用原因、`toggleCapability` 的依赖联动和互斥、`runningScriptIds`（启用的前 8 个里哈希对得上的）、列表标记、状态句、执行顺序的换位、授权对话框每一项的勾选、禁用和说明（新版本不再申请的一项不算勾着、不交出去），以及 `scriptResidualRisk` 的 Windows 那一句。`main.cpp` 只管调宿主和画控件：

- 「插件」页自己的目录、导入和授权请求带 `script_page`，`user_data` 取 `runtime-options.json` 的同名项（与 Tauri 的 `user_data` 同一项）。进入页面时和导入、删除之后，目录和授权一起重读。
- 列表在八类之后画「脚本插件（实验）」一组：开头是脚本插件的说明和残余风险，行尾是授权状态和第几位。「导入插件」的说明换成区分数据插件和脚本插件的那一句。
- 详情：信息（含内容哈希的前 12 位）、在这些时候运行、申请的权限和是否已授予、提供的 / 指令（每条的状态，`/` 指令关着时的说明和「打开 / 指令」按钮，见 [插件指令行](2026-10-13-script-plugin-command-rows.md) 的「投递」一节）、状态（启用时才写停用原因）、启用开关和「调整授权」、执行顺序（「提前」「推后」只在输入法会运行的插件之间换位，交给 `reorder_scripts`）、安全（沙箱说明和残余风险）、删除。关掉开关是 `set(enabled = false, granted = [])`。
- 授权对话框是 `ContentDialog`，默认按钮是「取消」；勾选时复选框就地更新，不重建对话框，键盘焦点留在刚点的那一项。宿主拒绝时对话框留着并写明原因；`plugin_script_changed` 时先重读目录和授权，对话框换成新版本申请的那几项再说明。内容变过时写「已更新，需要重新授权」，对话框里也写残余风险。关上后焦点回到打开它的开关或按钮。

**放开 Windows release 构建。** host-api `scripts` 的进程开关在三个桌面平台的开发构建和 release 构建里一样生效（`all(target_os = "windows", debug_assertions)` 换成 `target_os = "windows"`），`client-core` 的 `import_from_settings` 在所有桌面构建里接受脚本包，`host_surface.script_plugins` 在三个桌面平台为真（Windows 上的共享设置页如果显示，也和原生应用一致）。不变的：只有 Server 调 `msime_client_scripts_enable(true)`，Windows TIP 从不调用（`scripts/test-script-plugin-hosts.py`）；`ELEVATED`、`SECURE`、`PRIVATE` 字段里不运行脚本。

## Alternatives considered

- **另开一个 `msime_client_script_grants` 导出函数。** 最强的理由是边界一眼可见，移动端的调用方碰不到授权操作。不用：授权和包仓库是同一个插件根目录、同一把锁、同一套失败码和 `plugin_response` 形状，宿主那边的请求拼装和失败说明也是同一份；分成两个导出，Windows 设置应用要维护两条几乎一样的调用路径。三项操作按 cfg 只在桌面编译，移动端同样碰不到。
- **按平台决定目录列不列脚本包（Windows 上 `catalog` 恒列出），不加请求字段。** 最强的理由是不改请求形状，Windows 宿主什么都不用传。不用：同一个 Windows 进程里，社区发布对话框和辅助码方案也经 `catalog` 列目录，它们不该看到脚本包；"有没有授权页"是调用方的事实，不是平台的。缺省为假的字段让 Android 和 HarmonyOS 的行为不随这次改动变化。
- **Windows 设置应用的脚本插件交给共享 Tauri 页显示。** 最强的理由是一份实现、零移植。不用：Windows 原生设置应用的约定是每一页都在原生窗口里画，不再打开共享应用的任何设置页（platforms/windows/settings/README.md「与共享应用的分工」），而且授权和残余风险说明要出现在用户实际管理插件的那一页。
- **健康文件的读取在 host-api 里再写一份。** 最强的理由是不动 Tauri 层。不用：两份读取会在类别白名单、大小上限和格式版本上走样，页面对同一份文件给出不同的停用记录。
- **授权对话框每次勾选都整个重建，像发布对话框那样。** 最强的理由是代码更少。不用：重建后键盘焦点丢在对话框外，用键盘逐项勾选的用户每勾一项都要重新找回位置。

## Consequences

- 收益：Windows 用户能在原生设置应用里导入、授权、撤销、排序脚本插件，看到输入法停用了哪个和原因，残余风险写在分组、详情和授权对话框三处；Windows 的 release 构建和 macOS、Linux 一样运行脚本插件。Android 和 HarmonyOS 的 `msime_client_plugins` 请求不带新字段，回答不变。
- 代价：Windows 上同一用户下的程序能伪造授权，让脚本插件读取或改写输入的文字；这与 macOS、Linux 相同，由授权页写明，提权窗口里不运行脚本是 Windows 额外的一道。`PluginPageModel.h` 又多了一份共享页规则的移植，`plugin-script-model.ts` 的规则与文案改了要跟着改，头文件开头列了它。
- 「我的插件」列表后来换成 macOS 的插件库，见 [Windows 设置的「我的插件」改成 macOS 的插件库](../feature/2026-10-20-windows-settings-plugin-library.md)：「脚本插件（实验）」一组成了最后一个卡片分节，授权状态和第几位从行尾移到卡片里的强调色标记；「导入插件」一组和「从文件导入」一行去掉，区分数据插件和脚本插件的那句说明放进折叠的「插件导入说明」。详情、授权对话框和宿主调用仍按本篇。
- `msime_client_plugins` 的契约多了 `script_page` 和 `user_data`；以后再有宿主要授权页，带上 `script_page` 即可，不必再加接口。
- 设置应用的界面没有在本机编译和运行：它要 Windows App SDK 和 MSVC，PR 的 CI 也不编它（只有 Release Windows 工作流用 `Build-Client.ps1` 编），所以 `main.cpp` 的改动要到发版构建或真机才验证。

## Verification

- `cargo test -p msime-client-core -p msime-host-api`，debug 和 `--release` 都通过。client-core 的 `the_health_file_lists_only_the_four_disabled_categories`、`the_grants_view_keeps_the_order_and_the_capability_names`（从 Tauri 移过来），`the_settings_import_refuses_script_packs_in_release_builds_before_installing_anything` 改成有授权页的导入在所有桌面构建里接受脚本包。host-api 的 `only_a_host_with_the_grant_page_sees_or_installs_script_packs`：不带 `script_page` 的请求在 release 构建里拒绝导入脚本包，目录不列出脚本包和脚本文件夹的问题行；带上之后导入成功、回答里有钩子、申请的能力和内容哈希，目录列出两者。`the_grant_page_actions_grant_reorder_and_revoke_and_the_input_method_honours_them`：经 FFI 授权两个插件、`revision` 递增，不认识的能力、清单没申请的能力、旧哈希、没装的包各回自己的失败码，重排按给出的顺序、重复和没有条目的 id 整个拒绝，`user_data` 下的停用记录出现在回答里（没给时为空、相对路径是 `invalid`），会话按 FFI 写下的授权运行两个插件，撤销后条目留在原位、授予项清空。`the_real_process_switch_gates_delivery_in_debug_and_release_builds_without_any_key` 改为三个桌面平台打开开关都投递。
- `cargo clippy -p msime-client-core -p msime-host-api -p msime-desktop --all-targets -- -D warnings` 在 macOS 上干净，`cargo test -p msime-desktop --lib desktop_plugins` 通过；`cargo clippy -p msime-client-core -p msime-host-api --all-targets --target x86_64-pc-windows-gnu` 编译通过，警告都在这次没碰的文件里；`cargo check -p msime-client-core -p msime-host-api --all-targets --target aarch64-apple-ios` 通过（移动端的 cfg）。
- `platforms/windows/tests/ui/plugin_page_model.cpp`：依赖联动、互斥、只在申请过的能力里变动、对话框每一项的勾选、禁用和说明、新版本不再申请的一项不算勾着、前 8 个里哈希对得上的才运行、各种标记、状态句、换位、重新授权不预先勾选、删除脚本包不碰偏好；本机用 `clang++ -std=c++17 -Wall -Wextra -Werror -Wconversion` 和 `x86_64-w64-mingw32-g++ -std=c++17 -Wall -Wextra -Werror` 编过，clang 的那份运行通过。
- 没有验证：设置应用（`main.cpp`）没有在本机编译，也没有在真机上打开过：分组和详情的样子、授权对话框的初始焦点是否落在「取消」、勾选时焦点是否留在原处、关上后焦点回到开关、重排和撤销后的重画；Windows 上 Server 真的按授权运行脚本、提权窗口里不运行，也只有 host-api 的单测覆盖，没有在 Windows 真机上跑。

共享脚本模型与授权组件的模块边界见[脚本插件模型与授权对话框](../simplification/2026-10-23-script-plugin-modules.md)，Windows 继续以宿主授权规则为准，并同步共享模型的展示规则与文案。
