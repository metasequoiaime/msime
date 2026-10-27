# rare-chars-threshold：隐藏生僻字（可调）

和 [hide-rare-chars](../hide-rare-chars/)（隐藏生僻字）同一套判定，多两项设置，让你自己定隐藏得多狠。它也是插件设置项（`[[script.settings]]`、`setting_value`）的参考样板。

| 项 | 值 |
| --- | --- |
| 钩子 | `candidates` |
| 能力 | `read_candidates`（读候选窗口前 20 行的文字和来源）、`filter_candidates`（把其中一些行隐藏）；读设置不需要能力 |
| 设置 | `scope`（隐藏范围，选项）、`keep_rows`（保留前几行，数字 1 到 9） |
| 许可证 | 代码 MIT；内嵌的常用字集来自 Unihan（Unicode-3.0）和 OpenCC（Apache-2.0），见包里的 NOTICE.txt |
| 模块 | 约 8.5 KiB，内存 2 页 |

## 设置

在「插件 → 脚本插件（实验）」里打开插件详情，「设置」一组里有两项，改完点「保存设置」。改设置不需要重新授权，输入法在下一次聚焦时用新值重建插件。

| 设置 | 取值 | 默认 | 作用 |
| --- | --- | --- | --- |
| 隐藏范围（`scope`） | 只隐藏单字（`single`）、含生僻字就隐藏（`any`） | 含生僻字就隐藏 | `single` 只隐藏恰好是一个生僻字的候选，「囧」隐藏，「囧rz」「中囧」照常显示；`any` 和 hide-rare-chars 相同 |
| 保留前几行（`keep_rows`） | 1 到 9 | 1 | 窗口最前面这几行即使含生僻字也照常显示。hide-rare-chars 固定是 1 |

两项都取默认值时，行为和 hide-rare-chars 逐行相同。

| 候选窗口 | 默认 | 只隐藏单字 | 保留前 3 行 |
| --- | --- | --- | --- |
| 中、囧、中囧、钟、丂、囧rz、𠮷、终 | 隐藏 囧、中囧、丂、囧rz、𠮷 | 隐藏 囧、丂、𠮷 | 隐藏 丂、囧rz、𠮷 |

## 为什么另起一个插件

给 hide-rare-chars 加设置会改变它的清单和模块，内容哈希一变，已经装了它的用户要重新授权，行为也可能跟着变。另起一个插件，hide-rare-chars 的包逐字节不变（把逐字扫描挪进 `shared/rare_scan.rs` 之后，它编出的模块和内容哈希都和以前相同），只想要固定行为的用户也不必多看两项设置。两个插件二选一，一起开没有意义。

## 设置项怎么读

- 清单里每项写 `key`、`title`、可选的 `description`、`type` 和默认值，`choice` 另有 `options`，`number` 另有 `min` 和 `max`（见 `pack/plugin.toml`，规则见 docs/plugins.md 的「设置项」一节）。
- `settings::value(index, buffer)` 按清单顺序读第 `index` 项现在的值：选项是它的 `value`（`single`、`any`），数字是十进制文本（`3`）。宿主交来的值已经按清单核对过，不合规的会换回默认值。
- 插件在 `msime_init` 里读一次，存进静态变量：设置在所有钩子和 `msime_init` 里都能读，值变了输入法会重建实例，所以不用每次 `candidates` 都重读（每读一次是一次宿主调用，100 多 fuel）。读不出来时用默认值。
- `msime_init` 里写一行日志 `settings: scope=any keep_rows=1`，`msime-pack run` 把它打在实例化的那个事件上，夹具靠它核对设置确实读到了。

## 其余规则

什么算生僻、哪些行不动（引擎词库以外的来源、云和 AI、日文窗口、第 21 行以后）、工作量上限和日志格式都和 hide-rare-chars 相同，见[它的 README](../hide-rare-chars/README.md)。`keep_rows` 只是把「第 0 行不动」扩成「前几行不动」；`single` 时不止一个字的行只看有没有假名（判断是不是日文窗口），不再逐字查生僻。

## 夹具与核对

`fixture.jsonl` 是一条覆盖两项设置差别的窗口，加上 hide-rare-chars 的全部 27 个事件（含它的七个最坏情况）。三次运行：

- 默认设置：`expected.jsonl`。`check_output.py` 把第 2 行起的输出和 `hide-rare-chars/expected.jsonl` 逐行比较，两个插件以后不会悄悄分开。
- `cases/single/`：`settings.json` 写 `{"scope": "single"}`，`scripts/test-script-plugins.sh` 把它交给 `msime-pack run --settings`，输出和那里的 `expected.jsonl` 核对。
- `cases/keep-three/`：`{"keep_rows": 3}`，同上。

## fuel

实测（rustc 1.97.1、wasmi 2.0），`candidates` 单次上限 125 000：默认设置的最坏情况约 40 700（和 hide-rare-chars 的 40 600 几乎相同，多出的是每行一次「是不是单字」的判断），保留前 3 行约 40 300，只隐藏单字约 32 500（不止一个字的行不逐字查）。都在上限的一半 62 500 以内。读设置在 `msime_init` 里，算进初始化的 fuel，不算进 `candidates`。
