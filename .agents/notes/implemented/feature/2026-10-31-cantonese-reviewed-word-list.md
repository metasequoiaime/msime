# Agent Note: 粤语词库收录人工审校词表 custom/cantonese.txt

Status: implemented

## Problem

#10369 报告粤语双拼打 `zk gt lp` 出不来「周杰倫」，要先选「周」再选「杰倫」。固定的 `dict-v2.0.41` `msime-cantonese.db` 里没有这个整词：rime-cantonese 的 words 文件不收它，essay 给「周杰倫」711 分，低于构建器自动补词的门槛 `ESSAY_WORD_MIN_COUNT`（1000）。实测这份库里 `zkgtlp` 的前六个候选是「就、走、酒、找、洲、抓」，「周」不在其中。

[粤语整词与整句的分阶段评估](../../proposed/feature/2026-10-30-cantonese-word-sentence-evaluation.md)的路线 1 建议先做经审校、逐词明确注音的小批扩充，并否掉了整体降门槛（降到 100 多出 68,136 条、约 3.465 MiB）。可当时没有任何能放这种词的地方：粤语方案只读 `msime-cantonese.db`，不学习，没有用户词；msime-dictionary 的 `custom/` 也没有粤语文件，check-words 只认拼音、翻译和英文三份。

## Decision

- msime-dictionary 新增 `custom/cantonese.txt`，每行 `词<TAB>带声调粤拼`，一个汉字一个音节、音节间一个空格，`#` 行与空行跳过，写繁体字形，只追加。第一行是「周杰倫	zau1 git6 leon4」。这份文件是本项目自己的数据，以 CC BY 4.0 提供（`custom/` 其余文件是 GPL-3.0），这样 `msime-cantonese.db` 的 `license` 仍是单一的 `CC-BY-4.0`；署名写进 `resources/licenses/rime-cantonese-CC-BY-4.0.txt`，`scripts/test-language-data-notices.py` 要求那份文本保留 `custom/cantonese.txt` 这一段。
- `msime-dict-build languages` 读它（`cantonese::CUSTOM`、`cantonese::custom_source`、`cantonese::parse_custom`）。每一行必须满足：两个及以上汉字；音节数等于字数；每个字去声调后的音节是 `jyut6ping3.chars.dict.yaml` 给这个字的读音之一；同一个词在同一个去声调键下不出现两次。任何一行不满足，整次构建失败并写出行号。
- 收录的词在 words、essay、HKCanCor 三轮之后并入：权重取 essay 分数（「周杰倫」711），essay 没列时取 `ESSAY_WORD_MIN_COUNT`；已有条目只升不降，什么都不删。`Dictionary::custom_words` 记新增或抬高了多少行，写进构建摘要。
- checkout 里没有这个文件（加它之前的 msime-dictionary 提交）时按空表构建，摘要写「custom/cantonese.txt is not in the checkout」。用加入前的提交构建出的库与 `dict-v2.0.41` 逐字节相同。
- `check-words` 新增 `Kind::Cantonese`：`--cantonese-base`/`--cantonese-head` 必须和 `--cantonese-characters`（checkout 的字表）一起给，新行走构建同一套解析与读音检查；判重键是（去声调粤拼，词），只差声调的两行算重复；给了 `--cantonese-db` 时，已发布库里同键同词的条目也拒绝。报告里写原样的带声调粤拼。base 文件先整体走一遍 `cantonese::parse_custom`：上游字表换版后旧行的读音不在了，或有人绕过门禁写进两行只差声调的同词，构建会失败，check-words 此时也以退出码 2 停下，而不是继续把投稿判为通过。

## 读音检查管到哪里

读音检查拦得住笔误和字根本没有的读音（「倫」写成 `lun4`，字表只给 `leon4`），拦不住在多音字的几个读音里选错：「銀行	ngan4 haang4」能通过，因为「行」确实有 `haang4`。评估笔记里「銀行」「長大」那一类错误只能靠人工审校。曾考虑「words 文件已收的词只能用 words 文件的读音」作为第二道检查，它能拦下「銀行」，但也会挡住上游漏收的口语读音，而且 check-words 还要再多读一份文件；这次没有做，留给批量扩充时按实际误注情况再定。

## 跨仓顺序

msime-dictionary 每次合入 `main` 都用 msime `develop` 的构建器发布 `dict-v*`。构建器容忍文件缺失，所以两边谁先合都不会让发布失败：先合 msime 时文件不存在、库不变；先合词表时旧构建器不读它、库也不变。词表要等两边都合入后的下一次发布才进 `msime-cantonese.db`；开了词库自动更新的设备随之拿到，新装与关了自动更新的设备要等 `resources/language-dictionaries.lock.json` 换到那次发布。check-words workflow 要在 msime 合入后把 `custom/cantonese.txt` 加进触发路径和参数，并把固定的 `MSIME_COMMIT` 移到合入提交，之后投稿才有门禁。

## Alternatives considered

- **把补词门槛从 1000 降到 711 或更低**：一行代码，所有 essay 词按同一规则进库。但「周杰倫」只是众多 711 分附近的词之一，评估笔记已量过降到 100 会多 68,136 条、3.465 MiB，而且多读音字的词照样进不来；为一个名字移动全局门槛，等于不经审校地放进几千条。
- **在引擎里对「周杰倫」或这组双拼码特殊处理**：最快，但引擎会多出一份不在词库里的数据，其他方案和其他词得不到同样的处理，维护者在 issue 里已明确不这样做。
- **把文件放进 msime 仓库的 `resources/dictionary-sources/`**：不需要跨仓协调。但词条投稿与审校的入口一直是 msime-dictionary 的 `custom/`（官网投稿、check-words 都在那边），msime 侧只放构建规则；放回 msime 会让粤语词和其他投稿走两条路。
- **沿用 `custom/` 的 GPL-3.0**：与目录里其他文件一致。但那样 `msime-cantonese.db` 的 `license` 要改成 `CC-BY-4.0 AND GPL-3.0-only`，随库许可证文本要再附 GPL 声明，所有平台的打包与通知检查都要跟着动；改用 CC BY 4.0 只需在现有许可证文本里加一段署名。
- **构建器要求文件必须存在**：缺了就报错，能防止路径写错后静默丢表。但 msime-dictionary 的发布用的是 msime `develop`，两个仓的合入顺序一旦颠倒就会让下一次词库发布失败，旧提交也无法再用新构建器重建；改为缺失时在摘要里写明。

## Consequences

- 收益：「周杰倫」进库后，`zkgtlp` 在独立双拼入口和自动识别入口的首选都是「周杰倫」（覆盖三个音节，排在只覆盖一个音节的单字前面），`zau/zaau × git/gek × lun/lut/lung/leon` 的组合里没有别的三音节条目和它竞争。以后加粤语词有了固定入口和与构建一致的门禁，不必再动门槛或引擎。实测新库与不带词表的库同为 7,237,632 字节，只多一条记录。
- 代价：多了一份跨仓文件契约（路径、格式、许可），改它要两边一起改；多音字读音选错仍需人工把关；这个 PR 合入后用户还拿不到这个词，要等词表合入、新的 `dict-v*` 发布，未开自动更新的设备还要等换锁，所以这个 PR 只写 `Refs #10369`，由之后把 `resources/language-dictionaries.lock.json` 换到含「周杰倫」那次发布的 PR 写 `Fixes #10369`。
- 后来个人词库也能加粤语词，只在本机、不进发布词库，见[粤语个人词条](2026-11-02-cantonese-personal-words.md)；给所有人用的词仍走这份审校词表。
- 这次只收「周杰倫」一个词，「周傑倫」等异体与评估笔记建议的首批审校词（上傳、我打你等）都没有加入，需要粤语母语者逐条审校后再追加。
- 验证：`cargo test -p msime-dict-builder`（解析、读音检查、权重只升不降、文件缺失、check-words 的追加与判重、CLI 参数约束）；`cargo test -p msime-engine --lib cantonese` 的 `a_reviewed_three_syllable_word_comes_first_for_its_codes` 固定了有无整词时 `zkgtlp` 的候选顺序；用真实 msime-dictionary checkout 跑 `languages` 并用引擎打开产出的库核对过首选。没有在模拟器或真机上验收。
