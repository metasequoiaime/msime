# Agent Note: Shift 字母入口后只有字母时，被拒字母转回普通组字

Status: implemented

## Problem

#10123：macOS 拼音模式下打驼峰英文 `TextBlock`，没按回车也没切换，到 `B` 时 `Text` 自己上屏。报告列出的 `TxB`、`TbU`、`UbU`、`TuU`、`Ui`、`Uq`、`Uw` 等都复现，`Ua`、`Ue` 不复现。

根因在共享引擎 `InputSession::handle_local_character`，不在 macOS 宿主。Shift+T 进日期时间模式（只收小写），Shift+U 进 Unicode 模式（只收十六进制），两者默认开启。模式不收的键走通用拒绝分支：先原样提交模式原文，再把当前键交还宿主。`a`、`e` 是十六进制所以留在 Unicode 模式，`i`、`q` 被拒就把 `U` 上屏；`TextBlock` 走到大写 `B` 时日期时间模式拒绝，`Text` 被提交，`B` 交给宿主。[空入口大写衔接](2026-10-19-bare-shortcut-capital-continuation.md)只处理“入口后紧跟一枚被拒大写”，已有字母内容或被拒的是小写时仍提交。

## Decision

全拼、双拼中，本地模式拒绝一枚 ASCII 字母时，如果同时满足下列条件，就把模式原文连同这枚字母一起移进普通组字，消费按键、不提交：

- 模式由 Shift 大写字母进入，即预编辑首字节是大写字母；
- 预编辑全部是 ASCII 字母；
- 预编辑没到 `GENERATED_MODE_INPUT_LIMIT`，拒绝原因是字符集而不是长度上限。

判定集中在 `InputSession::rejected_letter_continues_composition`，取代原来只看“预编辑长 1 且被拒的是大写”的条件；转移本身仍用 `replace_active_raw_input`，之后的大小写字面分段、退格、原样提交复用[大写字面片段](2026-10-19-uppercase-literal-pinyin-boundaries.md)的规则。于是 `TextB` 进普通组字，`lock` 继续拼在后面；`Ui`、`UbU`、`TxB` 都留在候选框里。

下列情况保持[快捷模式拒绝键](2026-10-15-local-mode-rejected-key-passthrough.md)的提交并透传：

- 预编辑含数字或符号（`U0041g`、`V1Z`）；
- 模式原文到了长度上限；
- 被拒的是数字、标点或其他非字母；
- 符号入口 `/`、`@`；
- 五笔等非全拼、双拼方案；
- 光标在组字中间（走[光标拒绝键](2026-10-16-local-caret-rejected-keys.md)的 `insert_at_caret` 路径，本次不动）。

宿主（macOS、Windows、Linux 以及移动端实体键盘）都把字母交给同一引擎，不改宿主代码。

## Historical note audit

- [快捷模式拒绝键](2026-10-15-local-mode-rejected-key-passthrough.md)部分重叠：本篇只取代“Shift 字母入口加纯字母内容、再遇到被拒字母”的情形，含数字或符号的内容、非字母拒绝键、长度上限和符号入口仍归它。
- [空入口大写衔接](2026-10-19-bare-shortcut-capital-continuation.md)部分重叠：它的“空入口加被拒大写”是本篇条件的子集，转移机制不变；它记的“已有模式内容、小写拒绝仍提交”由本篇取代。
- [超级简拼空入口大写](2026-10-21-super-jianpin-capital-continuation.md)无冲突：`J` 加大写仍由接受判断拒绝后走同一转移；`J` 后的字母本来都被接受。
- [光标拒绝键](2026-10-16-local-caret-rejected-keys.md)无关：光标在中间时不经过这条分支。
- [五笔打开 Shift 模式](2026-10-25-wubi-scheme-free-shift-modes.md)无冲突：五笔下的 U、T、K 被拒仍提交并透传，五笔码不处理大写字面。
- [Shift 入口总开关](../feature/2026-10-11-shift-letter-entry-switch.md)持有默认值和入口集合，本篇不改。

## Alternatives considered

- **只在被拒大写时转回，小写仍提交**：和 #8809 的思路一致、改动更小；但 `Ui`、`Uq`、`Tb` 后的小写拒绝同样会把入口字母单独上屏，报告里一半的用例仍复现。
- **默认关闭 Shift 字母入口或 Unicode、日期时间模式**：大写英文行为最统一；但改变已有用户习惯和触屏菜单入口，issue 维护者没有要求改默认值。
- **把被拒字母追加进模式原文，留在模式里**：不会上屏；但模式本来就不认识这些键，留在模式里只会显示一个查不到东西的原文，回车、退格也仍按模式处理，不如交给已经支持大小写字面的普通组字。

## Consequences

收益：驼峰英文和以 U、T、K、V 开头的英文串在第一个被拒字母处回到普通组字，不再中途上屏；用户用空格、回车或原样提交时拿到完整原文。

代价和留下的边界：

- 全是十六进制字母的 U 开头词（`Uface`）、全小写的 T、K 开头词（`Today`）一直留在对应模式，靠模式自身的原文候选上屏；Emoji、颜文字、超级简拼、拆字接受任意字母，驼峰串整串留在模式里。这些属于入口设计取舍，用户可用 `shift_letter_entry` 总开关关掉。
- 表达式模式 `V` 后直接打字母（数字前的单位字母）不再提交 `V`，而是 `Vx` 进普通组字。
- golden `local_unicode_edges` 的 `U` 加 `g` 步骤改为保留 `Ug` 组字，随后的取消清空组字。

## Verification

新增引擎回归 `rejected_letter_after_letter_only_shortcut_keeps_the_whole_word_composing` 在旧实现上以 `Quanpin TextBlock: B gave KeyResult { handled: false, commit: Some("Text") }` 失败，修复后通过；它覆盖报告里的全部用例，全拼、双拼各一遍，并核对退格和十六进制字母仍留在 Unicode 模式。五笔下 `Ui` 仍提交 `U` 并透传另有回归。真实共享运行时回归覆盖 `TextBlock`、`TxB`、`UbU`、`Ui`、`Uq`、`KaA` 的无提前上屏和原样提交。原有拒绝回归里纯字母那几组改成非字母拒绝，含数字、符号的组合保留原断言。

全量结果：引擎库 2785 通过、57 个既有忽略；golden 31 通过（300 个合成场景，只有 `local_unicode_edges` 的 `Ug` 一步按新规则更新）；runtime 408 通过；两 crate 全部 targets 的严格 clippy、`cargo fmt --all --check` 和笔记校验通过。表达式回归 `expression_mode_evaluates_what_follows_shift_v` 中 `V` 后的 `x` 改为断言 `Vx` 留在普通组字。

没有在 macOS、Windows、Linux 真实编辑器里验收。
