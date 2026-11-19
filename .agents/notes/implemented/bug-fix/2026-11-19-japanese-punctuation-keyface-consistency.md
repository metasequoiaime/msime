# Agent Note: 日语标点键面与上屏统一到契约日文表

Status: implemented

## Problem

日语方案的标点「键面画什么」和「实际上屏什么」是两条独立的表，各自漂移：

- 共享引擎 `handle_punctuation` 对日语走的是中文标点表 `punctuation.translate`，`,` 写成 `，`、`[` 写成 `【`、`\` 写成顿号，与日文写法不符。
- Web 引擎另有一张私有 `japanese_mark` 表，与触屏键面表不同步；原生引擎和 Web 引擎对同一个日语标点键写出的字不一样。
- 触屏键面表（Android `ChineseSymbolFaces`、Harmony `ChineseSymbolFaces`、iOS `chineseSymbolFace`）只认中文标点开关：日语方案的 26 键罗马字字母行、`#+=` 符号层和底行标点键画的是中文或 ASCII 写法（`，【】/〜`），与引擎写出的日文写法对不上。修复前的行为测试 `japanese_scheme_commits_japanese_punctuation` 得到 `left: Some("，") right: Some("、")`。

这不是出厂默认值问题：没有改任何设置项、字段或缺字段读法，只把既有日语方案的标点输出与键面对齐。

## Decision

### 契约单一来源

把日文标点表下沉到 `shared/contracts/punctuation/policy.json` 的 `japanese` 段，`contractVersion` 升到 6，扩展 `generate.py` 生成 `policy.h` 的对应常量。表内容（15 项）：`,` `\`→`、`、`.`→`。`、`[` `]`→`「` `」`、`{` `}`→`『` `』`、`/`→`・`、`~`→`〜`、`?` `!` `(` `)` `:` `;`→全角形式。引擎 `crates/engine/src/punctuation.rs` 的 `JAPANESE` 表与契约逐字一致，`japanese_mark()` 供 `handle_punctuation` 在日语方案（`is_japanese()`，含临时日语）下取字，没有映射的键交回宿主原样打出。Web 引擎删掉私有 `japanese_mark`，日语跳过中文引号交替后统一走 `session.punctuation(byte)`，与原生走同一张引擎表；不做运行时 FFI，两张表在编译期由契约钉死。

### 各平台键面路由

| 平台 | 键面表 | 路由 |
| --- | --- | --- |
| Android | `ChineseSymbolFaces.JAPANESE_FACES` + `LetterHintTable.face(..., japanese, ...)` | `MSIMEInputService.japaneseKeyfaces()`（日语方案、非英文、非本地工具模式）统一给字母层符号键、角标、底行句号、`#+=` 层供面；`commitHint` 的表内标点交给引擎，表外按字面上屏 |
| Harmony | 新增 `JapaneseSymbolFaces.ts` | `KeyboardView.japanesePunctuationFaces()` 让 `symbolFace()`/`punctuationFace()` 优先走日文表，替换 `，` `。` `？` `！` 的 10 处三元 |
| iOS | `KeyboardViewController.japaneseSymbolFaces` + `SymbolLayerLayout` 的 `japanese*` 表 | 26 键符号行与底行由 `updateSymbolKeyFaces` 按日文表重画；`123`/`#+=` 层的 `characterRows` 与 `input(for:)` 加 `japanese` 参数，日文键面映射回对应 ASCII 键交给引擎；`layerCommaButton` 画 `、` |
| PC（Windows/macOS/Linux） | 无独立标点翻译表 | 物理键帽不动；ASCII 标点送给共享引擎，上屏由契约日文表修复，不存在需要同步的软件标点键面 |

### 布局适用面

按先前拍板的布局矩阵：日语方案在移动端只有 26 键罗马字和 9 键假名；14/18 键是 quanpin/shuangpin/wubi 专属，假名九键本就日文直接上屏，两者不动；数字九宫格已有日文写法的平台只做一致性复核（iOS 符号面板本就按字面插入所画字符，无翻译路径）。PC 只修上屏。

### 与顿号笔记的关系

[六宿主本地标点表跟进契约与顿号空格反查](../feature/2026-10-22-punctuation-host-tables-enum-comma-space.md) 曾把 `、` 移出 iOS `japaneseInputs`，让中文的顿号键统一走 `enumerationCommaInput(backslash, slash)`。本次把 `、` 放回 `japaneseInputs` 并映射到 `\`，同时给顿号特判加 `!japanese` 守卫：中文行为完全不变；日语的 `、` 是罗马字读点（`,\` 都出它），不随中文顿号开关变，否则关掉 `enumeration_comma_backslash`、打开 `enumeration_comma_slash` 后点日语 `、` 会送 `/`，引擎写出 `・`。iOS 的空格反查由 FFI bridge 负责，不走这张表，不受影响。

## Verification

- `python3 shared/contracts/punctuation/generate.py --check` 通过；`git diff --check` 通过。
- `cargo test -p msime-engine --lib`：3195 通过 / 0 失败 / 59 忽略；`session::tests::japanese_scheme_commits_japanese_punctuation` 通过（修复前 `left: Some("，") right: Some("、")`，RED→GREEN）。
- `MSIME_OPENCC_SOURCE_DIR=<缓存> cargo test -p msime-engine-wasm`：63 通过 / 0 失败；`japanese_punctuation_finishes_the_conversion_with_japanese_marks` 通过。
- 平台行为测试已写但本机缺工具链：Android `ChineseSymbolFacesSmoke`/`LetterHintTableSmoke` 需 JDK、Harmony `japanese-symbol-faces.test.ts` 需 Node、iOS `SmartPunctuationTests`/`NineKeyKeyboardTests` 需 Xcode，均未在本机运行，交给对应平台 CI；不记为已验证。
- 未做真机、模拟器或真实编辑器操作，不把行为测试当作设备复验。

## Historical audit

- [六宿主本地标点表跟进契约与顿号空格反查](../feature/2026-10-22-punctuation-host-tables-enum-comma-space.md)：**部分重叠**。同一批宿主键面表，但那篇管中文顿号开关与空格反查，本篇管日语标点表与键面上屏一致；`japaneseInputs` 的 `、` 归属本篇已在上文说明。
- [引号样式与方头括号样式](../feature/2026-10-13-punctuation-mark-styles.md)：**部分重叠**。同一份 `policy.json` 契约；本篇新增 `japanese` 段，日语明确不消费 styles 两个样式维度。
- [26 键句号移到空格旁](../feature/2026-11-12-twenty-six-key-period-beside-space.md)：**部分重叠**。同为底行标点键的键面/上屏一致性，那篇管中文句号布局，本篇管日语方案取日文写法。

## Alternatives considered

- **Web 引擎保留私有 `japanese_mark`，只把触屏键面改成和它一致**。最强理由是原生侧改动更少。没采用：两张表在 Web 与原生之间仍会漂移，本次根因正是表分裂；下沉到契约让每个平台编译期都从同一份表生成。
- **运行时 FFI 让 Web 每次读引擎翻译**。最强理由是完全没有本地表。没采用：每次标点都跨 FFI 且要处理异步，而契约生成已能保证编译期一致，代价与收益不成比例。
- **各平台键面继续读中文标点开关，只把开关对日语强制打开**。最强理由是不用新增日文表。没采用：中文表与客户期望的日文写法本身就不同（`【】` 对 `「」`、`/` 对 `・`），强制开关无法得到日文键面，也不解决引擎写错字。

## Consequences

- **收益**：日语标点键面与上屏共一份契约表，六平台一致；手写本地表与契约由 `generate.py --check` 和引擎契约测试防再漂移。
- **代价**：`policy.json` 契约版本升到 6；Android/Harmony/iOS 各多一张日文键面表和路由判定，iOS `characterRows`/`input(for:)` 多一个 `japanese` 参数；触屏平台的具体行为待 CI 在各自工具链下确认。
- **验证**：本机 Rust 与契约检查全绿，平台测试未能运行已如实记录；不声称设备或真实编辑器已验收。
