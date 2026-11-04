# Agent Note: 词库清洗的评测把关工具

Status: implemented

## Problem

词库仓库合入 main 就发布，约 24 小时后用户自动收到，而发布前没有任何排序评测。msime 现有的评测门禁是整份报告与提交的基线逐字节比较，只能说明「变没变」；`typing_sim` 的基线还记着资源的 `generation` 和 `source_commit`，换一份词库必然红。dict-cleanse 方案第 4 节要求每个清洗批次附上配对比较、成对偏好、粗俗曝光和五笔英文的差异清单，这些工具此前都不存在：车检/车件 这类有用户反馈的次序问题、打到一半时冒出来的粗俗词、九键第 2 位的英文脏词，现有评测集一条都量不到。词级集 `quanpin-words-v1.tsv` 还混着截断、读音不符和缺词的用例，同码用例让 top-1 的上限远低于 1，拿它比较两份词库，差别里有一部分是杂质在换人。

## Decision

评测把关工具全部是 `scripts/` 下的 Python 脚本，只读 `convert_eval --dump` 或数据库，不改评测二进制的口径。`convert_eval` 只加了一个默认关的 `--mixed-english`，现有基线逐字节不变。用法与现在的数写在 `resources/eval/README.md` 的「评测把关工具」一节。

- `scripts/eval_compare.py convert`：按用例 id 配对两份 dump，对 top-1、top-5 列出 fixed 和 broken，给精确 McNemar p 值；两份 dump 的 id、输入或金标准不一致时报错。同码用例按 `--set` 的标签分桶，只报告；`--gate` 是方案 4.3 的硬门槛（非同码桶里 broken > fixed 且 p < 0.05），必须带 `--set` 和 `--grid`，免得同码用例混进被卡的桶。`typing` 子命令比较两份 `typing_sim` 报告时忽略 `config.resources.generation` 和 `source_commit`，`--dumps` 按块配对。
- `resources/eval/quanpin-words-v2.tsv` 由 `scripts/build-quanpin-words-v2.py` 从 v1 派生，参照词库写死为 dict-v2.0.41 的 `msime-pinyin.db`（`eval_keys.py` 的 `REFERENCE_PINYIN_SHA256`，脚本先核对；不读当前资源锁，锁升级后照样能重新生成），去掉截断 1,694、读音不符 44、缺词 86 条，留下 23,295 条；同码用例打 `same-code-full|nine|fourteen` 标签。`lue`/`nue` 当作 `lve`/`nve` 比较，因为引擎把用户打的 `lue` 规整成词库里的 `lve`。v1 不动。
- `resources/eval/must-outrank.tsv` 只断言相对次序，每行必须有来源；两种行：`outrank` 和 `not-on-first-page`（锚定词不是 `-` 时必须在首屏，防止空过）。`scripts/must_outrank.py` 按键盘把夹具写成 `convert_eval` 的用例集，九键的数字换成一串编码相同的字母，加 `--mixed-english` 跑 dump 再判定。
- `scripts/vulgar_exposure.py` 的输入集冻结在 `resources/eval/exposure-quanpin-v1.tsv` 和 `exposure-nine-key-v1.tsv`：参照词库权重最高的 5,000 个 2 到 4 字词，每个读音打全、每一个字母前缀、前 k 个音节的首字母，按键盘去重，加方案点名的目标码。粗俗词判定照抄引擎 `lattice::offensive::is_offensive` 的规则、读引擎自己的 `offensive_words.txt`；英文冒犯词读 `english_supplement.rs` 的 `OFFENSIVE`。`--baseline` 比较三项计数，任何一项增加就失败。读音恰好是粗俗词自己的读音（`exact`）只在全键盘上豁免，九键的码几乎都有歧义，不豁免。
- `scripts/resource_lock_from_sums.py` 以桌面锁为模板，把 dry-run artifact 的 `msime-SHA256SUMS.txt` 换进去，URL 写成 `https://dry-run.invalid/<文件名>`，`--without-model` 去掉模型，`--stage` 建一个恰好只含锁里文件的资源目录。
- `scripts/dictionary_topn_diff.py` 读两份数据库按引擎的 SQL 排序列出五笔和英文的 top-N 差异，只作审阅附件。
- 码表（九键 `KEYPAD`、14 键 `FOURTEEN_KEYPAD`）在 `scripts/eval_keys.py`，`scripts/test-build-quanpin-words-v2.py` 核对它与 `crates/engine/src/nine_key.rs` 逐字相同；`scripts/test-dictionary-topn-diff.py` 核对英文补全的排序与 `english.rs` 的 `PREFIX_SQL` 相同。

### 为什么 must-outrank 现在不进 `verify-local.sh`

第一批 6 行在 dict-v2.0.41 上全部失败，都是方案里排了修复阶段的已知问题。必然红的门禁只会被跳过、或者被改成「已知失败」名单后再也没人看。夹具转绿后接进词库仓库的 PR 评测（方案 R-3），那里才是挡住发布的位置。

## Alternatives considered

- **把配对比较做成 `convert_eval --compare`** — 最强的理由是只有一个工具、不必再解析 dump。没有选，是因为比较本身不需要引擎：在 Rust example 里做，每改一次报告格式就要在编译机上重编，而且比较的输入常常是两台机器、两个提交跑出来的 dump，绑在二进制上反而要求两边同一个版本。仓库里的测量脚本（`docs/measurements/*/quality.py`、`scripts/voice_eval.py`）也都是这样拆的。
- **must-outrank 直接调引擎会话（照 `probe_ninekey` 那样）** — 理由是能拿到默认的 `prepare_options`，产品配置一项不差。没有选：它要一个新的 Rust example，而且那套配置开着学习、本地模式和日期时间，结果随运行日期和前后用例变化；`convert_eval` 已经把这些关掉，只差混输英文，所以只给它加一个开关。
- **曝光输入集每次从当前词库现生成** — 理由是不往仓库里放 87 万字节的输入集。没有选：清洗改的正是词库的权重，C 和 B 现生成出来的输入集本身就不同，「C ≤ B」就不可比了；评测集冻结、改了发新版本是这个目录一贯的做法。
- **v2 按方案文档的字面口径去掉 83 条读音不符** — 理由是与方案数字一致。没有选：其中 37 条只是 `lue`/`lve` 的写法差，用户打 `celue` 就是要 策略，引擎也打得出来，去掉它们等于丢掉有效用例。差异写进了文件头和 README。
- **沿用「top-5 下降不超过 0.5 个点」这种不配对的门槛** — 理由是简单、不需要 dump。没有选：方案复核已经指出那个半宽是 p=0.5 时的最坏值，两次独立运行的差值混着同一批用例的变化和抽样噪声；配对之后只看判定不同的用例，分辨力高得多。

## Consequences

- **收益**：同一批用例在两份词库上的变化能逐条列出并检验显著性；有出处的次序问题和打到一半时的粗俗曝光第一次有了可以重复运行、可以设门槛的量法；临时锁让 dry-run artifact 不经发布就能跑全部评测；五笔和英文至少有了并排的审阅清单。
- **代价与已知上限**：粗俗词和英文冒犯词只认现有两张表，英文脏话（shitty、gangbang）要从第 2 位英文清单人工看，H4/H7 的表迁到词库仓库后要用 `--extra-words` 接上或改默认路径。曝光输入集和 v2 的参照词库都是 dict-v2.0.41，参照换了要发新版本。码表和 `PREFIX_SQL` 是照抄的，引擎改了要同步，测试会先红。全拼曝光的 dump 带整张候选表，约 240 MB，只适合在编译机上跑。`dictionary_topn_diff.py` 是数据层的近似，量不到学习、四码自动上屏和九键英文的数字前缀。

## Verification

- `python3 scripts/test-eval-compare.py`、`test-build-quanpin-words-v2.py`、`test-must-outrank.py`、`test-vulgar-exposure.py`、`test-resource-lock-from-sums.py`、`test-dictionary-topn-diff.py` 随 `scripts/run-checks.sh` 运行，不需要词库。
- 在 Studio 上用 dict-v2.0.41 加 `sentence-model.safetensors`：`must_outrank.py run` 6 行全部失败（README 的表）；`vulgar_exposure.py run` 全拼 10 串、九键 9 串首屏有粗俗词。用 `resource_lock_from_sums.py` 给 dict-v2.0.51 的发布附件生成临时锁并 stage，`convert_eval` 校验通过；`eval_compare.py convert` 在 v2 的三种键盘上配对比较 A→B（构建器漂移），top-1、top-5 的 fixed 和 broken 都是 0，`vulgar_exposure.py run --baseline` 对 A 的报告通过，`must_outrank.py` 在 B 上同样 6 行失败。
