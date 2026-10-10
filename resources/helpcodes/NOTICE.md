# 辅助码表：来源、权利与分发限制

这个目录放的是本仓库携带的全部七套辅助码表，由 `crates/engine/src/assets.rs` 的 `HELPCODES` 登记。本文件记的是加加（`jiajia`）这一套。

另外五套（蓝天小雨点、自然码、首右 2.0、首右 Plus、小鹤）原先随 Engine 归档而来，来源说明原样保存在同目录 `ENGINE-NOTICE.md`。五笔 86（`wubi86`）由本仓库从 msime-dictionary 的 86 五笔码表生成，来源、许可与生成方法见同目录 `NOTICE-wubi86.md`。

## `jiajia_helpcode.txt`

| 项 | 值 |
| --- | --- |
| 方案标识 | `jiajia`（加加辅助码，拼音加加） |
| 来源仓库 | [metasequoiaime/MSIME-Windows](https://github.com/metasequoiaime/MSIME-Windows) |
| 来源路径 | `engine/helpcode/helpcodes/jiajia_helpcode.txt` |
| 来源提交 | `566ff8b8320e7f56256544b2b1f0da8c8e7f037e`（2026-09-20，`feat(engine): 新增加加辅助码（拼音加加），第六套辅助码方案`） |
| 内容摘要 | `sha256:6538d744547b590630e93160198ac16bf76112a51ec78b4dbae505b914761879`，7968 行 |

### 这张表是怎么来的

规则是「按笔顺拆出前两个部件、各取读音首字母」。本表按拼音加加输入法 5.x 安装包内的辅助码表 `fzm.bin` 对齐：该表覆盖 GB2312 全部 6763 字并含部分扩展字（基本区共 7259 字有码，其中 6576 字为两码）；本表 7968 字中有 6487 字从该表取到两码，其中 6349 字与本表逐字一致，其余 138 字保留本表规则（独体/部首字取「本字声母 + 起笔」，其中 13 个成字部件按俗称读音取音），未采用加加对这些字给出的另一套拆法（笔画码或不同部件拆分）；不在该表覆盖内的 800 字按同一规则由公开拆字数据重建。`fzm.bin` 内是加加双拼键位（`zh=v`/`ch=u`/`sh=i`），对照时已换算为声母 z/c/s。

部件拆分数据来自 [rime-radical-pinyin](https://github.com/mirtlecn/rime-radical-pinyin)（GPL-3.0，上游含 chaizi/CC-BY-3.0、CHISE/GPL-2+、yi-bai/ids/MIT），笔顺数据来自 cnchar（MIT）。

### 分发限制

**拼音加加是商业软件，本表是格式转换与整理，不是加加官方码表。** 本表的一部分条目直接来自该商业软件安装包内的数据表（`fzm.bin`）。

来源仓库的 `engine/helpcode/NOTICE.md` 对全部六张表写明：没有任何一项拿到了明确的再分发授权，需要逐个与方案作者确认，或改为在运行时由用户自行导入而不随包分发；并指出 `jiajia` 一行与其余各表性质不同（其余各表只是复现已发表的输入方案），若后续要做权利澄清应优先处理这一项。

**在澄清之前，不要假定这些数据可以自由再分发。** 本仓库以 GPL-3.0 分发，该许可**不覆盖**这张表的内容。打包发布前应确认这一项在目标渠道的分发是否可接受；如果不可接受，从 `crates/engine/src/assets.rs` 的 `HELPCODES` 去掉 `jiajia` 那一行，并让该渠道的资源打包不再带上这张表——设置页会随之少一个选项，其余各套不受影响。
