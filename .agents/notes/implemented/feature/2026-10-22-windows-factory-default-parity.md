# Agent Note: Windows 对照补齐共享偏好开关能力（不动默认值）

Status: implemented

## Problem

C 档对照复核（`/Users/admin/worktrees/_refs/parity-tier-c-review.md`）发现新仓共享偏好有三处「能力缺口」：候选窗 fit_content 只在皮肤包 `skin.toml` 上存在、用户不可达；日期时间混入候选和 T 模式共用一个开关、无法单独关掉行内混入；另外一批默认值与 Windows v0.11.1 出厂配置不一致（按门禁 pin `345cb87a` 猜的，pin 与 v0.11.1 本身也有出入）。任务范围定为**只补能力、不翻默认值**——默认值翻转会改变现有用户手感，那是产品决策，需单独授权。

## Decision

补齐三个缺失的用户开关；逐项记录默认值与 v0.11.1 的差异，**本次一律不改**。

### 补的能力

- `local_modes.date_time`（Shift+T 模式）与新增的 `local_modes.date_time_inline`（对应 Windows `utility.date_time_candidates`）分开。Engine 侧 `LocalModeOptions.date_time_inline`、`EngineOptions.local_date_time_inline`，26 键 `insert_inline_date_time` 与九键的时钟注入都只看行内开关，`date_time_menu`（📅 入口）改挂 `date_time_inline`。缺键读作开——此前行内混入由 `date_time` 一并控制且默认开，保持同样的有效行为；用户现在可以单独关掉行内混入而不动 T 模式。
- 新增顶层 `candidate_window_fit_content`（出厂关，关着不写进文档），与皮肤 `fit_content` 是或关系，用户键从此可达。macOS 的 `AppearancePreferences` 读共享文档该键，`InputController` 三处 `_candidateLayoutFitContent = skin.fitContent || appearance.candidateWindowFitContent`；共享设置页「外观」加「收紧窗口宽度」行，只向消费它的 macOS 显示。
- Windows 出厂模板（`platforms/windows/installer/config.default.toml`）补六个 post-pin 键（值填本仓现行默认），`test-reference-config-coverage.py` 的 MAPPING 同步登记六个映射。

### 默认值与 v0.11.1 的差异（记录，本次不改）

| 共享字段 | 参考键 | v0.11.1 | pin | 本仓出厂 | 备注 |
|---|---|---|---|---|---|
| `navigation.left_right` | `general.candidate_horizontal_arrow_navigation` | 关 | 无此键 | 开 | post-pin 键 |
| `local_modes.expression` | `utility.v_mode` | 开 | 无此键 | 关 | post-pin 键 |
| `local_modes.emoji` | `utility.emoji_mode` | 关 | 开 | 开 | pin 与 v0.11.1 不一致 |
| `local_modes.kaomoji` | `utility.kaomoji_mode` | 关 | 开 | 开 | pin 与 v0.11.1 不一致 |
| `local_modes.chaizi` | `utility.radical_mode`（pin 叫 `unicode_mode`） | 开 | 开 | 触屏开 / 桌面缺键读关 | 参考 Shift+U 拆字，本仓在 Shift+C；桌面读关是怕大写 C 被截，属有意差异 |
| `mixed_input.symbols` | `general.symbol_mixed_input` | 关 | 无此键 | 开 | post-pin 键 |
| `quanpin_helpcode.enabled` / `shuangpin_helpcode.enabled` | `helpcode.{quanpin,shuangpin}_helpcode` | 关 | 开 | 开 | pin 与 v0.11.1 不一致 |
| `local_modes.date_time_inline` | `utility.date_time_candidates` | 关 | 无此键 | 开 | 拆分后仍沿用改动前的有效行为 |

### 判为单独立项的项

`network.proxy_*` 自定义代理只覆盖共享 Rust 栈（reqwest 客户端），不覆盖 macOS 原生 HTTP（`TranslationCache.mm` 的 NSURLSession、C++ 侧 native transport）。把自定义代理接进原生传输要动每个原生网络栈并逐平台验证，超出本批范围，单独立项见 `../../proposed/feature/2026-10-22-custom-proxy-native-http.md`。

## Alternatives considered

- **默认值全部对齐 v0.11.1**：C 档复核初稿曾按这个方向实现，随后收窄——翻默认值改变现有用户手感，属产品决策，需单独授权；差异留在上表供拍板。
- **`date_time_inline` 缺键读关（严格按出厂）**：会把所有缺键用户的行内日期时间关掉，等于改了默认行为；选缺键读开保持有效行为，拆分只新增「能单独关」的能力。
- **fit_content 挂在皮肤字段不动**：六个出厂皮肤全是 false，用户永远到不了收紧宽度，等于没有这个功能。

## Consequences

- 行内日期时间混入可以独立关闭；`candidate_window_fit_content` 用户可达；Windows 模板把 post-pin 键写全。除此之外行为与改动前完全一致：所有既有默认、序列化省略规则和 `chaizi` 的宿主形态语义不变。
- Engine 默认值（`LocalModeOptions::default()`、`prepare_options`）保持「参考本地模式全开」的夹具语义：`date_time_inline` 在引擎层默认开。
- 测试按「缺键读现行默认 + 写明值不覆盖」保留（`emoji_and_kaomoji_modes_default_on_and_keep_written_values`、`expression_mode_defaults_off`、`date_time_inline_defaults_on_and_is_written_only_when_off`、`left_right_navigation_defaults_on`、`mixed_symbols_defaults_on`、`helpcode_enabled_defaults_on`、`candidate_window_fit_content_defaults_off`），引擎层 `inline_date_time_is_a_separate_switch_from_the_mode` 钉住 T 模式与行内混入的分闸，九键同样走该开关。
- `sentence_source_badge` 在 C 档清单里被错写为「已判定不做」，清单行更正为「proposed 待拍板」（`_refs/parity-gaps.md`），仓库里的 proposed 笔记维持。
