# Agent Note: Android 和 iOS 的 AI 辅助翻译改为显式选择

Status: implemented

## Problem

[候选翻译的「AI 辅助」是显式选择的服务](../../implemented/feature/2026-10-22-ai-translation-provider-selection.md)把桌面（macOS、Windows、Linux）和走 host-api 查询的 HarmonyOS 改成只认 `translation_ai`，但 Android 和 iOS 的键盘不经 `msime_client_translation_query`，各自镜像了一份选路：Android 的 `CandidateTranslationPolicy.route` 和 iOS 的 `TranslationProviderPreference.route` 仍按 [候选翻译复用 AI 配置](../../implemented/feature/2026-10-17-ai-candidate-translation.md) 时的隐式规则，`ai_assistant.enabled` 开着、又没选别的服务就用 AI 翻译候选，从不读 `translation_ai`。同一份偏好文档在不同平台得到不同的结果：桌面上「不使用在线服务」不发任何请求，手机上只要 AI 联想开着就把候选发给 AI 服务；手机的「翻译服务」页也列不出「AI 辅助」，用户既选不了它，也关不掉隐式的那一路，只能关掉 AI 联想。Android 的镜像还会在小牛、自定义选中但凭据不全时改用 AI，违背了「所选服务不可用时不改用别的服务」。

## Decision

各端统一按 host-api `selected_translation_services` 的规则：只有 `translation_ai` 为真且 AI 配置可用时才用 AI 辅助翻译，`ai_assistant.enabled` 不参与；水杉账号、小牛、自定义、腾讯排在它前面，其中一个被选中（腾讯按 `enabled`，没有凭据也算，与共享层相同）时轮不到 AI，选中的服务不可用时也不改用 AI。

- **Android**：`CandidateTranslationPolicy.route` 的十七参数版本把 `aiEnabled` 换成 `translationAi`，并在候选翻译关闭、小牛 / 自定义 / 腾讯任一启用时不走 AI；`MSIMEInputService.candidateTranslationRouteFrom` 读 `translation_ai`。「翻译服务」页（`TranslationServicePage`、`TranslationServiceSettingsPolicy`）加「AI 辅助」一项：选中时写 `translation_ai: true` 并关掉三个自有服务，选别的服务时删掉它；页面显示复用的服务商与模型，配置不全时提示去「AI 润色与回复」页补，配置能不能用由键盘的同一个 `candidateTranslationRouteFrom` 判断。「隐私」页打开「用水杉账号翻译候选」时一并删掉 `translation_ai`，与共享设置页选「水杉账号」相同。
- **iOS**：`TranslationProviderPreference` 的 `route`、`selected`、`select` 读写 `translation_ai`，`TranslationProvider.allCases` 列出「AI 辅助」。iOS 键盘拿 AI 令牌要靠「AI 润色与回复」页「在键盘中启用 AI」发布的配置，这是 iOS 上「AI 配置可用」的一部分；它和候选栏 AI 候选（写进 `ai_assistant.enabled`）是两个开关，后者不再影响翻译。键盘取翻译令牌和设置页判断「AI 辅助」是否就绪用同一个 `KeyboardAIService.translationToken(endpoint:)`：已发布配置的地址要等于文档里的 `ai_assistant.endpoint`，钥匙串里也要取得到令牌，页面不会显示就绪而键盘拿不到令牌。
- **HarmonyOS** 本来就按 host-api 的查询走，不改；它在拿到 AI 生成类资质之前不列「AI 辅助」（[鸿蒙隐藏 AI 生成类功能](../../implemented/feature/2026-10-27-harmony-hide-ai-generation.md)），这是有意的。

### 一次性迁移

规则翻转会让手机上一直在用隐式 AI 翻译的用户升级后突然没了译文，所以在共享层 `crates/client-core` 读偏好时迁移一次，形状照搬智能标点默认开的那次（`smart_punctuation_migrated`）：

- 新的内部标记 `translation_ai_migrated`，不是设置。`Default` 为真，缺键按假读（即「这是统一之前的文档」），为假时不写进文档。
- `Preferences::settle_translation_ai_selection` 在 `PreferencesStore` 读文档（`decode_document`）和修复损坏文档（`salvage_preferences`）时调用：标记缺失、平台是 Android 或 iOS（`TRANSLATION_AI_WAS_IMPLICIT`）、`ai_assistant.enabled` 为真且 `provider`、`endpoint`、`model` 都不为空、`translation_ai` 为假、`translation_account` 为假、小牛和自定义都没启用、腾讯仍是开着但两项凭据不全时，把 `translation_ai` 打开并把腾讯的 `enabled` 写成 false（与设置页选「AI 辅助」写下的样子相同，否则共享层的选路会先选中没有凭据的腾讯）。不论平台都标上标记。
- 腾讯关着的不迁移。腾讯在共享默认值里是开着的，旧版两端的「翻译服务」页选任何非腾讯项（包括「不使用在线翻译」）都会把它显式写成关，Android 隐私页开关「用水杉账号翻译候选」也会；旧版那一页没有 AI 选项，所以腾讯关着只可能表示用户明确选过别的，不可能是选了 AI。这类用户旧版下其实一直被隐式 AI 翻译（选了「不使用」，候选照样发给 AI），迁移成「AI 辅助」等于把泄漏定成显式选择，所以留在「不使用」，升级后不再发 AI 请求。
- AI 配置三项不全的不迁移：旧版两端此时都不发 AI 请求，迁移只会在页面上凭空多出一个「配置不完整」的「AI 辅助」。令牌不查，iOS 的令牌在钥匙串里，共享层看不到；Android 令牌不可用的少数文档会迁移成「AI 辅助」并在页面上提示配置不完整，不发请求。
- 只改内存里读到的那份；文件在下一次真正的保存时带上新值和标记。`PreferencesStore::save` 和 `settings_document::import_document` 让标记只增不减，写入方漏了这个键也不会让下一次读重新迁移、把用户刚改选的服务改回 AI 辅助。

判断「旧文档」用标记而不是「文档里没有 `translation_ai` 键」：`translation_ai` 自 2026-10-22 起总是随文档写出（`false` 也写），Android 和 iOS 存过一次设置的文档都带着 `"translation_ai": false`，可那个 `false` 在手机上从没被读过，代表不了用户的选择。按键是否存在判断会漏掉几乎所有手机用户。

迁移放在共享层而不是两个平台各写一份：两端读写偏好都经 `msime_client_load_preferences` / `msime_client_save_preferences`，迁移结果因此对键盘、设置页和账号同步一致，规则也只有一处。

## Alternatives considered

- **只改两端选路，不迁移**：代码最少，2026-10-22 那篇在桌面上就是这么做的。但手机上这条隐式路径已经发布了几个版本，AI 联想开着的用户升级后候选译文会静默消失，又没有入口能选回来（改动之前页面上没有「AI 辅助」），不采用。
- **按「文档里有没有 `translation_ai` 键」判断是否迁移**：不需要新标记。但如上所述，近期写过的手机文档都带着从未被读过的 `"translation_ai": false`，这条判断几乎对谁都不成立，不采用。
- **在 Android 和 iOS 原生代码里各自迁移**：不碰共享偏好结构。但要在两种语言里各写一遍条件和「只跑一次」的记录，两端还可能各自读到对方没迁移的文档（设置页和键盘、账号同步），一致性靠人守，不采用。
- **手机选路继续把「腾讯启用但没有凭据」当作未选中，不必在迁移里关腾讯**：与手机端账号选路对腾讯的处理一致。但这样手机和 host-api 在同一份文档上仍会给出不同结果（桌面上那份文档不会走 AI），统一就落空了；关掉没有凭据的腾讯不改变任何请求，不采用。

## Consequences

收益：同一份偏好文档在六个平台上选到同一个翻译服务；手机上「不使用在线翻译」真的不发 AI 请求；用户能在手机的「翻译服务」页显式选「AI 辅助」，不开 AI 联想 / 候选栏 AI 候选也能用；升级前手机上在用隐式 AI 翻译、又从没在「翻译服务」页存过选择的用户升级后照旧有译文，设置页显示为「AI 辅助」；存过「不使用在线翻译」的用户升级后真的不再把候选发给 AI。

代价：偏好结构多一个内部标记，`scripts/test-preferences-field-parity.py` 和 macOS 的 `preference_coverage.py` 把它登记为非设置。迁移会把手机文档里没有凭据的腾讯写成关闭，这与设置页选「AI 辅助」时写下的一致。Android 上以前「小牛 / 自定义选中但凭据不全时改用 AI」的行为没有了，这类用户需要补全凭据或改选「AI 辅助」。账号设置同步里只有 Linux 上传和应用 `translation.ai`，Android 和 iOS 不上传它，迁移结果只留在本机。

验证：`cargo test -p msime-client-core` 新增 `translation_ai_settle_selects_ai_only_where_it_was_implicit`（迁移条件逐条，含腾讯被显式关掉即选过「不使用」、AI 服务商 / 地址 / 模型缺一项时都不迁移；手机与其他平台各一遍、第二次不再改）、`translation_ai_migration_runs_once_and_a_later_choice_sticks`（经存储读写：只读不写文件，保存时补上标记，改选后再读不回退）、`translation_ai_recovery_migrates_a_legacy_document`、`a_new_document_carries_the_translation_ai_marker` 和 `settings_document` 的 `an_old_file_keeps_its_translation_service_and_this_machine_s_marker`。Android JVM 冒烟 `CandidateTranslationRouteSmoke`、`TranslationServiceSettingsPolicySmoke` 加了显式选择、AI 联想开关不参与、各服务优先于 AI、凭据不全不改用 AI 的用例；iOS `TranslationProviderTests` 加 `testAIIsUsedOnlyWhenChosenExplicitly` 和 `testSelectingAIWritesTheExplicitChoice`。
