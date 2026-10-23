# Agent Note: 六宿主本地标点表跟进契约与顿号空格反查随开关

Status: implemented

## Problem

契约 README 要求「加字或改字时这些手抄表都要跟着改，否则会出现错位」。契约第 3 版（`{`/`}` 直角引号，#9185）与第 4 版（`enumerationComma` 段，#9241）落地后，六处宿主本地表没有跟进：[顿号笔记](./2026-10-21-enumeration-comma.md) 的「未做」清单明确列了「触屏键面与空格反查的本地表同步」和「D4 `、` 空格反查」两项。

更具体的行为缺口：`、` 的「空格转回 ASCII」手势在四个宿主（macOS、Linux IBus/Fcitx5、iOS、Android、Harmony）都把 ASCII 孪生写死成 `/`，与顿号偏好无关——用户关掉 `enumeration_comma_slash` 后，按 `/` 仍能经空格反查把 `、` 转回 `/`，而一个「不是顿号键」的键不该产生这个结果。

## Decision

### `、` 的空格反查规则（D4 拍板）

ASCII 孪生按「产生它的键 + 当前开着的顿号键」判，与直角引号「ASCII 孪生按出的键算」（`MSIMEKeyProducesCornerQuote`）同一条先例：

1. `source` 是顿号键且开着 → 转回它自己。
2. `source` 不是顿号键（候选上屏、commit 里带出的 `、`）或那个键已关 → 按契约序取开着的：`\` 优先、`/` 其次。
3. 两个都关（`punctuation_marks` 读入前未归一的路径）→ `\`。

实现分两层：共享层 `client-core::punctuation` 的 `enumeration_comma_ascii(source, marks)` 持有全部规则，`ascii_for_chinese_mark` 与 `arm_space_convert` 各加 `source` 与 `marks` 参数；FFI `msime_client_smart_punctuation_arm` 把请求里的 `value.ascii`（按下键）和 `punctuation_marks(&session.applied)` 传下去，iOS/Android/Windows TIP 等走同一 FFI 的宿主自动吃到规则。Linux 与 Harmony 有各自的宿主侧表，各接同样规则；macOS 有自己的反查函数，同样按「键开着才算」判。

### 六宿主表逐个结论

| 宿主表 | 它是什么 | 改了什么 | 有意不改 |
| --- | --- | --- | --- |
| `shared/input/EnglishModeOutput.h` | macOS/Linux 英文模式的固定中文标点出字表（手抄契约） | 无代码改动——`{`/`}` 第 3 版已跟进；新增 Linux 侧「英文模式 ↔ policy.h 逐行一致」断言防再漂移 | `$` 按宽度分叉（半角 `¥`/全角 `￥`）是宿主有意行为，契约只写全角 `￥`，测试按宽度各断一条 |
| macOS `InputController.mm` | 反查表 `MSIMEASCIIForSmartChinesePunctuation` + 空格武装键白名单 | 反查函数加 `marks` 参，`、` 按顿号开关取开着的键；新增 `MSIMEKeyProducesEnumerationComma`（与直角引号同款「按出的键算」）与 `MSIMESmartPunctuationAsciiMatches`（读回验证认开着的键集合）；`MSIMESmartPunctuationSpaceKey` 补 `\` | `{`/`}` 不进反查表：宿主分支在到达引擎前把普通中文方案的 `{`/`}` 拦下用于直角引号，`MSIMEKeyProducesCornerQuote` 已覆盖；注音大千 `{`/`}` 由引擎 `SHIFT_PUNCTUATION` 出『』，按同一先例不算 |
| Linux `SmartPunctuationSpace.h` + 两个引擎 | 空格反查表 + IBus/Fcitx5 调用点 | `、` 移出静态表，改走 `enumeration_comma_ascii(key, marks)`；`smart_punctuation_ascii_mark`/`is_space_conversion_key`/`space_conversion_ascii_text` 加 `key`+`marks` 参；IBus/Fcitx5 各记 `space_convert_key(_)` 并在武装时传按下键与 `view_punctuation_marks` | `{`/`}` 不进空格键白名单：Windows 产品合同的转换名单本就不含花括号（注释原话「braces and corner brackets are not part of the product contract」），macOS 的 `{` 反查是它自有的拦截路径扩展 |
| iOS `KeyboardPunctuationContext.swift` | 符号面板选中中文标点 → 发给引擎的 ASCII 键 | `、` 移出 `chineseInputs`/`japaneseInputs`，改走 `enumerationCommaInput(backslash, slash)`；`MetasequoiaInputSnapshot` 解析 `punctuation_marks` 两个布尔 | `{`/`}` 不加：这张表把「一个字」折回「一个键」，直角引号是交替字，两个方向各归一键无意义；`「」『』` 经样式键 `'`/`"`/`[`/`]` 已可达 |
| Android `ChineseSymbolFaces.java` | 触屏符号键的中文键面 | `Marks` 记录加 `enumerationCommaBackslash`/`Slash`；`\`/`/` 键面按开关画顿号或 ASCII 字面；`InputViewValuePolicy.punctuationMarks` 解析两个布尔 | `{`/`}` 不加：`{`/`}` 键只在 `#+=` 层，那层的 `LayerKeys` 原样插入字面，不经引擎标点路由，键面自然是 ASCII |
| Harmony `ChineseSymbolFaces.ts` + `SmartPunctuationSpacePolicy.ts` | 触屏键面表 + 空格反查策略 | `face()` 加顿号开关参，`\`/`/` 按开关画；`PunctuationMarksView` 加两字段；`asciiFor`/`arm` 加 `key`+开关参；`KeyboardState` 与三处构造透传，`arm` 调用带按下键 `ascii` 与 `lastView.punctuation_marks` | `{`/`}` 不加：`{`/`}` 键不在符号面键位上 |

### 顺带的基线修复

- `crates/engine/src/session/tests.rs` 有两处 `cargo fmt` 漂移（基线 #9253 引入），本分支顺手 fmt 掉，否则 `cargo fmt --all --check` 过不了。

## 拍板记录

- **顿号反查「按下的键优先」而不是「契约序优先」**：候选带出 `、` 时 source 传 0 落契约序；按键产生的 `、` 转回按下那个键，与 macOS 直角引号先例一致——用户按 `/` 出 `、` 按空格拿回 `/`，比统一转 `\` 更可预期。
- **`/` 不进 `simple` 反向名单**：`is_supported('/')` 仍为假不变；空格反查不是「标点支持表」的读法，开着的顿号键集合才是。

## 历史检索

按 `顿号`、`enumeration`、`punctuation`、`空格转换`、`space convert` 检索 proposed + implemented + rejected：

- [顿号自定义](./2026-10-21-enumeration-comma.md)：**归属本篇**。本篇落地它「未做」清单里的「触屏键面与空格反查本地表同步」与「D4 空格反查」两项，D4 规则在上面拍板；Windows TIP 帧、原生设置页、msime-cloud 字段仍未做，留在那篇。
- [引号样式与方头括号样式](./2026-10-13-punctuation-mark-styles.md)：**部分重叠**。同一份契约同一批手抄表；它持有 styles 两个样式维度，本篇持有 enumerationComma 开关维度在宿主表上的消费，互链保留。
- [英文模式标点输出](./2026-10-09-caps-lock-ascii-punctuation.md)：**部分重叠**。英文模式手抄表是本篇六表之一；它那次没动契约，本篇给这张表补上契约一致性断言。

## Alternatives considered

- **顿号反查统一转 `\`（不按下的键）**。最强理由是实现最简，宿主不用记 source。没采用：用户按 `/` 上屏 `、` 按空格拿回 `\` 是可见的错；macOS 直角引号已证明「按出的键算」这条先例成立且更准。
- **只在共享层做、宿主表不写死 `\`/`/` 就算了**。最强理由是少改代码。没采用：Linux/Harmony 宿主表和 macOS 反查函数都是各自进程里的本地判断，FFI 的武装快照不带 source/marks 时宿主自己还得再判一遍，单点规则两边跑会漂移——这正是 README 防的「错位」。
- **`{`/`}` 也塞进所有宿主的空格键白名单**。最强理由是「契约第 3 版加过」。没采用：Windows 产品合同的空格转换名单没有花括号；`{`/`}` 产出的直角引号在 macOS 已有专门覆盖，Linux/Harmony 不经宿主拦截、也不在白名单里，与产品合同一致。

## Consequences

- **收益**：六处手抄表各自定位写清；顿号空格反查、iOS 符号面板顿号键、三个触屏键面都随 `punctuation_marks` 的两个布尔走，关掉 `/` 后 `/` 在任何宿主都不再被当顿号来源；Linux 测试新增「英文模式表 ↔ `policy.h` 全表一致」断言，第 5 版契约再加字时直接在本地红。
- **代价**：`ascii_for_chinese_mark`/`arm_space_convert`/FFI arm 与五个宿主的辅助函数签名都加了参数；`SmartPunctuationSpace.h` 的 `、` 不再静态可查，单测改为按 marks 组合断言。
- **验证**：`smart_punctuation_space.cpp` 断言顿号四种开关组合与英文模式契约全表（红证据：临时删 `EnglishModeOutput.h` 的 `{`/`}` 分支，断言 `english_mode_output('{') == "「"` 在 `smart_punctuation_space.cpp:176` 失败）；client-core `enumeration_comma_space_reverse_follows_the_enabled_keys` 覆盖四种开关组合与 arm+decide（红证据：`enumeration_comma_ascii` 改回写死 `/` 时断言 `ascii_for_chinese_mark('、','\\',backslash_only) == '\\'` 失败，left `/` right `\`）；`cargo fmt/test/check`、`scripts/run-checks.sh`、`apps/desktop tsc`、`pnpm run verify-notes` 全过。
