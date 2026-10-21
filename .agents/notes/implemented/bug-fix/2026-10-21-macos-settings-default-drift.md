# Agent Note: macOS 两套设置界面的默认值漂移

Status: implemented

## Problem

macOS 有两套设置界面：输入法 bundle 里的原生 AppKit 窗口（`platforms/macos/src/settings/AppearancePreferences.mm`）
和共享 React 设置页（`packages/ui/src/settings/`）。两边都读 `crates/client-core/src/preferences.rs` 的同一份
`Preferences` 文档，但原生窗口在旧 `NSUserDefaults` 键上各留一份出厂回退值。审计（`/tmp/msime-audit/native-vs-react.md`）
找到五处默认值漂移：`candidate_font_family`、`candidate_preedit_font_size`、`voice_input.hotkey_ralt`、
`voice_input.language` 的写法，以及 `candidate_translations` 的三处读数。同一份偏好在两套界面上给出不同的出厂值，
用户在一处看到的和输入法实际生效的会对不上。

## Decision

四条按共享偏好对齐；`candidate_translations` 判定为「不是漂移」，保持原样。每个修复都在对应的原生或 Rust
测试上先失败、改后通过。

### `candidate_font_family`：原生回退改用 `Noto Sans SC`（改）

依据：共享默认是 `default_candidate_font_family`（`crates/client-core/src/preferences.rs`）的 `Noto Sans SC`，
React 的 `defaultCandidateFontFamily`（`packages/ui/src/candidate/candidate-font-family.ts`）同值。原生旧回退
`Segoe UI` 是 Windows 的字体，macOS 上装不到，只会落到系统字体；它是唯一把 Windows 字体写进 macOS 回退的地方。
`AppearancePreferences.mm` 的 `-fontFamily` 回退改成 `Noto Sans SC`。

### `candidate_preedit_font_size`：原生回退 16 → 15（改）

依据：共享默认 `default_candidate_preedit_font_size` 是 15；React 的 `candidatePreeditFontSize` 回退 15
（`packages/ui/src/candidate/candidate-font-size.ts`）；Windows 原生设置页的「预编辑字号」滑杆和文档读取默认
都是 15（`platforms/windows/settings/main.cpp` 的 `slider_row(..., "candidate_preedit_font_size", 12, 32, 15)`
与 `std::clamp(document_.Number(L"candidate_preedit_font_size", 15), ...)`）。原生 16 找不到任何依据，是这一家独有的
值。getter 和 setter 的越界回退一起改成 15。

### `voice_input.language`：共享默认 `zh-cn` → `zh-CN`（改）

依据：React 的 `defaultVoiceInput.language` 是 `zh-CN`（`packages/ui/src/settings/voice-input-preferences.ts`），
macOS 原生回退窗口的 `-voiceLanguage` 也只回退 `zh-CN`/`en-US`，`AppearancePreferences.h` 的注释写明它是
BCP 47 的识别器 locale。字段本身按识别器分两种写法：系统识别器用区域形式（`zh-CN`、`en-US`），云端短形式用
`zh-cn`、`en`、`ja`、`auto`（`packages/ui/src/voice/voice-language-options.tsx`）。出厂默认跟着默认服务走，
取与两侧 UI 相同的区域形式。

核对大小写不会失配：React 的 `voiceLanguageTitle` 与 `mobileVoiceLanguageOf` 都先 `toLowerCase`；macOS
`HTTPVoiceRequest.mm` 同时把 `zh-CN`/`en-US`、`en-us` 归一成云端要的短形式，已是 `zh-cn` 的原样通过；
Windows 的系统识别用 `Language` 属性映射 LANGID，`system_asr_policy.cpp` 的注释写明两种写法都要找到同一个识别器；
HarmonyOS 的 `LocalAsrPolicy.senseVoiceLanguage` 先 `toLowerCase`。Rust 的 `settings_sync` 只限长度，不比较大小写。
新测试同时钉住出厂默认是 `zh-CN`、小写旧值原样往返，不做迁移改写。

### `voice_input.hotkey_ralt`：原生回退改开（改）

依据：共享默认 `VoiceInputPreferences::default()` 里三个按住录音键只有 `hotkey_ralt` 为 `true`
（`crates/client-core/src/preferences.rs`）；React 的 `HOLD_DEFAULTS` 与注释、Windows 的 `hold_keys`
（`platforms/windows/settings/VoiceSettings.h`：「client-core 的 `VoiceInputPreferences::default()`，只有右 Alt
默认打开」）、Linux 的 `ClientEngine.cpp`/`FcitxEngine.cpp` 都默认打开这一个。原生旧注释说三个按住键默认关，但设置
窗口是独立进程，可能在输入法把共享文档写进本机键之前打开，于是显示「未启用」，而输入法 `MSIMEApplySharedVoicePreferences`
载入文档后已按打开的默认值录音。三个键里另外两个本来就默认关（`hotkey_rctrl_ralt`、`hotkey_ctrl_win`），与共享一致；
只把 `hotkey_ralt` 的缺键回退改成开。`InputController.mm` 自己就是写回共享文档的那个进程，不改。

### `candidate_translations`：不是漂移，不改

共享 `Preferences::default()` 是 `false`（新装关闭），`#[serde(default = "enabled_by_default")]` 是缺字段读开
（旧文档沿用开启行为）。这是[新装默认关闭候选翻译并保留已有配置](../feature/2026-10-15-new-install-candidate-translations-off.md)
的决定，由 `candidate_translations_start_off_in_each_edition_and_the_first_document` 和
`candidate_translations_keep_legacy_and_explicit_choices` 两项测试钉住。React 的 `?? true`
（`packages/ui/src/settings/use-translation-settings.ts`）与原生 `candidateTranslations` 的三目回退
（`AppearancePreferences.mm`）对应的是「文档缺字段 = 旧行为开」，与 serde 默认一致，不是新装的出厂值。
共享快照由 Rust 产生，`candidate_translations` 没有 `skip_serializing_if`，字段始终存在：新装两边都读 `false`，
旧文档缺字段由 serde 补成 `true` 再写入快照，两边再读 `true`。所以三处已经自洽；把 UI 回退改成 `false` 会和
原生 `TestCandidateTranslationPreference` 冲突，把 serde 默认改成 `false` 会和上面两项 Rust 测试冲突。

## Verification

- 原生（不建窗口、不依赖图形会话）：
  - `candidate-appearance-defaults` 在改动前失败并打印「主字体回退 期望 Noto Sans SC，实际 Segoe UI」「预编辑字号回退
    期望 15，实际 16」，改后通过（`ctest --test-dir target/macos-isolated -R '^candidate-appearance-defaults$'`）。
  - `voice-hold-defaults` 在改动前失败并打印「按住右 Option 录音回退: 期望 开，实际 关」，改后通过；同一用例钉住
    识别语言把小写旧值按 `zh-CN` 显示。
- Rust：`voice_language_default_uses_the_bcp47_region_form` 在 `zh-cn` 默认下失败（`left: "zh-cn"`，
  `right: "zh-CN"`），改后通过。
- 远端 run `20261021-191249` 的完整 quick 通过；同份源码的 `local-mode-preferences`、`candidate-appearance-defaults`、`voice-hold-defaults`、`candidate-row-layout` 与 `skin-preview` 五项原生 CTest 全部执行通过，共 27.13 秒。布局断言按实际字体与预编辑居中位置核对；本轮没有运行 `shortcut`，不把它计入通过。

## Alternatives considered

- **`voice_input.language` 反向统一成小写 `zh-cn`** — 最强的理由是云端短形式本来就写 `zh-cn`，Linux、Windows、
  HarmonyOS 的宿主本机回退也是小写。没有采用：字段交给系统识别器时按区域码用，`AppearancePreferences.h` 的注释和
  React 的 `defaultVoiceInput` 都写 `zh-CN`，macOS 的 `HTTPVoiceRequest` 已把区域形式归一成云端短形式，统一到区域
  形式只剩共享默认这一处要改。
- **`hotkey_ralt` 反向统一成关** — 最强的理由是原生旧注释：一个没被要求的修饰键组合自己开始录音不好。没有采用：
  Windows、Linux 和共享设置页都默认打开，改成关要同时改三个平台和共享默认，而且输入法载入共享文档后本来就会写回
  打开；只把 macOS 窗口的缺键回退对齐共享值，改动最小且不改变有文档时的行为。
- **把 `candidate_translations` 的 serde 缺字段默认也改成关** — 表面更统一。没有采用：会在升级时替缺少新字段的
  旧用户改变行为，与「新装关闭」的诉求无关；[2026-10-15 的决定](../feature/2026-10-15-new-install-candidate-translations-off.md)
  已经否过同一条路。
- **给两个原生默认加一个「从 Rust 读默认」的运行时查询** — 可彻底消掉原生回退。没有采用：两个原生用例是不建窗口
  的 CTest，链 Rust 只为了拿两个常量会让这两个用例变重；回退值与共享默认的对应关系用测试钉住就够。

## Consequences

- **收益**：macOS 两套设置界面在候选字体、预编辑字号、识别语言写法和按住右 Option 的出厂值上一致；新装用户不会
  在原生窗口看到 Windows 的 `Segoe UI`、独一份的 16 或「未启用」的语音快捷键。
- **代价**：在原生窗口里从没设置过按住右 Option 的 macOS 用户，升级后按住右 Option 会开始录音（共享默认本来就
  如此，载入共享文档后也会这样，改动只是让窗口跟上）；原生回退值与共享默认从此由新增的两项 CTest 钉住，以后改
  共享默认要同步改原生并更新用例。
- **保留**：审计里「只有 React 23 条」「只有原生 1 条」「选项不同 9 条」是有意的界面划分和产品取舍，不在本次范围。
- `PRIVACY.md` 同步区分新装关闭与旧文档缺字段时开启；系统离线翻译的前提是用户已开启候选翻译。
- 字体与字号默认变化后，候选布局回归使用实际字体度量，并验证预编辑在首行中的居中偏移；不能沿用旧系统字体和 16pt 的固定高度。
