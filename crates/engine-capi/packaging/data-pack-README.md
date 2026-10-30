# msime-engine 最小数据包：全拼、双拼（词库 {{version}}）

Minimal runtime data for the msime-engine input engine (quanpin and shuangpin), taken unchanged from the msime-dictionary `dict-v{{version}}` release. The engine is MPL-2.0; this data is not. It is GPL-3.0 data with CC-BY-SA-4.0, CC-BY-4.0, MIT and WTFPL parts, see [Licences](#许可证).

这是水杉输入法输入引擎 `msime-engine`（Rust crate）和 `msime-engine-capi`（C 库）打全拼、双拼要的最小数据，五个文件逐字节取自 [metasequoiaime/msime-dictionary](https://github.com/metasequoiaime/msime-dictionary) 的 [`dict-v{{version}}`](https://github.com/metasequoiaime/msime-dictionary/releases/tag/dict-v{{version}}) 发布，没有任何改动。**引擎是 MPL-2.0，这些数据不是：** 它们按下文各自的许可证发布，主体是 GPL-3.0。

## 里面有什么

解开后的这个目录本身就是引擎的 `resources` 目录（只读，引擎不往里写）。

| 文件 | 大小（字节） | SHA-256 | 用途 |
| --- | --- | --- | --- |
| `msime-pinyin.db` | {{size:msime-pinyin.db}} | `{{sha256:msime-pinyin.db}}` | 拼音词库与快捷短语，全拼、双拼、九键必需 |
| `msime-english.db` | {{size:msime-english.db}} | `{{sha256:msime-english.db}}` | 英文词库，每个方案都要（准备代次时硬性要求） |
| `msime-bigram.bin` | {{size:msime-bigram.bin}} | `{{sha256:msime-bigram.bin}}` | 整句词格的二元表；和三元表一起时整句排序与 msime 自己的相同，缺了也能打字 |
| `msime-trigram.bin` | {{size:msime-trigram.bin}} | `{{sha256:msime-trigram.bin}}` | 整句词格的三元表 |
| `msime-scowl_Copyright.txt` | {{size:msime-scowl_Copyright.txt}} | `{{sha256:msime-scowl_Copyright.txt}}` | SCOWL 的版权与许可声明，分发 `msime-english.db` 时必须一同携带 |
| `licenses/` | | | 下文「许可证」要求随附的全文与来源说明 |

五笔、日文、粤语、注音、笔画要的文件不在这个包里，按方案的最小数据集见 [docs/data-formats.md §20](https://github.com/metasequoiaime/msime/blob/develop/docs/data-formats.md#20-每个方案的最小数据集)，文件都在同一个 `dict-v{{version}}` 发布里。表情、颜文字和符号目录用的 `msime-others.db` 也不在：它的部分来源没有再分发授权（§21）。文件格式见 [docs/data-formats.md](https://github.com/metasequoiaime/msime/blob/develop/docs/data-formats.md)。

## 怎么用

把这个目录的绝对路径交给引擎当 `resources`，另给两个可写、互不重叠的目录当 `user_data` 和 `cache`。准备代次时的 content id 随数据版本换，例如 `dict-v{{version_id}}`。

- Rust：`cargo run --release --example quickstart -- <这个目录>`（在 msime 仓库里），或照 [crates/engine/README.md 的「快速上手」](https://github.com/metasequoiaime/msime/blob/develop/crates/engine/README.md#快速上手) 写。
- C：照 [crates/engine-capi/README.md 的「快速上手」](https://github.com/metasequoiaime/msime/blob/develop/crates/engine-capi/README.md#快速上手)，`msime_engine_prepare_runtime` 的 `paths.resources` 就是这个目录。

## 许可证

本说明不是法律意见，也不是对再分发授权完整性的评估。每个来源的上游、固定的提交与条款以 msime-dictionary 的 `NOTICE.md` 为准，它在本仓库的副本是 `licenses/msime-engine-dictionary-NOTICE.md`；按文件的汇总是 [docs/data-formats.md §21](https://github.com/metasequoiaime/msime/blob/develop/docs/data-formats.md#21-数据许可证)。`dict-v` 发布是授权构建：没有再分发授权的输入（NOTICE 的「待解决」一节）不进入其中。

| 文件 | 数据许可证 |
| --- | --- |
| `msime-pinyin.db` | **GPL-3.0**：词条来自 [iDvel/rime-ice](https://github.com/iDvel/rime-ice) 及其补充和 msime-dictionary 的自建词条。与 [amzxyz/rime-wanxiang](https://github.com/amzxyz/rime-wanxiang) 共有词条的权重和字频改编自它（CC-BY-4.0，署名、改动说明与许可证在 `licenses/rime-wanxiang-CC-BY-4.0.txt`）；单字读音以 [mozillazg/pinyin-data](https://github.com/mozillazg/pinyin-data)（MIT，`licenses/pinyin-data-MIT.txt`）校正；行政区划地名来自 [modood/Administrative-divisions-of-China](https://github.com/modood/Administrative-divisions-of-China)（WTFPL，`licenses/Administrative-divisions-of-China-WTFPL.txt`） |
| `msime-english.db` | **GPL-3.0**：rime-ice 的 rime-ice-en 及补充。英文词表含 [SCOWL](https://github.com/en-wl/wordlist) 的词，条款要求在所有副本中保留版权与许可声明：`msime-scowl_Copyright.txt` 必须和它一起分发。中英释义取自 [skywind3000/ECDICT](https://github.com/skywind3000/ECDICT)（MIT，`licenses/ECDICT-MIT.txt`）。Google 词频（<https://www.norvig.com/ngrams/count_1w.txt>）以来源页面的说明为准 |
| `msime-bigram.bin`、`msime-trigram.bin` | **CC-BY-SA-4.0**：计数统计自[中文维基百科 20260901 的条目转储](https://dumps.wikimedia.org/zhwiki/20260901/)（Wikipedia contributors，CC BY-SA 4.0，<https://creativecommons.org/licenses/by-sa/4.0/>），只含词序列的统计量，不含原文；词表来自 GPL-3.0 的 `msime-pinyin.db` |
| `msime-scowl_Copyright.txt` | SCOWL 的声明本身 |

对采用方意味着：

- **分发这些文件（或从它们改出来的数据）时**，按 GPL-3.0 提供：附上 GPL-3.0 全文（`licenses/GPL-3.0.txt`），保留对 rime-ice 的署名，并能给出对应的源。这些数据的源是 msime-dictionary 仓库里的词条文件和 msime 仓库的构建器 [`crates/dict-builder`](https://github.com/metasequoiaime/msime/tree/develop/crates/dict-builder)，`dict-v{{version}}` 发布里的 `msime-dictionary-manifest.json` 记下了构建所用的两个提交。
- **n-gram 表**另外要保留对中文维基百科的署名（例如「Contains statistics derived from Chinese Wikipedia, CC BY-SA 4.0」），并按 CC BY-SA 4.0 提供这两个文件本身；CC BY-SA 4.0 单向兼容 GPL-3.0。
- **`msime-english.db`** 一定和 `msime-scowl_Copyright.txt` 放在一起。
- **`msime-pinyin.db` 的权重**有一部分改编自 rime-wanxiang（CC BY 4.0）：分发它时带上 `licenses/rime-wanxiang-CC-BY-4.0.txt`，或以同样完整的方式给出作者 amzxyz、来源链接、许可证链接和改动说明；再改这些权重时补上你的改动说明。
- **引擎本身仍是 MPL-2.0**，数据不随它改许可证。把这些数据和你的程序一起分发时要承担哪些义务，由你按自己的分发方式判断；不能接受 GPL-3.0 数据的采用方可以给引擎换一套自己的数据，格式见 [docs/data-formats.md](https://github.com/metasequoiaime/msime/blob/develop/docs/data-formats.md)。

`licenses/` 里的文件：

| 文件 | 内容 |
| --- | --- |
| `GPL-3.0.txt` | GNU General Public License v3 全文 |
| `Administrative-divisions-of-China-WTFPL.txt` | 行政区划名称的 WTFPL 全文与来源 |
| `pinyin-data-MIT.txt` | mozillazg/pinyin-data 的 MIT 许可证全文（`msime-pinyin.db` 的单字读音） |
| `ECDICT-MIT.txt` | skywind3000/ECDICT 的 MIT 许可证全文（`msime-english.db` 的中英释义） |
| `rime-wanxiang-CC-BY-4.0.txt` | amzxyz/rime-wanxiang 的署名、权重的改动说明与 CC BY 4.0 全文（`msime-pinyin.db` 里与它共有的词条的权重） |
| `msime-engine-dictionary-NOTICE.md` | msime-dictionary 每个来源的上游、提交与条款（含 SCOWL 声明、中文维基百科的署名要求） |
