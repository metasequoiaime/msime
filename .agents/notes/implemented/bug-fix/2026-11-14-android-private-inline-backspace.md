# Agent Note: Android 私有内联表情的退格交给编辑器

Status: implemented

## Problem

#11444 包含应用自己的表情。跨进程查询可能只返回表情的文本占位，不能从末尾字素推断编辑器持有的私有替换范围。用户明确要求同时处理组合 emoji 和应用私有表情；仅修 Unicode 字素还不覆盖后一类。

## Decision

在既有字素删除基础上，查询结果只确定单个 `]` 或对象替换符 U+FFFC 时，将一次退格交给编辑器。输入法不识别标签名、不删除方括号内的文字，也不使用应用包名清单；编辑器决定是否撤回整个内联对象，普通方括号仍只删一个字符。返回了完整替换范围时继续按已知范围删除。委托后不推测选区回声或保存横滑恢复快照，避免用错误长度恢复半个对象。

## Historical audit

- [字素退格](../../implemented/bug-fix/2026-11-14-android-backspace-graphemes.md)：部分重叠，保留 ICU、多码位删除和普通文字路径，仅扩展编辑器委托的范围。
- [空框删除引用](../../implemented/bug-fix/2026-10-14-android-empty-editor-backspace.md)：部分重叠，复用成对按键发送与异常保护。

## Alternatives considered

- 所有文字都发送 `KEYCODE_DEL`：能最大程度采用编辑器的规则，但会改变所有普通文字的回声追踪和按压内水平恢复，且网页及远程编辑器不一定支持。只委托可能承载内联对象的占位结尾。
- 将 `[文本]` 直接视为表情删除：不依赖编辑器按键，但会误删用户的普通括号文字，拒绝采用。

## Verification

API 28/35 专用模拟器均安装本分支 APK，经真实系统输入法连接运行 `EmptyEditorBackspaceDeviceSmoke`、`InputFailureCategoryDeviceSmoke` 和 `QuickDeleteUndoDeviceSmoke`，六次全部通过（远端运行 `20261114-181303`）。编辑器返回无样式占位文本、本地保留 `ReplacementSpan` 时，一次退格删完整对象并保留前后文字；相邻对象只删最后一个；普通括号只删 `]`，U+FFFC 只转发一次。夹具只挂样式，删除由系统 `TextView` 处理，没有复制产品的删除算法。组合 emoji、选区、读取失败及横滑恢复旧回归继续通过。

正式 APK 与设备套件构建、`bash scripts/verify-local.sh --quick` 通过（远端运行 `20261114-180330`、`20261114-180822`）。QQ、哔哩哔哩报告版本与 HyperOS 仍需实际复验，合成编辑器不冒充原环境。

## Consequences

只有编辑器知道私有对象语义；若它的占位不以这些字符结尾、或它自己不处理退格事件，此路径无法保证应用效果。委托不会根据返回值再补一次文本删除，避免异步事件造成双删。
