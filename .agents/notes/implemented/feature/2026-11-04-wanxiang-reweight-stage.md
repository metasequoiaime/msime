# Agent Note: 万象 7ab73ee 登记为上游，构建器 reweight 阶段在构建期重算全拼权重

Status: implemented

## Problem

msime-dictionary#84（提交 `060c102515f176ca3dfaf2a2cba1f320901ef414`）把 `single-chars.txt`、`rime-ice.txt`、`rime-ice-supplement.txt` 里与万象共有的行一次性改成 `round(万象权重 × 文件系数)`，改动直接写进源文件，没有生成器。这带来三个问题：

- 以后在原路径重新生成补充表（`rime-ice-supplement` 生成器）或换上游提交，对齐结果会被冲掉，没有任何东西能把它算回来。
- 仓里 `sources/pinyin/wanxiang/` 收的是 `aa9947f` 的 15 份，#84 实际用的是 `7ab73ee`，少了 `renming`、`lianxiang`，`jichu` 等也不同；拿旧副本判断「万象收不收」会把约 2.5 万行误判为不收（李四、林克、马克）。旧副本也没有登记进任何锁。
- #84 把万象所有词库一视同仁地乘系数：`renming`（正则抽出、未经筛选的人名表）的量级比其他词库同一「词＋全拼」高约 2.45 倍，`duoyin`（兼容读音）也按普通词频进了常用区；万象不收的碎片词（门的 435135 压过 闷的 208546，或在 压过 火灾）照样压在万象收录的词上面，构建链路里没有任何能降权的机制。

## Decision

**数据侧（msime-dictionary）**：`sources/pinyin/wanxiang/` 换成万象 `v18.0.16`（`7ab73ee0c7f80190004233e70f954833df64c80e`）`dicts/` 下除 `abbrev`、`t9_abbrev`、`en` 以外的全部 17 份，逐字节（Git blob 与上游相同），`upstream.lock.json` 以上游名 `rime-wanxiang` 固定每份的大小和 SHA-256。

**构建器侧（msime）**：

- `resources/dictionary-sources.lock.json` 加 `rime-wanxiang` reference，`sources::UPSTREAM_FILES` 登记目录前缀 `sources/pinyin/wanxiang/`；checkout 记录的提交必须等于这个 reference，`scripts/test-language-data-notices.py` 还要求它等于署名文件 `rime-wanxiang-CC-BY-4.0.txt` 写的提交。
- 新阶段 `reweight`（`crates/dict-builder/src/reweight.rs`）在 `quanpin` 之后、`places-supplement` 之前，只改权重，不增删行、不改行序：
  - **第一步总是做**，逐行复现 #84：读 `wanxiang.dict.yaml` 导入的 15 份（`cuoyin`、`mixed` 只校验字节），去调、ü 写 v、取最高权重，只认非负整数权重，乘文件系数（`FILE_SCALES`：9.42、9.2、9.28），整数四舍五入。读源文件的规则与 `msime::insert_quanpin_file` 逐条一致，同一行出现在两份输入里时取较高的换算结果，与 `quanpin` 阶段去重相同。
  - **第二步由 msime-dictionary 的 `curation/pinyin-reweight.txt` 打开**，文件不在就不做：`renming`、`duoyin` 的权重先乘比例再取最高（同一「词＋全拼」在别的词库更高时比例不起作用）；`unattested-cap on` 让 15 份都不收的多字词（没写权重的万象行也算收录）权重不高于同一全拼键上万象收录的最高词减 1，单字不动。表写错、重复、每项都是默认值时构建失败。
  - checkout 的 `upstream.lock.json` 没有记录 `rime-wanxiang`（`dict-v2.0.56` 及更早）时整个阶段不做，产物与没有这个阶段时相同，所以构建器可以先于数据合入。
- 第二步的参数是 `unattested-cap on`、`renming 0.41`、`duoyin 0.5`，写在 msime-dictionary 的 `curation/pinyin-reweight.txt` 里：`renming` 用的是 #84 定文件系数的同一个算法（与其他 14 份同一「词＋全拼」、两边都大于 10 的 267 行上，其他词库权重之和 ÷ renming 权重之和 = 0.408）；`duoyin` 取同一个词在其他词库里另一读音与 duoyin 读音权重之比的中位数 2.1 的倒数。

## 第一步为什么不改产物

三份文件里与万象共有的 597343 行（`rime-ice.txt` 521669、`rime-ice-supplement.txt` 30499、`single-chars.txt` 45175）全部等于 `round(万象 × 系数)`，#84 改过的 521502 + 30494 + 45172 行一行不差，另有 175 行碰巧本来就等于换算值。方案文档里写的「514501 行」与这几个数都对不上，按实际逐行复算为准。整数算法与 #84 的浮点算法在这 597343 行上没有一行落在需要分辨 .5 的边界上。去重之后全拼表里与万象共有的是 588636 行，阶段摘要报 `0 realigned`。

## Alternatives considered

- **只把万象登记进锁，不加阶段，继续靠 #84 写进源文件的权重。** 改动最小，产物同样不变。但它解决不了第一个问题：任何一次重新生成补充表都会把对齐冲掉，而且第二步（降碎片词、给人名单独定系数）没有地方放。
- **把 `rime-ice.txt` 恢复成上游原样，权重全部在构建期从万象算。** 源文件回到「上游原样」最干净。但会丢掉 5375 行万象收录、上游已删的行，`pinyin-reading-corrections.txt` 的 189 条要同步删，是另一项需要维护者拍板的决定（方案第 6 节第 3 条的后半），不该夹在这次改动里。
- **第二步写死在构建器里，不要开关文件。** 少一张表。但第二步是这一轮清洗里风险最高的大规模改权重，回滚要能只动数据仓库：删掉 `curation/pinyin-reweight.txt` 发一版就回到第一步，不用回滚构建器、不用等客户端。
- **renming、duoyin 直接排除，不参与对照。** 最省事。但 renming 里有大量本身就是常用词的行（张开、刘海、张扬、王道、马丁），duoyin 里有 不了 bu'liao、同行 tong'hang、命中 这类正当读音，排除等于把它们当成「万象不收」，会被上限压下去，误伤比降系数大得多。duoyin 取 0.25 时 命中 掉到 鸣钟 后面、落下 掉到 落霞 后面，所以没有取更小的值。

## Consequences

**收益**：对齐有了生成器，换上游提交就是换文件、改 reference，权重在构建时重算。第二步在 C=试跑产物、B=`dict-v2.0.56` 上过了方案 4.3 的硬门槛（模型臂 / 无模型臂，非同码桶，top-1 fixed/broken）：quanpin-words-v2 全拼 66/13、66/13，九键 23/4、30/6，14 键 21/11、29/10，top-5 没有一臂 broken 多于 fixed；ctx-v1 sampled 选词三种打法 top-1 全部 0 broken（全拼、混打各修好 1 条）；粗俗首屏曝光 C 与 B 相同（全拼 6 个输入、九键 0）；全拼表 954115 行一行不少；must-outrank 从 1/6 过到 3/6（闷的 > 门的、火灾 > 或在 转绿）。产物只改 `msime-pinyin.db` 7019 行的权重和由它定先后的 `en_zh_glosses` 31 条释义次序。

**代价与已知上限**：

- must-outrank 还有 3 行红：车件 > 车检（mo-0001、mo-0002）是上游 rime-ice 原值，万象不收 车件 但它低于同键的 车间，上限管不到，要靠方案 3.1 的权重覆盖；挽昼 > 万州（mo-0005）是 custom 投稿词，归 H11。这三行在 B 上同样红，第二步没有弄坏任何一行。
- 上限会误伤万象漏收的正当词。全拼的 13 条 broken 里，上调 打 shangdiao 时让位给 上吊 是方案 2.1 认定的方向（shang'diao 是非标准读音）；其余大多是这种误伤：便函 < 便喊、顾盼 < 骨盘、纵谈 < 总坛、下调 < 瞎掉。它们在评测上被修好的 66 条抵过，但要修就得进 `curation/pinyin-weight-overrides.txt` 逐条抬回去。人名降系数后 李四 退到 李斯 后面、林克 退到 临客 后面，这类在评测集里量不到。
- 第二步的权重只进全拼表。`PinyinCuration::rows`（五笔三个补充表与区划生成器的门槛和同码排序）、`curation::custom_promotions`（判断 custom 是否提了权）和 `cleanse-report` 读的仍是源文件的原始权重，所以五笔码表不随第二步变化，清洗报告里的碎片词清单会包括已经被上限压下去的行。打开第二步时这几处与全拼表不一致是已知的，要一致就得让它们改读阶段之后的权重。
- 参考门槛里没跑 typing_sim、sentences-v1、九键长句、particles、neutral；整句只量了 ctx-v1 的分句子集（top-1 模型臂 0/0、无模型臂 5/4）。

相关：[词库清洗走 curation/ 表](2026-11-04-dictionary-curation-tables.md)定了 `curation/` 的共同规则，这张开关表是它的一张；[万象权重的署名](../process/2026-10-29-rime-wanxiang-weights-attribution.md)定了署名渠道，本次只在署名文件和来源说明里补上构建期重算和第二步的改动说明。
