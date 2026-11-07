# Agent Note: macOS 设置应用每次请求现读 runtime-options.json

Status: implemented

## Problem

#11098：macOS 上设置应用一直开着，输入法在这期间换了词库代次（在线词库更新装好后输入法载入新版本，或升级换了词库锁）。之后在「词库 → 我的词条」里新增、编辑、删除的词条，输入法里都看不到：新增的快捷短语打不出来，删掉的词条还在候选里。一台真实的 Mac 上现场核对过：在线词库更新夜里装好，输入法早上换到新代次并改写了 `runtime-options.json`；从前一天起就开着的设置应用把随后新增的一条快捷短语写进了旧代次的 `msime-pinyin.db`，新代次里没有；同一时段删掉的两条，新代次里还在。

改动本身没有丢。设置页的编辑同时写日志（`msime_user.db`）和 `dictionaries` 指向的工作词库，日志要等下次重新准备代次时才回放进新代次。

原因在设置应用拿 HostOptions 的方式：

- `DictionaryHostOptions` 在 macOS 上存的是启动时 `macos_launch` 解析出的文档（`Arc<Value>`），「我的词条」（`dictionary_request`）、词条同步、云词库、辅助码表列表和版本判断都用它。输入法换代次时改写的是文件，设置应用手上那份不会变。
- Linux 的同一个结构存的是路径、每次现读，起因是首次运行时文件还不存在，不是为了跟上代次，所以 macOS 一直没有跟进。
- `RuntimeOptionsState` 也只在 Linux 上现读。macOS 只有「准备词库」（`repair_macos_oobe_resources`）会在内存里换掉它，`desktop_resource_packs::host_options` 为此在 macOS 上优先取它，绕开过期的 `DictionaryHostOptions`。

Windows 上同一类症状是 #9434，成因不同（那边的文件本身没人刷新），见 [Windows 词库维护让 TIP 宿主会话放开，设置窗口写进输入法读的代次](2026-10-25-windows-dictionary-maintenance-tip-sessions.md)。

## Decision

macOS 的设置应用和 Linux 一样，每次请求都现读 `runtime-options.json`。

- `DictionaryHostOptions` 在 Linux 和 macOS 上都存 `runtime-options.json` 的路径（macOS 上是启动时 `macos_launch` 解析并刷新过的 `options_path`），`snapshot()` 每次经 `read_runtime_options` 读：拒绝符号链接和非普通文件，检查父目录链，读取上限 2 MiB，内容必须是 JSON 对象。去掉 `candidate_skin_catalog` 只在 Linux 上做，macOS 不往这份文件里发布皮肤目录。
- `RuntimeOptionsState::snapshot()` 在 macOS 上有路径时同样现读，内存里那份只是上一次读到的结果。`repair_macos_oobe_resources` 照旧写它，但之后的读取以它刚刷新过的文件为准。
- `desktop_resource_packs::host_options` 不再区分平台，直接用 `DictionaryHostOptions`。
- 启动时仍读一次文档：`publish_native_options` 发布定位文件，`RuntimeOptionsState` 也以它为初值。
- 文件读不了或不是合法文档时，这一次请求答 `storage`，不退回启动时那份。
- Windows 不变。Windows 上的 Tauri 只承载面板，设置窗口是原生的，本来就每次现读（`runtime_options_document`）。

## Alternatives considered

- **在设置应用自己装好词库更新、或窗口重新获得焦点时刷新内存里那份。** 读文件的次数最少，内存里始终只有一份文档。但换代次的是输入法进程（`msime_client_refresh_host`，时机由它决定），设置应用收不到任何事件；按焦点刷新仍留着一段窗口，而且多出一条要和文件保持一致的路径。一份十来 KiB 的文件每次请求读一遍，相对请求本身要打开的 SQLite 可以忽略，`msime-mcp` 也是每次请求现读同一份文件。
- **向输入法进程询问它此刻用的选项。** 结果最准确。但要新增一条跨进程契约，输入法没在运行时还得另有退路；`runtime-options.json` 本来就是两边共用的契约，输入法换代次时也会改写它。
- **现读失败时退回启动时那份。** 文件偶尔读不了时请求不会失败。但这样会悄悄写进可能已被回收的旧代次，正是这次要消除的症状；Linux 一直是读不了就答 `storage`。

## Consequences

- **收益**：输入法换代次之后，设置页的增删改直接落在输入法读的代次里，维护一结束就能打出来，不必重启设置应用或等下次准备代次。[回收用不上的工作词库代次](2026-11-03-dictionary-working-generations.md)里设置宿主报给回收的 `configured` 在 macOS 上也是当前代次，旧代次不会因为设置应用没重启而一直被留着。资源包判断看到的也是当前的 `resources`。
- **代价**：每次请求多读一次文件。文件被意外改坏时，设置页的词库请求会失败，而不是继续用启动时那份。
- **没有覆盖**：修复之前已经写进旧代次的改动，不会自动进新代次，这里不做迁移。重开设置应用也补不回来：`runtime-options.json` 记的已经是当前代次，启动刷新（`refreshed_host_options`）直接返回、不重新准备，日志不回放。这些改动要么在设置里重做一遍，要么等下一次换代次时随整份日志回放进去。报告现场就是这样：重开设置应用之后，新代次里仍没有那条快捷短语，删掉的两条也还在。

## Verification

- `cargo test -p msime-desktop --lib settings_edits_reach_the_generation_the_input_method_switched_to`（macOS 本机跑过；CI 的「Linux settings app」作业在 Linux 上跑 `msime-desktop` 的全部单测）：搭两个合成的工作词库代次，按指向旧代次的文件建好 `DictionaryHostOptions` 和带启动文档的 `RuntimeOptionsState`，再用 `publish_prepared_dictionaries` 把文件原子地改写成新代次。断言两个快照都换到新代次；经设置页的入口 `settings_dictionary::request` 新增一条快捷短语后，新代次里列得出、旧代次里列不出。把 macOS 临时改回只在第一次读文件（`OnceLock`）时，这条测试在快照断言上失败（左边是 `aaaa…` 代次，右边是 `bbbb…`）。
- `cargo test -p msime-desktop`：macOS 上 295 项通过。改动的文件 rustfmt 干净；`cargo clippy -p msime-desktop --all-targets` 只剩 develop 上原有的 `read_aloud::available` 未使用警告。
- Linux、Windows 的编译没有在本机跑（本机没有 Docker 守护进程，也没装 Windows GNU 目标）。PR 上 `ci-platforms.yml` 的「Linux settings app」对 `msime-desktop` 跑 `clippy -D warnings` 和单测，Windows 作业跑 `cargo check -p msime-desktop`。
- 真机：报告现场在修复前复现过，装上带这个修复的构建后的验收还没做。

## Related notes

[Windows 词库维护让 TIP 宿主会话放开，设置窗口写进输入法读的代次](2026-10-25-windows-dictionary-maintenance-tip-sessions.md)：部分重叠，同为「设置写进的不是输入法读的代次」，成因是 Windows 上的文件本身没人刷新（#9434）。[回收用不上的工作词库代次和 Linux 换下的词库目录](2026-11-03-dictionary-working-generations.md)：那篇对 macOS 设置应用拿着启动时文档的描述随本篇就地改了。
