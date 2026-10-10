# Agent Note: 网页引擎的手到、微软双拼和辅助码

Status: implemented

## Problem

官网（msime.app）的每个文本框都跑 `@msime/web-engine`，但网页引擎只有全拼、小鹤、自然码和五笔 86（另有日语、韩文）。用手到或微软双拼的人打不了字，习惯辅助码的人也没有辅助码可用。引擎早就支持这两种双拼和六套辅助码，缺的是 wasm 层和 SDK 的出口，以及辅助码表怎么进到没有文件系统的浏览器里。

## Decision

- **两种双拼是两个新方案名**：`shoudao`、`microsoft`。`crates/engine-wasm/src/host.rs` 的 `Scheme` 把它们映射到 `SchemeType::Shuangpin` 和 `ShuangpinProfileKind::{Shoudao, Microsoft}`；SDK 的 `PINYIN` 集合收进它们，所以全拼和四种双拼之间 `setScheme` 就地切换、共用拼音库。微软双拼的 `;` 是韵母 ing：`WebHost::punct` 在组字中先把 `;` 当字符交给引擎，引擎不收（不是音节的第二键）才按标点处理，和全拼撇号走同一条路。
- **辅助码默认关闭，开关是运行时的**：`session_options` 仍然 `helpcode = false`，D15 评测的 40.9% 基线不变。`WebHost::set_helpcode(Option<&[u8]>)` 把表字节交给引擎，引擎用新增的 `HelpcodeKeymap::from_table_bytes` 解析（与读文件同一个 `parse_helpcode_table`，同样 1 MiB 上限），再走 `Session::set_helpcode_table` / `set_helpcode_enabled`；表和开关同时写回 `options`，`reset` 重建的会话保持这个设置。辅助码怎么读全在引擎：全拼音节后的大写字母（一个调序、末尾两个筛选），双拼完整音节后的第三键（调序）和第三键加大写第四键（筛选）。wasm 层只做一件事：辅助码开着且在组字时，大写字母交给 `Session::character` 而不是像原来那样上屏原码再打出字母。
- **候选带辅助码提示**：`Row` 多一个 `hint`，取引擎的 `SessionSnapshot::candidate_annotations`，只在辅助码开着时填，关着时恒为空串，帧对现有接入方只是多了一个字段。默认候选栏有 `hint` 就显示在候选后（`::part(code)`），`createCandidateBar({ helpcode })` 的旧含义不变：只管没有提示的行要不要显示 `code`。
- **SDK 接口**：`createMsimeEngine({ helpcode })` 收 `HELPCODES` 里的名字或 `null`，`engine.setHelpcode(name | null)` 就地开关，`engine.helpcode` 读当前值；五笔、日语、韩文忽略它，也不下载表。表在打开时才下载（init 时随引擎一起，运行时由 Worker 的 `helpcode` 消息下载、按方案名缓存）；运行时下载失败用新的 `rejected` 消息只让这一次调用 reject，引擎照常可用。
- **表只进 npm 包，不进 dist**：`scripts/build-web-engine.sh` 把每张表 gzip -9n 成 `assets/helpcode-<方案>.txt.gz`（现为七张，26–82 KB，共约 350 KB；第七张五笔 86 见 [五笔 86 辅助码表](2026-10-09-wubi86-helpcode.md)），名字和大小写进生成的 `assets.js` 的 `helpcodes`，不写进 `web-engine-manifest.json`。CLI `copy` 默认复制它们，`--no-helpcode` 或 `--no-pinyin` 时不复制，`info` 也列出它们。方案名与文件和 `msime_engine::assets::HELPCODES` 一一对应，`crates/engine-wasm/tests/helpcodes.rs` 核对打包脚本、SDK 的 `HELPCODES` 和类型里的 `MsimeHelpcode`。
- **许可**：`scripts/web-engine-notice.sh` 的 `repo` 段收进 `resources/helpcodes/ENGINE-NOTICE.md` 和 `resources/helpcodes/NOTICE.md` 原文；`web-engine-NOTICE.md` 新增手写的「辅助码表」一节，写明这些表没有明确的再分发授权、GPL-3.0 不覆盖、`jiajia` 有一部分条目来自商业软件，以及不能分发时用 `--no-helpcode`。

## Alternatives considered

- **表进 dist 和 `web-engine-manifest.json` 的 `artifacts`**：最直接，SDK、CLI 和 TapTapGo 都从同一份清单读文件，`assets.js` 也不用另外拼。不用它是因为那份清单是 release 的契约：`release-web-engine.yml` 的「Verify the release files」按角色集合精确核对 `artifacts`，多六个角色下次发版就失败；publish 步骤逐个列出要上传的文件，表进了 dist 却不上传，`SHA256SUMS.txt` 就会列出 release 里没有的文件；TapTapGo 按这份清单钉 release，而它用不到辅助码。只进 npm 包不需要碰 release workflow。
- **表进 dist，但写在清单的另一个键里**：release 核对能过，但上面那个 `SHA256SUMS.txt` 与 release 资产不一致的问题仍在，除非同时改 publish 的上传列表。
- **把表编进 wasm（`include_bytes!`）**：不用下载、不用改打包，但六张表约 650 KB 原始数据会让每个方案（包括五笔、日语、韩文）都多下载几百 KB，而辅助码默认关闭、绝大多数访客用不到；`MAX_WASM_BYTES` 的余量也不该这样花掉。
- **在 wasm 层或 JS 里自己解析表、判断哪些键是辅助码**：引擎已有完整规则（全拼大小写、双拼单码和双码、码序对调），再写一份就是 ARCHITECTURE.md 说的两份实现开始漂移。wasm 层只转交大写字母，解析也用引擎的同一个函数。
- **`createCandidateBar({ helpcode: true })` 改成显示辅助码提示**：选项名字合适，但 TapTapGo 和演示页的五笔一直用它显示编码补全，改语义会让它们在拼音方案下显示拼音之外的东西又不再显示编码。新开 `hint` 字段，默认候选栏有提示就显示，不动旧选项。

## Consequences

- **收益**：网页引擎有了四种双拼和六套辅助码，与桌面端同一套规则；官网可以随用户设置 `setHelpcode`，不用重建引擎、不用重新下载词库。辅助码关着时帧、排序和评测配置都与原来相同。
- **代价**：`assets.js` 和 CLI 现在有两个文件来源：清单里的 release 文件和只在 npm 包里的辅助码表。GitHub 上的 `web-engine-v*` release 不含辅助码表，从 release 而不是 npm 取文件的接入方要辅助码得自己托管表。
- **分发风险**：六张表都没有明确的再分发授权，npm 包是公开渠道。NOTICE 和 README 写明了这一点并给出 `--no-helpcode`，但发布前是否带上这些表（尤其 `jiajia`）仍需要维护者决定；不带的话把它们从打包脚本里去掉即可，SDK 打开辅助码时会以 `unsupported`（缺表）失败。

## Verification

- 引擎：`cargo test -p msime-engine --lib helpcode`（新增 `table_bytes_parse_like_the_file_and_share_its_bound`）。
- wasm 层：`cargo test -p msime-engine-wasm`，`routing.rs` 新增手到、微软（含 `;` 在组字中、音节后、空闲时）、辅助码默认关闭、全拼大写字母调序和筛选、四种双拼第三键调序和第三四键筛选、`reset` 后保持、组字中关闭、超大表被拒、五笔无效等用例，`host.rs` 单测覆盖方案名和键位表映射，`helpcodes.rs` 核对打包清单。`cargo clippy -p msime-engine -p msime-engine-wasm --all-targets` 和 `--target wasm32-unknown-unknown` 都是零警告。
- 包：`scripts/build-web-engine.sh --no-data`，然后 `node crates/engine-wasm/tests/smoke.mjs`、`node --test 'packages/web-engine/test/*.test.mjs'`、`node packages/web-engine/test/smoke.mjs`（真实 wasm、包里真实的小鹤形码表：手到 `nihd`、微软 `b;`、双拼和全拼的双码筛选、`setScheme` 保留辅助码、关闭、换表、缺表、404、五笔忽略、CLI `--no-helpcode` 和 `info`）、`node packages/web-engine/test/candidates.browser.mjs`（候选栏显示 `hint`）。
- 演示页「看它打字」的手到和微软样例用构建出的 wasm 和出货的完整拼音库逐条打过，首选分别是 你好、世界、中国、你好、世界、北京。演示页本身在无头 Chromium 里用夹具词库跑过一遍：切到微软双拼打 `b;` 出冰，选小鹤形码后 `ni` 的候选带 `(rX)`、`(kB)`、`(uB)`，`niuB` 只剩 尼，接入代码写出 `helpcode: "xiaohe"`，切到五笔时辅助码选单变灰，页面没有脚本错误。没有在真实浏览器里手动操作过。
