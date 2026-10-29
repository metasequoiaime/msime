# Agent Note: 评测按宿主的每页候选数和本地模式配置

Status: implemented

## Problem

[#10268](https://github.com/metasequoiaime/msime/pull/10268) 查明 `parity.rs` 的两千多例差异出在测试自己的 runtime 一侧：`Runtime::new(engine, 9)` 只把 9 用在分页上，引擎的首屏门槛 `first_page_rows` 停在 `prepare_options` 的默认 6；行内日期时间和「📅日期」菜单也没关。产品不受影响，因为 host-api 建引擎时在选项里写了 `options.first_page_rows = page_size`，而 `Runtime` 只在 `set_page_size` 里把页大小交给引擎，`new` 和 `replace_engine` 都不交。任何直接 `Runtime::new` 的调用方（示例、测试、评测工具）都会悄悄得到「分页 9、门槛 6」。

`convert_eval` 不给 `--first-page-rows` 时正是这样，另外也没关行内日期时间和日期菜单。后者让 `quanpin-words-v1` 上的结果随运行的日期、星期几和钟点变化：`houtian`、`qiantiao` 后面跟着当天推算的日期，`shijian` 带着运行时的钟点，`yingqi`、`xingsi` 只在星期四多出「星期四」「周四」。同一份二进制换一天跑，基线就可能对不上。

## Decision

### `Runtime` 总是把页大小交给引擎

`Runtime::new_with_touch_layout`（`new` 经它）和 `replace_engine_with_touch_layout`（`replace_engine` 经它）在取快照之前调 `InputEngine::set_first_page_rows(page_size)`，`set_page_size` 照旧下发。于是三处入口都让引擎门槛等于 runtime 的分页，调用方不必再在引擎选项里另设一份，也设不出不一致的两份。

host-api 的输入会话本来就在选项里写了同一个数，建会话、重建引擎后门槛不变。它的 `lookup_candidates`（MCP 和 `msime expand` 用的查码）原先 `Runtime::new(session, 9)`、引擎门槛取偏好的每页候选数；改成用选项里那个每页候选数建 runtime，引擎门槛与改动前相同，查到的仍是用户打这串键时看到的那张表。

### `convert_eval` 的默认配置

- 每页候选数同时交给引擎和 runtime。不给 `--first-page-rows` 时取共享偏好的出厂值 `Preferences::default().candidate_page_size`（现在是 6）：桌面三端、Android 和鸿蒙都用它，只有 iOS（`fixed_candidate_page_size` 是 9）和网页引擎（默认 9）不同，它们的数用 `--first-page-rows 9` 量。参数范围 1 到 10，超出时报错，不再悄悄截到 9。
- 本地模式全关：在原有的 Unicode、T 模式、快捷短语、emoji、颜文字、超级简拼、临时英文、临时日文之外，再关行内日期时间、日期菜单、表达式、`/` 命令、`@` 提及、拆字、符号和 Shift 字母入口，与 `parity.rs` 的 runtime 一侧相同。`convert_eval` 量的是转换本身，混输和本地模式一直是关的，行内日期时间是后来加的开关，没跟上。

分页本身不进 `convert_eval` 的任何指标：它读 `all_candidates`，不分页，也没有云候选和 AI 行，runtime 用到 `page_size` 的地方（翻页、数字键、云和 AI 的座位）一处都不经过。引擎门槛在改动前后都是 6。所以这次口径变化对数字的影响全部来自日期时间候选，见下文的对照。

## 前后对照

锁定资源 dict-v2.0.39（`install_resources` 下载校验，挂 `sentence-model.safetensors`，没有 settled 模型），Studio 上 release，同一天（2026-10-29，星期四）先后用改动前的 develop（`1ffd90e137`）和本分支的二进制跑 `verify-local.sh` 的 13 份集合。改动前的 13 份报告与提交的基线逐字节相同。

| 集合 | 用例 | 改动前 top1 / top5 / top9 / found / mrr | 改动后 | |
|---|---|---|---|---|
| `sentences` | 60 | 0.817 / 0.833 / 0.833 / 0.833 / 0.825 | 同左 | 整份相同 |
| `harvested` | 312 | 0.494 / 0.772 / 0.779 / 0.885 / 0.619 | 同左 | 整份相同 |
| `neutral` | 1105 | 0.648 / 0.825 / 0.825 / 0.878 / 0.729 | 同左 | 整份相同 |
| `words` | 2791 | 0.750 / 0.943 / 0.981 / 0.988 / 0.830 | 0.750 / 0.943 / 0.982 / 0.988 / 0.830 | 有变 |
| `nine-key` | 2791 | 0.553 / 0.834 / 0.892 / 0.968 / 0.674 | 0.553 / 0.835 / 0.892 / 0.968 / 0.675 | 有变 |
| `nine-key-prefix` | 2775 | 0.193 / 0.342 / 0.426 / 0.821 / 0.273 | 同左 | 两音节分桶 top5 0.200 → 0.201 |
| `nine-key-sentences` | 81 | 0.556 / 0.630 / 0.667 / 0.704 / 0.595 | 同左 | 整份相同 |
| `fourteen-key` | 2791 | 0.630 / 0.894 / 0.937 / 0.975 / 0.742 | 0.630 / 0.894 / 0.938 / 0.975 / 0.742 | 有变 |
| `fourteen-key-prefix` | 2775 | 0.328 / 0.552 / 0.649 / 0.960 / 0.436 | 同左 | 整份相同 |
| `fourteen-key-sentences` | 81 | 0.630 / 0.679 / 0.691 / 0.802 / 0.654 | 同左 | 整份相同 |
| `particles` | 103 | 0.670 / 0.951 / 0.971 / 1.000 / 0.791 | 同左 | 整份相同 |
| `particles-nine-key` | 103 | 0.223 / 0.786 / 0.893 / 0.990 / 0.405 | 同左 | 整份相同 |
| `particles-fourteen-key` | 103 | 0.379 / 0.883 / 0.913 / 1.000 / 0.562 | 同左 | 整份相同 |

两边各用 `--dump` 导出四份词级集的整张候选表逐例比较，候选表不同的用例全部是日期时间行被拿掉，没有一例是别的行换了位置：全键盘 4 例（`houtian`、`jinnian`、`qunian`、`shijian`，金标准名次只有 `shijian` 的世间 10 → 6 变了），九键 10 例（另有 `qiantiao` 欠条 8 → 2、`shijiao`、`shiliao`、`xingsi`、`xinji`、`yingqi`），九键截短 3 例，14 键 4 例。

双拼触屏误触的词级基线（`baseline-shuangpin-touch-words.json`，5 个方案 × 4 种模式 × 4 个 σ × 2 个偏置）改动前后的报告都与提交的基线逐字节相同；整句那份（`baseline-shuangpin-touch-sentences.json`）改动后的报告也与提交的基线逐字节相同（改动前没有用旧二进制重跑）。

## Alternatives considered

- **默认每页 9 个，与 `parity.rs`、iOS 和网页引擎一致** — `parity.rs` 和 `typing_sim --profile convert-eval` 都用 9，`convert_eval` 原来的分页也是 9，改成 9/9 看起来改动最小。但用户最多的宿主（桌面三端、Android、鸿蒙）出厂是 6，`convert_eval` 的引擎门槛也一直是 6；默认改成 9 等于把十几份基线的口径换成少数宿主的，有 6 到 8 个精确同音词的输入首页组成全都跟着变，还与「默认匹配产品宿主」相反。9 的数照旧能用 `--first-page-rows 9` 量。
- **行内日期时间留着，换成钉住的虚拟时钟**（`typing_sim` 的做法）— 更像产品，结果也确定。但 `convert_eval` 一直关着所有混输和本地模式，量的是转换本身，日期时间行只会把金标准往后挤一两格，钉住时钟也只是把「哪天跑」换成「钉在哪天」；而且词级集里只有十来个输入碰得到它。关掉与其余本地模式一致，也和 `parity.rs` 相同。
- **只改 `convert_eval`，不动 `Runtime`** — 在 `open_runtime` 里把页大小写进选项就够修评测。但同样的坑在每个直接 `Runtime::new` 的地方都在（`harvest_eval_set`、`rerank_latency` 和别的示例都是 `Runtime::new(engine, 9)` 配默认选项），#10268 的 `parity.rs` 就是这样掉进去的；由 runtime 在建立和换引擎时下发，调用方就不可能再给出两份不同的数。

## Consequences

- **收益**：任何 `Runtime` 的引擎门槛都等于它的分页；`convert_eval` 的默认数字就是出厂每页 6 个的桌面、Android 和鸿蒙宿主会看到的首页组成，换哪天跑都一样。
- **代价与已知上限**：直接 `Runtime::new(engine, 9)` 配默认选项的示例和工具（`harvest_eval_set`、`rerank_latency`、`japanese_conversion` 等）的引擎门槛从 6 变成 9，这次没有重跑它们；`rerank_latency` 的延迟门禁按机器比较、不记毫秒数，`harvest_eval_set` 的产物是已经冻结的评测集。`typing_sim --profile convert-eval` 仍是改口径之前的 `convert_eval`（每页 9、行内日期时间开着但钉在虚拟时钟上），README 和 `profile.rs` 写明了这一点，要和现在的 `convert_eval` 完全对上需要另改并重跑它的参考臂。共享偏好的出厂每页候选数以后变了，`convert_eval` 的默认跟着变，相关基线要随那次改动重跑。

## Verification

- `crates/input-runtime/src/tests.rs` 的 `constructing_or_replacing_the_engine_tells_it_the_first_page_gate`（`new`、`new_with_touch_layout`、`replace_engine`、`replace_engine_with_touch_layout` 之后引擎门槛等于页大小）和改过的 `resizing_the_page_tells_the_engine_its_first_page_gate`（建立后就是 6）在改动前都失败（门槛是 0）。
- `cargo test -p msime-input-runtime`、`cargo test -p msime-host-api --lib`、`cargo test -p msime-engine --test golden`。
- 上文 13 份集合的对照：改动后的四份报告原样写进对应的基线文件（与 `--update-baseline` 写出的内容相同），其余九份与提交的基线逐字节相同。
