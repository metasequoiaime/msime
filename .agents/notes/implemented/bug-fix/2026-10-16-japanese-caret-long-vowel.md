# Agent Note: 日文光标插入接受长音符按键

Status: implemented

## Problem

日文方案在末尾键入 `-` 时，`handle_character` 把它作为 `SchemeKey::Minus` 交给罗马字方案，读音中得到长音符 `ー`。光标放到组字内部后，同一入口先走 `insert_at_caret`，该分支只认字母与撇号，没有接收日文的 `-`。例如 `kohi-` 在 `ko` 后补长音时，按键未处理，组字保持 `kohi-`，无法改成 `ko-hi-`。

活跃笔记按 `insert_at_caret`、日文光标和长音符检索，没有持有光标插入规则的旧笔记。[罗马字拼法与假名位置](../../implemented/bug-fix/2026-10-08-japanese-romaji-spellings-and-kana-slot.md)约束正反转换，与本次会话编辑过滤无关；本次直接复用现有方案的 `set_raw_input`。

## Decision

普通日文方案的光标插入接受 `-`，沿已有编辑重建路径保留原文、更新读音及候选，并把光标放在新键之后。范围仅限 `LocalInputMode::None` 下的 `JapaneseRomaji`。

## Alternatives considered

- **由宿主对 `-` 特判并重写整段组字。** 宿主已知道日文的长音键，可以在自己的输入路径补齐。但每个平台都要复制原文编辑规则，且共享会话 API 仍拒绝此键；过滤规则应与末尾输入一样由引擎决定。
- **所有方案的光标插入都接受 `-`。** 条件最短，也方便编辑带连字符的原文。但其他方案并不把它当拼写键，会改变既有标点和翻页处理，不能为日文扩大接收范围。

## Verification

两项真实 `Session` 测试在修复前均因 `handled` 为假失败；修复后在开头和中间插入 `-` 都保持组字、光标和假名候选正确，开头插入后退格能删回原文。全拼、双拼对照用例确认 `-` 未处理且快照不变。

`cargo test -p msime-engine --no-fail-fast` 通过 2204 项单测和 31 项 golden，34 项需要显式启用的词库及性能用例忽略；`cargo clippy -p msime-engine --all-targets -- -D warnings`、格式和笔记检查通过。

独立审查未发现阻塞问题，复跑三项新增及三项相邻测试通过；验证范围仍是共享引擎会话。

`bash scripts/verify-local.sh --quick` 完整复跑通过。首次 macOS 构建失败，独立重跑捕获到上游语音运行库下载临时 `502`，重试后宿主构建通过；随后整轮 quick 通过。Linux 容器、Windows 交叉编译及 HarmonyOS 完整工程因缺少对应环境跳过，不能算平台验收。

## Consequences

收益是日文长音符的插入与末尾输入一致，修正 `kohi-` 时不必先删掉后半段。新增一个与方案绑定的接收条件，依旧走原有重建逻辑；其他方案与临时模式没有扩展字符集。不改变宿主的按键路由。自动化覆盖共享引擎会话，未覆盖系统输入框里的实际操作。
