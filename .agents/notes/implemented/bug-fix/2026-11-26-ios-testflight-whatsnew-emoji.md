# Agent Note: TestFlight「What to Test」里的 emoji 被 App Store Connect 拒绝

Status: implemented

## Problem

2026-11-26 nightly（run `45778076004`，`3d5cd828`）的 `iOS nightly / release` 在「Hand the build to its testers」失败：构建、打包、上传都成功，失败出在往 `betaBuildLocalizations` 写 `whatsNew` 时，App Store Connect 回

```
ENTITY_ERROR.ATTRIBUTE.INVALID.INVALID_TEXT
Text for whatsNew contains invalid characters:'[⚡, 🔧, 🐞, ✨, 🙏]'.
```

这五个字符正是发布说明分组标题开头的 emoji（`### ✨ 新功能`、`### ⚡ 性能`、`### 🐛 问题修复`、`### 🔧 开发与构建`、`### 🙏 感谢`；模型自己写标题时用哪个同类符号不固定，这次「问题修复」是 🐞 而不是 🐛）。`platforms/ios/scripts/submit_testflight_review.py` 的 `testflight_text()` 只处理 Markdown 记号——去掉 `#`、`**`、反引号，把小组名换成方头括号——emoji 原样传到 Apple 的接口。

这条 nightly 之所以一直没暴露：此前 `iOS nightly / release` 在更早的「Build native iOS host」就失败（#11912 / #11913），这一步从未走到；构建修好后它成了下一个红点，而且每天都会红，外部测试组也收不到构建。

## Decision

`testflight_text()` 在生成 TestFlight 文本时去掉 emoji：

- 用一个 `EMOJI` 正则去掉「杂项符号」「装饰符号」「杂项符号与箭头」「补充符号与象形文字」这几段、变体选择符（`U+FE0F`）、零宽连接符（`U+200D`）与键帽组合符（`U+20E3`）。正文里的 emoji 与标题里的走同一条规则，不单独放行。
- 去掉后清掉行首空格（标题的 `emoji + 空格` 会剩一个前导空格），并把连续空格并成一个；`utf16_length` 与 4000 码元上限按去掉之后的文本计算，和真正发给 Apple 的内容一致。
- 只改 TestFlight 这一路；GitHub Release 正文继续用 `generate-release-notes.py` 的原始 Markdown（那里 emoji 是格式的一部分）。

## Alternatives considered

- **在 `generate-release-notes.py` 里去 emoji**：能一次覆盖所有消费方，但 GitHub Release 与官网的发布说明也会失去分组图标；那些地方 emoji 有意义，没有必要为 TestFlight 的限制牺牲它们。
- **只去掉报错里点名的五个字符**：下一次模型把「问题修复」写成 🐞 或别的符号就再次失败；按 Unicode 区块整体去掉更稳，也不依赖模型措辞。
- **只保留 ASCII**：会把中文标题和条目一起删掉，TestFlight 的说明本来就是给中文测试者看的。
- **在 `set_whats_new()` 里过滤**：也能拦住 Apple，但长度上限已在 `testflight_text()` 里算过一遍，先发后过滤会让截断按偏长的文本判断；放在 `testflight_text()` 里只算一次。

## Consequences

收益：TestFlight 的「What to Test」只剩纯文字，分组标题读作「新功能」「问题修复」等，App Store Connect 不再拒绝；外部组能继续拿到 nightly。代价：测试者看不到分组图标，正文里作者写的 emoji 也会消失；这是 Apple 的接口限制，不改就只能二选一。改动只在 iOS 的 TestFlight 文本生成，其他平台、发布流程的其他步骤和设置字段都不受影响。

## Verification

- `platforms/ios/tests/TestFlightSubmissionTests.py` 新增 `test_emoji_are_removed_before_app_store_sees_the_text`：标题与正文里的 emoji 都被去掉，且结果里搜不到 `EMOJI`；原有 `test_release_notes_become_plain_text_for_testflight` 的期望改成没有 emoji 的标题。
- 本机用装了 `pyjwt[crypto]==2.10.1` 的虚拟环境跑该文件，16 项全部通过（此前缺 pyjwt 时整组跳过）。
- 没有在真实 App Store Connect 上重跑；下一轮 nightly 的「Hand the build to its testers」是这次改动的线上验收。
