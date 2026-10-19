# Agent Note: 移动端把单个 .txt 辅助码表导入成辅助码表包

Status: implemented

## Problem

#7176：手机上用双拼的用户想像桌面一样用自己的辅助码表，但 Android 和 iOS 只能在七套内置方案里选。桌面有两条自定义路径：手工把 `.txt` 放进资源目录的 `helpcodes/custom/`（Engine 按 `custom/<stem>` 宽松读），或在「插件」页导入 `kind = "helpcode"` 的辅助码表包（`plugins.helpcode_pack_quanpin` / `helpcode_pack_shuangpin` 选中，host-api 的 `plugin_tables` 替换方案的表，包坏了退回 `schema`）。手机上两条都走不通：没有用户看得见的资源目录，也没有「插件」页（`packages/ui/src/index.tsx` 在 mobile 上隐藏它），用户手里只有一个 `.txt`。

## Decision

- #7765：辅助码表不设条数上限，统一以 1 MiB 原始文件字节为边界；严格格式、重复字和空表检查不变。解析器只把初次预分配限制在三万条，之后按实际条目增长，避免注释占满文件的小表一开始就分配二十万条容量。文件上限与 Engine 一致，插件导入、目录扫描和会话读取都执行同一规则。

- client-core 的 `plugins::helpcode_pack::import_table(root, file_name, bytes)` 把单个码表包成辅助码表包：先按辅助码表包的严格规则 `parse_table` 校验（失败是 `PluginError::Invalid`，原因带用户的文件名和行号；重复字报出两处行号），显示名取开头的 `# name:`（`helpcode::display_names`，与 `custom/` 列表共用），没有时用去掉扩展名的文件名，在字符边界上截到清单 `name` 的 80 字节。id 是 `custom-<小写文件名主干>`，满足 `safe_id`（含 64 字节上限）才用，否则是 `custom-<小写主干 SHA-256 前 12 位>`；同名再次导入整包替换。`plugin.toml` 经 toml 序列化写出（`license = "NOASSERTION"`、`version = "1"`、`description = "从 <文件名> 导入"`），和原样字节的 `table.txt` 一起放进 `root` 下 `.staging-` 前缀的临时目录，再交给现有的 `plugins::import`，复用它的校验、`packs.lock`、遗留清扫和原子换入。
- host-api 的 `msime_client_plugins` 增加 action `import_helpcode_table {file_name, text}`：码表正文是 JSON 里的 UTF-8 字符串，1 MiB 的合法码表在 2 MiB 请求上限之内。宿主负责先按原始字节限 1 MiB、再严格按 UTF-8 解码，解码失败不调用。失败码沿用 `plugin_invalid` + `detail`。
- Android（`TypingPage`，规则在 `HelpcodeChoicePolicy`）：「辅助码方案」列表在内置方案之后列出 `catalog` 里 `kind = "helpcode"` 的包（「名称（插件）」，选中的包不在了显示「id（插件，未找到）」），然后是「导入辅助码表…」（SAF `OpenDocument`，MIME 放行 `text/plain` 和 `application/octet-stream`）和「管理已导入的码表」（逐个删除）。选包只写当前方案那一份的 `plugins.helpcode_pack_*`；选内置方案写 `schema` 并把包清成 `""`，与共享页 `helpcode-page.tsx` 相同。导入成功后自动选中到发起导入的那一份并在提示里说明已切换；删除正在用的包时把全拼、双拼两份里指向它的选择都清掉，退回内置方案。`state_root` 是 `HostStore.directory`，即键盘读的 runtime option `preferences_directory`。
- iOS（`HelpcodeSettingsView`，规则在 `HelpcodeChoicePolicy.swift`，与 Android 的同名类逐条对应）：「输入 › 辅助码 › 更多辅助码设置」里双拼、全拼两组各有「导入辅助码表…」（SwiftUI `fileImporter`，放行 `.plainText`、`.text` 和 `.data`），导入成功后选中到发起导入的那一组（发起的方案存在 `importScheme`，与选择器的开关 `importing` 分开：SwiftUI 在调用 `fileImporter` 的完成回调之前先把 `isPresented` 置为 false，第一版用 `Binding(get: { importTarget != nil }, set: { … importTarget = nil })` 当开关，回调里读到的永远是 nil，双拼组的导入被选进了全拼）；两组的「辅助码方案」在七个内置方案之后列出导入的包，「已导入的码表」一节左滑删除。「输入」页的「辅助码方案」行用同一套规则列出和选择。读文件用 `BoundedFileReader` 先按字节限 1 MiB，再 `String(data:encoding: .utf8)` 严格解码，解码失败不调用。`state_root` 是 App Group 下的 `MSIME` 目录（`MetasequoiaInputSessionBridge.sharedStateDirectory`），键盘的 `preferences_directory` 就是它；桥接层的 `MetasequoiaInputSessionBridge.plugins` 保留失败回答里的 `detail`，现有的 `decodeResponse` 只留失败码。`appEditedKeys` 加上整个 `plugins` 对象：iOS 键盘自己不写 `plugins` 的任何键，文档里的就是准的；交给活着的会话后，会话空闲时用新选择重建 Engine，聚焦时 `refresh_plugin_tables` 再按包目录的戳换表。
- HarmonyOS 的后续入口见[鸿蒙单表导入](2026-10-16-harmony-helpcode-table-import.md)，该决定取代本篇原先因独立沙箱暂缓鸿蒙的安排。[共享沙箱镜像](../../implemented/architecture/2026-10-09-harmony-ime-shared-sandbox.md)已经覆盖偏好与插件目录；共享输入页在宿主提供导入能力时显示系统选择器入口，仍依赖签名授权，模拟器不能证明跨沙箱生效。Android、iOS 与本篇的 Rust 单表导入契约不变。

Android 设备夹具复用已校验的只读资源，在合成目录内复制一份拼音主库作为独立工作库；用户目录、缓存与偏好也保持独立。它不再把安装产品的可写工作库与合成用户目录配对，避免与活跃输入法争抢维护锁。副本使用 `File.toPath` 和 `Files.copy`，兼容 API 28，随夹具目录清理。

## Alternatives considered

- **同时取消预分配容量上限** — 直接按文件长度预估全部条目可减少大表增长次数；但 1 MiB 注释加一条数据也会预分配约二十万条，移动端会为不存在的数据占用内存。三万只作为初始容量，不作为接受条件。

- **复制进资源目录的 `helpcodes/custom/`，选 `custom/<stem>`** — 与桌面手工放文件的路径完全一致，Engine 已经会读，规则也宽松（后出现的行覆盖前面的，码截到两个字母），用户在桌面能用的表在手机上一定能用，这是它最强的理由。不用它：iOS 的资源目录是只读的 app bundle，鸿蒙每个进程各自暂存一份资源，Android 的资源目录由 `Bootstrap.prepare` 在安装包变化后管理；而插件目录是设置进程和键盘都看得见的那一份，有跨进程锁、原子换入，坏表在导入时就拒绝而不是在键盘里静默截断。代价是与桌面 `custom/` 的宽松规则不一致，桌面能用的表在手机上可能被拒，报错里因此写明文件名、原因和行号。
- **扩展 `import` 接受 `.txt` 路径** — 少一个 action，宿主像导入文件夹一样传路径。不用它：移动端选到的是 content URI（Android）或 security-scoped URL（iOS），传路径就得先在宿主里复制一份到可读的位置，再让核心再复制一遍；正文放进 JSON 只读一次，严格解码也留在宿主，GBK 表能在调用前就给出「不是 UTF-8」而不是被替换字符变成「等号左边必须恰好是一个字」。
- **在系统临时目录（`tempfile::tempdir()`）暂存** — 写法最短。不用它：Android 应用没有可写的 `TMPDIR`，`env::temp_dir()` 落到 `/data/local/tmp`，应用写不进去；暂存在 `root` 下用 `.staging-` 前缀，中断留下的目录还能被下一次导入按一小时规则清掉。

- **在辅助码用例前临时停用输入法**：可以避免复制主库，耗时较少；但会改变设备全局状态并依赖恢复顺序，仍保留合成用户目录与产品工作库混用的前提。测试夹具采用完整隔离，产品维护锁的规则不变。

## Consequences

- **收益**：Android 和 iOS 用户不写 `plugin.toml` 就能用自己的码表，键盘经已有的 `plugin_tables` 读它，重新导入同名文件后下一次聚焦就跟上；核心只多了一个包装函数和一个 action，校验、加锁、换入全部复用。
- **代价与已知上限**：严格规则会拒掉桌面 `custom/` 能用的表（重复字、三字母码、大写码）；用户手动导入过 id 恰好是 `custom-...` 的社区包时会被同名导入替换。本地备份恢复可能把 `plugins.helpcode_pack_*` 带到没有这个包的设备上，那时 `plugin_tables` 退回 `schema`，设置页显示「未找到」（账号设置同步不带辅助码设置）。iOS 键盘扩展有内存上限。在模拟器的测试宿主进程里量过一次：导入并选中 1,020,018 字节、30000 条的合成码表后，让已打开的会话重读设置、重建 Engine 并用它缩小候选，`phys_footprint` 从 25.7 MB 变成 23.8 MB，进程峰值停在导入那一步留下的 31.1 MB，载入没有抬高它；键盘扩展进程本身和真机上还没有量过。
- 与 [Windows 原生设置选辅助码插件](2026-10-11-windows-settings-helpcode-packs.md) 部分重叠：几处都照搬 `helpcode-page.tsx` 的选择规则，共享页改了规则要同步改 `main.cpp`、Android 的 `HelpcodeChoicePolicy.java` 和 iOS 的 `HelpcodeChoicePolicy.swift`。

## Verification

- #7765：旧实现拒绝十万条回归（`条数超过 30000`）；取消条数限制后，client-core 的 17 项辅助码回归、132 项插件回归及 host-api 导入到 Engine 会话的回归通过。十万条表恰好 1 MiB 可导入、扫描和完整载入；多一个字节、末尾重复字或非法长码会被拒绝，已安装的好表不被替换；注释填充文件的初始容量仍受限。Android API 28、35 各跑完整 client-core 套件，1377 项通过、13 项忽略，7 项无法在设备准备硬链接的测试按既有规则排除。
- #7765 的宿主验证：Android `HelpcodeImportDeviceSmoke` 在 API 28、35 的独立 AVD 上经安装产品 JNI 导入十万条、重新读目录、用末行「你=zz」过滤真实 Engine 候选并删除；API 28 `smoke.sh --core` 与 API 35 默认完整 `smoke.sh` 均整套通过。API 35 使用包含固定日文、注音资源的完整 APK（SHA-256 `cc2c4039393b20612468f61d0b3600326331f7f7c818d5a86afa0c1b011ee9fe`），语言切换、九键、十四键、辅助码和剪贴板保存生命周期均通过；不把测试夹具缺少语言资源的早期失败算作产品通过。iOS 使用完整 12 项固定资源，`HelpcodeSettingsTests` 五项通过，十万条表的末行「是=ya」在已打开会话重读后成为首选，删除后回到内置表；夹具显式创建目录并断言初始化诊断，避免资源准备失败被误报为输入行为失败。Clippy、Rustfmt、笔记校验、差异检查和本地 quick 门禁通过。这里验证的是模拟器测试宿主，不能由此推导 iOS 真机扩展的内存峰值；上述三万条的旧内存测量也不代表十万条的开销。


- `cargo test -p msime-client-core --lib helpcode` / `plugins`：`plugins/tests.rs` 覆盖可读 id、哈希 id、57/58 字节主干的边界、`# name:` 和 80 字节截断、带引号和反斜杠的文件名、重复字/三字母码/空表/GBK/超长各自被拒并带行号、二次导入替换且不留 `.staging-*`、`scan` 列出与 `load_codes` 读出。
- `cargo test -p msime-host-api --lib plugins`：`plugins_boundary_imports_a_helpcode_table_the_session_then_reads` 导入后把 `plugins.helpcode_pack_shuangpin` 设成返回的 id，会话里「你」的码被替换；坏表返回 `{ok:false,error:"plugin_invalid",detail}`。
- `bash platforms/android/check-host.sh` 编译并运行 `tests/home/HelpcodeChoicePolicySmoke.java`。
- iOS：`MSIMEClientTests` 里的 `HelpcodeChoicePolicyTests` 覆盖选包只写这一份的包键且保留按键音包、选内置方案清包、没选包时不碰 `plugins`、删除清掉两份选择、「未找到」项、目录过滤、严格解码和失败提示；`HelpcodeSettingsTests.testImportedTableReachesTheLiveSessionAndRemovalFallsBack` 用合成码表导入后让已打开的会话 `reloadSharedPreferences`，Shift Y 的首选从自然码的「识」变成表里的「是」，删除后退回「识」。去掉 `appEditedKeys` 里的 `plugins` 时这一条失败。`scripts/test-ios-file-importer-target.py` 要求 `platforms/ios` 下每个 `fileImporter`/`fileExporter` 的 `isPresented` 都是单纯的 `$` 状态，并要求辅助码页按 `importScheme` 导入；第一版的写法让它失败。单元测试直接调静态的 `importTable(_:into:)`，走不到选择器回调，这一处只有这条源码检查守着，还没有在模拟器里真的选一个文件走完双拼组的导入。改了文件读写，按 AGENTS.md 合并前还要在 API 28（`--core`）和 35 模拟器上跑 `smoke.sh` 并手工导入、选用、删除一次码表。

Android 整套冒烟在「载入码表到 Engine」处复现 `dictionary maintenance busy`，原因是旧夹具仍引用产品可写工作库。只改成空目录会得到空候选，必须从只读资源复制主库；首轮误用 `Path.of` 被 API 28 实测拒绝，现用 `File.toPath`。最终测试 APK 在 API 28、35 均通过十万条导入、目录重载、末行候选过滤与删除，API 35 保留 `--no-restart`，没有通过重启产品回避锁竞争。该夹具修正在[三万行词库容量](../bug-fix/2026-10-19-dictionary-collection-capacity.md)验收时发现，不改变辅助码产品行为。

辅助码夹具后续分支以 `e4252f4499` 为基线重新构建测试 APK，复用设备上已验证的原生 release 产品；API 28、35 的同一专项再次通过（API 35 继续附着运行）。该分支 quick（`20261019-220541`）通过，按改动范围实际检查 Rust 工作区、Android 目标、Java 宿主与 Apple bridge，其余平台跳过；笔记校验在本机单独通过。没有重跑完整产品 APK 吞吐测量或将此前失败的整套界面冒烟记为通过。
