# Agent Note: 悬浮工具栏自动隐藏与阴影搬进共享偏好

Status: implemented

## Problem

Windows 旧正式版 v0.11.1 的悬浮工具栏有三个用户可调项：`floating_toolbar_auto_hide`（出厂 `false`）、`floating_toolbar_auto_hide_delay`（出厂 5 秒，可选 1–60）和 `floating_toolbar_shadow`（出厂 `true`），见 `installer/default_config/config.default.toml`、`server/src/config/ime_config_toolbar.cpp` 和 `server/src/settings/native/native_page_floating_toolbar.cpp`。

macOS 把这两个行为都写死了：没有输入 10 秒后自动隐藏且始终开启（`platforms/macos/src/core/FloatingToolbarPanel.mm` 的 `_idleTimer`），窗口阴影固定 `self.hasShadow = YES`。用户改不了，而且自动隐藏的出厂行为和 Windows 相反（Windows 默认关）。`FloatingToolbarPreferences` 里也没有对应字段，共享设置页没有这些行。

限定：新架构 Windows 宿主（`platforms/windows/`）同样还没有搬这三个设置，所以这不是 macOS 独有的落后，是共享偏好缺一项共同能力。

## Decision

`crates/client-core/src/preferences.rs` 的 `FloatingToolbarPreferences` 增加三个带 serde 默认值的字段，出厂值对齐 Windows v0.11.1：

- `auto_hide: bool`，默认 `false`；
- `auto_hide_delay: u16`，默认 `5`；
- `shadow: bool`，默认 `true`。

旧文档缺这些键时按出厂值读，不写回也不迁移。

共享设置页 `packages/ui/src/settings/pages/floating-toolbar-page.tsx` 在「显示」组里加三行：`auto_hide` 开关、「自动隐藏延时」滑块（1–60 秒，关闭自动隐藏时置灰但保留已选秒数，与 Windows 设置页一致）和 `shadow` 开关。延时的显示值经 `floatingToolbarAutoHideDelay` 投影：缺值、越界、非有限值都回退出厂 5 秒，滑块不会被一个坏值卡住。类型和缺省投影同步在 `floating-toolbar-preferences.ts` 与 `floating-toolbar-defaults.ts`。能力位 `floating_toolbar_idle_behavior` 仅在 macOS 为真，Linux、Windows 和 HarmonyOS 不显示这几行；Linux 即使能绘制独立悬浮条，也尚未接入这些行为。

判定抽成纯函数 `platforms/macos/src/core/FloatingToolbarBehavior.h`，不依赖任何窗口：

- `ResolveFloatingToolbarBehavior(hasAutoHide, autoHide, hasDelay, delay, hasShadow, shadow)` 返回 `FloatingToolbarBehavior{autoHideEnabled, autoHideDelaySeconds, drawsShadow}`。缺键取出厂值；延时按最近整数取整后落在 1–60 内才采用，越界、缺值和非有限值回退到 5 秒（与 `server/src/config/ime_config.cpp` 的载入校验一致，不是夹到最近边界）；阴影缺键取 `true`。
- `ShouldHideFloatingToolbarForIdle(behavior, imeActive, requestedVisible)` 只在自动隐藏开着、输入法仍活动且工具栏仍被要求显示时为真。自动隐藏关掉后残留的计时器仍会调它，所以在隐藏前再判一次。

`FloatingToolbarPanel.mm` 的 `applySizingPreferences:` 从 `floating_toolbar` 读这三个键、调上面的解析函数，把结果存进 `_behavior` 并参与「偏好没变就跳过」的比较；自动隐藏关闭时作废计时器并清掉已隐藏状态。`noteInputForDelegate:` 按 `_behavior.autoHideDelaySeconds` 排 `NSTimer`，关闭时不排；计时器回调先过 `ShouldHideFloatingToolbarForIdle`。`self.hasShadow = behavior.drawsShadow` 取代写死的 `YES`。

## Alternatives considered

- **把 macOS 旧的「常开 10 秒」设为缺省。** 老用户升级后行为不变，最省解释成本。但同一个共享偏好要有唯一的出厂值，Windows 出厂是关；让 macOS 单独缺省为开会把两个宿主的默认继续分成两套，且无法再用「Windows 出厂配置」这一条依据。改为默认关，需要旧行为的用户自己打开并选延时。
- **越界延时夹到最近的 1 或 60。** 更能保住用户填写的意图。但 Windows 的载入校验是回退到默认 5（`ime_config.cpp`），夹边界会让两端对同一个坏值给出不同结果；对齐 Windows 更可预期。
- **在 `FloatingToolbarPanel.mm` 里就地判断，不抽纯函数。** 改动最少。但 `FloatingToolbarPanelTest.mm` 会创建 `NSPanel`，没有图形会话的机器上直接 abort；抽成纯 C++ 后 `floating-toolbar-behavior` 这个 CTest 在无图形会话的本机也能跑，覆盖缺键、越界、非有限值和三个隐藏条件。
- **用一个值同时表达开关和延时（如 0 表示关闭）。** 字段少。但关闭时用户已经选过的延时就没有地方存，重新打开会丢；Windows 也是两个独立键。

## Consequences

- **收益**：macOS 用户可以关掉自动隐藏、把延时调到 1–60 秒、关掉阴影；新装的出厂行为与 Windows v0.11.1 一致。三个判定都有不依赖窗口的 CTest 覆盖。
- **代价与已知上限**：
  - macOS 的默认行为翻转：升级后不再自动隐藏，靠 10 秒隐藏的用户要在设置里打开并选延时。这是对齐 Windows 出厂的直接结果。
  - 新架构 Windows 宿主还不读这三个键，能力位因此为假；Windows 的原生设置页也没有新增这三项。共享设置页只在真实消费它们的 macOS 显示，其他宿主接入后再开启能力位。
  - `auto_hide_delay` 不做 Rust 层的拒绝式校验：坏值由 macOS 的纯函数回退到 5 秒，避免整份偏好文档因此载入失败。

## Verification

- `ctest --test-dir target/macos-isolated -R '^floating-toolbar-behavior$' --output-on-failure`：先对着写死 10 秒常开的占位跑红（断言失败），替换 `FloatingToolbarBehavior.h` 后转绿。
- `cargo test -p msime-client-core`：`floating_toolbar_component_defaults_and_roundtrip` 与 `legacy_preferences_without_toolbar_use_component_defaults` 断言出厂值 `auto_hide=false`、`auto_hide_delay=5` 和序列化往返。
- `pnpm --filter @msime/desktop test`：共享叶子模型的缺省投影断言自动隐藏默认关、延时 5 秒。
