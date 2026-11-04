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
  - **第一步总是做**，逐行复现 #84：权重取自 `wanxiang.dict.yaml` 导入的 16 份里除 `cuoyin` 以外的 15 份（不用 `cuoyin` 是 #84 的选择，上游方案导入了它），去调、ü 写 v、取最高权重，只认非负整数权重，乘文件系数（`FILE_SCALES`：9.42、9.2、9.28），整数四舍五入。读源文件的规则与 `msime::insert_quanpin_file` 逐条一致，连检查次序也一致（先要第 2 列、读音不是小写字母开头就跳过、之后才要第 3 列），同一行出现在两份输入里时取较高的换算结果，与 `quanpin` 阶段去重相同。
  - **第二步由 msime-dictionary 的 `curation/pinyin-reweight.txt` 打开**，文件不在就不做：`renming`、`duoyin` 的权重先乘比例再取最高（同一「词＋全拼」在别的词库更高时比例不起作用）；`unattested-cap on` 按方案 2.1「碎片、虚词组合」的口径压碎片：万象全套 17 份（含 `cuoyin`、`mixed`）在任何读音下都不收的多字词，权重不低于同键上「万象原始权重 ≥ `cap-leader-minimum`（默认 100）」的词里换算后最高的那个时，压到它下面；同键被压的几行按原次序依次减 1、减 2（最低到 1），上限不到 2 的键不压，单字不动。`curation/pinyin-reweight-exempt.txt`（`词<TAB>全拼`）列出的行停在第一步的结果；每一条都必须是第二步本来要改的行，豁免表只能和打开第二步的设置表一起存在。表写错、重复、每项都是默认值时构建失败。
  - checkout 的 `upstream.lock.json` 没有记录 `rime-wanxiang`（`dict-v2.0.56` 及更早）时整个阶段不做，产物与没有这个阶段时相同，所以构建器可以先于数据合入。
- **第二步这次不打开。** msime-dictionary 的这条分支只做第一步；发布 workflow 的触发路径加上 `curation/`，发布说明写明 reweight 做了哪一步。

## 第一步为什么不改产物

三份文件里与万象共有的 597343 行（`rime-ice.txt` 521669、`rime-ice-supplement.txt` 30499、`single-chars.txt` 45175）全部等于 `round(万象 × 系数)`，#84 改过的 521502 + 30494 + 45172 行一行不差，另有 175 行碰巧本来就等于换算值。方案文档里写的「514501 行」与这几个数都对不上，按实际逐行复算为准。整数算法与 #84 的浮点算法在这 597343 行上没有一行落在需要分辨 .5 的边界上。去重之后全拼表里与万象共有的是 588636 行，阶段摘要报 `0 realigned`。

## 第二步为什么不打开

第一版第二步（`unattested-cap on`、`renming 0.41`、`duoyin 0.5`）在评测集上是净改善（quanpin-words-v2 全拼非同码桶 top-1 66/13），审查和逐行复算却查出它的口径和参数都不对，而改对之后它在当前数据上已经没有要做的事：

- **上限的口径比方案 2.1 宽。** 第一版只要同键有任何一条万象收录的行就定上限，万象底值 10（换算后 92）也算，于是 咨询有限公司 51835 被 资讯有限公司 的 92 压到 91；收录又是按「词＋全拼」判的，盖茨 gai'ci、长于、调入、下调 这类万象收了词、只是读音不同的行也被当成碎片压下去，等于没经审核就处理了方案 3.2 的「非标准读音」。改成 2.1 的口径（17 份任何读音都不收、领头词万象原始权重 ≥100）后，压的行从 3651 变成 631。
- **这 631 行已经有人逐条审过。** cleanse-data/vetoes-overrides 按同一口径从 cleanse-report 出了 1663 行碎片、逐条审过，1612 行写进 `pinyin-weight-overrides.txt`，51 行审定为误伤、特意不降。631 行里 608 行在那 1612 行里（`weight-overrides` 阶段在 `reweight` 之后、按绝对值覆盖，上限对它们不起作用），剩下 23 行正是那 51 行误伤里的（顾盼、恢宏、等的、电传、之三、汉密尔顿、余华、美杜莎……）。现在打开上限，唯一的效果是把审过、决定保留的词再压下去。上限的长期用处是以后重新生成补充表、新增的行也受约束，到那时要带着这 23 行的豁免一起打开。
- **duoyin 0.5 没有依据。** 用 #84 定文件系数的算法（同一「词＋全拼」在其他词库与 duoyin 的权重之比，两边都大于 10 的 223 行）算出来是 3.98：duoyin 的量级偏低，不是偏高。第一版用的「同一个词另一读音 ÷ duoyin 读音」的中位数量的是读音偏好，不是量级，取倒数等于把偏好又算了一遍；很多词的主读音只收在 duoyin 里（肚子 dù zi、长了、盛放、重活），减半后 肚子 从 du'zi 第 1 掉到第 3。上游 `wanxiang.dict.yaml` 对 duoyin 的说明也是「兼容词库，是基础词库的扩充」。多重、血条 这类错读归方案 3.2，vetoes-overrides 已经逐条审过非标准读音。
- **renming 0.41 要先审。** 系数本身复算得 0.408，算法与 #84 一致，但它改 2864 行，让 60 个键的首选换人（算上 vetoes-overrides 的覆盖之后）：戴维 > 代为、孔蒂 > 空地、杨涛 > 杨桃 这类是改善，李四 < 李斯、林克 < 临客、齐平 < 气瓶、周波 < 周勃、肖恩 < 小恩 这类是误伤。这 60 个键要逐条审、把误伤写进豁免表之后才能打开。

第二步的机制（口径、豁免表、发布说明）都留在构建器里，打开它只需要在 msime-dictionary 加 `curation/pinyin-reweight.txt` 和豁免表。改过的规则在真实数据上试过：msime 分支的构建器对这条词库分支加上 `unattested-cap on`、`renming 0.41` 跑 `quanpin`、`reweight` 两个阶段，摘要是 `2864 rescaled, 635 capped`，全拼表 952506 行与按同一规则用 Python 逐行复算的结果一行不差。

## Alternatives considered

- **只把万象登记进锁，不加阶段，继续靠 #84 写进源文件的权重。** 改动最小，产物同样不变。但它解决不了第一个问题：任何一次重新生成补充表都会把对齐冲掉，而且第二步（降碎片词、给人名单独定系数）没有地方放。
- **把 `rime-ice.txt` 恢复成上游原样，权重全部在构建期从万象算。** 源文件回到「上游原样」最干净。但会丢掉 5375 行万象收录、上游已删的行，`pinyin-reading-corrections.txt` 的 189 条要同步删，是另一项需要维护者拍板的决定（方案第 6 节第 3 条的后半），不该夹在这次改动里。
- **第二步写死在构建器里，不要开关文件。** 少一张表。但第二步是这一轮清洗里风险最高的大规模改权重，回滚要能只动数据仓库：删掉 `curation/pinyin-reweight.txt`（连同豁免表）发一版就回到第一步，不用回滚构建器、不用等客户端。
- **把第一版第二步留在分支上、只补豁免。** 能保住评测上的净改善。但第二步与第一步捆在一条分支里没法分开回滚，must-outrank 也没有全过（车件 > 车检 要靠权重覆盖，挽昼 > 万州 归 H11），而上限要做的事已经由审过的覆盖表做了。
- **renming、duoyin 直接排除，不参与对照。** 最省事。但 renming 里有大量本身就是常用词的行（张开、刘海、张扬、王道、马丁），duoyin 里有 不了 bu'liao、同行 tong'hang、命中 这类正当读音，排除后它们的权重就退回源文件里换算前的值，还可能被当成碎片压下去。
- **豁免写进 `pinyin-weight-overrides.txt` 抬回原位，不另设豁免表。** 少一张表。但覆盖是绝对值，第一步的权重一变（换上游提交、补充表重新生成）覆盖值就过期；豁免表只说「第二步不碰」，权重跟着第一步走。

## Consequences

**收益**：对齐有了生成器，换上游提交就是换文件、改 reference，权重在构建时重算。第一步在真实数据上逐字节不改产物。第二步的口径与方案 2.1、cleanse-report 一致，误伤有豁免表可救，回滚只需删两份数据文件、发布说明看得出这一版做了哪一步。

**代价与已知上限**：

- 第二步的权重只进全拼表。`PinyinCuration::rows`（五笔三个补充表与区划生成器的门槛和同码排序）、`curation::custom_promotions`（判断 custom 是否提了权）和 `cleanse-report` 读的仍是源文件的原始权重。第二步没打开，这个不一致现在不会出现；打开前要让它们改读阶段之后的权重。
- 上限在补充表重新生成之前没有收益，打开它的前提是带上 vetoes-overrides 审定的误伤作豁免；renming 要先审 60 个首选变化的键。复算用的两份清单（上限压的 631 行和它们在覆盖表里的值、renming 首选变化的 60 个键）留在本地评测目录，没有进仓库。

相关：[词库清洗走 curation/ 表](2026-11-04-dictionary-curation-tables.md)定了 `curation/` 的共同规则，这张开关表是它的一张；[万象权重的署名](../process/2026-10-29-rime-wanxiang-weights-attribution.md)定了署名渠道，本次只在署名文件和来源说明里补上构建期重算和第二步的改动说明。
