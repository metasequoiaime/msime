# Agent Note: 日文与临时日文接受长音符按键

Status: implemented

## Problem

日文方案在末尾键入 `-` 时，`handle_character` 把它作为 `SchemeKey::Minus` 交给罗马字方案，读音中得到长音符 `ー`。光标放到组字内部后，同一入口先走 `insert_at_caret`，该分支只认字母与撇号，没有接收日文的 `-`。例如 `kohi-` 在 `ko` 后补长音时，按键未处理，组字保持 `kohi-`，无法改成 `ko-hi-`。

#9069 暴露同一遗漏在临时日文的入口过滤中：`Rko-` 的减号既不被末尾字符路径接收，也不在临时模式的 `spelling_symbols` 中。宿主把它交给标点入口时，Runtime 先结束组字，再输出减号；中途补长音也被光标插入过滤掉。

[罗马字拼法与假名位置](../../implemented/bug-fix/2026-10-08-japanese-romaji-spellings-and-kana-slot.md)约束正反转换，不持有会话入口的按键过滤规则；光标编辑继续复用现有方案的 `set_raw_input`。

## Decision

普通日文方案及临时日文模式的光标插入接受 `-`，沿已有编辑重建路径保留原文、更新读音及候选，并把光标放在新键之后。临时日文的末尾字符入口同样把 `-` 转成 `SchemeKey::Minus`；模式通过 `spelling_symbols` 声明减号属于拼写，Runtime 的字符、标点两条宿主路径都保留组字。选词后回到原中文方案，不继续声明减号为拼写。

临时英文、表情等其他本地模式不接收减号；原中文方案和双拼布局不改变临时日文的罗马字规则。macOS 已有的日文减号避让翻页与以词定字逻辑继续生效，无需在宿主复制组字规则。

## Historical audit

- [本地模式拒绝键直通](2026-10-15-local-mode-rejected-key-passthrough.md)：部分重叠，临时日文的减号是有效拼写，其他拒绝键继续直通。
- [本地光标拒绝键](2026-10-16-local-caret-rejected-keys.md)：部分重叠，中途与末尾接收范围保持一致，只扩充临时日文的长音键。
- 临时日文候选缓冲、日文模型缓存、日文读音转换的命中管理存储与转换，不改变其决定。本篇原有普通日文的长音修复继续成立，原地补齐同一规则的临时模式路径。

## Alternatives considered

- **由宿主对 `-` 特判并重写整段组字。** 宿主已知道日文的长音键，可以在自己的输入路径补齐。但每个平台都要复制原文编辑规则，且共享会话 API 仍拒绝此键；过滤规则应与末尾输入一样由引擎决定。
- **所有方案的光标插入都接受 `-`。** 条件最短，也方便编辑带连字符的原文。但其他方案并不把它当拼写键，会改变既有标点和翻页处理，不能为日文扩大接收范围。

## Verification

普通日文的两项真实 `Session` 回归在修复前均因 `handled` 为假失败；修复后在开头和中间插入 `-` 都保持组字、光标和假名候选正确，开头插入后退格能删回原文。全拼、双拼对照用例确认 `-` 未处理且快照不变。

临时日文新增两项引擎回归和一项真实引擎 Runtime 回归，覆盖全拼、双拼、字符及标点两条宿主入口，检查 `Rko-hi-` 全程不提前上屏、候选包含并可提交「コーヒー」、选词后恢复原方案；中途插入减号再退格恢复原文与光标。

`cargo test -p msime-engine -p msime-input-runtime --locked` 通过引擎 2383 项、golden 31 项及 Runtime 327 项；49 项需要显式启用的词库及性能用例忽略。两个 crate 的 `cargo clippy --all-targets -- -D warnings`、修改文件的 Rust 格式和笔记检查通过。

macOS 的 `shortcut-test --temporary-japanese-long-vowel` 使用真实 Rust 会话、原生 `handleEvent:` 和合成编辑器，覆盖全拼、双拼，以及减号翻页打开和关闭的四种组合。夹具复用引擎 golden 的微型日文模型，满足 Host API 的资源门槛，不读取本机词库或输入资料。

`ctest --test-dir target/macos-isolated -R '^shortcut(-translations)?$' --output-on-failure --no-tests=error` 通过完整快捷键与释义专项。首次独立运行曾在 `TestAccountGlossWaitsForDictionary` 等待释义时失败；完整 quick 构建后专项及整套重跑通过，未放宽等待或删除断言。验证覆盖原生按键入口，不包含系统 IMK 投递或已安装输入源的人工验收。

`bash scripts/verify-local.sh --quick` 通过，包含 macOS 原生宿主与共享 Apple bridge 构建；Windows 在本机可运行的 171 项测试通过，2 项因宿主差异排除、63 项需 Windows 构建。Linux 容器、Windows 交叉编译及 HarmonyOS 完整工程因缺少对应环境跳过，不能算平台验收。

## Consequences

收益是日文长音符的插入与末尾输入一致，临时日文也能输入 `Rko-hi-` 并选择「コーヒー」，修正拼写时不必先删掉后半段。代价是临时日文多声明一个拼写符号，宿主必须尊重该声明，不能先按标点结束组字；其他方案与本地模式没有扩展字符集。自动化验证范围以本篇实际记录为准，不以共享引擎结果代替系统输入框验收。
