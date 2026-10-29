# 这两个评测集的上游

`quanpin-words-v1.tsv` 与 `sentences-v1.tsv` 同时存在于 [chinese-ime-lm](https://github.com/metasequoiaime/chinese-ime-lm/tree/main/eval)。

这里保留一份副本，因为 `convert_eval` 要用真实引擎和真实词库跑它们——那是本仓库特有的验收路径，不该依赖网络。上游那份是给其他输入法用的。

**新增用例请提到上游**，再同步回来。反向操作会让两份悄悄分叉，而分叉的评测集比没有评测集更糟：两边都报数字，却不再可比。

## 打字流评测集的来源

这两份只在本仓库里，`typing_sim` 用（见 README「打字流评测」）：

- `typing-tatoeba-v1.tsv`：[Tatoeba](https://tatoeba.org) 的普通话句子，许可 [CC BY 2.0 FR](https://creativecommons.org/licenses/by/2.0/fr/)。取自 2026-10-24 的导出 `https://downloads.tatoeba.org/exports/per_language/cmn/cmn_sentences_detailed.tsv.bz2`（sha256 `7af009b234bbb21ed6a254c2b55fa7c81c9dd53509d0982cd92c1097573f9af5`），抽样规则写在文件头。署名随文本走：文件头写明许可的地址和本仓库做的改动（抽样、分 warmup/dev/test、加切词和拼音的 `tokens` 列、人工改正部分读音；`text` 列是原句，只去掉首尾空白），每行最后一列 `author` 是导出文件第 4 列里这句的 Tatoeba 用户名，id 去掉 `t` 就是句子编号，原文在 `https://tatoeba.org/sentences/show/<编号>`。`typing_sim` 不读 `author` 列。
- `typing-chat-v1.tsv`：为本仓库手写的单人聊天记录，源文件是同目录的 `typing-chat-v1.txt`，随仓库以 GPL-3.0 发布。
- `typing-readings-reviewed.tsv`：两份集合人工改正的读音。

两份都不用 LCCC：CDial-GPT 写明 LCCC「仅限科研用途」，不能进仓库。也不是从 `chinese-ime-lm` 训练模型用的 C4、LCCC 里取的；拿 8 个汉字的窗口去对 `chinese-ime-lm/data/c4.txt` 和 `lccc.txt`，Tatoeba 的 1074 句计入行里有 59 句（5.5%）、聊天的 249 行里有 11 行（4.4%）碰上，碰上的都是「我也不知道为什么」「的时候给你打电话」这类常用说法，不是同一篇文字；和仓库里已有的整句评测集一个窗口也不重合。
