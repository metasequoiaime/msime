# Agent Note: 新世纪五笔单字码表取自 byujiang/wubi06_rime

Status: implemented

## Problem

新世纪五笔要一张独立的单字全码表：GB2312 的 6763 字里约 23% 的全码与 86 版不同，二码简码 587 个码位里有 194 个不同，拿 86 表打补丁或按规则推导都不可行（不少变化来自拆分方式改了，例如段 wdmc→wdwc、为 ylyi→ytny）。公开的新世纪码表都能追溯到王码新世纪版这个商业产品，各表单字全码在 GB2312 范围内最多只差 13 字，不是彼此独立的来源；看得到的许可（GPL-3.0、Apache-2.0、MIT、LGPL）全是转载者加的，没有一份达到 98 表「权利声明来自数据作者本人」的标准。

## Decision

- 来源是 [byujiang/wubi06_rime](https://github.com/byujiang/wubi06_rime) 提交 `fc1cedbb21cde3ca160ce291888fa70697720ab3`（默认分支 `master` 的最新提交，2020-12-13）的 `dicts/wubi06.dict.yaml`，1421790 字节，SHA-256 `0c668f59b13b6f201d479678883925957989156b69eeaec163870ace2b011875`，仓库级 GPL-3.0。msime-dictionary 把它逐字节收成 `sources/wubi/wubixsj-byujiang.txt`。
- msime 一侧的登记：`resources/dictionary-sources.lock.json` 的 references 加 `wubi06-rime`，`crates/dict-builder/src/sources.rs` 的 `UPSTREAM_FILES` 把这个路径记为 `wubi06-rime` 的数据（16 项变 17 项）。`Dictionary::open` 因此要求 msime-dictionary 的 `upstream.lock.json` 用同一个上游名、同一个提交记录它，字节不符就拒绝构建。
- 只用单字行，丢掉上游的词组（子词库疑似从搜狗细胞词库导入），许可链里只剩「单字码」这一环；词组由本项目按新世纪单字全码从自己的词表生成，由后续的构建阶段实现。上游权重是个人手调的数值，不进表，只用来定同一编码下几个单字的先后：文件里同码单字的先后既不按常用程度也不按码位（`g` 下「王」在「一」之前、`hgd` 下「丄」在「上」之前），权重是表里唯一带着一级简码、二级简码层次的信息。
- `resources/licenses/msime-engine-dictionary-NOTICE.md` 如实写明：许可来自转载者、来源没有记录、首次数据提交的 27533 字与王码新世纪版的收字数相同所以很可能派生自那张商业表、文件头的 Wozy 署名沿用自 86 极点表。这一项写在末尾新加的「已登记、尚未随当前词库发布的来源」一节，因为说明的前半部分必须逐字等于词库锁所指发布的 msime-dictionary `NOTICE.md`；词库锁换到收录这张表的 `dict-v*` 后删掉这一项。`scripts/test-language-data-notices.py` 的 `ENGINE_NOTICE_REFERENCES` 加上 `wubi06-rime`，换了提交却忘了改说明会失败。
- GPL-3.0 全文就是项目根目录的 `LICENSE`，已经随六个平台分发，不另放一份许可证文件。
- 维护者 2026-11-02 拍板接受这个风险，标准与同样由转载者给出 Apache-2.0 许可的 jidian 86 表相同；同时向 CNMan 请求明确许可。专利与“新世纪”商标由维护者在带新表的 `dict-v*` 发布前查 CNIPA。界面称“新世纪五笔”“五笔 新世纪”，不出现“王码”“五笔字型”。

## 跨仓顺序

`Dictionary::open` 遇到 `upstream.lock.json` 里有 msime 锁文件没有的上游名会失败，所以 msime 这次登记必须先合，msime-dictionary 收文件的 PR 后合。反方向是安全的：没收这个文件的 checkout 照常能打开，只有读这张表时才报错，而读它的构建阶段在后续 PR 里才加。`the_wubixsj_table_is_wubi06_rime_data_at_the_pinned_commit` 固定这两点。

## Alternatives considered

- **登记进 `licensing.rs` 的 `UNLICENSED_INPUTS`，放在 `sources/unlicensed/` 下，只在 `--include-unlicensed` 的本地构建里用。** 最强的理由是不替来源存疑的数据背书，和 CustomPinyinDictionary 的处理一致。不用：发布版的 `msime-wubi.db` 里就没有这张表，各平台的新世纪入口都没有数据可读，等于代码合进去、功能不上线；维护者选择接受与 jidian 86 表同等的风险。
- **从 86 表按规则推导新世纪码。** 不依赖任何转载的码表。不用：变化的码有很大一部分来自拆分方式改了，没有可以机械套用的规则；推导出来的东西仍然在实现同一份编码规范，许可和专利问题并没有绕开。
- **换用 CNMan 维护的表。** 它是唯一还在逐条纠正错码的来源，质量可能更好。现在不用：它的 GBK 部分自述取自王码大一统 2018，许可同样不来自数据作者；拿到明确许可后再换源或互相校对。
- **用 byujiang 的 `words/single.txt`。** 与 dict 有 178 字不一致，只能二选一，dict 是 Rime 实际加载的那份。

## Consequences

- **收益**：新世纪五笔有了覆盖 GB2312 6763 字、URO 20902 字的单字全码来源，字节由两边的锁共同固定；来源存疑的事实写在随包说明里，读者看得到。
- **代价**：许可链里有一环来自转载者，很可能派生自商业码表；仓库 2020 年后无人维护，上游错码只能靠请求 CNMan 许可后对照修正。专利和商标没查之前，带这张表的词库不能发布。

## Verification

`rbuild cargo test -p msime-dict-builder`，`python3 scripts/test-language-data-notices.py`（把说明里的提交改坏时报 `does not name wubi06-rime`）。上游文件按提交下载后核对过大小和 SHA-256。
