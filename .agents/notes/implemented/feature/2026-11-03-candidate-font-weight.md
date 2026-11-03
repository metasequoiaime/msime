# Agent Note: 桌面候选窗字重设置

Status: implemented

## Problem

#10748：候选字体只能选字体族，选不到同一字体族里的粗体等字面。偏好里只有 `candidate_font_family`、`candidate_english_font` 和 `candidate_fallback_fonts` 三个族名字段；macOS 的 `CandidateFontCache.mm` 只按族名匹配、`candidateFontOfSize:` 不加任何字重，Windows 的 `CandidateWindow.cpp` 把正文写死成 `DWRITE_FONT_WEIGHT_NORMAL`，Linux 写给面板的 Pango 描述也没有样式词，所以候选窗永远画常规字面。手打 `JetBrainsMono-Bold` 这样的字面名在 macOS 上还会因为不是族名而退回系统字体。

维护者 2026-11-03 拍板：只做桌面的字重，不做斜体、不做移动端；默认常规，等于现在的画法；macOS、Windows、Linux 的候选窗都读它；Windows 原生设置页加控件；macOS 的控件等原生设置迁移时再加，期间经 MCP 或偏好文档设置。

## Decision

- 共享规则在 `crates/client-core/src/preferences.rs`：`CandidateFontWeight` 只有 `regular`、`medium`、`semibold`、`bold` 四档，对应 OpenType 字重 400、500、600、700（`value()`）。`Preferences::candidate_font_weight` 默认 `regular`，`skip_serializing_if` 让默认值不写进文档，没有这个键的旧文档和旧版本照常读。提交时不认识的取值被 `from_submission` 拒绝，读盘时由 `salvage_preferences` 退回默认，与其他枚举一样。宿主读不到 Rust 的常量，三个候选窗各抄一份字重表（macOS 和 Windows 的 `CandidateFontWeight.h`、Linux 的 `candidate_font_weight_value`），`scripts/test-candidate-font-weight-parity.py` 核对三份的取值、数值、缺省和预编辑下限都与 Rust 的枚举、`value()`、`preedit_value()` 一致，加档或改名时哪一端没跟上就红。
- 作用范围是候选窗里的全部文字：候选、注释、序号、释义、页码、首行状态和光标后编辑提示。macOS 的页码、行号、首行状态和提示用系统字体画（等宽数字），也按这个字重取系统字体，与 Windows、Linux 一样跟着变粗；常规时传的仍是 `NSFontWeightRegular`，与原来相同。预编辑行一直是半粗，候选字重更粗时跟到同一档，即 `preedit_value()` = max(600, 字重)，避免首行比候选还细。连击计数等特效徽标保持原来的半粗。
- 族里没有这一档字面时不换族，按 CSS Fonts 的字重匹配规则在族里就近选：要中等（500）时先找 400–500 之间不轻于它的字面，再找更轻的，最后才找更粗的；要半粗、粗体时先找更粗的，再找更轻的。所以只有常规和粗体的族（Helvetica、Menlo、宋体、多数中文字体）选中等时三端都画常规，不会一端画粗一端画常规。Windows 交给 DirectWrite 的族内匹配，Linux 写成 Pango 样式词（`Medium`、`Semi-Bold`、`Bold`）交给 fontconfig，两者本来就按这个规则选。macOS 不能直接用 CoreText 的匹配：把字重当非必需键交给 `matchingFontDescriptorWithMandatoryKeys:` 时它取下一个更粗的字面，Helvetica、Menlo、宋体选中等都会落到粗体（审查时在本机实测过），所以 `MSIMEInstalledFontFamilyDescriptorWithWeight` 列出族里与常规字面同宽的正体字面，自己按上面的规则挑。合成加粗的差别仍在：族里根本没有更粗字面时，macOS 不合成，DirectWrite 和 fontconfig 可能按系统配置合成，这是各自字体系统的行为，不在宿主里抹平。
- macOS：`AppearancePreferences` 只读共享文档的 `candidate_font_weight`（缺省读作常规，认不出的值不改现状），合并回文档时不写它；`candidateFontOfSize:` 对链上每个族和最后的系统字体都用这个字重。常规时走原来的族名缓存键和匹配，画法与加入字重之前逐字节相同。没有界面，MCP 的 `get_preferences` / `update_preferences` 和 CLI 的 `config set candidate_font_weight=bold` 是迁移前的入口。
- Windows：`CandidateFontSettings` 带上字重，一起比较、一起过邮箱；`CandidateWindow` 量和画都用同一个字重，量宽缓存随 `set_fonts` 的 `invalidate_geometry` 清空。原生设置「候选窗口 › 字体与大小」在字号下面加「字重」下拉，选项与字重表在 `src/candidate/CandidateFontWeight.h`，预览同步。Win7 设置窗口没有加控件，它的枚举字段要求文档里一定有这个键，而常规时文档里没有。
- Linux：`CandidateFontPolicy.h` 在字体列表之后、字号之前写样式词，常规不写，所以没动过字重的用户写给面板的描述不变，「没改过就不碰桌面字体」的规则照旧；选了字重算一次字体选择。Fcitx5 原生画布的序号、释义、页码按 `NativeCanvasContent::font_weight` 画，拼音行取 `candidate_preedit_font_weight`（至少半粗）；这个字重由 `FcitxEngine` 从偏好读，不从字体描述里取：画布的描述就是 classicui 的 `Font`，偏好是默认值时 `CandidateFontSync` 不碰它，用户在 fcitx5-configtool 里写的「Sans Bold 12」「Sans Light 12」原样留在描述里，若从描述取字重，没动过候选字重的用户序号和页码也会变粗或变细。候选正文仍按描述画，与原来相同。IBus 面板认不认这个样式词取决于面板本身，GNOME Shell 仍然不写。
- 顺带把 `system_fonts.rs` 和 `aliases_macos.rs` 里「macOS 候选窗把名字直接交给 CoreText、PostScript 名也能用」的过时说法改成实际情况：macOS 候选窗只认族名。

## Alternatives considered

- **列出族里真实存在的字面让用户选**（把字体下拉改成族加字面两级）：最精确，能选到 Light、Heavy 这类字面；但每个平台列字面的方式不同，存下的字面名换一台机器、换一个平台就对不上，跨端同步的偏好会失效，macOS 现在还会把字面名退回系统字体。四档抽象字重在三个字体系统里都有就近匹配，能跨端同步。
- **再加 light、heavy 等更多档位**：CSS 有九档；但中文字体大多只有常规和粗体两三个字面，多出来的档位在多数字体上落到同一个字面，看起来像设置失效。
- **斜体**：问题里也提了；汉字没有真斜体，三端都只能合成倾斜，效果差，维护者决定不做。
- **预编辑行完全跟随候选字重**：选常规时首行会从现在的半粗变成常规，改变了现有画法；保持「至少半粗」只在选粗体时变化。
- **macOS 直接用 CoreText 的就近匹配**（字重作非必需键）：代码最少；但 CoreText 遇到缺档时取更粗的字面，只有常规和粗体的族选中等会画成粗体，与 Windows、Linux 不一样，同一份同步的偏好在两台机器上一粗一细，中等和粗体在 macOS 上也分不开。
- **Linux 画布从字体描述里取字重**：不用往画布内容里多传一个字段；但描述来自 classicui 的 `Font`，用户自己写的样式词会带到序号和页码上，偏好是常规时也改变现有画法。
- **只在 Rust 留枚举、删掉 `value()` 和 `preedit_value()`**：宿主反正各抄一份；但那样数值规则就没有一个权威的出处，三份之间也没有东西核对。留着 Rust 的数值作准，用对照脚本把三份宿主表钉在它上面。
- **macOS 先在共享 React 页或冻结的 AppKit 设置窗口里加一行**：提建议的用户在 macOS 上，加一行最快；但 2026-11-03 的决定是 `packages/ui` 只修缺陷、AppKit 设置窗口冻结，维护者选择等原生设置迁移。

## Consequences

- 收益：桌面三端读同一个字段，默认值不改任何现有用户的画法；Windows 用户在原生设置页就能改，macOS 和 Linux 用户可以经 MCP 或偏好文档改。
- 代价：macOS 暂时没有界面，提建议的 macOS 用户要等原生设置迁移；macOS 设置窗口里的候选预览不读共享文档的字重（只有输入法进程调用 `applySharedCandidatePreferences:`），共享 React 设置页的预览也不画字重，两处预览一直显示常规，等原生设置迁移时一起做；三端在族里根本没有更粗字面时的合成加粗不一致（macOS 不合成，Windows 和 Linux 可能合成）；Linux IBus 面板对样式词的支持没有逐个验证。
- 移动端键盘不读这个字段，存着的值在手机上原样保留、不起作用；以后要做移动端，iOS、Android、HarmonyOS 要一起做。
- 相关笔记：[Windows 原生设置窗口自己画候选字体各行](2026-10-11-windows-settings-candidate-fonts.md)（字体组的现有各行），[全部界面改为原生](../../proposed/architecture/2026-11-03-all-native-ui.md)（macOS 控件的去处）。

## Testing

- Rust：`crates/client-core/tests/candidate_font_weight.rs`（缺省不写、存取往返、拒绝未知取值、四档数值和预编辑下限），MCP 的 `the_candidate_font_weight_is_set_and_returned_to_regular` 与镜像枚举序列化测试。
- 对照：`scripts/test-candidate-font-weight-parity.py` 核对三端字重表与 Rust 一致（改一端的数值或下限会红，提交前手动改坏验证过）。
- macOS：`candidate-font-cache` 核对常规与原匹配同一个描述符、粗体落到 Helvetica Bold、只有一个字面的族退回自己的字面、Helvetica 和 Menlo 选中等落到常规字面而不是粗体、半粗先取更粗的 Menlo-Bold 和 Helvetica-Bold、苹方选中等和半粗落到 PingFangSC-Medium 和 PingFangSC-Semibold；`skin-preview` 核对共享文档的字重作用到主字体和苹方回退、认不出的值不改现状、合并时不写回、缺省回到常规。
- Windows：`tests/ui/candidate_font_reload.cpp` 核对四档解析、预编辑下限、只换字重也算字体变化、非法取值被拒；渲染和设置页要在 Windows 上做设备验收。
- Linux：`candidate_font_policy.cpp` 核对样式词的位置、常规时描述不变、选字重算字体选择；`candidate_native_canvas_chrome.cpp` 核对画布各段按 `font_weight` 取字重、拼音行下限，以及偏好是常规而描述带「Bold」「Light」时序号、释义、页码仍是常规、拼音行仍是半粗。
