# Agent Note: 九宫格左列音节先列正好拼完的、再按单字频度排

Status: implemented

## Problem

#6654：安卓九宫格打 `74`，左列是 shi、pi、pian、piao、pie、pin、ping、qi、qia……，最常用、正好两键拼完的 qi、ri、si 排在几十个 p、q 开头的音节后面，si 在最底下。

左列由 `SpellingTable::spellings_for` 给出，规则来自参考实现（NK:238-250）：按覆盖的数字位数从多到少，位数相同按字典序。`74` 能起头的音节都覆盖两位数字，于是整列退化成字典序；只有首选候选的读音会被 `refresh` 转到最前面（这里是 shi），其余顺序和用户打得多不多、还要补几键都没有关系。出货词库上复现的左列与截图逐项一致。

## Decision

- `spellings_for` 先按覆盖位数排（不变），位数相同时：
  - 正好拼完已打数字的音节（编码长度不超过已打位数）排在还要往后补键的音节前面。`74` 下 pi、qi、ri、si 在 shi、pian 前面；`426` 下 gan、gao、han、hao 在 gang、hang 前面。
  - 同一组里按 `SyllablePrior` 的单字频度从高到低，频度相同才按字典序。
- 频度用音节自己的分数（新增 `SyllablePrior::syllable_score`），不用切分路径用的前缀分数：前缀分数取以它开头的音节里最高的那个，pin 会沾 ping 的光、qia 会沾 qian 的光。
- `refresh` 把 `SyllablePrior` 的构造挪到 `spellings_for` 之前；数字全部锁定时 `spellings_for` 本来就返回空，不需要它。
- 首选候选的读音仍被转到最前面，按键字母和数字仍跟在音节后面，`nine_key_spellings` 的组成不变（见 [九宫格展开面板](../feature/2026-10-08-nine-key-expanded-panel.md)）。

出货词库上 `74` 现在是 shi、qi、si、ri、pi、shuo、shang、qing……，候选本身的先后不变：`convert_eval --nine-key` 在同一份词库上改动前后输出逐字节相同。

## Alternatives considered

- **只把正好拼完的音节提前，组内仍按字典序。** 最强的理由是不碰频度、规则一眼可见，也足以让 qi、ri、si 回到前几位。没有采用：补键的那一组有三十多项，字典序下 sha、shai 排在 shang、shuo 前面，滚动找常用音节的问题只是往后挪了；频度已经在切分剪枝里算好，拿来排左列没有额外开销。
- **按频度排、不分正好拼完和补键。** 理由是频度最能反映用户想打什么。没有采用：shuo、shang 的单字频度高过 ri、pi，会把正好两键的音节又挤下去，而 issue 要的正是「更简单的组合」先出现；左列是给用户钉音节边界用的，已打的数字恰好拼完一个音节是最直接的信号。
- **按覆盖位数之外先比编码长度（短的在前）。** 等价于把 `426` 下的 ga、ha 也提到 gan 前面，违背「覆盖越多越前」——用户多打的数字应该被尽量用上，这条参考实现的规则没有理由改。

## Consequences

- 收益：九键左列最前面几项就是已打数字的完整音节和最常用的补全，不再需要滚动到列表底部。iOS、HarmonyOS 用同一份 `nine_key_spellings`，一起受益。
- 代价：一位数字时正好拼完的单字母音节排在前面，`6` 的左列在首选读音之后先是 `o`，再是频度最高的 m、n 开头音节；`bridge_nine_key_spelling_choices` 里选第 0 项因此从 `ma` 变成 `o`。
- 偏离了参考实现，golden 里 5 个场景的 `nine_key_spellings` 顺序随之更新，只换顺序、不增删项。
- `real_dictionary_lists_exact_syllables_before_completions` 在设了 `MSIME_EVAL_RESOURCES` 时用出货词库核对 `74`，没设时跳过。
