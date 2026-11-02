# Agent Note: 双拼预编辑的第三档「双拼分隔」存成单独的布尔键

Status: implemented

## Problem

#10717：双拼的「双拼预编辑」只有两档。「原始按键」画 `nimf`，字母对得上但看不出音节边界；「拼音分词」画展开的全拼 `ni'men`，有边界但字母换掉了（自然码 `mf` → `men`），核对按键时看的是另一串字母。报告者要第三档：按音节在原始按键之间插 `'`，`nimf` → `ni'mf`，默认值不变。

引擎早就算出这一串：双拼请求的 `raw_segmentation` 是按音节切开、保留大小写的原始按键，`pinyin_segmentation_with_cases` 在原始按键时也已经取它（再接回句中辅助码段和末尾的隔音符），只是快照的 `preedit` 从来不用它。

偏好 `shuangpin_preedit_uses_raw` 是布尔，而且已经发出去的各端都按布尔读它：Win7 原生设置的 `SchemeForm::valid` 要求它是布尔、`refresh` 直接 `get<bool>()`，iOS 用 `as? Bool`，IBus 和 Fcitx5 的状态菜单是开关，macOS 账号同步把它作为 `platform.macos.shuangpin_preedit_uses_raw` 上传，`MSIMEValidateCloudAppearanceForSchemes` 不仅要求它是布尔，还要求快照里的键数正好是固定值。

## Decision

共享偏好新增一个布尔 `shuangpin_preedit_separated`（缺省假，等于缺省时不写进文档），旧键 `shuangpin_preedit_uses_raw` 一字不动。三档与两个键的对应：

| 档位 | `shuangpin_preedit_uses_raw` | `shuangpin_preedit_separated` | 预编辑（自然码 `nimf`） |
| --- | --- | --- | --- |
| 原始按键（缺省） | 真 | 假 | `nimf` |
| 双拼分隔 | 真 | 真 | `ni'mf` |
| 拼音分词 | 假 | 不读（设置页写假） | `ni'men` |

- 引擎：`SessionOptions`、门面 `EngineOptions` 各多一个 `shuangpin_preedit_separated`，host-api 从偏好抄过去。`Session::snapshot_with_display_options` 在双拼原始按键且分隔时也走分词分支，取 `pinyin_segmentation_with_cases()`，也就是 `raw_segmentation`（带句中辅助码段和末尾隔音符）。`editing_text`、`caret_position` 和回车上屏仍是按键原文。字面输入（句中大写）、本地模式和专用英文与拼音分词一样不切。粤语双拼不读这个键，照旧跟随 `shuangpin_preedit_uses_raw`。
- 光标：`preedit_caret`（[组字光标在 preedit 里的位置由引擎给出](../bug-fix/2026-11-02-engine-preedit-caret.md)）不用改。`ni'mf` 只是在 `nimf` 的字母之间加了隔音符，走的是全拼分词那条按字母数换算的路：光标在音节边界上时停在插入的隔音符前面，在第二个音节中间时越过它。只按字母数换算的宿主（macOS `MSIMEPreeditCaretPosition`、Windows `preedit_display_caret`）原本就会这样换算，结果和引擎给的一致。
- 设置页：共享设置页（macOS、Linux、鸿蒙）的「双拼预编辑」下拉框按 原始按键 / 双拼分隔 / 拼音分词 排列，读写都经 `preedit-preferences.ts` 的 `shuangpinPreeditStyle`、`shuangpinPreeditPatch`，选拼音分词时把分隔一并写假，以后改回原始按键时它不会悄悄生效。Windows 现代设置和 Win7 设置共用 `InputSettingsModel.h` 的 `shuangpin_preedit_options` 与换算函数，顺序、文字与共享页相同；Win7 的 `valid` 只在这个键存在时要求它是布尔。iOS 在「双拼显示原始按键」下面加一个开关「原始按键按音节分隔」，只在上一项开着时列出，并加进桥接层交给运行中会话的字段。Android 没有「双拼预编辑」这一项（`HostCapabilities::shuangpin_preedit` 不含 Android），这次也不加。
- 不改：IBus 和 Fcitx5 的「双拼原始预编辑」菜单开关仍只切 `shuangpin_preedit_uses_raw`，分隔与否留在文档里、由共享设置页改。macOS 账号同步的快照不加新键，冻结的 AppKit 回退设置窗口不加这一档。

## 兼容与同步

- 没有这个键的旧版本读新文档：偏好结构不拒绝未知键，读入时记下、保存时放回（`deserialize_keeping_unknown` / `restore_unknown_fields`），所以旧版本既读得了，也不会把它抹掉；它看到的是 `shuangpin_preedit_uses_raw: true`，即「原始按键」，只是不切。
- macOS 账号同步：`platform.macos.shuangpin_preedit_uses_raw` 照旧是布尔，双拼分隔时为真。新键不进同步快照：`MSIMEValidateCloudAppearanceForSchemes` 要求键数正好是固定值，多一个键会让所有旧版本整份拒收云端设置。代价是「双拼分隔」不随账号漫游，另一台设备收到的是「原始按键」或「拼音分词」，那台设备自己的分隔选择不变；收到拼音分词时分隔不读，三档的结果仍然自洽。
- Windows、Linux 的桌面账号导出（`export_desktop_settings_for`）本来就不导出双拼预编辑，不受影响。

## Alternatives considered

- **把 `shuangpin_preedit_uses_raw` 改成三选一的字符串，或者另起一个字符串键 `shuangpin_preedit` 取代它。** 一个键表达一个选择，最整齐，Linux 菜单也能直接显示三档。但旧键改成字符串，已经发出去的 Win7 设置会在 `valid` 里整页拒绝这份文档、iOS 读成缺省，macOS 账号同步校验整份失败；另起字符串键同时保留旧布尔，又要定两个键冲突时听谁的。产品未发布可以不管旧数据迁移，但新旧版本会通过同一份文档和账号同步同时存在，这条不能靠「未发布」豁免。
- **只改设置页预览 `ni'mf`，让它不再误导。** 改动最小。但报告者要的是这种显示本身，而且引擎已经算好了这一串，加一档的成本只在设置页。
- **在宿主里自己用 `raw_segmentation` 拼分隔。** 不用碰引擎选项。但每个宿主都要自己接辅助码段和末尾隔音符，还要自己换算光标，这正是 [组字光标在 preedit 里的位置由引擎给出](../bug-fix/2026-11-02-engine-preedit-caret.md) 刚收回引擎的那类规则。

## Consequences

收益：双拼用户能看见音节边界，看到的又是自己按下的键；光标、提交和候选都不变，旧版本和账号同步不受影响。代价：偏好里多一个只在另一个键为真时才有意义的键，读它的地方都要先看 `shuangpin_preedit_uses_raw`（共享页、Windows 两套设置各有一个换算函数，换算规则写在一处之外的地方都有测试钉住）；「双拼分隔」不随 macOS 账号同步漫游；IBus、Fcitx5 菜单仍只有原始/展开两态。

验证：引擎门面 `host::tests::separated_shuangpin_preedit_keeps_the_keys_and_carries_the_caret` 用真实会话打自然码 `nimf`，`preedit` 为 `ni'mf`、`editing_text` 为 `nimf`，光标在末尾、第二个音节中间、音节边界、行首分别是 5、4、2、0，还覆盖了半个音节、用户打的隔音符、末尾隔音符、字面大写，以及拼音分词时不读分隔；把引擎分支改回去后这条测试失败。client-core 的 `shuangpin_preedit_separated_is_a_separate_key_left_out_until_chosen` 钉住缺省不写进文档、旧键仍是布尔。前端 vitest 覆盖三档的读写与选项顺序，`input_settings_model.cpp` 覆盖 Windows 的换算，Win7 原生控件测试覆盖第二、三档写回两个键。iOS 加了活会话重读的单测，但没有在本机跑；鸿蒙的组字行画哪一段没有在设备上核对。

## Existing notes audit

检索 `shuangpin_preedit_uses_raw`、双拼预编辑、`raw_segmentation`、`preedit_caret`。[组字光标在 preedit 里的位置由引擎给出](../bug-fix/2026-11-02-engine-preedit-caret.md) 是部分重叠：本篇沿用它的换算，没有改它的决定。[预编辑偏好类型](../simplification/2026-10-15-preedit-preferences.md) 规定了 `PreeditPreferences` 与 `preeditControlState` 的归属，本篇在同一处加类型和换算，部分重叠。[Windows 组字细节对齐 macOS](2026-10-10-windows-input-compose-mac-parity.md) 给 Windows 加了两档的设置项，本篇在同一行加第三档，部分重叠。[macOS 输入设置精简](../simplification/2026-10-16-macos-input-settings-prune.md) 决定了这一行放在哪一页，本篇不动位置，无关。
