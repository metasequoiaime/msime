# Agent Note: 桌面候选窗字重设置

Status: implemented

## Problem

#10748：候选字体只能选字体族，选不到同一字体族里的粗体等字面。偏好里只有 `candidate_font_family`、`candidate_english_font` 和 `candidate_fallback_fonts` 三个族名字段；macOS 的 `CandidateFontCache.mm` 只按族名匹配、`candidateFontOfSize:` 不加任何字重，Windows 的 `CandidateWindow.cpp` 把正文写死成 `DWRITE_FONT_WEIGHT_NORMAL`，Linux 写给面板的 Pango 描述也没有样式词，所以候选窗永远画常规字面。手打 `JetBrainsMono-Bold` 这样的字面名在 macOS 上还会因为不是族名而退回系统字体。

维护者 2026-11-03 拍板：只做桌面的字重，不做斜体、不做移动端；默认常规，等于现在的画法；macOS、Windows、Linux 的候选窗都读它；Windows 原生设置页加控件；macOS 的控件等原生设置迁移时再加，期间经 MCP 或偏好文档设置。

## Decision

- 共享规则在 `crates/client-core/src/preferences.rs`：`CandidateFontWeight` 只有 `regular`、`medium`、`semibold`、`bold` 四档，对应 OpenType 字重 400、500、600、700（`value()`）。`Preferences::candidate_font_weight` 默认 `regular`，`skip_serializing_if` 让默认值不写进文档，没有这个键的旧文档和旧版本照常读。提交时不认识的取值被 `from_submission` 拒绝，读盘时由 `salvage_preferences` 退回默认，与其他枚举一样。
- 作用范围是候选窗里用候选字体画的全部文字：候选、注释、序号、释义、页码和首行提示。预编辑行一直是半粗，候选字重更粗时跟到同一档，即 `preedit_value()` = max(600, 字重)，避免首行比候选还细。连击计数等特效徽标保持原来的半粗。
- 族里没有这一档字面时不换族：macOS 用族名作必须匹配的键、字重作偏好去匹配（`MSIMEInstalledFontFamilyDescriptorWithWeight`），落到 CoreText 选出的就近字面，不合成加粗；Windows 交给 DirectWrite 的族内匹配，族里没有足够粗的字面时 DirectWrite 可能合成加粗；Linux 写成 Pango 样式词（`Medium`、`Semi-Bold`、`Bold`），由 fontconfig 就近选或按桌面配置合成加粗。三端的回退结果不完全一样，这是各自字体系统的行为，不在宿主里抹平。
- macOS：`AppearancePreferences` 只读共享文档的 `candidate_font_weight`（缺省读作常规，认不出的值不改现状），合并回文档时不写它；`candidateFontOfSize:` 对链上每个族和最后的系统字体都用这个字重。常规时走原来的族名缓存键和匹配，画法与加入字重之前逐字节相同。没有界面，MCP 的 `get_preferences` / `update_preferences` 和 CLI 的 `config set candidate_font_weight=bold` 是迁移前的入口。
- Windows：`CandidateFontSettings` 带上字重，一起比较、一起过邮箱；`CandidateWindow` 量和画都用同一个字重，量宽缓存随 `set_fonts` 的 `invalidate_geometry` 清空。原生设置「候选窗口 › 字体与大小」在字号下面加「字重」下拉，选项与字重表在 `src/candidate/CandidateFontWeight.h`，预览同步。Win7 设置窗口没有加控件，它的枚举字段要求文档里一定有这个键，而常规时文档里没有。
- Linux：`CandidateFontPolicy.h` 在字体列表之后、字号之前写样式词，常规不写，所以没动过字重的用户写给面板的描述不变，「没改过就不碰桌面字体」的规则照旧；选了字重算一次字体选择。Fcitx5 原生画布的 `native_canvas_font_variant` 改成取描述里的字重和调用方下限中更粗的一个，序号、释义和页码沿用描述里的字重，拼音行下限半粗。IBus 面板认不认这个样式词取决于面板本身，GNOME Shell 仍然不写。
- 顺带把 `system_fonts.rs` 和 `aliases_macos.rs` 里「macOS 候选窗把名字直接交给 CoreText、PostScript 名也能用」的过时说法改成实际情况：macOS 候选窗只认族名。

## Alternatives considered

- **列出族里真实存在的字面让用户选**（把字体下拉改成族加字面两级）：最精确，能选到 Light、Heavy 这类字面；但每个平台列字面的方式不同，存下的字面名换一台机器、换一个平台就对不上，跨端同步的偏好会失效，macOS 现在还会把字面名退回系统字体。四档抽象字重在三个字体系统里都有就近匹配，能跨端同步。
- **再加 light、heavy 等更多档位**：CSS 有九档；但中文字体大多只有常规和粗体两三个字面，多出来的档位在多数字体上落到同一个字面，看起来像设置失效。
- **斜体**：问题里也提了；汉字没有真斜体，三端都只能合成倾斜，效果差，维护者决定不做。
- **预编辑行完全跟随候选字重**：选常规时首行会从现在的半粗变成常规，改变了现有画法；保持「至少半粗」只在选粗体时变化。
- **macOS 先在共享 React 页或冻结的 AppKit 设置窗口里加一行**：提建议的用户在 macOS 上，加一行最快；但 2026-11-03 的决定是 `packages/ui` 只修缺陷、AppKit 设置窗口冻结，维护者选择等原生设置迁移。

## Consequences

- 收益：桌面三端读同一个字段，默认值不改任何现有用户的画法；Windows 用户在原生设置页就能改，macOS 和 Linux 用户可以经 MCP 或偏好文档改。
- 代价：macOS 暂时没有界面，提建议的 macOS 用户要等原生设置迁移；三端在字体族缺字面时的回退不完全一致（macOS 不合成加粗，Windows 和 Linux 可能合成）；Linux IBus 面板对样式词的支持没有逐个验证。
- 移动端键盘不读这个字段，存着的值在手机上原样保留、不起作用；以后要做移动端，iOS、Android、HarmonyOS 要一起做。
- 相关笔记：[Windows 原生设置窗口自己画候选字体各行](2026-10-11-windows-settings-candidate-fonts.md)（字体组的现有各行），[全部界面改为原生](../../proposed/architecture/2026-11-03-all-native-ui.md)（macOS 控件的去处）。

## Testing

- Rust：`crates/client-core/tests/candidate_font_weight.rs`（缺省不写、存取往返、拒绝未知取值、四档数值和预编辑下限），MCP 的 `the_candidate_font_weight_is_set_and_returned_to_regular` 与镜像枚举序列化测试。
- macOS：`candidate-font-cache` 核对常规与原匹配同一个描述符、粗体落到 Helvetica Bold、只有一个字面的族退回自己的字面；`skin-preview` 核对共享文档的字重作用到主字体和苹方回退、认不出的值不改现状、合并时不写回、缺省回到常规。
- Windows：`tests/ui/candidate_font_reload.cpp` 核对四档解析、预编辑下限、只换字重也算字体变化、非法取值被拒；渲染和设置页要在 Windows 上做设备验收。
- Linux：`candidate_font_policy.cpp` 核对样式词的位置、常规时描述不变、选字重算字体选择；`candidate_native_canvas_chrome.cpp` 核对画布各段字体的字重和拼音行下限。
