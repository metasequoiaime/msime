# Agent Note: 官方插件包随三个桌面安装包分发：只读的内置目录、保留 id、锁定的内容哈希

Status: implemented

## Problem

脚本插件 v3 让插件能在自己的 `/` 指令里出候选（[脚本插件在自己的 `/` 指令里出候选](2026-10-13-script-plugin-command-rows.md)），但插件到不了用户手里：[脚本插件合集](2026-10-12-script-plugin-collection.md) 定下的是「不随输入法分发」，用户要装 Rust 1.97.1 加 wasm32 目标、从源码构建再导入；脚本包又不能发布到社区库；没有任何 CI 作业上传构建好的包（审查意见 M5）。LaTeX 符号表是纯数据的短语表，本来能走社区库，但社区库里的包要用户自己去找、自己装。

要回答的是：哪些包随安装包走、放在每个平台的什么位置、输入法和设置页怎样认出它们是内置的、它们的授权怎么算、安装包里的那一份怎样证明就是评审过的那一份，以及移动端怎么保证不受影响。

## Decision

**两个官方包。** 脚本插件 uuid（`plugins/script/uuid/`，`/uuid`、`/guid` 给出同一个随机 v4 UUID 的四种写法）和短语表 latex-symbols（`plugins/phrase_table/latex-symbols/`，369 条 LaTeX 命令名到符号）。uuid 是 `provide_commands` 和 `random_fill` 的参考样板（审查意见 M6）：README、清单的 `description` 和插件指南都这样写，不当作卖点；真要给所有平台用会做成内置的 `/uuid`。两个包都在 `plugins/official-packs.json` 里登记，`format_version` 1，每条 `kind`、`id`、`source`，脚本包另有 `content_hash`。

**保留 id，所有平台都生效。** `client-services::plugins` 新增 `BUILTIN_SCRIPT_PACKS = ["uuid"]`、`BUILTIN_PHRASE_TABLES = ["latex-symbols"]`，`is_builtin` 对 `Script`、`PhraseTable` 查这两张表；`is_official` 是「内置、但不是音效或音乐包」。导入、社区发布照旧按 `is_builtin` 拒绝（`plugin_reserved`），测试用 `include_str!` 读 `official-packs.json`，断言两边的 id 完全相同。动手前查过社区后端：公开目录里没有 `latex` 或 `uuid` 的包（2026-10-13 的匿名查询，全目录 18 个包）。

**内置目录 `BuiltinDirs { sounds, packs }`。** 替换原来的 `builtin_sounds: Option<&Path>` 参数：`scan` 多扫一遍 `packs` 下按 `<kind>/<id>/` 排的包（只认官方 id，别的文件夹报为问题，因为 `load_package` 不会从那里读它）；`load_package`、`load_package_unread`、`script_pack::load` 经同一个 `pack_parent` 解析：官方 id 永远从 `packs` 读，宿主没有这个目录时报「内置插件包不可用」，绝不退回插件目录。`pack_directory` 按同一规则给出包文件夹，宿主拿它盖清单的戳。脚本插件和短语表的函数只要官方目录，参数是 `builtin_packs: Option<&Path>`：`script_grants::{set, view}`、`script_commands::{granted_commands, granted_commands_of}`、`phrase_table::enabled_phrases`。

**影子副本。** 官方包随安装包分发之前导入过的同 id 包仍在用户插件目录里时，`scan` 把它报为问题（「这个 id 属于内置插件」），载入、授权、指令和 K 模式都只认内置的那一份。`remove` 对官方 id 只删用户目录里的副本，内置目录不碰，授权条目也不动（授权归内置的那一份，副本的哈希本来就对不上）；音效、音乐的内置 id 照旧拒绝删除。

**授权照旧。** 官方脚本插件默认停用，要在授权页逐项授予；授权写在用户插件目录的 `script-grants.json`，绑定内置那一份的内容哈希。`script_grants::set` 清理「包已不在」的条目时按 `pack_directory` 找官方包：宿主给了内置目录而包真的不在（新版本去掉了它）才清掉，宿主没给内置目录时官方条目一律保留，因为这只是这一次调用看不见它。插件目录只在导入插件或保存 @ 名单时才建出来，从没导入过插件的用户第一次授权官方脚本插件时还没有它，所以 `set` 和导入一样先建好插件目录（链接或不是目录照样拒绝），不再要求它已经存在。新版本带来新哈希时授权失效，页面写「已随输入法更新，需要重新授权」；共享页和 Windows 原生页在官方脚本插件的详情里写「随输入法安装和更新，不能删除；需要你授权后才会运行。」，不显示删除按钮。短语表照旧要在偏好 `plugins.phrase_tables` 里启用。

**宿主怎么知道目录：`plugin_packs` 选项和默认推导。** 宿主选项和 `msime_client_plugins` 请求各加一个可选的 `plugin_packs`（绝对路径）。没给时，`PluginRoots::new` 取内置音效包目录旁边的 `plugin-packs`（存在才用）：macOS 输入法 bundle 是 `Contents/Resources/{sound-packs,plugin-packs}`，Linux 是 `share/<client>/{sound-packs,plugin-packs}`，这两个宿主本来就传 `sound_packs`，宿主代码不用改。Windows 的内置音效包在普通用户可写的 DataDir 里，不能从那里推，所以 Server 经 `src/system/PluginPackRoot.h` 总是写上自己所在目录（Program Files 下的 server）的 `plugin-packs`，目录不在（开发运行）时也写，免得共享库退回 DataDir 旁边；原生设置应用和 Tauri 外壳都取自己所在目录的 `plugin-packs`（两者都装在 server 目录里）。会话把它记在 `PluginRoots::builtin_packs`，交给脚本集合（编译线程的 `Job::Compile` 带上它）、插件表（官方短语表的戳和行）和设置页的授权命令。`msime_client_key_sound_pack`、`msime_client_music_pack` 只处理音效和音乐，不加这个字段。

**三个平台的位置，都是只读的。**

- macOS：`platforms/macos/CMakeLists.txt` 把 `MSIME_PLUGIN_PACKS_DIR`（默认 `target/plugin-packs`，存在才用）下的文件放进输入法 bundle 的 `Resources/plugin-packs/<kind>/<id>/`，bundle 的签名封住它们，随 app 更新整体替换。`MSIME_REQUIRE_OFFICIAL_PLUGINS=ON`（发版）时没有官方包就配置失败。`bundle-contents` 拿到 `--plugin-packs` 时逐个核对每个登记的包都到了、文件和构建出的逐字节相同。
- Windows：`Prepare-PackageFiles.ps1` 把 `target/plugin-packs` 暂存为 `server_exe/plugin-packs`，Inno 随 Server 装进 Program Files（`PrivilegesRequired=admin`，普通用户写不了），`[InstallDelete]` 先删整个 `server\plugin-packs`，新版去掉的包不残留。不放 DataDir：那里是 `users-modify`，放在那里的「内置」包谁都能换。
- Linux：CMake 选项 `MSIME_PLUGIN_PACKS` 给了才 `install(DIRECTORY …)` 到 `share/<client>/plugin-packs`。`package-container.sh` 在 `target/plugin-packs` 存在时传它，`MSIME_REQUIRE_OFFICIAL_PLUGINS=1` 时缺了就失败；`build-container.sh` 存在时装一遍并逐个 `diff` 两个包。从源码构建的发行版定义（Arch `msime`、Gentoo、OBS、Debian 源码包、nix）不跑 wasm 构建，不带官方包；`msime-bin` 重新打包发布页的 deb，所以带。
- 移动端：Android、iOS、鸿蒙的打包脚本一个都没碰，那里没有 `plugin-packs` 目录，推导什么都找不到，保留 id 照样生效（移动端本来就没有脚本类型，也没有短语表的导入入口）。

**锁文件和确定构建。** `scripts/build-official-plugins.sh [--check]` 读锁：脚本包用 `build-script-plugins.sh` 构建（`strip`、`panic = "abort"`、`codegen-units = 1`，见 `plugins/script/.cargo/config.toml`），短语表先跑生成器的 `--check`；每个包过 `msime-pack validate --builtin`，复制进暂存目录，脚本包再用新的 `msime-pack hash --builtin` 和锁里的 `content_hash` 比对，不一致就失败并打印两个值；不带 `--check` 时最后把暂存目录整个换成 `target/plugin-packs`。`test-script-plugins.sh` 对锁里的脚本 id 加 `--builtin`，并在插件目录有 `check_output.py` 时多核对一步（uuid 按 RFC 9562 从夹具的随机数独立算一遍）。CI 的 Script plugins 作业在 `test-script-plugins.sh` 之后跑 `build-official-plugins.sh --check`，路径过滤加上这个脚本。

**发版里一次构建、三处共用。** `release-macos.yml`、`release-windows.yml`、`release-linux.yml` 各有一个在 `ubuntu-24.04` 上跑的 `official-plugins` 作业：装钉住的工具链和 wasm32 目标，跑 `build-official-plugins.sh`，上传 `target/plugin-packs`。打包作业依赖它、下载到 `target/plugin-packs`，并设 `MSIME_REQUIRE_OFFICIAL_PLUGINS=1`。macOS 和 Windows 的 runner 不用装 wasm 目标；锁核对保证这份构建就是评审过的那份。

**latex-symbols 的编码规则（审查意见 m8）。** 命令名去掉反斜杠和花括号、全部小写（`\mathbb{R}` 是 `mathbbr`），短语表的编码只能是小写字母。只差大小写的命令合到同一个编码下，全小写的那一行在前（`gamma` 出 γ、Γ；`rightarrow` 出 →、⇒）。生成器 `plugins/phrase_table/tools/gen_latex_symbols.py` 只用标准库，在编码不合规、命令名重复、码位是 ASCII 或控制、格式、私用、未分配、同一编码超过 2 行、合并了两行却没有全小写的那一个、两行同一个字符、总条数不在 250 到 400 之间、`pack/LICENSE.txt` 不是 `plugins/phrase_table/LICENSE` 的副本时失败。字形按 LaTeX 排出来的样子取码位（`\phi` 是 ϕ、`\varphi` 是 φ）。数据是本项目自己写的，MIT，不照录 unicode-math（LPPL）、fcitx5 的 `latex.mb`（LGPL）或许可未核实的 Julia 表。README 写明和内置符号面板（`piliapp_symbols.txt` 的希腊字母、数学符号分类）的重叠只在「按名字查」。

**uuid 的工作量是常数（审查意见 M4）。** 每次投递在快照上运行：读触发词和参数各一次、读 16 字节随机数、设版本位和变体位、格式化四行、交出四行、写一行日志；参数不为空时不给行。夹具覆盖 `/uuid`、`/guid`、两份随机数、全 0、全 FF 和 64 字节的最长参数，峰值 9 407 fuel，`check_run.py` 断言不超过 50 000。

## Alternatives considered

- **发版时附上构建好的 zip 和 sha256，让用户自己导入**（审查意见 M5 的第一种）。最强的理由是安装包不变，输入法和设置页一行代码都不用改，用户想要才装。不用：用户要先找到发布页、下载、再在插件页导入，这正是「插件到不了用户手里」的原样；zip 和安装包分开发，版本对不上的组合也会出现。
- **latex-symbols 走社区库**（审查意见 M5 的「另外」）。最强的理由是短语表本来就能发布到社区，社区就是数据包的分发渠道，安装包不用多带东西。不用：社区库里的包要用户自己去找；一个插件页里同时有「内置」和「社区」两条来源，官方表就得另做一套标记。它只有 30 KiB 左右，随包带的代价可以忽略；以后要收回到社区库，只要从锁里删掉这一条。
- **把构建好的 wasm 提交进仓库**。最强的理由是打包不再需要 wasm 工具链，CI 和三个发版 runner 都省一步，锁住的就是那份字节。不用：仓库的约定是不提交 `.wasm`（`plugins/script/.gitignore`），提交进来的二进制没人能在评审里看懂；源码和二进制会漂移，要另加一道「重新构建后字节相同」的检查，那其实就是现在这条锁核对。
- **Windows 放进 DataDir**（和内置音效包一样）。最强的理由是和音效包同一个位置，宿主不用新的推导，host-api 的默认规则在 Windows 上也能用。不用：DataDir 是 `users-modify`，同一用户下的任何程序都能把「内置」包换掉；授权虽然绑定哈希，但设置页把它标成「随输入法安装和更新」，这句话在可写目录里不成立。
- **只靠目录推导，不加显式选项**。最强的理由是宿主选项和请求的形状都不变，旧宿主天然兼容。不用：Windows 的推导会落到 DataDir 旁边，正是上一条要避开的地方；没有显式选项，Server 也就没有办法说「这里没有官方包」。
- **每个发版 runner 各自构建**。最强的理由是少一个作业、少一次 artifact 传递，每个平台的打包自给自足。不用：macOS 和 Windows 的 runner 都要装 wasm 目标；三次构建得出三份字节，要靠锁核对三次才能知道它们一样，而 x86_64 和 arm64、Linux 和 macOS 的构建是否逐字节相同只能靠运气。一次构建、三处共用，装进三个安装包的就是同一份。
- **`msime-pack validate` 对官方 id 直接放行，不加 `--builtin`**。最强的理由是命令行少一个开关，仓库脚本不用区分插件。不用：社区后端和作者用同一个工具，放行之后作者会写出用保留 id 的包，导入时才被拒；开关让这件事只在仓库的构建脚本里发生。

## Consequences

- **收益**：装好桌面版的用户在插件页就能看到 UUID 和 LaTeX 符号两个官方包，授权或启用之后就能用，不用装 Rust。安装包里的脚本插件是锁定哈希的那一份，PR 的 diff 里能看见它什么时候变。移动端的安装包和代码路径都没变。
- **代价：工具链升级会改哈希，迫使用户重新授权。** 升级 `rust-toolchain.toml` 或改 uuid 的源码、清单，`build-official-plugins.sh --check` 在 CI 变红，要在同一个 PR 里更新锁；发出去之后，所有授权过 uuid 的用户都要在授权页重新授权一次，发版说明要写。
- **代价：源码构建的 Linux 发行版不带官方包**，需要它们的用户只能从源码构建后导入（导入同 id 会被拒，只能等发行版包定义接上 wasm 构建）。这一点写在 `platforms/linux/README.md` 和插件指南的「内置插件」一节。
- **代价：影子副本只能经 API 删**。插件页把它列在「无法载入的插件」里，那一行没有删除按钮；它不会被使用，只占一点磁盘。
- **代价：`msime_client_plugins` 的请求和宿主选项多了一个字段**，旧版输入法读到带 `plugin_packs` 的宿主选项会拒绝整份文档（`HostOptions` 拒绝未知键）。宿主选项里的 `plugin_packs` 只由 Windows Server 在进程内写进会话选项，不落盘，所以不会被旧版读到。
- **限制：** 锁以 CI 的 ubuntu x86_64 构建为准；本地证明了 macOS arm64、Linux arm64 容器和在 Studio 上经仿真运行的 Linux x86_64 容器（`docker run --platform linux/amd64`，同一个 1.97.1 工具链）得到相同的 wasm 字节，但没有在 CI 那台真实的 x86_64 runner 上跑过。那里若不同，CI 的 Script plugins 作业会在合入前失败。
- **重访信号**：再加官方包时，在锁和 `BUILTIN_*` 常量里一起加（一致性测试守着）；要收回某个包时从锁里删掉，`[InstallDelete]` 和整目录替换会让它从已装的机器上消失。

## Verification

- `cargo test -p msime-client-core --locked`（Mac Studio）：1399 过，1 ignored。新增 `official_packs_match_the_repository_lock`、`official_phrase_tables_come_from_the_packs_directory_and_stay_off_until_enabled`（列出、只认官方 id、没有内置目录时载不入也列不出、K 模式只读启用的、影子副本报为问题且能删、删两次都不碰内置的、导入和 `validate` 拒绝、`validate_with(Allow)` 放行）、`a_linked_official_packs_directory_is_not_scanned`、`official_script_packs_are_granted_from_the_packs_directory`（从内置目录读和算哈希、默认停用、授权后算进指令、宿主不给内置目录时官方条目不被清掉、影子副本删了授权还在、内置包真的不在了才清掉）、`an_official_script_pack_is_granted_before_the_plugin_root_exists`（插件目录还不存在时授权官方脚本插件，目录和授权文件随之建出）。
- `cargo test -p msime-host-api --locked`：603 过，2 ignored。新增官方脚本插件经 `msime_client_plugins` 列为内置、默认没有插件行、经授权页授权后由后台编译线程从内置目录编出手写的二进制模块、`/uuid` 出它的行并能上屏；没有内置目录时官方 id 什么都不认、影子副本不顶替；官方短语表从推导出的目录读、只在启用后进 K 模式、显式给出的不存在的目录不退回影子副本；`plugin_packs` 的推导和显式优先；状态目录下还没有 `plugins` 时，经 `msime_client_plugins` 列出并授权官方脚本插件照样成功。
- 真实构建的官方包：`cargo test -p msime-host-api --locked official_packs_from_the_build -- --ignored` 在 `scripts/build-official-plugins.sh` 之后通过：`Kalpha` 出 α，`Kgamma` 出 γ、Γ；授权 uuid、打开 `/`、打 `/uuid` 出四行（注释「UUID 小写」等），选第一行上屏的是 36 个字符、版本 4、变体 `10` 的小写 UUID。
- `cargo test -p msime-pack-tool --locked`：17 + 2 + 7 过；`hash` 等于 `script_pack::content_hash`，官方 id 在 `validate`、`run`、`hash` 里都要 `--builtin`，`--builtin` 不能重复，仓库里的 latex-symbols 包只有加了它才通过。
- `MSIME_FETCH=1 MSIME_REQUIRE_SCRIPT_PLUGINS=1 bash scripts/test-script-plugins.sh`：三个插件通过，uuid 9 个事件、峰值 9 407 fuel，`check_output.py` 按 RFC 9562 核对通过。`bash scripts/build-official-plugins.sh`：uuid 的内容哈希 `ccadf8c4…c667` 和锁一致，latex-symbols 生成器一致、369 条。同一份源码在 `rust:1.97.1-bookworm` 容器（Linux arm64）里编出的 wasm、zip 和内容哈希与 Studio（macOS arm64）逐字节相同；同一镜像以 `--platform linux/amd64` 运行（`uname -m` 为 `x86_64`，rustc 的 host 是 `x86_64-unknown-linux-gnu`）编出的 `uuid.wasm` 和组装后的包里那份也逐字节相同（sha256 `57287707…54b8`）。
- 界面：`apps/desktop/tests/settings/script-plugins.test.tsx` 加两条（官方插件标「（内置）」、写「随输入法安装和更新…」、没有删除按钮、授权过期时说「已随输入法更新，需要重新授权」；导入的插件照旧有删除按钮和原来的说法）；`vitest run tests/settings` 321 个文件、1710 条通过；`pnpm --filter @msime/desktop typecheck` 通过。Windows `tests/ui/plugin_page_model.cpp`（内置插件的标记、状态、对话框和「来源」一句）和 `tests/runtime/plugin_pack_root.cpp` 用 Studio 的 `clang++ -std=c++17 -Wall -Wextra -Werror` 编译运行通过；MinGW 交叉构建、`main.cpp`、`server_main.cpp`、安装器脚本和 `installer/tests/package-files.ps1` 只做了源码审阅，交给 CI。
- 三个平台的安装包（容器和 bundle 构建）：Studio 上按 `launch-platform-host` 的命令编出 macOS 输入法 bundle（`-DMSIME_REQUIRE_OFFICIAL_PLUGINS=ON`），`Resources/plugin-packs` 下是两个包的五个文件，`ctest -R bundle-contents` 通过；把 `target/plugin-packs` 里的清单改一个字节后它失败（「was not staged as built」），重新构建后通过。`platforms/linux/build-container.sh` 在有官方包时 83 个 ctest 全过，装出来的 `share/msime-client/plugin-packs` 和构建出的逐个 `diff` 相同。Linux 的 Tauri 外壳在 `Dockerfile.desktop-check` 容器里 `cargo check -p msime-desktop --all-targets` 通过。`scripts/run-checks.sh` 全部通过或跳过：`test-linux-distro-packaging.py` 把 `MSIME_PLUGIN_PACKS` 记为只有发布页的包才传的选项。
- 没有做的：三个桌面平台的装机打字验收（5 级）。在本机装 macOS 开发构建会替换用户日常用的输入法，这一轮没有做；Windows 只到 CI 的交叉构建，安装包只在发版工作流里产出。

当场分类：[脚本插件合集](2026-10-12-script-plugin-collection.md) 被部分取代——它「不随输入法分发」的决定对官方包翻转了，其余（目录约定、构建和测试脚本、fuel 计法）仍然成立，那篇只加了指向这里的链接；[脚本插件在自己的 `/` 指令里出候选](2026-10-13-script-plugin-command-rows.md) 部分重叠，uuid 是它的第一个真实消费者，互链；[脚本插件进入插件包体系](2026-10-12-script-plugin-kind-and-grants.md) 部分重叠（授权绑定哈希、导入拒绝保留 id 的规则对官方包照旧），那篇加了链接；[Windows 设置的插件页](../feature/2026-10-11-windows-settings-plugins.md) 部分重叠，就地补了请求里的 `plugin_packs`；[删除与内置重复的脚本插件](../simplification/2026-10-13-remove-duplicate-script-plugins.md) 无关（被删的三个插件仍不分发）。

Windows 的内置辅助码表也按同一个先例由宿主写明目录：码表在 DataDir 的 `helpcodes`，Server 和 TIP 经 `src/system/HelpcodeRoot.h` 把 DataDir 写成 `helpcode_fallback_resources`，见 [下载的词库代次回退到随包辅助码表](../bug-fix/2026-10-23-downloaded-generation-helpcode-fallback.md)，部分重叠，互链。
