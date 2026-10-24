# Agent Note: Android 补齐 Unicode 18 的 w/y/z 下标

Status: implemented

## Problem

#8635 的字符清单包含 U+209D、U+209E、U+209F；当前 `EnglishLetterHoldPolicy` 的 w/y/z 下标槽位为空。[Unicode 18 官方字符表](https://www.unicode.org/charts/PDF/U2070.pdf)已经把它们分配为拉丁小写 w、y、z 下标。旧版 Python `unicodedata` 仍报告未分配，不能据此排除这些需求。

旧 Android 字体也可能缺少新码位，仅在字符表添加选项会把菜单画成缺字方框。

## Decision

英文长按加入三个真实下标码位，点击仍提交原始 Unicode 字符。复用 `CandidateSymbolLabel` 的局部 `ReplacementSpan`：设备字体支持时正常绘制，不支持时用当前字体的小写字母缩小并降低基线，只替代绘制。长按按钮与候选标签共用该处理，原有字符、无障碍描述、点击回调和候选索引不改。

回退 `Paint` 是当前文字画笔的副本，保留颜色和字体；缩小后的下降部与原行的下降部对齐，避免下标 y 被裁剪。字号、颜色与皮肤变化在绘制时读取，不缓存字形支持结果，不引入字体资源或网络依赖。

## Historical audit

- [英文长按变体](../../implemented/feature/2026-10-21-android-english-letter-options.md)：部分取代。继续保留真实字符、缺项不伪造及既有长按生命周期；三个已分配字符通过局部绘制补上旧字体的限制。
- [粗等号候选绘制](../../implemented/bug-fix/2026-10-15-android-heavy-equals-candidate-glyph.md)：扩展同一个局部标签绘制机制，不改变粗等号规则与上屏码位。
- [手势候选图像](../../implemented/bug-fix/2026-10-19-android-hand-emoji-candidate-glyph.md)：同属字体兼容，但位图资源与肤色策略无关，保持独立。
- 其余英文长按手势、符号目录和九键笔记仍持有各自交互，没有同一字符补齐提案。

## Alternatives considered

- 等待系统字体支持后再展示：不增加绘制代码，但 Android 老版本没有固定的字体更新路径，会持续缺少 issue 明确列出的三个选项。
- 内嵌完整的新字体：可以直接使用新码位的字形，但为三个可由当前拉丁字体绘制的下标增加字体体积、来源与许可证管理，没有必要。
- 把选项或上屏结果替换成普通 w/y/z：所有编辑器都能显示，但会改变用户选中的字符。局部绘制保持真实下标的复制和上屏语义。

## Verification

`EnglishLetterHoldPolicySmoke` 新增三个明确的字符断言，原实现先在 w 下标断言失败，补表后通过。`LetterHintPunctuationDeviceSmoke -e english-hold true` 增加实际长按并选中三个下标，核对合成编辑器的原始码位与英文状态；`-e unicode18-hold true` 可独立执行相同的三个新增字符步骤，完整套件仍保留原有 26 键及符号、中文切换验收。

`HeavyEqualsCandidateDeviceSmoke -e subscripts-only true` 从安装产品加载真实标签，检查局部 span 范围、原始字符、缺字时缩小和降低基线、浅深色及高亮颜色，以及不污染原画笔。API 35 上，旧安装产品先按预期报「Unicode 18 下标缺少字形兜底」。本分支产品在独立 API 35 模拟器的字形专项与原有粗等号回归均通过；本地完整 quick 通过。API 28 的字形专项与原有粗等号回归均通过，独立新下标专项通过真实触摸依次上屏三个原始码位。API 35 真实长按完整套件在 b 键步骤失败，独立新下标专项在 y 键步骤遇到编辑器与系统 ANR，均未计为通过；API 28 的完整英文长按套件通过，覆盖三排 26 个字母、多行菜单、三个新下标、下滑数字和 O 键返回中文输入。只读复审未发现阻塞缺陷。设备专项包使用本分支重新编译的原生 Java 宿主，资源与未修改的 JNI 二进制取自该设备已安装 APK，保留 SHA-256；这项证据仅覆盖本次 Java 字符表和绘制，不代表重新构建了 Rust/JNI。

## Consequences

PR 首轮 CI 的失败来自基线 Windows 引导文案与共享设置不一致，定位到 `windows settings art`，修复归属 #9389；Android 宿主等其余检查通过。基线修复合入后重新验证 PR 合并结果，设备验收仍以上述原始码位和实际字体绘制证据为准。

菜单和候选里的新下标可在旧字体上辨认。外部编辑器仍使用自己的字体；绘制回退只属于输入法界面，上屏保留新码位，不能保证旧版外部编辑器也有相应字形。
