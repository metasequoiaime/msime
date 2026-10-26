# Agent Note: 设置页插件编辑器生成文本类插件包，走导入同一条安装路径

Status: implemented

## Problem

插件生态要靠普通用户自己做插件长起来，但做一个短语表、指令表、符号集或单词本都得手写 `plugin.toml`（单词本还要一个 TSV），再打包导入。不会写 TOML 的人卡在第一步，社区里「发布我的插件」也就没有东西可发。四种文本类插件的内容只是几张表，本来就适合在表单里填。

## Decision

设置页的「插件」页有「新建插件」：选类型，填名字、作者、许可证（默认 `CC0-1.0`）、版本（默认 `1.0.0`）和内容表，保存即安装，然后打开这个包的详情；用户插件目录里这四种包的详情有「编辑」，读出原内容，保存时整包替换。

规则只有一份，在 client-services：

- `plugins::authoring` 定义编辑器与宿主交换的 `AuthoredPack`（通用字段加与 `kind` 对应的恰好一个内容字段），`load` 从已安装的包读出它，`save` 把它写成 `plugin.toml`（单词本另写 `words.tsv`）。
- `save` 不自己校验内容，而是交给 `import::install_staged`：这是从 `import_inner` 抽出来的同一段流程，拿插件根目录写锁、清扫残留、建暂存目录，由调用方往暂存目录里放文件，再跑 `check_staged` 并整包替换。导入和编辑器因此共用写锁、校验和替换，编辑器保存的包与从磁盘导入的包按同一套规则检查。
- `id` 为 null 时在写锁里生成 `my-` 加 8 位小写十六进制随机数，被占用就换一个；这个形状同时满足单词本更严的 id 规则。`replace` 为假而同 id 已装时是新失败码 `plugin_exists`；读不了的包（内置或官方 id、四种以外的类型、没装或载入失败）是 `plugin_not_editable`。
- 替换时原包里的 `.txt`、`.md` 说明文件原样带进新包：社区装来的 CC-BY 包改过内容之后，许可证全文和署名不会因为保存而消失。

宿主只是垫片：桌面外壳的 `load_authored_plugin_pack`、`save_authored_plugin_pack` 命令，以及 `msime_client_plugins` 的 `authoring_load`、`authoring_save` 操作（Windows 原生设置应用和鸿蒙插件页用它，鸿蒙的桥白名单加上这两项）。`msime_client_plugins` 的请求上限从 2 MiB 提到 8 MiB，容得下一本 4 MiB 词表的 JSON。

页面侧 `plugin-editor-model.ts` 按同样的上限先查一遍，像 @ 名单那样在保存按钮上方只报第一处问题，宿主再查一次，拒绝时把 `detail` 就地显示在表单里。短语表和单词本可以批量粘贴（短语一行「编码 文字」，单词按 Tab 分列），长列表每次只画 100 行。保存后详情顶上提醒短语表、指令表还要打开「启用」，K 模式或 `/` 指令关着时也要打开。

## 编辑器自己多挡的两条

TSV 写不出来的内容在写文件之前拒绝：某一列含有制表符或换行（不拦的话，`foo` 的释义 `bar\nbaz\tqux` 会被读成两个合规的单词），以及以 `#` 开头的单词（会被读成注释、悄悄丢掉）。生成的 `plugin.toml` 超过 256 KiB 时换成编辑器用户看得懂的说法报错，上限本身不变。

## 旧笔记审计

检索活跃笔记里的插件导入、插件页、单词本、短语表和社区插件。[Windows 原生设置的插件页](2026-10-11-windows-settings-plugins.md)和[Windows 插件库](2026-10-20-windows-settings-plugin-library.md)定义 `msime_client_plugins` 的既有操作，本篇在同一接口上加两项，部分重叠、继续有效；[macOS 插件库](2026-10-19-macos-plugin-library.md)只约束卡片布局，「新建插件」放在它的导入按钮旁边，不改布局决定；[脚本插件作者工具](2026-10-12-script-plugin-author-tools.md)面向会写代码的作者，无关。本篇不取代已有决定。

## Alternatives considered

- 页面里拼 TOML 字符串，再走现有的导入命令：宿主一行都不用改，最快。但导入要一个磁盘路径，HarmonyOS 和 Windows 原生设置都拿不到页面生成的临时文件；而且 TOML 的转义、单词本 TSV 的格式规则会在 TypeScript 里再写一份，正是要避免的第二套规则。
- `authoring::save` 学辅助码表导入（`helpcode_pack::import_table`）的做法，先把文件写进一个临时目录，再调用 `import`：不用改 `import.rs`。但 `plugin_exists` 的检查和 id 生成就落在写锁之外，两个设置窗口同时新建会互相覆盖；抽出 `install_staged` 让这两步都在锁里。
- 编辑器只做新建，不做编辑：读回结构要多一个 `load`，单词本还要读词表文件。但不能编辑就意味着改一个字要删包重做，社区装来的包也没法按自己的需要改，起不到「在别人的包上接着做」的作用。
- 保存时只写生成的文件，丢掉原包的说明文件：实现最简单。但社区包的许可证文本常在 `LICENSE.txt` 里，丢掉它会让改过的 CC-BY 包失去署名。

## Verification

- `rbuild cargo test -p msime-client-services --locked plugins::authoring`（Mac Studio，macOS）：11 项通过，覆盖四种类型新建、读回、再保存得到逐字节相同的清单，生成 id 时跳过已占用的名字，按导入规则拒绝的内容，TSV 写不出的单词，内容字段与类型不符，内置、别的类型和没装的包不能编辑，编辑导入的包保留 `LICENSE.txt`，以及 JSON 契约的形状。
- `rbuild cargo test -p msime-host-api --locked plugin`（同上）：78 项通过，其中新增的两项经 `msime_client_plugins` 走完四种类型的保存、读回、`plugin_exists` 和替换，以及各种拒绝。
- 前端：`apps/desktop` 里 `npx vitest run tests/settings` 和新增的 `tests/settings/plugin-editor.test.tsx`（10 项：四种类型新建、表单报第一处问题、批量粘贴、宿主拒绝就地显示、编辑社区包、放弃修改要确认、读不出的包、没有编辑器的宿主），`pnpm run typecheck`（desktop 与 harmony），根目录 `pnpm run lint` 与 `format:check`。
- 没有在真实的设置应用、鸿蒙设备或 Windows 原生设置里点过；Windows 原生设置的编辑器由另一个改动按同一契约实现。

## Consequences

收益是普通用户能在几分钟内做出一个可以发布的插件，而规则仍只有 client-services 一份：清单校验改了，编辑器自动跟着改。`import_inner` 变成 `install_staged` 的一个调用方，导入行为不变。

代价是 `PluginError` 多了 `Exists` 和 `NotEditable` 两个变体、页面多了两个失败码；页面上的预检是规则的一份镜像，与 client-services 不一致时以宿主的拒绝为准，只是提示晚一步。生成的清单不保留原文件的注释和键顺序，编辑一个手写的包后 `plugin.toml` 会变成生成的样子；单词本的词表文件名变成 `words.tsv`。移动端（Android、iOS）没有插件页，也就没有编辑器。
