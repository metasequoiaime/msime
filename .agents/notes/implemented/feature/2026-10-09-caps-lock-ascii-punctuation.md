# Agent Note: 大写锁定时使用英文标点

Status: implemented

## Problem

#6370：开着中文标点时，按下大写锁定打大写英文，标点仍是中文标点；只有切到英文模式才会出英文标点。用户希望大写锁定期间标点自动改用英文标点，并且做成开关。

大写锁定只有宿主知道。标点是中文还是 ASCII 却不在宿主决定：多数宿主把标点键当普通字符交给 `msime_client_character`，由 Engine 按中文标点开关出字；`msime_client_punctuation_with_context` 走 `client-core` 的 `punctuation::route` 共享判断（智能标点、固定标点）。

## Decision

规则放在共享层，宿主只报告大写锁定状态：

- 共享偏好 `Preferences::caps_lock_ascii_punctuation`（`crates/client-core/src/preferences.rs`），serde 缺省和 `Default` 都是 `false`，打开前行为不变。
- `punctuation::route` 的 `PunctuationContext` 多一个 `caps_lock_ascii`（大写锁定打开且开关打开）。它排在「组字中」「宿主上下文不可用（英文模式、本地模式、韩文/越南文/藏文/注音/日文）」和 `punctuation_lock` 之后：组字中仍由 Engine 决定，固定中文标点仍是中文标点，其余没有组字的标点一律走 ASCII 路线，和英文模式一样。
- `host-api` 的会话记住宿主报告的状态（新 C 接口 `msime_client_set_caps_lock`，会话状态而非偏好）。`dispatch` 在分发前用 `HostSession::caps_lock_punctuation` 把 `Action::Character`（ASCII 标点）和 `Action::Punctuation` 改成 `Action::PunctuationAscii`；`msime_client_punctuation_with_context` 用同一份 `HostSession::punctuation_context`。没有组字时 ASCII 路线不出字（`handled: false`），键交回宿主按普通字符输出，全角开关照旧由宿主处理。
- `HostCapabilities::caps_lock_punctuation` 只在 macOS 为真，共享设置页「标点」组的「大写锁定时使用英文标点」只在声明了它的宿主上显示。
- macOS 宿主（`InputController.mm` 的 `syncCapsLock`）在激活、会话建立和按键时大写锁定状态变化时报告；`MSIMEClientSession` 在内部重建会话后恢复这个状态。

### 平台接入情况

- 已接入：macOS。
- 待跟进：Windows（Server 已有 `caps_lock` 状态，`server_main.cpp`）、Linux 的 IBus 与 Fcitx5、带实体键盘的 Android、iOS 和 HarmonyOS。各自在按键路径上调用 `msime_client_set_caps_lock`，再把平台加进 `HostCapabilities::caps_lock_punctuation`，共享层不需要再改。

## Alternatives considered

- **只在 macOS 宿主里改**：在 `InputController.mm` 里大写锁定时直接调 `punctuationASCII:`，改动最小，也不用碰 C 接口。但标点去向本来就是共享层在决定（Engine 的中文标点开关、`punctuation::route`），宿主里再写一条规则，其他平台要做同样的事就得各抄一份，而且会和固定标点、英文模式这些规则的先后顺序各自漂移。
- **新增带大写锁定参数的字符入口**：例如给 `msime_client_character` 加一个参数。状态跟着每次按键走，不会过期，但要改每个宿主的每个调用点和 JNI/NAPI/Swift 绑定；大写锁定本来就是一个持续的状态，宿主已经在跟踪它的变化（工具栏的大写指示），报告变化就够了。
- **组字中也改成 ASCII**：`Action::PunctuationAscii` 在组字中会上屏候选再接 ASCII 标点，技术上可行。但大写锁定下新起的大写字母本来就交还给应用（`MSIMECapsLockFreshUppercaseBypass`），组字中按标点通常是锁定大写之前开始的中文输入，`route` 的既有约定也是组字中一律交给 Engine，这里不破例。

## Consequences

- **收益**：一条规则、一个开关，任何宿主接上 `msime_client_set_caps_lock` 就能用，不用再写标点逻辑。默认关，现有用户无变化。
- **代价与已知上限**：
  - 开关打开且大写锁定时，没有组字的 `/`、`@` 等也是字面符号，不会打开 `/` 指令或 `@` 提及模式，和英文模式下一样。
  - `caps_lock_ascii_punctuation` 是顶层字段，`Preferences` 带 `deny_unknown_fields`，早于它的设置应用或 `msime-mcp` 会拒读写过这个字段的文档；正式包里输入法和设置应用版本一致。
  - 没有加进账号设置同步，也没有加进 macOS 原生设置窗口和 MCP 的 `update_preferences`。

## Verification

- `client-core`：`punctuation::tests::caps_lock_switch_*` 覆盖没有组字时全部 ASCII 标点改走 ASCII、组字中、宿主上下文不可用和固定中文标点时不变；`preferences::tests::caps_lock_ascii_punctuation_defaults_off_and_survives_a_save` 覆盖默认关、旧文档缺键、保存读回；`host_surface` 断言只有 macOS 声明能力。
- `host-api`：`caps_lock_sends_idle_punctuation_to_ascii_when_the_switch_is_on` 经三个标点入口验证改道，以及组字中、固定中文标点、大写锁定关闭、开关关闭时仍出中文标点。
- macOS CTest `shortcut`：大写锁定变化时控制器向会话报告一次，关闭时再报告一次。
- `apps/desktop/tests/settings/punctuation-section.test.tsx`：开关只在宿主声明时出现，默认关，点开写入 `caps_lock_ascii_punctuation: true`。
- 没有在真实 macOS 输入法里装机验证。
