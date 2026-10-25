# Agent Note: 下载的词库代次在 host-api 里统一回退到随包辅助码表

Status: implemented

## Problem

用户反馈 iOS 双拼辅助码不生效。辅助码表随安装包放在资源目录的 `helpcodes/` 下，所有平台都从 `resources/helpcodes` 打包进去，但它不在词库发布里。iOS、Android、Windows 和 HarmonyOS 都有在线词库更新，iOS 设置 App 每次回到前台就跑一次自动检查。装上新代次后，`prepare_host_with_updates` 把 `resources` 换成下载的代次目录，那里只有锁文件列出的词库文件，没有 `helpcodes/`。Engine 读不到内置码表就得到空表，会话照常建立，辅助码字母什么也筛不掉。`prepare_host_with_updates` 已经把音效包、插件包、整句模型和语言词库指回随包目录，唯独漏了码表。macOS 的 #8931 是同一个缺口，#8939 只在 macOS 宿主里注入了回退目录。

## Decision

在 host-api 里补两处，所有宿主都不用改：

- `prepare_host_with_updates` 换成下载代次时，和 `sound_packs`、`plugin_packs` 一样把当前安装包的资源目录写进 `helpcode_fallback_resources`，前提是那里有 `helpcodes/`。这里拿的是本次准备传进来的真实安装目录：iOS 每次 App 更新都换安装路径，更新记录里的随包目录会失效，只靠记录会在 App 更新后再坏一次。
- `HostOptions::into_engine_options` 在文档没有这个键、`resources` 自己又没有 `helpcodes/` 时，经 `helpcode_fallback_resources()` 从更新记录（`installed.json` 的 `bundled_resources`）里找随包目录，要求那里确实有码表。这一处覆盖只经 `refresh_host_options` 换过 `resources` 与 `dictionaries`、不会新增键的落盘文档（Android、macOS、Linux、Windows 的运行时配置文件）。

Windows 是例外，宿主自己写回退目录。安装器把 Server 装在 `{commonpf64}\<edition>\server`（installer/msime_setup.iss 复制 `server_exe\*`），资源目录是那里的 `server\resources`，没有 `helpcodes\`；码表由 installer/Prepare-PackageFiles.ps1 放进 `app_data\helpcodes`，装到 DataDir（`{code:GetDataDir}`），而 DataDir 是 Server 和 TIP 的状态根。随包资源和更新记录里的随包目录都是 `server\resources`，上面两处都找不到码表，所以 windows-v0.3.0 起 Windows 上每张内置码表都是空的，辅助码既不注释也不筛选，与是否装过在线词库更新无关。修法照内置音效包（`src/system/SoundPackRoot.h`）：`src/system/HelpcodeRoot.h` 的 `name_builtin_helpcodes` 在 DataDir 下有 `helpcodes` 目录、选项里还没有这个键时，把 DataDir 写成 `helpcode_fallback_resources`。Server 在 `server_main.cpp` 里紧跟 `name_builtin_sound_packs` 调用它；TIP 不读 Server 的内存选项，而是自己读 DataDir 的 `runtime-options.json` 建会话（`tsf/HostConfiguration.cpp` 的 `LoadConfiguration`，`hostOptions` 交给 `EngineSessionAdapter::create`），所以在那里同样调用一次。两处都只改内存里的选项，不写回 `runtime-options.json`。

文档里已有的回退目录（macOS 输入法在内存里注入的那份）照样优先。Engine 只在主资源确实缺少内置表时读回退目录，已有表、自定义表和插件表的优先级不变。

## Alternatives considered

照 macOS 的做法在每个宿主里各自注入回退目录。最强的理由是不改 host-api 准备出的文档，旧版输入法读文档不会多出不认识的键。不用它，是因为缺口在共享层，每个有在线更新的宿主都要再补一遍，漏一个就是一次同样的用户反馈；iOS 的第一版修复就是这样做的，后来撤掉了。

只在 `into_engine_options` 里从更新记录推导，不往准备出的文档写键，可以完全不碰落盘格式。不单独用它，是因为 iOS 的记录路径会随 App 更新失效。写键的风险与 `plugin_packs` 相同：只有新准备的文档带这个键，`refresh_host_options` 改写已有文件时仍只换两个键，所以[运行中的旧版输入法读到未知键](2026-10-20-macos-bundled-helpcode-fallback.md)这一约束对刷新路径不变。

Windows 不由 host-api 在 `helpcode_fallback_resources()` 里加第三个来源「状态根下的 `helpcodes/`」。它能一处覆盖 Server、TIP 和 `msime-mcp candidates` 这类直接用 `runtime-options.json` 建会话的进程，这是最强的理由。不用它，是因为 DataDir 放码表是 Windows 安装器自己选的布局，与音效包放在 DataDir 是同一个选择，那一个也由宿主经 `SoundPackRoot.h` 写明；让共享层在每个平台的可写状态根里找内置码表，等于把一个平台的布局变成所有平台的读路径。

把码表复制进每个下载代次也能修好，但代次目录按锁文件精确校验内容，要改词库更新的安装和校验流程。

## Consequences

`downloaded_generation_falls_back_to_the_bundled_helpcode_tables` 用合成词库装一个下载代次：新准备的文档带回退目录且指向有码表的随包目录；去掉这个键的旧文档从更新记录里找到同一目录；随包目录没有码表时不回退；还在用随包词库时不设回退。去掉两处修复后该测试失败。

Windows 这一处由 host-api 的 `windows_sessions_read_the_builtin_helpcode_tables_from_the_data_directory` 端到端验证：合成词库锁装在没有 `helpcodes/` 的 `server/resources`，码表只在 `DataDir/helpcodes/xiaohe_helpcode.txt`，词库里「阳」词频高于「杨」，码表给「杨」辅助码 mu；经 `prepare_host_with_updates` 准备出的文档没有回退目录，加上 `name_builtin_helpcodes` 写的 DataDir 后建会话，`yh` 下「杨」带 mu 注释，`yhm` 把「杨」提到首位。去掉那个键（即修复前 Windows 传的选项）时断言失败，`yh` 下「阳」「杨」的 `annotation` 都是空串。`platforms/windows/tests/runtime/helpcode_root.cpp` 检查 helper 本身（没有目录不写、已有键不覆盖、相对根和同名文件不算），由 `scripts/test-windows-native-run.py` 在非 Windows 主机上实际执行；`tsf/tests/runtime/host_configuration.cpp` 断言 TIP 的 `hostOptions` 带着状态根，只在 Windows 构建里跑。Windows 真机不可达，Server 和 TIP 的接线只经这些测试和 PR 的 Windows CI 验证，没有装机验收。`msime-mcp candidates` 在 Windows 上读同一份 `runtime-options.json` 但不经这两个宿主，候选预览里仍没有内置辅助码。

自定义码表（`custom/<stem>`）不受这次影响：Engine 和两个原生设置窗口（`settings/main.cpp` 的 `helpcode_schemas_call`、Win7 `settings/win7/SettingsWindow.cpp` 的 `helpcode_sources`）都从 `resources/helpcodes/custom` 读，设置窗口取 `runtime-options.json` 的 `resources`（首次运行写下的 `server\resources`，在 Program Files 下，普通用户写不进去），Server 取它启动时准备出的 `resources`（装了在线词库更新后是下载的代次目录，由词库更新管理）。Windows 上没有往这两处写自定义表的界面，用户导入的码表走辅助码表插件包（`plugins/helpcode`，在 DataDir 下），不经这个目录。不把自定义目录也指到 DataDir：回退只管内置表，而安装器把 DataDir 的 `helpcodes` 当作安装包自己的条目（msime_setup.iss 的 `IsPackageAppDataItem`），每次完整安装都清掉重写，放在那里的自定义表会随升级丢失。Win7 设置把七个内置方案写死在下拉框里，不检查码表文件，没有同样的假设。

`HelpcodeSettingsTests.testXiaoheZiranmaYangCandidateAndCommit` 补充 #9416 的 iOS 原生桥接回归：在独立状态目录开启小鹤双拼和自然码辅助码，分别输入 `yh` 后的裸 `m` 与 Shift `M`，检查「杨」进入前两项、「阳」被过滤、注释为 `(mY)`，并选择「杨」验证上屏和组字清空。不固定前两项的先后顺序，避免上一轮选词学习改变排序后误报。

iOS 27 模拟器上的全部六项 `HelpcodeSettingsTests` 与原生键盘的 `testShiftEntersHelpcodeWhileComposingShuangpin` 均通过，共七项、零失败。测试使用真实 Rust 静态库与随包词库；资源校验、原生测试构建和 quick 在远端完成，因远端模拟器服务无响应，编译后的测试包转到本机通过 `test-without-building` 执行。这验证随包词库下的候选、注释与提交，不是下载代次的 iOS 端到端验收，也不是系统键盘扩展或真机验收。尚未拿到报告者的设备状态，不能据此确认其根因或关闭 #9416；是否装过在线更新仍是从代码推出的可能原因。

## 历史笔记检索

[macOS 随包回退](2026-10-20-macos-bundled-helpcode-fallback.md)部分重叠：其回退语义和 macOS 的内存注入保留，「准备出的配置省略这个键」对下载代次这条路径不再成立，两篇互链。[Linux 随包码表](2026-10-18-linux-packaged-helpcodes.md)管安装时把码表放进资源目录，与本篇互补。[移动端码表导入](../feature/2026-10-12-mobile-helpcode-table-import.md)管插件包码表，不受影响。Windows 的写法沿用内置音效包和[官方插件包](../architecture/2026-10-13-official-plugin-packs-in-desktop-installers.md)的先例：宿主按安装布局在会话选项里写明目录（platforms/windows/README.md 记着音效包和插件包的这两条），部分重叠，互链。
