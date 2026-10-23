# Agent Note: 脚本插件的授权页：授权不签名、不用钥匙串，macOS 和 Linux 的 release 构建打开脚本并在授权页写明残余风险

Status: implemented

## Problem

[脚本插件进入插件包体系](2026-10-12-script-plugin-kind-and-grants.md) 把用户的授权记在 `<plugins root>/script-grants.json`，[脚本插件在输入法进程里](2026-10-12-script-plugins-in-the-input-method-process.md) 让 4 个桌面输入法进程按它运行脚本。两件事还没做：

- 授权文件在用户可写的目录下，同一用户下任何没有特权的进程都能放进一个脚本包、再手写一条带正确内容哈希的授权，在输入法进程里拿到读取、改写上屏文本的能力。在 macOS 上这绕过了 TCC 的输入监控授权。所以 release 构建里进程开关恒为关，脚本只在开发构建里运行。
- 设置页看不到脚本插件：目录命令经 `without_scripts` 去掉它们，`host_surface` 没有对应的开关，用户只能手写授权文件。

设计给 release 构建定的前提原本是三条对策：macOS 的授权条目带钥匙串密钥签的 MAC，Windows 的提权窗口里不运行脚本（第 3 阶段已经按 `ELEVATED` 做到），Windows 和 Linux 把同一用户能伪造授权这一残余风险写在授权页上。这一阶段先按设计实现了 macOS 的钥匙串 MAC，评审中发现它的保护靠不住、又会让输入法启动卡在钥匙串对话框上（见下面的备选方案），用户决定脚本插件的授权不使用钥匙串。于是 macOS 和 Linux 一样：授权不签名，残余风险写在授权页上。

Windows 的设置是原生 WinUI 应用（platforms/windows/settings），不是这里的共享设置页：它的插件页经 host-api 的 `msime_client_plugins` 列目录，没有授权页，也就没有地方写这一残余风险，所以 Windows 的 release 构建仍不运行脚本，等它有了同样的授权页再打开。原生设置应用的授权页和 Windows release 构建打开开关见 [Windows 授权页](2026-10-12-windows-script-grants-page.md)。

## Decision

**授权不签名，所有桌面平台一样（client-core `plugins::script_grants`）。** 条目只有 `id`、`hash`、`enabled`、`granted`，`ScriptGrants::enabled` 列出启用的前 8 个，哈希由输入法对着已安装的包核对。同一用户下的任何进程都能手写一条带正确哈希的授权，这是设计接受的残余风险，由授权页在 macOS 和 Linux 上写明。这一阶段的早期开发构建在 macOS 上给条目写过 `mac` 字段：读取方本来就容忍未知字段（不用 `deny_unknown_fields`），带着它的文件照常读出、照常运行，没有 `mac` 也不拒绝任何条目；下一次写入时不再写出它。

- **`revision` 每次写入都变大，并且不被冻结**：取上一份加 1 和当前时间的微秒数中较大的一个（`next_revision`），写出的值总小于 `MAX_REVISION`（2^53，也是 JavaScript 能精确表示的最大整数）；读到 `MAX_REVISION - 1` 或更大的值时不接着它加。输入法只在 `revision` 比以前看到过的都大时复位熔断：只加 1 的话，一份 `revision` 为 `u64::MAX` 的伪造文件会让以后每次写入停在同一个值上、熔断再也不随用户的操作复位；改小 `revision` 或放回旧文件之后，用户在设置页上的下一次写入也可能不比输入法以前看到过的大。
- **熔断只随变大的 `revision` 复位**：输入法的 `observe_revision` 记下每个插件根目录看到过的最大 `revision`，只在新读到的比它大时复位这个根目录下的熔断。放回一份旧文件、删掉文件（读出 `revision` 0）都不复位，来回换文件不能让熔断反复复位。同一用户下的程序直接写一份 `revision` 更大的文件仍然能让它复位，这和它能伪造授权是同一个残余风险。
- **准入按输入法占位置的方式计数**：其他启用的条目，不管包载不载得了、哈希对不对得上，都占着输入法运行的前 8 个位置（输入法取前 8 个之后才核对哈希）；内存总账只算真会占内存的那些。`set` 写入时顺带去掉包目录已经不在的条目（和删包时一样）：它们永远不运行，却占着位置，页面上又列不出来、撤销不了。读取只认前 `MAX_ENTRIES` 条，`set` 不再往已满的文件里加新条目。
- **哈希绑定不变**：条目的 `hash` 必须等于包现在的内容哈希才生效；导入时内容变了就停用并清空授予项、保留旧哈希，页面显示"已更新，需要重新授权"；`set` 在锁里重新算哈希，和页面给出的不一致时返回 `plugin_script_changed`。
- 新增 `reorder(root, ids)`：`ids` 按给出的顺序排在最前，没列出的按原来的相对顺序排在后面；重复或没有条目的 id 整个拒绝。`capabilities(names)` 把页面传来的能力名换成集合，不认识的名字整个拒绝，不像读文件时那样丢掉：那是用户刚勾选的授权。

**进程开关在 macOS 和 Linux 的 release 构建里也生效。** `msime_client_scripts_enable` 在 macOS 和 Linux 上去掉了 `debug_assertions` 条件，Windows 仍只在开发构建里生效（原因见上；原生设置应用有了授权页之后也去掉了，见 [Windows 授权页](2026-10-12-windows-script-grants-page.md)），移动端仍恒为 false。4 个入口、Windows TIP 从不调用、五项运行条件都不变。输入法启动时不读任何密钥，host-api 没有交密钥的 FFI。

**设置页（`packages/ui`、`apps/desktop`）。**

- `host_surface.script_plugins` 在三个桌面平台为 true（Windows 随 [Windows 授权页](2026-10-12-windows-script-grants-page.md) 打开）。桌面外壳的 `plugin_catalog` 不再经过 `without_scripts`，目录里带脚本包的钩子、申请的能力和内容哈希；`without_scripts` 只剩 host-api 的 `msime_client_plugins` 不带 `script_page` 的请求（Android、HarmonyOS）在用。页面只在 `script_plugins` 为真、并且客户端有授权方法时显示"脚本插件（实验）"分组，否则脚本包和脚本文件夹的问题行都不列出。
- 新增 Tauri 命令 `script_grants`、`set_script_grant`、`reorder_scripts`，都回整份 `ScriptGrantsView`（client-core 的 `script_grants::view`，`msime_client_plugins` 的同名操作回答同一个）：条目、`<user_data>/script-health.json` 里的停用记录，以及会运行的插件声明的 `/` 指令和各自的状态（`commands`，按偏好里启用的指令表算冲突，见 [插件指令行](2026-10-13-script-plugin-command-rows.md) 的「投递」一节）。`PluginClient` 上对应的三个方法是可选的，HarmonyOS 的客户端不用改。这几个命令和导入、删包都不碰钥匙串。
- 列表的标记按顺序判断：没有条目是"未授权"，条目的哈希和包现在的内容哈希不同是"已更新，需要重新授权"，启用的是"已启用 · 第 N 位"（被输入法停用时是"已被输入法停用"，排在第 8 个之后是"已启用 · 超过 8 个，不运行"），其余"未启用"。第几位按输入法实际运行的集合算：启用、哈希对得上，前 8 个。
- 详情页显示声明的钩子（白话的"在什么时候运行"）、申请的每项能力和它的白话说明与是否已授予、内容哈希前 12 位、授权状态和停用原因、启用开关、执行顺序（提前、推后只在运行的插件之间换位）和删除。打开开关先弹授权对话框，对话框的初始焦点在"取消"，关掉后焦点回到打开它的开关或按钮；停用原因只在启用时显示。关掉开关就是 `set(enabled = false, granted = [])`，撤销全部授权，按设计在下一次聚焦时生效。
- 提供 `/` 指令的插件：详情页多一组「提供的 / 指令」，每条一行，写明生效、和用户的指令或排在前面的插件冲突而没生效，或者要怎样才生效；授权对话框列出这些指令，调整授权时带上状态。`/` 指令关着时这一组写明要先打开它、用中文标点，并给「打开 / 指令」按钮，点了才改偏好，授权本身从不改。
- 授权对话框：清单申请的每项能力一个复选框，新授权时一项都不预先勾选；勾选一项把它依赖的那一项一起勾上，取消一项把依赖它的一起取消（`toggleCapability`）；`rewrite_commit` 和 `read_composition` 一项勾着时另一项禁用并写明原因。被宿主拒绝时对话框留着并说明原因，超出内存总账（`plugin_script_memory`）、同时启用超过 8 个、包在打开对话框之后被换过各有一句话。包被换过（`plugin_script_changed`）时页面先重读目录和授权，换成现在的内容哈希，再说明；再确认时只交出新版本申请、对话框列着的那几项，旧版本申请过、新版本不再申请的一项即使原来勾着也不交出去。
- **残余风险写在用户做决定的地方**：macOS 和 Linux 的列表分组、详情页和授权对话框写明"以你的身份运行的其他程序可以不经过这个页面，直接写入授权，让脚本插件读取或改写你输入的文字。只安装你信任的脚本插件。"（`scriptResidualRisk`）。Windows 的同一句另写明以管理员身份运行的窗口里不运行脚本插件，原生设置应用照搬了它（`PluginPageModel.h` 的 `script_residual_risk`，见 [Windows 授权页](2026-10-12-windows-script-grants-page.md)）。
- 社区的发布对话框只列 `communityPluginKinds` 里的类型：桌面的目录里有了脚本包，但它们不能发布（`PUBLISHABLE_KINDS` 里没有 script）。

## 残余风险

授权文件不签名之后，同一用户下的程序能做的事在三个桌面平台上相同：

| 同一用户下的程序做的事 | 结果 |
|---|---|
| 手写一条带正确哈希的条目，或改已有条目的能力、`enabled`、执行顺序 | 生效。授权页写明这一风险（macOS、Linux 是共享设置页，Windows 是原生设置应用，见 [Windows 授权页](2026-10-12-windows-script-grants-page.md)） |
| 写一份 `revision` 更大的文件，让熔断复位 | 生效，属于同一个残余风险 |
| 放回一份旧文件、删掉文件，让熔断复位 | 不复位：`revision` 没有比以前看到过的大 |
| 把 `revision` 写成 `u64::MAX` 或改小 | 写入方不接着越界的值加，`revision` 取当前时间下限，用户的下一次写入照样让熔断复位 |
| 结束输入法进程让它重启，熔断随之清空 | 挡不住；每用一次要让输入法重启一次 |
| 在提权窗口（Windows `ELEVATED`）、密码框和隐私输入框里运行脚本 | 不运行，和授权文件无关 |

## Alternatives considered

- **macOS 上用钥匙串里的密钥给授权条目签 MAC（这一阶段先实现过，又整个去掉）。** 做法是登录钥匙串里的一个通用密码项存 32 字节随机密钥，访问控制表只信任输入法和设置应用；每个条目带 HMAC-SHA256，覆盖 id、哈希、启用、能力、`revision`、条目位置和总数；输入法启动时取密钥，只运行校验通过的条目，熔断只认签过名的 `revision`。最强的理由是它让同一用户下的程序在 macOS 上伪造不了授权（回滚除外），堵住绕过 TCC 输入监控的那条路。去掉，原因有三：
  - 保护靠不住：只能用旧式（文件型）钥匙串，在临时钥匙串上实测，不在访问控制表里的程序能不弹框地改写这一项的数据（把访问控制表的 encrypt 授权也限定给创建者仍然挡不住），改写成它知道的密钥之后，输入法和设置应用下次启动就会接受它签的授权；登录钥匙串的分区挡不挡得住这一步没有验证。
  - 启动可能卡在钥匙串对话框上：输入法在 `main` 里、IMK 服务起来之前取密钥，`kSecUseAuthenticationUIFail` 管不到旧式钥匙串访问控制表的确认框和解锁框（实测 30 秒后仍停在确认框上），要靠 `SecKeychainSetUserInteractionAllowed(false)` 这类弃用接口绕开；同一用户下的程序抢先放一个只信任它自己的同名项，就可能让每次启动都停住或取不到密钥。设置应用读密钥时还会弹系统授权框。整套实现建立在 macOS 10.10 起标为弃用的 SecKeychain 接口上。
  - 用户决定：脚本插件的授权不使用钥匙串。
- **数据保护钥匙串，用 `keychain-access-groups` 或 App Group 在输入法和设置应用之间共享密钥。** 最强的理由是这是 Apple 现行的接口，访问组按签名强制，没有弃用的调用。不用：`keychain-access-groups` 是要描述文件背书的受限权利，Developer ID 签名的输入法带上它会被 AMFI 在启动时拒绝，输入法直接从输入法列表里消失（`platforms/macos/tests/settings/entitlements_guard.py` 记的就是这类事故）；仓库的签名流程里没有描述文件。而且它同样是钥匙串，用户的决定也排除了它。
- **Windows 的 release 构建也运行脚本，和 macOS、Linux 一致。** 最强的理由是三个桌面平台行为相同，Windows 用户也能用上。不用：Windows 的原生设置应用没有授权页，用户没有地方授权、撤销或看到残余风险说明，接受这一风险的前提不成立。原生设置应用有了授权页之后这一条被采用，见 [Windows 授权页](2026-10-12-windows-script-grants-page.md)。
- **`revision` 只加 1，用 `checked_add` 和读取上限防止停在 `u64::MAX`。** 最强的理由是值小、好读，测试能断言具体数字。不用：同一用户下的程序把 `revision` 改小或放回旧文件之后，用户的下一次写入可能不比输入法以前看到过的大，熔断不随用户的操作复位；取当前时间作下限，不用额外的持久状态就让每次写入的 `revision` 都比以前写出的大。
- **授权对话框里勾选一项时把互斥的那一项自动取消，而不是禁用它。** 最强的理由是少一步操作。不用：静默改掉用户已经勾选的另一项，用户不一定注意到；禁用并写明"不能和……同时授予"更清楚。清单本身不允许同时申请这两项，所以对话框里实际上不会同时出现它们，这条规则在页面上是和宿主一致的兜底。

## Consequences

- 收益：用户第一次能在设置页上看到、授权、撤销、排序脚本插件，看到输入法停用了哪个、为什么。输入法启动和设置页都不碰钥匙串，不会弹钥匙串对话框、不会卡在启动上，也不依赖弃用的 SecKeychain 接口。三个桌面平台读写同一种格式，开发时手写授权文件自测在 macOS 上也照常生效。
- 代价：macOS 上同一用户下的程序能伪造授权，一个没有输入监控授权的程序可以借一个脚本插件读到用户输入的文字，绕过 TCC；这和 Linux 一样，写在授权页上，用户只该安装信任的脚本插件。熔断能被写一份 `revision` 更大的文件复位。
- 各宿主接入生命周期核心（`msime_client_set_lifecycle`）之前，会话不满足运行条件：授权页在 macOS 和 Linux 上出现、授权能写进去，但输入法还不会运行任何脚本。
- Windows 的 release 构建不运行脚本，原生设置应用的插件页也不列出脚本包；在那里导入脚本包，release 构建拒绝。要在 Windows 上打开，先给 platforms/windows/settings 加上授权页（列出脚本包、授权、调整顺序、写明残余风险），再放开 `scripts` 的进程开关和 `script_plugins`。这两步已由 [Windows 授权页](2026-10-12-windows-script-grants-page.md) 完成。
- `revision` 现在是十几位的数（微秒），设置页和测试只按"变大"判断，不再断言具体值。
- 已知缺口：结束输入法进程让熔断清空；替换用户可写目录里的输入法 bundle。
- 设置页的导入分两条：有授权页的宿主（桌面外壳的 `desktop_plugins::import_pack_at`，以及带 `script_page` 的 `msime_client_plugins`，即 Windows 原生设置应用）调 `plugins::import_from_settings`，在所有桌面构建里接受脚本包，与 `msime_client_scripts_enable` 打开的构建一致；没有授权页的宿主（不带 `script_page` 的 `msime_client_plugins`：Android、HarmonyOS）调 `plugins::import_without_script_page`，只在开发构建里接受。否则 release 构建运行脚本，用户却装不进脚本包。

## Verification

- `cargo test -p msime-client-core` 的 `plugins::script_tests`：每次写入 `revision` 都变大且在上限之内；`revision` 为 `u64::MAX` 的伪造文件之后两次写入的 `revision` 不同且在上限之内，改小 `revision` 之后的写入比以前写出的大；带早期 `mac` 字段（有的条目没有）的文件照常读出、按顺序运行，下一次写入后文件里不再有 `mac`；哈希对不上的条目占着准入的位置，包目录被删掉的条目在写入时去掉、腾出位置；重排按给出的顺序、重复或没有条目的 id 整个拒绝；内容变了之后旧哈希的条目对不上、拿旧哈希写入返回 `plugin_script_changed`；删包清掉条目；错误码各自对应。
- `cargo test -p msime-host-api`（debug 和 `--release`）的 `tests/script_plugins.rs`：`the_real_process_switch_gates_delivery_in_debug_and_release_builds_without_any_key` 不交任何密钥，macOS 和 Linux 上 debug 和 release 构建打开开关都投递，Windows 的 release 构建打不开；`a_grant_with_a_leftover_mac_field_still_runs` 带早期 `mac` 字段的条目照常投递；`putting_back_an_older_grant_file_never_resets_the_breaker` 熔断之后换成 `revision` 相同但没有条目的文件、更旧的文件、删掉文件，再放回原来那份，插件都仍然停用，`revision` 变大后才恢复；`a_restarted_process_clears_the_stopped_plugins_its_predecessor_recorded` 上一个进程留下的停用记录在第一次失焦时被重写掉，没有健康文件时不写。
- `apps/desktop/src-tauri` 的 `desktop_plugins` 测试：健康文件只认四个停用类别，格式不认识或读不懂时当作没有记录；授权视图原样保留条目顺序和能力名。
- 移动端：`cargo check -p msime-client-core -p msime-host-api --all-targets` 在 `aarch64-apple-ios`、`aarch64-linux-android`、`aarch64-unknown-linux-ohos` 上通过，`cargo check -p msime-desktop --target aarch64-linux-android --lib` 通过，`cargo tree -p msime-host-api -e normal -i wasmi` 在这三个目标上为空（钥匙串 MAC 去掉之前的结果，这次改动没有碰移动端的编译条件）。
- `apps/desktop/tests/settings/script-plugins.test.tsx`：插件指令的列表、两种冲突的说法（冲突的插件用名字）、没授权和宿主没列出时的说法、`/` 关着时有说明和按钮且授权不改偏好、开着时没有按钮；依赖联动、互斥、对话框的"已更新，需要重新授权"和被总账拒绝时的原因、分组只在 `script_plugins` 时出现、macOS、Linux 和 Windows 都写明残余风险、macOS 页面不提钥匙串并且能打开授权对话框、撤销和重排调用的参数；包被换过时页面重读后按新的哈希写入，新版本不再申请的能力即使原来勾着也不交出去；对话框从"取消"开始、关掉后焦点回到开关；停用原因只在启用时显示。
- 没有验证：签了名的真实安装上的端到端流程（设置应用授权、输入法运行、授权页的风险说明）。

共享 React 的脚本规则由 `plugin-script-model.ts` 持有，授权选择与写入状态由 `use-script-grant-dialog.ts` 持有，对话框和详情分别在 `script-grant-dialog.tsx`、`plugin-script-view.tsx` 组合；拆分理由见[脚本插件模型与授权对话框](../simplification/2026-10-23-script-plugin-modules.md)，本篇的安全模型和授权语义继续有效。
