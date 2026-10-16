# Agent Note: 九键读音段数按 ASCII 分隔字节计数

Status: implemented

## Problem

固定 `b3ba3480b009f7c6d0e7f1d52022811a9b084586` 的九键排名、简拼判定、合成行权重校准及双拼读音长度使用 `split` 统计段数，刷新前置过滤使用 `matches` 统计分隔符。旧真实九键采样中 `rank_key_for_reading` 直接调用 `CharSearcher`，显示这条字符搜索路径仍有实际成本；该采样有观察与外部构建干扰，只定位调用链，不作为本片延迟成绩。

本片修改的四个旧表达式：`KeyGrid::reading_code_len`的`2 * canonical_pinyin.split('\'').count()`、刷新前置过滤的`canonical.matches('\'').count() >= full_len`、`comparable_weight_for_reading`的`canonical.split('\'').count()`、`is_initials_reading`的`canonical.split('\'').count() == digits`。

## Decision

用九键模块私有 `reading_syllable_count` 统计ASCII撇号字节数加一，替换上述段数计算；前置过滤的“撇号数大于等于full_len”等价改为“段数大于full_len”。保持空串为一段、首尾与连续分隔符产生空段，以及所有Unicode字节不作为ASCII撇号的语义。不分配、不扩容、不缓存、不改变排序键或候选布局。

## Historical audit

- [学习段数](../../implemented/testing/2026-10-08-learning-sentence-segment-count.md)与 `shuangpin::engine::segment_count` 提供同仓库字节计数范式，但空串返回零，不可直接复用，否则九键双拼读音长度与空读音排名改变。
- [九键缓存排名键](../../implemented/testing/2026-10-16-nine-key-cached-rank.md)：部分重叠，只降低每次求键成本，保留32条门槛与排序/去重/交错/调频策略。
- [组码前缀](../../implemented/testing/2026-10-16-nine-key-code-prefix.md)：只管相容覆盖长度；不改其短路、非法音节与双拼完整编码边界。
- [领先表负结果](../../rejected/testing/2026-10-17-nine-key-ranked-leaders.md)：部分相关，继续否掉扩大map复用获胜下标；本片不重试该结构方案。
- 检索proposed/implemented/rejected里的音节计数、split/count、reading_code_len、字节计数；其他结果涉及词库格式、学习持久化或其他平台，无关，没有完全吸收或过时提案。

[Rust split](https://doc.rust-lang.org/std/primitive.str.html#method.split)保留字符分隔的首尾空段，[字节视图](https://doc.rust-lang.org/std/primitive.str.html#method.as_bytes)暴露原UTF-8字节；ASCII撇号不可能作为多字节码点的内部字节。本片不把全角或弯引号当分隔符。

## Alternatives considered

- 维持split计数：语义直接且无申请，但在热点中反复创建字符搜索状态。完整旧实现保留为冻结对照。
- 复用现有学习/双拼segment_count：减少helper，但空串零段与此处原split的一段不一致，不能共享不同契约。
- 同时统计小写字母与分隔符：可减少扫描次数，但会让原来短路掉的分支也工作，增加本片变量；先只改变已确认字符搜索路径。
- 缓存每条读音的段数：重复行可能省扫描，但增加布局或状态，上一片已显示元数据成本可能抵消收益；本片不新增状态。

## Verification protocol

先冻结完整旧push/rank及其传递调用的读音长度、权重、简拼判定、交错和调频helper，再在旧生产入口跑同一完整配对预算，实际观察预算失败。行为对照覆盖空串、纯分隔、首尾连续分隔、ASCII大小写、Unicode、超长合成读音；全部来源、权重、覆盖长度、prefer_exact/initials_lead、九键/十四键/三个双拼方案；全字段、容量、同分、同词改善、简拼交错及调频不变，分配次数和峰值不得增加。

完整流水线配对包含输入字段复制、push、map申请/析构、排名/后处理/结果析构。沿用126形状：7长度（8/32/33/64/128/512/2048）×3模式（混合全唯一/同键全唯一/17词连续改善）×3网格×2最终过滤；21对交替先后，旧臂校准至少10ms。无最终过滤且长度至少64的36个目标中位新旧比全部<0.98，其余90个控制全部<1.05。普通System分配器、无并行构建/采样，每对原始数据全保留，收集完整矩阵后统一判定，不删形状/放宽/重跑找绿。

固定模型质量比较、实际九键逐键计时与仓库门禁单独记录；合成流水线收益不能代替16ms逐键目标。

## Consequences

短读音字节循环不一定比标准库搜索快；编译器向量化与内联可能使个别形状回退。基准冻结范围必须覆盖所有改动的传递调用，不能让两臂共用新版helper掩盖成本。空串段数必须保持一，前置过滤不得产生off-by-one。


## Paired results

[原始记录](../../../../docs/measurements/2026-10-17-nine-key-syllable-byte-count.csv)保存6轮完整矩阵，包含每轮126个中位统计和2,646对原始耗时，没有删除污染轮或失败形状。`grid` 0/1/2为九键/十四键/Migala2，`mode` 0/1/2为混合全唯一/同键全唯一/17词连续改善；目标是长度至少64且`filtering=false`，其余为控制。

| 轮次 | 并行构建观察/峰值 | 目标失败/36 | 控制失败/90 | 采信 |
| --- | --- | --- | --- | --- |
| red-paired | 508/13 | 35 | 0 | 否，污染 |
| red-clean-paired | 1111/21 | 34 | 0 | 否，污染 |
| red-quiet-paired | 46/7 | 36 | 0 | 否，污染 |
| red-final-paired | 3/1 | 36 | 0 | 否，污染 |
| red-verified-paired | 0/0 | 36 | 0 | 是，旧生产失败对照 |
| new-paired | 0/0 | 0 | 0 | 是，新生产 |

污染时不修改门槛或矩阵，保留同一旧二进制，在安静环境完整重测。新生产只有一轮正式完整计时。采信旧生产目标中位比范围0.995456–1.002724、控制0.985337–1.011817；新生产目标0.859138–0.978177（节省2.18%–14.09%），控制0.628984–0.984240。数字只描述本机合成完整入列加排名，不能外推为整体逐键收益。

正式计时的测试二进制使用普通System分配器：构建时只临时移除`ime/personal_rerank.rs`的`#[global_allocator]`，通过`try/finally`逐字恢复。旧测试二进制SHA-256为`6976eaf4b1a509cb3bc69d0f87f0294b1ae6f6d2e26c035550256de5af32b0a3`，新为`8264f2ecf1554158e403c986633af01be044ca01199414714f3638f35f9da0e0`；恢复的分配器源码SHA-256为`4cbb8b21744f1d395a559957e5b21f98b01e988e16f73a3dce2a81d9a28a9552`，最终与基线无差异。每秒监测进程名`rustc/clang/clang++/cc1/cmake/ninja/swift-frontend`；该监测只排除编译污染，不能排除操作系统或其他应用噪声。

## Behavior and model verification

新增5项行为测试通过：完整字段与容量、各排名维度、真实交错/个人调频、全部长度分配次数及峰值相等、空段与Unicode及前置过滤边界。计数器分配器只用于行为测试，不用于上面的正式计时。性能测试保持手动忽略，避免把机器相关阈值混入普通单测。

设置`MSIME_NEURAL_MODEL_DIR`到固定句子模型目录后，`cargo test --locked -p msime-engine -p msime-input-runtime --no-fail-fast --quiet`通过2,247项引擎单测、31项golden与317项runtime单测，39项保持忽略。此命令没有设置`MSIME_EVAL_RESOURCES`，四项真实词库golden按已有逻辑跳过；不能把31个通过理解为旧词库回放也验证过。全目标引擎Clippy `-D warnings`及Rustfmt通过。

资源配置失败的两轮也保留在本地记录：把只含模型/拼音/英语/五笔的评测目录误传给`MSIME_EVAL_RESOURCES`时缺少n-gram和日语模型，5项单测与4项真实golden失败；换成当前`dict-v2.0.24`完整缓存后，日语词条数为1,285,198而旧断言要求1,284,987，四项旧golden也与新词库不一致。固定旧生产System二进制在同一完整缓存独立复现日语词条数断言失败。这些轮次不充当验收通过，也不修改既有断言或质量基线。


## Reproduction

行为与计数器分配验证：`cargo test --locked -p msime-engine --lib nine_key::syllable_count_tests -- --test-threads=1`。正式计时必须使用普通System分配器，不能直接用默认计数器版本的`cargo test --release`成绩代替。以下脚本从Cargo消息取得测试程序、复制到普通`target/`，并在构建失败时也恢复源码；运行期间不要并发编辑该分配器文件。

```python
import json
import shutil
import subprocess
from pathlib import Path

source = Path("crates/engine/src/ime/personal_rerank.rs")
original = source.read_bytes()
marker = b"#[global_allocator]"
assert original.count(marker) == 1
try:
    source.write_bytes(original.replace(marker, b"#[allow(dead_code)]"))
    output = subprocess.check_output([
        "cargo", "test", "--release", "--locked", "-p", "msime-engine",
        "--lib", "--no-run", "--message-format=json",
    ], text=True)
    artifacts = [json.loads(line) for line in output.splitlines() if line.startswith("{")]
    binaries = [item["executable"] for item in artifacts
                if item.get("reason") == "compiler-artifact"
                and item.get("executable") and item["target"]["name"] == "msime_engine"]
    assert len(binaries) == 1
    shutil.copy2(binaries[0], "target/nine-key-byte-count-system-tests")
finally:
    source.write_bytes(original)
    assert source.read_bytes() == original
```

构建结束、源码恢复且没有其他编译/采样后执行：

```sh
target/nine-key-byte-count-system-tests --exact nine_key::syllable_count_tests::full_pipeline_paired_budget --ignored --nocapture --test-threads=1
```

重现RED时只把四处生产调用改回本篇Problem所列的原`split`/`matches`表达式，保留新测试、注册及只被边界测试使用的计数helper，沿用完全相同的构建与计时步骤；不用另一份不完整冻结参考。正式旧二进制在新增直接helper边界断言之前构建，保留的是当时完全未改的生产入口。性能验收不通过时收完整矩阵后报告失败，不删形状或改阈值。


## Fixed resource output comparison

同一词库/模型目录下分别执行`convert_eval --set resources/eval/sentences-v1.tsv`（60例）与`--set resources/eval/sentences-nine-key-v1.tsv --grid nine`（81例），每臂均生成`--report` JSON和`--dump` JSONL，载入后整体相等；指标、来源分桶、失败样例与每例全部候选文字/来源/顺序均不变，不只比较top1。没有启用`--keyboard-neural`，也没有改质量基线。句子模型SHA-256仍为`86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1`，模型依赖pin保持`6acc7839572fb7df221a8e4910467c9dedd72fb8`。

旧轮的`convert_eval`/`rerank_latency`二进制虽来自源码等价提交，却在运行时读取已经清理的编译worktree里的`desktop-dictionary.lock.json`，因此不能直接复用。实际比较从本worktree的固定基线`b3ba3480b009f7c6d0e7f1d52022811a9b084586`重新构建旧examples，前后锁文件/依赖不变；优化源码通过finally逐字恢复。实际旧/新`convert_eval`二进制SHA-256分别为`2832e4b666d2fe6693fdd01153af1037e4fa7f27a5fc4a15589f610318f14c38`、`49307f8cc250f88677d0d08b1d9b989adcf4e51cf9786e8d73f22f616e34e645`。

评测目录输入指纹（两臂相同）：

| 文件 | SHA-256 |
| --- | --- |
| `msime-english.db` | `83f8c2fe7a33f9797392cd914056cc97fc6e62480c2d787e17019ce6daddba8a` |
| `msime-pinyin.db` | `495f9c5a736a70ed03369c80b0be556094ae44a3a673788ae32036174f63d154` |
| `msime-wubi.db` | `851ed68211311a76847fe6a26096702747abbaab8dffb141698e0365143b31c1` |
| `sentence-model.safetensors` | `86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1` |


## Runtime latency

固定`resources/eval/sentences-nine-key-v1.tsv`的81例（1,791键）、`--grid nine`、5个预热用例，按旧、新、新、旧串行测量；四轮前后没有编译进程，二进制SHA不变。四轮都超过16 ms预算，不能宣称整体目标达成。

| 轮次 | engine mean/p95 ms | with model mean/p95 ms | reranker adds mean/p95 ms | 超16 ms |
| --- | --- | --- | --- | --- |
| 旧1 | 3.50 / 6.73 | 6.50 / 17.34 | 3.00 / 12.01 | 119/1,791 |
| 新1 | 3.44 / 6.64 | 6.44 / 17.29 | 3.00 / 12.05 | 120/1,791 |
| 新2 | 3.51 / 6.74 | 6.50 / 17.28 | 2.99 / 11.95 | 120/1,791 |
| 旧2 | 3.49 / 6.64 | 6.43 / 16.83 | 2.95 / 11.61 | 114/1,791 |

旧二进制`1a21b3d762c9291adcd4b68a361e8de723d7f26c1325f70e5bc285de093588f6`，新二进制`a88cb427ad8a488c780f3f2e8e4617394f325aebad570e14200b80b7ca226c69`。该ABBA仅用于真实九键路径回归观察；合成完整流水线的稳定收益不外推成端到端16ms收益。

仓库`verify-local.sh --quick`通过：notes 1,204篇、Rust workspace、Android target host、WASM、macOS原生宿主、Apple bridge与可用契约检查通过；Docker桌面/ Linux、MinGW Windows、HarmonyOS与Windows pipe-only阶段按缺少环境跳过。`pnpm run verify-notes`在最终笔记和互链修改后再次通过。
