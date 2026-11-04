# Agent Note: 词库构建器改动在 PR 上出产物差异报告

Status: implemented

## Problem

词库产物（dict-v 的各个附件）由两样东西决定：msime-dictionary 的源数据提交，和 msime 的构建器（`crates/dict-builder` 及它读的 `resources/dictionary-sources/`、`resources/dictionary-sources.lock.json`、几份许可证文本、它用到的引擎文件、工作区的依赖锁和工具链）。msime-dictionary 的发布 workflow 默认用 msime 的 develop 构建，所以构建器的改动合进 develop 时没有任何门禁，会跟着下一次与它无关的数据推送一起发布。dict-v2.0.51 就是这样：它唯一的数据改动是粤拼 #120，拼音主库却随构建器一起变了，发布说明里看不出来。词库清洗计划（§3.4 R-2）要求构建器一有改动，就用当前 main 的数据构建一次、逐表比较，并作为 PR 检查。

## Decision

`.github/workflows/dict-builder-artifact-diff.yml` 在改动上面这些路径的 PR（以及合进 develop 后、手动运行时）上，用同一个 msime-dictionary 检出（默认 main 在运行时的提交）分别以 PR 的构建器和基准构建器跑主构建和 `languages`，与 msime-dictionary 的 `release-built-dictionaries.yml` 构建同一组产物，再由 `scripts/dict_artifact_diff.py` 比较两份 `--out`。两次构建用同一份数据，差异只来自构建器。

- 基准：PR 用合并提交的第一个父提交（PR 要合入的 develop），推送用推送前的提交，手动运行用本次提交与 `base_ref` 的合并基点。
- 报告写进作业摘要，上传为 artifact `dict-builder-artifact-diff`（Markdown 和完整 JSON），同仓库的 PR 上更新一条带标记的评论。评论由只有 `pull-requests: write` 的单独作业发，它不运行 PR 的任何代码；构建作业只有读权限。fork 来的 PR 拿不到写权限，只有摘要和 artifact。
- 默认只是信息：新构建器编不过或构建失败才算失败。PR 打了 `expect-no-artifact-change` 标签（手动运行时勾选 `expect_no_change`）时，任何内容差异都算失败；基准构建器构建失败时也算失败，因为没法确认没有变化。标签在运行时经 API 读取，加上或去掉标签后重跑作业即可。
- 基准构建器用这份数据构建失败（main 的数据已经要求更新的构建器）时不比较，非严格模式只给警告，不让与 PR 无关的状况把 PR 弄红。

### 比较规则（`scripts/dict_artifact_diff.py`）

- SQLite 对象定义只折叠引号和注释之外的排版空白；字符串常量、带引号的标识符以及 SQL 的引号转义原样保留。不能对整条 SQL 用 `split()`，否则默认字符串 `two  words` 改成 `two words` 会漏报，严格模式也会错误通过。行为回归覆盖这条漏报、包含转义单引号的字符串及只改外部换行的无变化对照。
- SQLite 库逐字节相同就记为相同；否则比较库头 pragma、`sqlite_master` 里的定义，再逐表比较行。每张表的键是声明的主键，没有就取第一个 UNIQUE 约束，都没有就取除权重类列（`weight`、`sort_order` 等）以外的全部列；拼音主库的 `tbl_*` 没有约束，键因此是 key、jp、value。行按多重集合比较，差异分为新增、删除、改权重（键两边都有、只有权重类列变了）和改内容（别的列或重复次数变了）。比较在 SQLite 里用 `EXCEPT` 和临时表完成。
- 行的次序也比较。引擎在同权重的候选之间按 rowid 排：五笔查询写明 `ORDER BY weight DESC, rowid ASC`，拼音按 key 的索引扫，同键的行也按 rowid 出来（拼音源数据里权重恰为 100 的就有 19.4 万行，同分很常见）。所以只改插入顺序的构建器改动会改变用户看到的候选，内容却完全相同。脚本把两边都有的行（相同的行按出现次序逐个配对）按旧库 rowid 排好，取它们在新库的 rowid，最长递增子序列以外的行记为改顺序，个数是让新库次序回到旧库最少要挪动的行数；只有一边有的行不参与，所以中间插进新行不算改顺序。WITHOUT ROWID 的表按主键存，次序由内容决定，不比较。内容、行序、库头和定义都相同、只有字节不同（页布局、空闲页）的库不算差异，引擎看不出区别。
- v2.0.41 真实库副本上的最坏情况（五笔 73 万行三分之一改权重、wubi98 删 10 万行并在末尾补回一部分，拼音一张表改权重）本机约 16 秒、124 MB 内存。
- 无法比较（目录不存在、库打不开）时以 2 退出，也照样写一份说明原因的 Markdown 报告；脚本崩溃没写出报告时由 workflow 补一份，并且不把崩溃的退出码 1 当成「声明不改产物却有差异」。
- 样例按权重从高到低列，排在前面的候选最影响手感；Markdown 每类 5 行、每个库最多 8 张表，JSON 每类 100 行。报告长度不超过 60000 个字符（PR 评论上限 65536）。数据一律放进代码片段，里面的 `@`、`#` 不会通知人或链出去。
- manifest 的 `source.commit`、`source.dirty`、`format_contract_commit` 和 `files` 只报告不计入：前三个是构建器自己的提交，`files` 是各产物的大小和摘要，逐文件比较已经覆盖，SQLite 库内容相同而字节不同时也不该因此判为有差异。`*SHA256SUMS*` 由其他产物推出，同样不计入。其余文本按行比较，二进制比较大小、摘要和不同的字节数。
- `wubi86-supplement.txt`、`wubi98-supplement.txt`、`wubixsj-supplement.txt` 首行的构建器提交号也不计入；只识别已知文件名和对应源码路径，头部其余说明、命令及所有数据行照常比较。首次真实运行证明两个相同构建器产物只有这三处提交号不同，若不排除，严格模式每次都会误报。回归同时验证词条权重、生成命令以及其他文本文件的提交号变化仍然失败。

### 触发路径

`crates/dict-builder/**`、`resources/dictionary-sources/**`、`resources/dictionary-sources.lock.json`、构建器复制进产物的四份许可证（libchewing-data、rime-cantonese、rime-stroke、SCOWL）、构建器直接用到的引擎文件（`assets.rs` 的日语模型文件名、`cantonese/scheme.rs` 的 `SPAN_LIMIT`、`format.rs`、`language_dictionary.rs`、粗俗词表的解析与内嵌后备表 `lattice/offensive.rs` 和 `lattice/offensive_words.txt`、`wubi/code.rs`、决定注音库收哪些音节的 `zhuyin/layout.rs`）、工作区的 `Cargo.lock`、`Cargo.toml`、`rust-toolchain.toml`，以及 workflow 和比较脚本本身。`resources/licenses/**` 整个目录两个月里改了 68 次，绝大多数与词库无关，所以只列构建器真正读的那几份。

- 清单不能靠人记：`scripts/test-dict-artifact-diff.py` 找出 `crates/dict-builder/src` 里每处 `msime_engine::…`（经 `lib.rs` 的 `pub use` 解析 crate 根上的导出），解析到 `crates/engine/src` 下的文件，不在清单里就失败；同时检查 pull_request 和 push 两份清单相同、每条路径都存在。只用于 `rime-ice-supplement` 子命令的 `rime_ice_supplement.rs` 不在发布构建里，列为例外。这些引擎文件自己再引用的引擎代码不追：构建器用到的都是常量、表名和读写格式，传递依赖改了还没改到这些，产物不会变。
- `Cargo.lock` 一个月改八十多次（依赖机器人每月的 cargo 批量升级、别的 crate 加依赖），每次都编两遍、构建两遍不划算。所以只有它落在这份清单里时，作业先在两边跑 `cargo tree -p msime-dict-builder`（包、版本、特性，去掉检出路径）比较，相同就不编译不构建、报告写明原因，声明不改产物也照样通过；不同才走完整流程。根 `Cargo.toml` 和工具链改动不走这条捷径：profile 和编译器变了，依赖树看不出来。

## Alternatives considered

- **与上一个 dict-v 的发布附件比较**（计划 R-2 的原话）。它直接回答「下次发布会变什么」，也不用构建两次。但差异里混着 main 上自那次发布以来的全部数据改动，构建器的漂移和数据改动分不开，而 PR 要回答的恰恰是「这个构建器改动让产物变了什么」。同一份数据、两个构建器只多一次构建（两棵树共用 `CARGO_TARGET_DIR`，第二次只重编工作区里的 crate）。
- **把数据钉在 `desktop-dictionary.lock.json` 那个 dict-v 的 `custom_dictionary_commit`**。重跑结果完全可复现。但 msime 的锁常常落后 main 好几个版本，main 的 `upstream.lock.json` 一旦随 msime 的源数据锁前进，旧提交就过不了构建器对上游引用的核对；而且计划要的是「当前 main 的数据」。现在的做法把实际用到的数据提交写进报告开头，手动运行可以用 `dictionary_ref` 指定提交来复现。
- **只比较文件摘要**。实现最简单，但 SQLite 库的字节会因写入顺序、页布局变化而变，内容不变也会报差异；而且只能说「变了」，说不出哪张表、哪些行、权重怎么变，审阅者还得自己下载两份库去比。
- **`--expect-no-change` 下要求 SQLite 逐字节相同**。最严，计划 1.1 写的也是「逐字节不变」。但把建索引挪到插入之前、换一种批量写法都会改页布局而不改任何查询结果，这类重构正是要靠这个标签放行的；行序比较已经覆盖引擎能看出来的那部分。
- **按 `cargo tree` 或改动文件在作业里判断要不要跑**，代替静态的路径清单。`cargo tree` 只能说到 crate 一级，而构建器依赖整个 msime-engine，引擎任何一处改动都会触发两次完整构建；路径清单加上面的检查更准，代价是构建器开始用新的引擎文件时要改清单，而测试会提醒。
- **由标签事件触发重跑**。打标签后自动生效。但 pr-triage 的 labeler 每次推送都给 PR 打标签，每个标签事件都会多出一个被跳过的运行，盖住上一次的检查结果；改为运行时读标签，打标签后手动重跑。

## Consequences

- 改动构建器的 PR 多一个作业（两次编译、两次完整构建、比较），上限 60 分钟，实际耗时要等第一次真实运行才知道；首次运行要下载约 500 MB 的源数据，之后按锁的摘要走缓存。缓存的保存单独一步，构建或比较失败时也存下已经下好的文件。合进 develop 后再跑一次，既留下这次合并让产物变了什么的记录，也让 PR 能读到 develop 作用域里的缓存。这次运行登记在 `ci-red-tracker.yml` 里：它红了说明 develop 的构建器（msime-dictionary 发布默认用的那个）编不过或构建不出当前 main 的数据，会开 ci-red 跟踪单。
- PR 评论在构建作业没上传报告时（例如检出就失败了）也会更新，写明这次没有结果，不留着上一次「没有差异」的报告。
- 这个作业现在不是必过检查。它按路径触发，直接设为必过的话，没触发它的 PR 会一直等待；要设为必过，得先改成每个 PR 都跑、路径无关时直接成功（`ci-docs.yml` 替必过检查兜底的做法）。声明不改产物的重构要靠打 `expect-no-artifact-change` 标签，这个标签需要维护者在仓库里建好。
- 报告依赖 msime-dictionary main 在运行时的状态，同一提交在不同时间重跑，数据提交可能不同；报告开头写明了用的是哪个提交。
- 比较脚本由 `scripts/test-dict-artifact-diff.py` 用测试里现建的小 SQLite 库和文件覆盖，作为契约检查在 Core CI 里跑。本轮补修后的 21 项比较测试、14 项红灯跟踪测试、完整本地 quick、笔记校验及差异检查均通过；quick 按 workflow 范围跳过无关平台构建，Apple 桥接仍实际编译。
- PR #10849 原提交 `754f8899c6` 的首次真实运行 [41811424813](https://github.com/metasequoiaime/msime/actions/runs/41811424813) 成功：两边构建器均完成主构建和语言词库构建，报告 artifact 与评论均产出；二进制及 SQLite 内容相同，仅三份补充文本的构建器提交号被报成变化，已按上述规则补修。首次运行成功不替代补修提交的新 CI。
