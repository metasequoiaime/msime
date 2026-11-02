# Agent Note: 粤语香港字形的候选异体对改读 HKVariants，加人工否决表

Status: implemented

## Problem

#10697 报告粤语双拼打 `SD`（sap）只出「溼」不出「濕」，`SDZIGF` 只出「溼紙巾」。根因和 #10420 的 為/爲 一样，见 [粤语词库按 HKCanCor 换用香港字形](2026-10-31-cantonese-hong-kong-forms.md)：那次的替换规则只从 OpenCC `TWVariants.txt` 取候选异体对，而臺灣也写「溼」，这一对根本没进候选。#10711 先在构建器里加了一个只有一对的常量 `HONG_KONG_ONLY_VARIANTS = [('溼', '濕')]` 补上它。这种逐对手列的办法每个漏掉的异体都要等一次反馈、改一次构建器代码。

维护者 2026-11-03 决定换成通用机制：把 OpenCC `HKVariants.txt` 也作为候选异体对的来源，仍按原来的 HKCanCor 证据规则判定，另加一份人工否决表，至少否决「搵」（保留「搵」，不换成「揾」）；「菸→煙」接受。

同一位报告人随后提了 #10725（「啓和啟兩者是同一字，但是啟字用得會多一點」）。「啓→啟」在 `TWVariants.txt` 里本来就有，#10451 起已经按规则替换：`dict-v2.0.50` 的 `kai` 下「啟」1924 排第一、「啓」1154 排第五，含「啓」的词全部写成「啟」；`dict-v2.0.48` 及更早的库还是「啓」1155 排第一，报告人看到的是旧库。

## Decision

- msime-dictionary 原样收录 OpenCC 固定提交 `26753884f1984add422f3b0249ccee8613deaff6` 的 `data/dictionary/HKVariants.txt`，放在 `sources/opencc/HKVariants.txt`（1001 字节，SHA-256 `e5cd4345303224587102f2c9e4d2b67d2b7e349c6ce9152e4a118f4656cf7302`），按 OpenCC 上游登记进 `upstream.lock.json`。它只由 `languages` 构建读取，不在 msime 的 `resources/opencc.lock.json` 里，不内嵌进客户端。
- 否决表是 msime-dictionary 的 `custom/cantonese-variant-vetoes.txt`：每行一个 OpenCC 字形，`#` 开头是注释。构建器已经从 `custom/` 读人工审校词表，否决表放在同一处，维护者改它不用动 msime 的代码。它由维护者审定，不接受投稿，也不受 `custom/` 其他文件的只追加规则限制，撤销否决就删掉那一行。现在只有「搵」。
- 构建器（`crates/dict-builder/src/cantonese.rs`）：
  - 常量 `TW_VARIANTS`（原 `VARIANTS`）、`HK_VARIANTS`、`VARIANT_VETOES`；`HONG_KONG_ONLY_VARIANTS` 删除。
  - `parse_variants(name, source)` 解析两张表，报错时写出是哪张表。
  - `candidate_variants(tw, hk, vetoes)` 把两张表合成一份候选：`TWVariants.txt` 的行在前、顺序不变；`HKVariants.txt` 里臺灣表没有的键按原顺序接在后面；两张表都有的键合成一行，臺灣表的值在前，重复的值只留一个，所以 `hong_kong_forms` 遇到并列时仍优先臺灣表的值。否决表里的键整行去掉。
  - `parse_variant_vetoes` 要求每行正好一个字、不重复；`candidate_variants` 要求每个否决的字都是两张表里某一行的键，否则构建失败，笔误不会让否决悄悄失效。
  - 判定规则 `hong_kong_forms` 不变：键在 HKCanCor 多字词里出现过就不换，否则换成值里次数最多的那个。
- `languages.rs` 用 `Sources::pinned` 读两张异体表和否决表，三个文件都必须在 checkout 里。旧的 msime-dictionary 提交没有它们，构建在读文件时就失败并写明缺哪个文件，不退回只读 `TWVariants.txt`（理由见 Alternatives）。构建摘要在替换清单后面写出否决表留下的字（`kept by custom/cantonese-variant-vetoes.txt: 搵`）。
- `EXPECTED` 新增三项，`verify` 每次发布都检查：(`sap zi gan`, 濕紙巾, 1)、(`kai`, 啟, 3)、(`wan`, 搵, 1)。最后一项守住否决表：「搵」一旦被换成「揾」，`wan` 的第一位就变成「揾」，发布构建失败。

## 实际替换的异体对

用 msime-dictionary 分支 `fix/cantonese-hk-variants-10697`（`92eed36`，即 `main` `27d4289` 加上 `HKVariants.txt` 与否决表）构建，摘要列出 15 对：`啓→啟 1, 峯→峰 2, 溼→濕 4, 爲→為 505, 牀→床 2, 祕→秘 20, 纔→才 1, 羣→群 1, 脣→唇 5, 菸→煙 6, 蔘→參 15, 衆→眾 14, 覈→核 10, 鍼→針 4, 麪→麵 7`。与 #10711 之后的 14 对相比只多了 菸→煙；搵→揾 按语料该换（`揾` 34 次、`搵` 0 次），被否决表留下。`HKVariants.txt` 里其余的对：說/説、溫/温、戶/户、衛/衞、線/綫、稅/税、脫/脱 等语料照用 OpenCC 字形，不换；僞/偽、竈/灶、糉/粽、鉢/缽 等两形语料都没有，不换；啓 一行的值先列它自己（`啓 啓 啟`），与臺灣表合并后是 `啟 啓`，结果仍是 啓→啟。

## 构建结果

与已锁定的 `dict-v2.0.50`（`1f8c372f…`，构建器早于 #10711）逐行比较：音节 729 不变；条目 139,354 → 139,346（−8）。所有变化都落在 溼/濕 与 菸/煙 上，其余条目的字面和权重完全一致：

- 溼/濕：和 #10711 相同。含「溼」的 170 条改写成「濕」；`sap` 前三是 十 80444、濕 49742、溼 49741；`sap zi gan` 只有「濕紙巾」12。
- 菸/煙：含「菸」的 188 条里，187 个词改写成「煙」，其中 8 个和库里已有的「煙」字词合并（防煙門、戒煙、香煙、煙花、煙海、煙雨、煙霧、煙通），所以条目少了 8 条；单字「菸」保留，权重从 1868 降到 266（它原来的分数主要来自「词带字」下限，词都改写成「煙」后下限归了「煙」），「煙」20644 不变。合并后「戒煙」266 → 1232、「香煙」1334 → 1690。另有 19 个单字的权重因为「词带字」下限的来源词变了而小幅变动（如「囪」622 → 734、「烽」961 → 1192、「霾」363 → 552；「戒」1880 → 1879，因为「戒菸」「戒煙」两个词合成了一个）。
- 「搵」「揾」「啓」「啟」与 `dict-v2.0.50` 完全相同：`wan` 下「搵」525289 排第一，含「搵」的 145 条不变；`kai` 下「啟」1924 排第一，「啓」1154。

msime-dictionary#119 往 `custom/cantonese.txt` 加「濕紙巾 sap1 zi2 gan1」。自 #10711 起词表里的「溼紙巾」就写成「濕紙巾」，权重取 essay 里「溼紙巾」的 12；人工词按香港字形查到同样的 12，只升不降，不改变输出，所以那个 PR 对构建结果是多余的。

## 合入顺序

先合 msime-dictionary 的 PR：它触发的自动发布用 msime `develop` 的构建器，旧构建器不读这两个新文件，产出和 #10711 之后一样（已经含 溼→濕）。再合 msime 的 PR，然后手动重跑一次 msime-dictionary 的发布（或等下一次源数据推送）让 菸→煙 与否决表生效，再按惯例更新 `resources/language-dictionaries.lock.json`、同步 `resources/licenses/msime-engine-dictionary-NOTICE.md`、重新生成 `tools/cantonese-evaluation/report.json`。顺序反了也不会发出错误数据：msime 先合入后，msime-dictionary 在收录 `HKVariants.txt` 之前的任何一次发布都会因为缺文件而失败。

## Alternatives considered

- **继续逐对手列（保留 `HONG_KONG_ONLY_VARIANTS`）**：影响面最小，每一对都经过人工确认，不会出现 搵→揾 这种意外。但漏掉的异体只能等用户一个个报，每次都要改构建器、发 msime 版本再发词库；`HKVariants.txt` 就是现成的香港异体清单，HKCanCor 规则已经挡住了它里面香港照用 OpenCC 字形的那些对（說/説 等），真正需要人工判断的只剩少数几对。维护者选了通用机制加否决表。
- **checkout 里没有 `HKVariants.txt` 时退回只读 `TWVariants.txt`**（仿照 `custom/cantonese.txt` 缺失时按空表构建）：能用新构建器构建旧的 msime-dictionary 提交。但旧提交的词表都有「溼」，退回后「濕」又只剩下限 1 分，`EXPECTED` 的 (`sap`, 濕) 照样让构建失败，退回路径对真实数据从来走不通；若为了让它走通而放宽 `EXPECTED`，msime 先合入时 msime-dictionary 的下一次发布就会悄悄把 #10711 修好的「濕」改回去。直接要求文件存在，失败信息写明缺的是哪个文件，比在 `verify` 里报「sap 前三没有濕」好查。否决表同理：缺了它「搵」会被换掉。
- **否决表写成构建器常量**：和原来的 `HONG_KONG_ONLY_VARIANTS` 一样放在代码里，有单元测试直接覆盖。但改一个字就要走 msime 的 PR 和发版，维护者倾向于放在 msime-dictionary 的数据文件里；构建器已经读 `custom/cantonese.txt`，再读一份同目录的文件没有新机制。
- **按对否决（写成 `搵<TAB>揾`）而不是按键**：能精确到某一个值。但 `hong_kong_forms` 对一个键只会选一个值，否决这个值等于让它退到下一个值或不换，语义不直观；按键否决就是「这个 OpenCC 字形不换」，目前的需求只有这一种。
- **把「菸」也放进否决表**：「菸」的 6 次语料里 5 次是烟雾义（煙花、煙幕），香烟义只有「吸煙」1 次，证据薄。维护者接受 菸→煙：「香煙、食煙、煙草」本来就是香港常见写法。

## Consequences

- 收益：香港字形的候选范围扩大到 `HKVariants.txt` 的 66 行，以后 HKCanCor 或 OpenCC 换版本时新出现的对会自动按同一规则判定，不再要求每对都改构建器；人工判断集中在一份可以随时增删的否决表里；`EXPECTED` 从此守住 濕、濕紙巾、啟 和否决的「搵」。
- 代价：「搵」的否决和固定提交的 HKCanCor（只写「揾」）相反，是维护者对用户习惯的判断，不是语料证据。菸→煙 证据只有 6 次。HKCanCor 或 OpenCC 换版本后替换清单可能变化，发布时要看构建摘要里的替换清单和否决表留下的字。msime-cloud 的词库管理页把 `custom/` 下的文件都当作只追加的词表显示，否决表的删行要直接在 msime-dictionary 提 PR。`check-words` 仍不读异体表，这一点和原笔记一样留作后续。
- 用户要等 msime-dictionary 发布含本改动的 `dict-v*`、msime 换锁之后才拿到 菸→煙；溼→濕 在 msime-dictionary 下一次发布（#10711 的构建器）里就有。#10725 的「啟」在已锁定的 `dict-v2.0.50` 里已经生效，开了词库自动更新、或装了带这把锁的版本即可看到。

## Verification

- `rbuild cargo test -p msime-dict-builder`：217 passed，1 ignored；另一组 13 passed。新测试覆盖两张表合并的顺序与去重（`the_hong_kong_table_joins_the_taiwan_table`，含 啓 两种写法合并后仍换成啟）、否决表挡住「搵」且否决表外的字照换（`vetoed_opencc_forms_are_never_replaced`）、否决表写了两张表都没有的字时报错、否决表行格式（`veto_rows_are_single_characters_listed_once`）、溼→濕 的权重与「濕紙巾」（`opencc_wet_is_spelled_the_hong_kong_way`）、`verify` 在「搵」被换掉时失败；样例库加了 啓/啟、搵/揾、溼紙巾，满足新的 `EXPECTED`。
- `rbuild cargo clippy -p msime-dict-builder --all-targets -- -D warnings` 无警告，`cargo fmt -p msime-dict-builder --check` 通过。
- `rbuild cargo run -p msime-dict-builder --bin msime-dict-build -- languages --offline --cache target/dict-cache --dictionary <msime-dictionary 92eed36> --out target/lang-10697`：exit 0，含 `verify` 与新的 `EXPECTED`；用 sqlite 对比 `dict-v2.0.50` 得到上面的差异。
- msime-dictionary：`python3 scripts/check_upstream.py`（46 个文件、14 个上游）与 `python3 scripts/validate_packs.py` 通过。
- 没有在模拟器或真机上验收。
