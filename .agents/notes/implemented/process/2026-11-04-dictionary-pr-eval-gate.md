# Agent Note: 词库仓库 PR 的排序评测门禁

Status: implemented

## Problem

msime-dictionary 合入 main 就发布 dict-v，客户端默认每 24 小时自动更新，所以一个词库 PR 一旦合入，约一天后就到了用户手里。dict-cleanse 方案 §3.4 的 R-3 要求在合入之前跑 §4.3 的硬门槛，并设为 main 的 required check，否则评测结果只能事后看。[评测把关工具](../testing/2026-11-04-dictionary-eval-gate-tools.md)已经把每一项检查做成了脚本，但它们各自回答一个问题，要在 PR 上挡住发布，还缺一个入口把它们按同一份资源、同一套参数跑完，合成一个退出码和一条 PR 评论，以及词库仓库里调用它的 workflow。

## Decision

msime 侧是 `scripts/dict_eval_gate.py`，测试是 `scripts/test-dict-eval-gate.py`（随 `scripts/run-checks.sh` 运行）；词库仓库侧是 `.github/workflows/eval-gate.yml`，作业名 `Eval gate`。用法和五道门槛的口径写在 `resources/eval/README.md` 的「`scripts/dict_eval_gate.py`」一节。

- **入口只编排和判定。** 判定规则全部来自 `eval_compare.py`、`must_outrank.py`、`vulgar_exposure.py`，入口不重写任何口径；所有 `convert_eval` 都加 `--mixed-english`、每页 6 个，与产品出厂设置一致。评测集目录里每一份 `.tsv` 都必须归进「取金标准的词级集 / 打字流集 / 不取」三张清单之一，测试检查这一点：新加的集合要先决定它算不算金标准。
- **退出码三态。** 0 全部通过，1 五道门槛都算出了结果且有一道失败，2 没跑完（输入不全、`convert_eval` 失败、被调用的脚本以 `SystemExit` 报不可比、库文件损坏、脚本自己出错）。入口捕获全部异常、打印 traceback 后返回 2，写报告也算在内：Python 默认以 1 结束未捕获的异常，不这样做的话一个打不开的库会被 workflow 报成「有门槛失败」。workflow 把 2 也判为检查失败，但报告里写的是「没跑完」，不冒充门槛结论。
- **must-outrank 用棘轮。** B 上通过、C 上失败的行算退步，门槛失败；两边都失败的行逐条列在报告里，不卡。`--must-outrank-strict` 是方案字面的「全部通过」。这改了方案 4.3 一道硬门槛的写法，不在维护者已采纳的第 6 节决定里，**需要维护者确认**；夹具全部转绿后在 workflow 里加上 `--must-outrank-strict`。
- **粗俗曝光两边用同一张词表。** 引擎内嵌的表，加 B 和 C 两份 `msime-pinyin.db` 的 `sentence_excluded` 表的并集。C 新加进 `curation/offensive-words.txt` 的词也拿来量 B，C ≤ B 才可比。
- **基线 B 是运行时最新的 dict-v 发布。** 那就是用户此刻自动更新到的版本，按发布自带的 `msime-SHA256SUMS.txt` 下载并校验。
- **构建器和评测脚本取同一个 msime 提交。** 来自候选提交自己的 `builder.lock.json`：升级构建器的 PR，评测的就是新构建器构建、新门槛判定的结果。固定提交里还没有 `dict_eval_gate.py` 时检查失败并说明要先升锁。手动运行可以用 `msime_ref` 换一个分支或提交试跑，报告里注明「不是固定的提交」。
- **作业总是运行，范围在作业里判定。** 没有改动 `sources/`、`custom/`、`curation/`、`upstream.lock.json`、`builder.lock.json`、`release-built-dictionaries.yml` 和 workflow 自己的 PR，作业判定范围后以成功结束，不用 `on.pull_request.paths` 过滤：被过滤掉的 PR 不会出现这个检查，设为 required 后会一直等待。
- **候选清单与发布 workflow 同样核对。** 授权构建、构建器不脏、`source.commit` 等于检出的 msime 提交、`custom_dictionary_commit` 等于本次检出的词库提交；构建器调用与 `release-built-dictionaries.yml` 相同，主构建和 languages 都跑、分两个目录放（评测只读主产物，languages 构建是为了让粤拼、注音、笔画的构建错误在合入前暴露）。
- **upstream.lock.json 和发布 workflow 也算发布输入。** 构建器逐项核对 `upstream.lock.json`（版本、提交、引用对不上就构建失败），它也算进 manifest 的 `source.dirty`；发布 workflow 决定发布用什么命令构建。只改它们的 PR 原来会跳过门禁，构建错误要到合入后的发布才暴露。
- **方案 4.3 的参考项和第 5 节的附件在同一个作业里，只报告。** 入口另跑 sentences-v1、九键长句、particles（三种键盘）、neutral 在每一臂上的配对比较，并列出方案点名的粗俗目标码在 B、C 首屏上的冒犯词，针对性清洗批次靠它确认清零；这些不进退出码。workflow 再用 `dict_artifact_diff.py` 把 C 与 B 的两组产物逐表逐行比较（JSON 里每张表每类差异最多 200000 行，就是逐词的删除、降权清单），用 `dictionary_topn_diff.py` 列五笔和英文的 top-N 差异，摘要附在报告后面；候选产物整组上传为 artifact `dictionary-candidate`，保留 7 天。
- **评论另起一个作业。** 只有 `pull-requests: write`，不运行 PR 的任何代码，按 HTML 标记找自己的评论更新（同一个 bot 还发 check-words 的评论）；fork 来的 PR 只有作业摘要和 artifact。

### 时间

2026-11-04 在词库分支上试跑（msime `cleanse/eval-gate-ci`，Depot 8 核 runner，构建器命中 sccache）：两臂、quanpin-words-v2 全量，门槛 12 分钟，整个作业约 16 分钟；加上参考项、languages 和产物差异之后约 16 到 18 分钟。离 120 分钟上限很远，所以 PR 上默认全量、两臂都跑；构建器缓存全部失效时编译会多出十几分钟。作业逼近上限时把 workflow 的 `QUANPIN_V2_LIMIT` 设成等距子集的大小，B、C 用同一批用例，其余门槛仍然全量。

## Alternatives considered

- **must-outrank 照方案字面「全部通过」。** 最强的理由是与方案一致，已知问题会一直挡着，逼着修。没有选：第一批 6 行里 5 行在当前发布上失败，都是排了后续阶段的已知问题；照字面卡，每个与之无关的投稿 PR 都会红，门禁很快会被跳过或被去掉 required。棘轮保证修好的不再坏，夹具全部转绿后它就等于「全部通过」，届时可以在 workflow 里加 `--must-outrank-strict`。
- **基线固定为某个 dict-v（例如桌面锁里的版本）。** 理由是结果可复现、不随别的合入变化。没有选：门禁要回答的是「合入后用户手里的词库会不会变差」，用户手里的是最新发布，不是锁里的旧版本；固定基线会把之前已经发布的变化反复算进每个 PR。
- **评测脚本跟随 msime develop，构建器仍用固定提交。** 理由是评测口径的改进不用等升锁就能生效。没有选：评测集、词表和判定规则一变，同一个 PR 前后两次运行的结论就可能不同，门槛口径应当和构建器一样显式升级、在升级 PR 上看得到差别。
- **在 msime 的 CI 里跑，词库仓库只看结果。** 理由是 Rust 工具链和缓存都在 msime。没有选：门禁要挡的是词库仓库的合入，required check 只能是词库仓库自己的检查；跨仓触发还要额外的 token 和状态回写。
- **用 `on.pull_request.paths` 只在相关 PR 上运行。** 理由是省 runner。没有选：见上面 required check 一条；判定范围本身只要几秒。

## Consequences

- **收益**：词库 PR 合入之前第一次有了配对、可检验显著性的排序评测；粗俗曝光和有出处的次序问题有了不会退步的保证；报告直接写在 PR 上，维护者不用自己跑工具。
- **代价**：每个改动发布输入的 PR 多一个约 16 分钟的作业。评测脚本跟构建器一起固定，`dict_eval_gate.py` 或评测集的改进要等升 `builder.lock.json` 才在 PR 上生效；在那之前（包括这个门禁本身的第一次启用）检查会因固定提交里没有入口脚本而失败，必须先在 msime 合入、再在词库仓库升锁。
- **维护者操作**：把 `Eval gate` 设为 main 的 required check 要在 GitHub 的分支保护里手动设置，workflow 不改仓库设置。
- **已知上限**：B 和 C 用同一个 `convert_eval`（固定提交编出来的），所以量到的只是数据和构建器的差别；客户端引擎的改动不在这里评。曝光输入集和 v2 的参照词库冻结在 dict-v2.0.41，参照换了要发新版本。ctx-v1（dump 约 680 MB，只能在 Studio 上跑）和 typing_sim 不在这个作业里，大规模改权重的批次（方案第 5 节 4.1 阶段的万象重加权）要另外在 Studio 上跑它们。评论作业只在 `pull_request` 事件上运行，分支推送的试跑走不到它，第一次真实运行是在加这个 workflow 的词库 PR 上。

## Verification

- `python3 scripts/test-dict-eval-gate.py`：五道门槛各自的判定、全拼表名和行数下限与 `format.rs`、`product.rs` 一致、评测集都已归类，并用假的 `convert_eval` 端到端跑通过、门槛失败、跑不完三种退出码。
- 词库分支 `cleanse-data/eval-gate-ci` 上用分支推送临时触发（workflow 还不在 main 上，`workflow_dispatch` 不能用），msime 指向 `cleanse/eval-gate-ci`：https://github.com/metasequoiaime/msime-dictionary/actions/runs/41830735038 （msime `c0ef194171`）和 rebase 到 main 之后的 https://github.com/metasequoiaime/msime-dictionary/actions/runs/41833821442 （msime `51b40d84e8`，同时核对了候选清单的 `source.commit` 和 `custom_dictionary_commit`）都全部通过，作业分别 15 分 50 秒和 16 分 4 秒；基线 dict-v2.0.56，金标准对 B、C 各 24851 对，must-outrank 6 行里 5 行两边都失败，全拼粗俗首屏 6 → 6、九键 0 → 0，v2 六组 fixed/broken 全是 0，全拼 953736 行。
- 补上参考项、languages 构建和产物差异之后，又在同一分支上用临时推送触发、并临时加上 `--must-outrank-strict` 跑了两次，专门走一遍真实数据上的门槛失败路径：https://github.com/metasequoiaime/msime-dictionary/actions/runs/41839691028 （msime `ca200b9c4a`，作业 17 分 42 秒）和 https://github.com/metasequoiaime/msime-dictionary/actions/runs/41842679421 （候选目录改为只放校验和清单列出的文件之后，作业 15 分 54 秒）。两次都按预期以「a hard gate failed」失败，报告的结论是「有门槛失败」，must-outrank 一行标为失败（严格模式下 5 行两边都失败），其余四道门槛通过；参考项 12 组整句比较全部 0/0，10 个粗俗目标码在 B、C 首屏上都没有冒犯词；产物差异如实报出构建器漂移（C 的构建器带上了内置的英文蔑称否决，english_words 删了 tranny、nigger、beyatch 三行）。第一次运行还暴露出构建目录里有不发布的五笔补充码表中间文件（约 37 MB）被报成新增，第二次运行确认已经去掉。评论作业仍未运行过，见上面的「已知上限」。
