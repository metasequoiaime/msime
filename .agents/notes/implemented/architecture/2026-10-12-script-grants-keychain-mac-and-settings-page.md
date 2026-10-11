# Agent Note: 脚本插件的授权页与 macOS 钥匙串 MAC：设置应用签名、输入法只认签过名的条目，桌面 release 构建打开脚本

Status: implemented

## Problem

[脚本插件进入插件包体系](2026-10-12-script-plugin-kind-and-grants.md) 把用户的授权记在 `<plugins root>/script-grants.json`，[脚本插件在输入法进程里](2026-10-12-script-plugins-in-the-input-method-process.md) 让 4 个桌面输入法进程按它运行脚本。两件事还没做：

- 授权文件在用户可写的目录下，同一用户下任何没有特权的进程都能放进一个脚本包、再手写一条带正确内容哈希的授权，在输入法进程里拿到读取、改写上屏文本的能力。在 macOS 上这绕过了 TCC 的输入监控授权。所以 release 构建里进程开关恒为关，脚本只在开发构建里运行。
- 设置页看不到脚本插件：目录命令经 `without_scripts` 去掉它们，`host_surface` 没有对应的开关，用户只能手写授权文件。

设计给 release 构建定的前提是三条对策同时落地：macOS 的授权条目带钥匙串密钥签的 MAC，Windows 的提权窗口里不运行脚本（第 3 阶段已经按 `ELEVATED` 做到），Windows 和 Linux 把同一用户能伪造授权这一残余风险写在授权页上。

## Decision

**授权条目的 MAC（client-core `plugins::script_grants`）。** 每个条目的 `mac` 是 HMAC-SHA256，64 位小写十六进制，覆盖的字节依次是域前缀 `msime-script-grant\0`、整份文件的 `revision`、id、hash、`enabled`、授予的能力个数和按 `Capability::ALL` 顺序的能力名，每一段前面带 8 字节小端的长度。签的是能力名而不是位：位编号可以变，名字是授权文件里的契约。密钥是进程级的 32 字节，经 `set_mac_key` 交进来，只认第一次交进来的那把（`OnceLock`）。

- **校验只在 macOS 要求**（`MAC_REQUIRED = cfg!(target_os = "macos")`）。`read` 给每个条目算 `verified`：macOS 上是"本进程有密钥且 MAC 对得上"，其他平台恒为 true。`ScriptGrants::enabled` 只列 `enabled && verified` 的条目，所以输入法进程的 `scripts::set` 不用改就只运行签过名的条目。校验不过的条目照样读出来给设置页，页面显示"需要重新授权"。
- **写入方只给原来校验通过的条目和这次写的条目签名**，从不替一条没校验过的条目补签：`revision` 每次写入都加 1，所以每次写入都要重签全部条目，"重签全部"如果不看原来的校验结果，设置应用的任何一次写入都会替伪造者背书。用户在授权页上重新授权一条，这一条才重新生效。
- **macOS 上没有密钥时 `set` 和 `reorder` 一律拒绝**，错误码 `plugin_script_keychain`：写出去的条目全都校验不过，连别的条目原有的签名也因为 `revision` 变了而失效，等于撤销全部授权。导入和删包照常进行（它们在同一把锁里改条目），没有密钥的写入方写出的条目全都不带 MAC，在 macOS 上全部停止运行，属于失败即关闭。
- 新增 `reorder(root, ids)`：`ids` 按给出的顺序排在最前，没列出的按原来的相对顺序排在后面；重复或没有条目的 id 整个拒绝。`capabilities(names)` 把页面传来的能力名换成集合，不认识的名字整个拒绝，不像读文件时那样丢掉：那是用户刚勾选的授权。

**密钥在钥匙串里（`crates/host-macos/native/script_grant_key.mm`，输入法和设置应用编译同一个文件）。**

- 登录钥匙串里的通用密码项，服务名 `<本版本设置应用的 bundle identifier>.script-grants`（full 是 `app.msime.macos.script-grants`），账户 `script-grants-mac-key`。还没有这一项时由先到的一方用 `SecRandomCopyBytes` 生成并写入；两边同时写入时后到的一方拿到 `errSecDuplicateItem`，改读先写入的那一把。
- 访问控制表信任创建者自己和对方：输入法创建时信任 LaunchServices 找到的设置应用，设置应用创建时信任 `~/Library/Input Methods` 里已安装的输入法和自己资源里带的那份（签名相同）。别的程序读它时系统弹出授权框。输入法进程带 `kSecUseAuthenticationUIFail`，不弹框，读不到就没有密钥；设置应用允许弹框。
- 只在登录钥匙串里查找（`kSecMatchSearchList`），写入也指定登录钥匙串：同一用户下的程序能新建一个它知道密码的钥匙串，在里面放同名的项、把分区改成任意值，再排到搜索列表前面。
- 读到的项要核对分区：系统按创建者的签名写下 `teamid:<团队标识>`，改分区要钥匙串的密码（登录钥匙串的就是用户登录密码）。本进程签名里有团队标识时，只认分区里有本团队的项，所以同一用户下别的程序抢先放进去的项（分区是 `unsigned:`、`apple-tool:` 或别的团队）不被采用，它没法借此知道密钥。本进程没有团队标识（ad-hoc 签名或没签名的开发构建）时跳过这一项检查。
- 这些是 macOS 10.10 起标为弃用的 SecKeychain 接口（`SecAccessCreate`、`SecTrustedApplicationCreateFromPath`、`SecKeychainItemCopyAccess`、`SecACLCopyContents`、`SecKeychainOpen`），整个文件关掉弃用告警。没有新增任何依赖。

**密钥怎样到达校验方。** 输入法：`input_method_main.mm` 在独立设置窗口的分支返回之后、`msime_client_scripts_enable(true)` 之前调用 `MSIMEInstallScriptGrantKey`，取到就经新的 FFI `msime_client_script_grants_key(key, 32)` 交给 host-api，再由 host-api 交给 client-core；取不到记一行日志，这个进程里没有任何条目生效。设置应用：`desktop_plugins::install_grant_key` 调用 `msime_host_macos::script_grant_key`，时机是写授权和调整顺序之前、读授权时（只在装了脚本包或已有授权文件时）、导入之前（只在已有授权文件时）和删脚本包之前，所以没用到脚本插件的用户打开插件页不会碰钥匙串；取到之后不再重复（`has_mac_key`），取不到时下一次命令再试。Windows 和 Linux 不调用它。

**进程开关在桌面的 release 构建里也生效。** `msime_client_scripts_enable` 去掉了 `debug_assertions` 条件，移动端仍恒为 false。4 个入口、Windows TIP 从不调用、五项运行条件都不变。

**设置页（`packages/ui`、`apps/desktop`）。**

- `host_surface.script_plugins` 在 macOS、Windows、Linux 为 true。桌面外壳的 `plugin_catalog` 不再经过 `without_scripts`，目录里带脚本包的钩子、申请的能力和内容哈希；`without_scripts` 只剩 host-api 的 `msime_client_plugins`（Android、HarmonyOS 用）在用。页面只在 `script_plugins` 为真、并且客户端有授权方法时显示"脚本插件（实验）"分组，否则脚本包和脚本文件夹的问题行都不列出。
- 新增 Tauri 命令 `script_grants`、`set_script_grant`、`reorder_scripts`，都回整份 `ScriptGrantsView`：条目（含 `verified`）、`<user_data>/script-health.json` 里的停用记录、`mac_required` 和 `signed`。`PluginClient` 上对应的三个方法是可选的，HarmonyOS 的客户端不用改。
- 列表的标记按顺序判断：没有条目是"未授权"，条目的哈希和包现在的内容哈希不同是"已更新，需要重新授权"，校验不过是"需要重新授权"，启用的是"已启用 · 第 N 位"（被输入法停用时是"已被输入法停用"），其余"未启用"。第几位按输入法实际运行的集合算：启用、校验通过、哈希对得上，前 8 个。
- 详情页显示声明的钩子（白话的"在什么时候运行"）、申请的每项能力和它的白话说明与是否已授予、内容哈希前 12 位、授权状态和停用原因、启用开关、执行顺序（提前、推后只在运行的插件之间换位）和删除。打开开关先弹授权对话框；关掉开关就是 `set(enabled = false, granted = [])`，撤销全部授权，按设计在下一次聚焦时生效。macOS 上 `mac_required && !signed` 时开关和调整按钮禁用，并说明读不到钥匙串。
- 授权对话框：清单申请的每项能力一个复选框，新授权时一项都不预先勾选；勾选一项把它依赖的那一项一起勾上，取消一项把依赖它的一起取消（`toggleCapability`）；`rewrite_commit` 和 `read_composition` 一项勾着时另一项禁用并写明原因。被宿主拒绝时对话框留着并说明原因，超出内存总账（`plugin_script_memory`）、同时启用超过 8 个、包在打开对话框之后被换过、钥匙串读不到各有一句话。
- 社区的发布对话框只列 `communityPluginKinds` 里的类型：桌面的目录里有了脚本包，但它们不能发布（`PUBLISHABLE_KINDS` 里没有 script）。
- Windows 和 Linux 的列表分组、详情页和对话框写明：以你的身份运行的其他程序可以不经过这个页面直接写入授权；Windows 另写明以管理员身份运行的窗口里不运行脚本插件。macOS 写明伪造的授权不会生效。

## MAC 挡住和挡不住的

| 同一用户下的程序做的事 | macOS |
|---|---|
| 手写一条带正确哈希的条目，或改已有条目的能力、`enabled` | 校验不过，不运行 |
| 改 `revision` 让熔断复位 | 全部条目校验不过，不运行 |
| 在登录钥匙串里抢先放一把它知道的密钥 | 分区不是本团队，不采用（只限团队签名的构建） |
| 自建钥匙串放同名项并排进搜索列表 | 只查登录钥匙串，看不到 |
| 把以前某个时刻签过名的整份授权文件原样放回去（回滚） | **挡不住**：那份文件的签名本来就对 |
| 替换 `~/Library/Input Methods` 里的输入法 bundle（用户可写） | 不在 MAC 的范围内；替换后的 bundle 签名不同，读密钥会被访问控制表挡住，但它本身就是一个输入法，看得到全部击键 |

## Alternatives considered

- **数据保护钥匙串，用 `keychain-access-groups` 或 App Group 在输入法和设置应用之间共享。** 最强的理由是这是 Apple 现行的接口，访问组按签名强制，没有弃用的 SecKeychain 调用，也不用自己核对分区。不用：`keychain-access-groups` 是要描述文件背书的受限权利，Developer ID 签名的输入法带上它会被 AMFI 在启动时拒绝，输入法直接从输入法列表里消失（`platforms/macos/tests/settings/entitlements_guard.py` 记的就是这类事故）；仓库的签名流程里没有描述文件。
- **设置应用经 `security-framework` 这类 crate 访问钥匙串。** 最强的理由是 Rust 一侧直接调用，少一层 C 接口。不用：不新增依赖，而 host-macos 已经有用 `cc` 编译的 Objective-C 原生代码和 Security 框架的链接，输入法也已经编译 host-macos 的原生文件（`uninstaller.mm`），同一个 `.mm` 两边共用就够了。
- **写入时给全部条目重新签名。** 最强的理由是没有密钥的写入方（或者校验过程里的任何差错）不会让已有的授权失效，用户少重新授权。不用：设置应用的任何一次写入都会把伪造者手写的条目签成真的，MAC 等于没有。
- **MAC 不覆盖 `revision`。** 最强的理由是导入、删包这类不碰别的条目的写入不必重签全部条目，没有密钥的写入方也不会让别人的签名失效。不用：熔断按 `revision` 变化复位，伪造者改一次 `revision` 就能让一个被熔断的插件重新失败一次、重新读一次健康文件，设计接受的一次性信道就变成可以反复用的。
- **把一个只增不减的 `revision` 下限也存进钥匙串，挡住回滚。** 最强的理由是它补上表里唯一挡不住的一行。没有做：设置应用每次写授权都要多写一次钥匙串，输入法每次重读授权（聚焦时文件戳变了）都要在会话线程上读一次钥匙串，`SecItemCopyMatching` 可能等 securityd；文件被删掉重建时 `revision` 从 1 重新开始，下限还要有恢复办法。回滚只能恢复用户曾经亲自授予过的条目，列为已知缺口。
- **输入法只在启动时读密钥，不自己生成。** 最强的理由是只有设置应用写钥匙串，访问控制表只由一方决定。不用：输入法是长期运行的进程，用户第一次在设置应用里授权时如果密钥才刚生成，输入法要到重启才拿得到，授权要很久以后才生效。让先到的一方生成，输入法启动后就一定有密钥（钥匙串可用时）。
- **授权对话框里勾选一项时把互斥的那一项自动取消，而不是禁用它。** 最强的理由是少一步操作。不用：静默改掉用户已经勾选的另一项，用户不一定注意到；禁用并写明"不能和……同时授予"更清楚。清单本身不允许同时申请这两项，所以对话框里实际上不会同时出现它们，这条规则在页面上是和宿主一致的兜底。

## Consequences

- 收益：macOS 上授权文件不再能被同一用户下的程序伪造（回滚除外），release 构建因此可以运行脚本；Windows 和 Linux 的残余风险写在用户做决定的地方。用户第一次能在设置页上看到、授权、撤销、排序脚本插件，看到输入法停用了哪个、为什么。
- 代价：用了弃用的 SecKeychain 接口，哪天 Apple 移除它们，macOS 上就取不到密钥，所有脚本停止运行（失败即关闭），要改走数据保护钥匙串和描述文件。macOS 上手写授权文件自测不再生效，开发时要经设置页授权（或用测试里的 `entry_mac` 按本机密钥签名）。输入法启动时登录钥匙串还锁着、或者访问控制表里没有它，这个进程里就不运行任何脚本，直到下次启动。输入法创建密钥时 LaunchServices 找不到设置应用的话，设置应用第一次读密钥会弹系统授权框。
- 各宿主接入生命周期核心（`msime_client_set_lifecycle`）之前，会话不满足运行条件：授权页在三个桌面平台上都出现、授权能写进去，但输入法还不会运行任何脚本。
- 已知缺口：整份授权文件的回滚；替换用户可写目录里的输入法 bundle；没有团队标识的构建不防预先放置的密钥。钥匙串的整条路径（创建、对方读取、分区核对、拒绝弹框）只有参数检查和分区解析有自动化测试，没有在签了名的真实安装上走过。

## Verification

- `cargo test -p msime-client-core` 的 `plugins::script_tests`：MAC 覆盖每个字段和 `revision`；四种伪造（改 `revision`、换签名、去掉签名、换一把密钥签）在 macOS 上列出但不运行、在其他平台照常运行；写别的条目不会替校验不过的条目补签，用户重新授权后恢复；重排保持签名；内容变了之后旧哈希的条目对不上、拿旧哈希写入返回 `plugin_script_changed`；删包清掉条目。
- `cargo test -p msime-host-api`（debug 和 `--release`）的 `tests/script_plugins.rs`：测试夹具经 `msime_client_script_grants_key` 交进密钥，授权文件按它签名；`a_grant_whose_mac_does_not_verify_never_runs_on_macos` 用另一把密钥签的条目在 macOS 上一个事件都不投递，同一条用正确密钥重签后照常运行；`the_real_process_switch_gates_delivery_in_debug_and_release_builds` 在 release 构建里打开开关也投递。
- `platforms/macos` 的 ctest `shared-script-grant-key`：分区说明的解析（本团队、别的团队、`unsigned:`、`apple-tool:`、读不懂的说明）和参数检查，不碰真实钥匙串。
- `apps/desktop/src-tauri` 的 `desktop_plugins` 测试：健康文件只认四个停用类别，格式不认识或读不懂时当作没有记录；授权视图带 `mac_required` 和 `signed`。
- 移动端：`cargo check -p msime-client-core -p msime-host-api --all-targets` 在 `aarch64-apple-ios`、`aarch64-linux-android`、`aarch64-unknown-linux-ohos` 上通过，`cargo check -p msime-desktop --target aarch64-linux-android --lib` 通过，`cargo tree -p msime-host-api -e normal -i wasmi` 在这三个目标上为空。
- `apps/desktop/tests/settings/script-plugins.test.tsx`：依赖联动、互斥、对话框的"已更新，需要重新授权"和被总账拒绝时的原因、分组只在 `script_plugins` 时出现、Windows 和 Linux 的残余风险说明、macOS 没有密钥时不能启用、撤销和重排调用的参数。
