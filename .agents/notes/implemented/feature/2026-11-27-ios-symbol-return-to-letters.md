# Agent Note: iOS 符号页输入指定符号后返回字母键盘

Status: implemented

## Problem

中文键盘上的「符」和 `#+=` 是为了打一两个标点才打开的页。打完 `，` 或 `。` 之后还要再点一次「拼音」或 123 才回到字母层，写一句话就要多按一次。用户要的是：在设置里列出这些符号，符号页上写出它们就自动回主键盘。

## Decision

- 新共享偏好 `touch_return_to_letters_symbols: Vec<String>`（`crates/client-core/src/preferences.rs`）。规则与快捷符号相同（单个字素、去重、最多十二项、不含空白与控制字符），**空列表就是关闭**。
- 默认值 `default_return_to_letters_symbols()` 是空列表，没配置过的人行为完全不变；字段带 `skip_serializing_if = "Vec::is_empty"`，空列表不写进文档。
- 清空时写空数组（不删键）：iOS 侧把空数组原样写进文档和 App Group，读不到这一项时也按空列表处理。
- 规范化入口 `normalize_return_to_letters_symbols`（与快捷符号共用内部的 `normalize_single_grapheme_symbols`），校验失败是 `PreferencesError::InvalidTouchReturnToLettersSymbols`。ABI 是 host-api 的 `msime_client_touch_return_symbols`，iOS 用 `@_silgen_name` 同名函数，与其他偏好一样由 Rust 持有规则。
- 字段进账号设置同步白名单（`settings_sync/fields.rs`）。只有 iOS 键盘读，Android、macOS、Windows 原样保留；`scripts/test-preferences-field-parity.py` 与 `platforms/macos/tests/settings/preference_coverage.py` 都登记为「只有 iOS 有入口」。
- App「设置 → 键盘 → 布局」页，「快捷符号」下面新增「返回主键盘符号」组：一个输入框加「保存」，info 写明最多 12 个字符、留空关闭。保存时按字素拆开交给共享层规范化，回显规范化后的结果；留空保存写入空数组。
- 判定放在键盘扩展的 `SymbolReturnPolicy`（`KeyboardLayoutTables.swift`）：比较的字面集合是「键入的符号 + 它此刻画在键上的字」。这样「符」页上画着 `，`、实际送出 `,` 的键，填 `，` 或 `,` 都能命中；中文标点关着时键面就是 `,`，只认 `,`，日文方案则按它画出的 `、` 判断。空清单直接返回 false。
- 三个接点：`typeSymbolLayerFace`（`#+=` 层、大千共享符号行、数字页补充行）、`makeSymbolRow`（「符」页共用的十键符号行）、`showSymbolPanel` 的 `onInsert`（「符」面板）。命中后走的是现成「回到字母」键的同一条路径：`showsSymbols = false` + `updateKeyboardLayout()` + `updateLetterCaseControls()`。
- 「符」面板自己的收起逻辑（插入后收起，用户锁定面板连续输入时不收）不改；面板开在 123 或 `#+=` 页上时，我们的切页发生在它收起之后，露出来的就是字母页。
- 「恢复默认」（`resetGeometry`）连这份清单一起清掉，与快捷符号一致，清掉后清单为空（关闭）；按钮下方的说明补上「快捷符号、返回主键盘符号」。

## Alternatives considered

- **复用「快捷符号」清单**：少一个字段，但快捷符号默认非空，等于给所有人默认打开；两者用途也不同（一个决定键面与菜单，一个决定跳转）。
- **做成开关加固定的中文标点**：不可配置，而用户要的是「指定符号」。
- **比较真正写出的文字**：要截获每次提交，成对引号、书名号还会变换；比较键面已经覆盖 `，` 与 `,` 两种写法，成本低得多。
- **改在 `handleSymbol` 里统一判断**：快捷标点菜单、字母键滑动、九键 1 键候选条都走它，那些不是符号页，按 `showsSymbols` 之外再判断容易误伤。
- **面板插入后强制关闭面板**：面板的锁就是给连续输入符号用的，强制关闭违背用户意图。

## Consequences

- 缺省关闭（清单为空）；换设备时清单随账号设置同步，但只有 iOS 会照它跳转。
- 只在中文模式生效：英文、韩文、越南文、藏文写 ASCII 键面，日文按日文键面匹配，`123` 数字九宫格上的符号也照同一规则。
- 面板开在字母页上时命中清单只会关掉面板（它本来就插入后收起），不切键面。
- 快捷标点菜单（逗号长按）、工具栏符号面板、字母键滑动夹带的符号都不触发，它们不在符号页上。

## Testing

- Rust：`preferences.rs` 的 `return_to_letters_symbols_are_validated_and_default_to_empty`（缺省空列表、缺键按空列表、空列表不写进文档、规范化、校验、上限）。
- host-api：`touch_return_symbols_keep_an_empty_list_as_off_across_the_abi`。
- iOS：`QuickPunctuationSettingsTests` 四条——`testSymbolReturnPolicyComparesEveryFace`、`testReturnSymbolsSaveNormalizesAndEmptyTurnsItOff`（清空后文档里是空数组）、`testReturnSymbolsFallBackToEmpty`、`testReturnSymbolsSendTheSymbolLayerBackToLetters`（123 → `#+=`，点清单外的键留在符号页，点 `，` 回到字母页）。
- 本机没有 Xcode 与 cargo：Rust 与 XCTest 都未编译、未运行。跑过 `scripts/test-preferences-field-parity.py`、`platforms/macos/tests/settings/preference_coverage.py`（两者只剩同一工作区里另一个未提交特性 `common_phrases_sync_webdav` 的旧失败）、`scripts/test-ios-key-layout-settings.py`（32 项）、`scripts/test-android-preference-keys.py`。
