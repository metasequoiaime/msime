# 测试用的小模型

`lm_step.ort`、`model.json`、`vocab.json` 和 `golden.jsonl` 是 chinese-ime-lm 的 `neural/choice/export_step.py` 导出的一个随机初始化模型（2 层、宽 32、只认 `vocab.json` 里那一百多个字），只用来核对这个 crate 的前缀簿记和缓存：权重是随机的，分数没有语言上的意义。

`golden.jsonl` 是同一个 `lm_step.ort` 在 ONNX Runtime 的 Python 包里逐键跑出的分数：每条用例先把上文编码进空缓存，再把候选截成 1 个字、2 个字……逐页打分，缓存在页与页之间延续。用例是合成的句子，不是真实输入。

重新生成（chinese-ime-lm 仓库里）：

```sh
python neural/choice/export_step.py --random-shape 2x32 --random-chars "$(cat chars.txt)" \
  --out <目录> --golden pool.jsonl --golden-cases 10
```

`pool.jsonl` 是 `golden.jsonl` 里那十条用例的上文和候选，`chars.txt` 是它们用到的字按出现顺序去重。模型的算子和类型与正式模型相同（`required_operators_and_types.config` 一致），所以按正式模型精简构建的 ONNX Runtime 能直接加载它。
