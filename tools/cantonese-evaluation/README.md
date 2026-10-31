# 粤语整词与整句评估

本工具回答 [#10222](https://github.com/metasequoiaime/msime/issues/10222) 的数据评估问题。路线、许可证、验收样例和质量门槛见[评估笔记](../../.agents/notes/proposed/feature/2026-10-30-cantonese-word-sentence-evaluation.md)。只使用 Python 3.10+ 标准库，不修改产品词库、资源锁或引擎。

```sh
python3 tools/cantonese-evaluation/evaluate.py --fetch
cmp tools/cantonese-evaluation/report.json target/cantonese-evaluation/results/report.json
```

首次运行下载 `inputs.json` 固定的六个公开输入，按大小和 SHA-256 校验；约 13.7 MB。缓存位于 `target/cantonese-evaluation/`，之后省略 `--fetch` 即可离线复跑。两份本仓资源锁及词库仓的来源锁也必须与输入一致，锁更新后应重新评估。

输出为 `target/cantonese-evaluation/results/` 下的三个实验 SQLite 和 `report.json`。JSON 的统计项、探针与增量字节来自固定基线；`measurement` 记录 SQLite 版本与基线页大小。提交的结果使用 SQLite 3.54.1、4096 字节页；同环境复跑应逐字节相同。其他 SQLite 版本可能改变压实后的文件大小，比较时要使用该次运行的空增量对照，不应直接把跨版本的文件大小差异当作补词增量。

- 一致性实验仅对已有明确注音的多汉字词面比较：逐字唯一单音节读音，或逐字最高读音份额（去声调合并取最大份额，平局按字典序）。这不是独立标注的错误率，也没有测真实候选排序。
- 补词实验只从 essay 中取基线缺失的多汉字词面，只接受每字恰有一个去声调后的单音节读音；分数门槛分别为 1000、100、1，不修改已有条目。当前门槛 1000 的实验没有新增条目，作为同环境空增量对照。
- `probes` 检查报告例子、口语及多音字的库内条目、essay 分数、语料词频与自动注音。未训练整句模型，未做原生设备验收。

原始词表和实验数据库不提交、不发布。本次只提交分析器、输入摘要及聚合统计；复用数据时保留评估笔记列出的来源、署名、许可和修改说明。
