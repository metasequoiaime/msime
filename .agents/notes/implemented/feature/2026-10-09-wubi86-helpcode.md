# Agent Note: 五笔 86 辅助码表（方案 wubi86）

Status: implemented

## Problem

从落格输入法迁来的用户想在双拼里用 86 五笔的编码当辅助码（#5985）。引擎的辅助码机制与码表无关，内置的六套却都是音形码，没有一套是五笔。会五笔的人只能自己做插件码表。

## Decision

- **码表从 msime-dictionary 生成并提交**：`msime-dict-build wubi86-helpcode --dictionary <checkout> --out resources/helpcodes/wubi86_helpcode.txt`（`crates/dict-builder/src/wubi86_helpcode.rs`）读 `sources/wubi/wubi86-jidian.txt`，每个字取全码（该字单独出现时最长的编码，与 `wubi86-supplement` 共用 `full_codes`）的前两码，写成 `字=前两码`。几个全码的前两码不一致的字跳过（317 个，GB2312 里只有 41 个偏旁部首和麴、麸），扩展 A 区和基本多文种平面以外的字跳过（与 `msime-wubi.db` 的 wubi86 表同一个判断 `outside_basic_cjk`），非汉字跳过。结果 20614 字、145068 字节，低于引擎 1 MiB 的码表上限。表头三行 `#` 记输入的 SHA-256 和规则、不记提交，同一份输入逐字节可复现。
- **匹配规则不变**：单字比整码，词比首字首码加末字首码，双拼完整音节后第三键调序、第三键加大写第四键筛选。五笔的「词」辅助码因此是首字第一码加末字第一码。
- **登记一处，各宿主照列**：`crates/engine/src/assets.rs` 的 `HELPCODES` 追加第七项 `("wubi86", "helpcodes/wubi86_helpcode.txt")`；`client-core` 的 `HelpcodeSchema::Wubi86` 序列化为 `"wubi86"`；Android 的 `helpcode_schemas` 列出它。提供辅助码方案选择的宿主都加了这一项，标签一律是「五笔 86」：共享设置页（Windows、Linux、Harmony 和各桌面壳的 Tauri 设置）、macOS 设置窗口与后台同步页、iOS、Android、Windows 原生设置页、Linux 的 IBus 与 Fcitx5 菜单、网页引擎 SDK 与演示页。码表随 macOS、iOS、Android、Harmony、Linux 的暂存或安装脚本进包，Windows 的 `Prepare-PackageFiles.ps1` 按 `*.txt` 自动带上，网页引擎只进 npm 包。`scripts/test-settings-label-parity.py` 用 `OWN_SCHEMES` 放行参考实现没有的 `wubi86`，只要求各处写法一致。
- **来源声明单列一份**：`resources/helpcodes/NOTICE-wubi86.md` 写来源、上游许可、生成命令、表和输入的 SHA-256，并附 Apache-2.0 全文（上游没有 NOTICE 文件）。各平台随表复制为 `helpcodes/NOTICE-wubi86.md`，macOS 与 Linux 另装为 `msime-helpcode-wubi86-NOTICE.md`，Windows 的 `Collect-Notices.ps1` 收集它，网页引擎的 NOTICE 由 `scripts/web-engine-notice.sh` 收进原文。两份 `THIRD_PARTY_NOTICES.txt` 各加一条，这两条和 `Collect-Notices.ps1` 的标签都写明有 16 个字来自 rime/rime-wubi、按 GPL-3.0 分发，汇总处与随附的 NOTICE 说法一致。
- **输入文件不全是极点数据**：msime-dictionary 在 `0ccfa82` 把 rime/rime-wubi（LGPL-3.0）的行并进了 `wubi86-jidian.txt`。与并入前的版本逐字对照，两边都有的字前两码全部相同；本表另有 16 个字只来自并入的行，这 16 行按 GPL-3.0 第 7 条去掉 LGPL 的附加许可后随本项目以 GPL-3.0 分发。NOTICE 写明了这件事和这 16 个码点。
- **云同步**：只有 macOS 的外观快照同步辅助码方案，用的是 `MSIMECloudHelpcodeSchemas()` 的下标。`wubi86` 追加为下标 6，旧下标不变。服务端字段表（msime-cloud `internal/account/preferences_fields.json`）里 `platform.macos.*_helpcode_schema` 是不带取值范围的 `integer`，`helpcode.*_helpcode_schema` 是不带枚举的 `string`，不会滤掉新值，服务端不用改。客户端这边，快照里超出本机方案目录的下标不再让整份快照无效：校验只要求非负整数，应用时保留本机的选择，其余键照常应用。后台同步页下载预览时同样把这样的下标换成本机的值（`MacSettingsModel.helpcodeSchemeTitles` 的长度就是本机方案数），预览里不会出现一个替换后并不生效的数字。Android 的设置同步从来不导出辅助码方案（`settings_sync` 的测试锁住了这一点），其他宿主没有这项同步。

## Alternatives considered

- **从词库发布的 `msime-wubi.db` 的 wubi86 表生成** — 那是用户实际在用的表，不需要 msime-dictionary 检出，还已经去掉了扩展区。没用它是因为数据库里极点行和 86 词组补充表的行混在一起、行序也丢了，要靠「单字」再筛一遍才能得到同样的集合；而 `wubi86-jidian.txt` 是那张表的源头，`full_codes` 也已经按它写好。两条路结果相同时取源头，生成命令和来源声明都更直接。
- **只用并入 rime-wubi 之前的极点原始行**（msime-dictionary `0ccfa82^` 的 `cn/Wubi86.txt`），让整张表只受 Apache-2.0 约束 — 许可最干净。没用它是因为这份文件只在历史提交里，生成就不能从当前检出复现；而对照结果显示差别只有 16 个生僻字或兼容字、没有一个字的码不同，把这件事写进 NOTICE 比为它固定一个历史快照代价小。
- **几个全码前两码不一致时取第一个或出现最多的** — 能多收几百个字。没用是因为这些几乎都是偏旁部首（`copp`、`zzpp` 一类部首查询码与真正的全码并存）和少数有两种拆法的字，猜错一码就让辅助码筛掉用户要的字，跳过只是让这些字不参加筛选。
- **把五笔 86 的说明写进现有的 `resources/helpcodes/NOTICE.md`** — 各平台已经在复制这个文件，不用改任何打包脚本。没用是因为它在每个平台都被改名为 `NOTICE-jiajia.md` / `msime-helpcode-jiajia-NOTICE.md`，五笔的许可和 Apache-2.0 全文放进一个叫「加加」的文件里，读者找不到。打包脚本反正要为码表本身改一遍。
- **macOS 收到未知下标时仍拒收整份快照** — 与其余整数键（`input_scheme`、`candidate_page_shortcut`）的处理一致。没用是因为辅助码方案会继续追加：今后再加一套，这一版的客户端就会因为别的设备选了新方案而一项设置都同步不下来。保留本机这一项、照常应用其余项是更小的损失。

## Consequences

- **收益**：双拼和全拼都能用五笔 86 当辅助码，所有提供方案选择的宿主同时上线；码表来源、许可和生成方法可查、可复现。
- **代价与已知上限**：已经发出去的 macOS 客户端仍按旧规则校验，同一账号里有一台设备选了五笔 86 并上传后，旧版本会拒收整份外观快照，直到升级。已经发出去的其他宿主的 `client-core` 读到偏好里的 `"wubi86"` 会让整份偏好文档解析失败；这只在同一台设备上降级时发生，因为没有宿主跨设备同步这个字符串。服务端 `/v1/input` 的 `helpcode` 操作（`crates/engine/src/backend/text_tools.rs` 与 msime-cloud 的 `input.go`）仍只接受原来五套，没有加 `wubi86`。`packages/web-engine/src/index.d.ts` 第一次过了格式化，联合类型改为每行一项，`crates/engine-wasm/tests/helpcodes.rs` 随之改为忽略空白比较。
- **重访信号**：msime-dictionary 的 `wubi86-jidian.txt` 变化时重新生成并更新 NOTICE 里的摘要；要做 98 版时照同一个生成器加一个读 98 表的子命令；用户反馈词的「首字首码加末字首码」与落格的五笔辅码习惯不符时，再看匹配规则。

## Verification

- 生成器：`cargo test -p msime-dict-builder`（`wubi86_helpcode` 的合成表测试覆盖取两码、全码不一致和不足两码跳过、扩展区跳过、渲染结果按引擎的读法解析）。生成结果另用一段独立的 Python 按同样规则重算，20614 字逐个相同。
- 引擎：`cargo test -p msime-engine`，`helpcode::tests::the_generated_wubi86_table_loads_whole` 载入仓库里的表，`shuangpin::tests::wubi86_helpcodes_filter_by_the_first_two_letters` 用合成码表经方案名 `wubi86` 载入后筛选单字和词，`host::tests::helpcode_settings_reach_the_real_engine` 用 `wubi86` 建会话。
- 偏好：`cargo test -p msime-client-core`，`HelpcodeSchema::Wubi86` 存取往返，Android 列出的每个方案都能解析成 `HelpcodeSchema`。
- 设置页：`apps/desktop` 的 vitest，两个方案下拉框都列出「五笔 86」并能保存。
- macOS：`tests/settings/CloudAppearanceSettingsTest.mm` 覆盖下标 6 的导出与导入，以及未知下标保留本机选择、其余键照常应用；`tests/core/BackendAccountTests.swift` 用合成的云端下标 6 和 9 核对下载预览（6 照常进预览，9 换成本机的值）；`tests/input/ShortcutTest.mm` 覆盖设置窗口的七个选项，在 Studio 编译后拉回本机图形会话运行通过。
- 网页包：`cargo test -p msime-engine-wasm --test helpcodes` 核对打包脚本、SDK 与类型声明；`scripts/web-engine-notice.sh --check`。
- 标签：`python3 scripts/test-settings-label-parity.py`，四处界面对七套方案的写法一致。

相关：[网页引擎的手到、微软双拼和辅助码](2026-10-08-web-engine-shuangpin-helpcode.md)（网页包怎么带辅助码表；本篇在那份清单上加了第七张）。
