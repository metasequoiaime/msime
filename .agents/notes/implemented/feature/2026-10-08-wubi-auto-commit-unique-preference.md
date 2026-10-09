# Agent Note: 四码唯一自动上屏接进共享偏好，删掉 macOS 那条永不触发的第二实现

Status: implemented

## Problem

「五笔四码唯一候选自动上屏」有开关、有界面，但开关不起作用：

- 行为在共享 `crates/input-runtime` 的 `Runtime::dispatch` 里**无条件**执行——字符键、快照 `wubi_unique_four_code`、不在造词，就 `select(首选)`。`Runtime` 没有任何 setter 能关掉它，Engine 的 `WubiInputOptions` 里也没有这个选项。
- 唯一的开关只在 macOS：原生 defaults 键 `MSIMEClientWubiAutoCommitUnique`，经 `InputController.mm` 的 `MSIMEShouldAutoCommitWubi` 判断后补一刀。但 macOS 走的正是共享路径，`Runtime::dispatch` 已经把词上屏，`transition[@"commit"]` 就是 `NSString`，而那段判断的前提是 `![transition[@"commit"] isKindOfClass:NSString.class]`——**条件恒为假，分支不可达**。
- 共享偏好契约（`crates/client-core/src/preferences.rs`、`crates/mcp-server/src/preferences.rs`）里没有这个字段；Windows、Linux、Android、HarmonyOS 连开关都没有。

结果是 macOS 的设置页展示了一个无效控件，README 写着「默认关闭」而实际行为是恒定开启，`docs/macos-assertion-audit.md` 记录的那条断言守着一段死代码。

## Decision

开关落在共享运行时和共享偏好契约里，所有平台读同一份文档、走同一条路径：

- `Runtime` 新增 `wubi_auto_commit_unique`（缺省 `true`）和 `set_wubi_auto_commit_unique`。`dispatch` 里的判定抽成 `wubi_should_auto_commit`，把开关作为第一个条件；两处调用点夹在 `refresh()` 前后，故意读到不同代的快照，语义不变。它是宿主状态而不是 Engine 状态：改在组字中途也安全，下一键生效，已打好的四码不会因为关掉它就自己上屏。
- 共享偏好加 `wubi_auto_commit_unique`，`#[serde(default = "enabled_by_default")]`，缺省 `true`。缺省是开，因为接入之前它是唯一可能的实际行为——缺省关等于让所有平台升级后手感突变，而 macOS 那个 `NO` 从来没兑现过。旧文档没有这个键，serde 缺省读出来仍是开。
- `host-api` 在会话建立（`ffi/session.rs`）和 `HostSession::update`（偏好到达）时把值交给 `Runtime`，与 `set_ai_provider_cache` 同一条「跟着最新请求的偏好走、不等 Engine 重建」的规矩。
- 各平台设置面补上控件：共享 React 页去掉 `macos ?` 限制（macOS/Android/HarmonyOS 一起生效）；Windows 原生设置窗口加一行 `bool_row`；Linux 的 IBus 属性菜单和 Fcitx5 `FcitxSchemeBooleanAction` 各加一项；iOS 输入设置页加一行 `DesignToggleRow`。macOS 原生外观面板和云同步沿用既有机制（`SharedOverrideProperties()`、`MSIMECloudBooleanPreferences()`）并入共享文档。
- iOS 的三个五笔开关（混拼、剩余编码、四码唯一自动上屏）统一到共享文档：设置页三项都写文档、键盘渲染改读 `session.sharedPreferences`，删掉 App Group 的读、写与文档→App Group 镜像，以及桥上为此存在的 `setWubiMixedPinyin`。`wubi_mixed_pinyin`、`wubi_auto_commit_unique` 两个引擎字段还要加进 `reloadSharedPreferences` 转发给活动会话的 `appEditedKeys`，否则键盘扩展进程活着时改了设置，要等它被杀掉、重建会话才生效。`WubiCodeHintPreference`、`WubiMixedPinyinPreference` 两个文件保留原名，内容改成"文档键 + 缺省 + `isEnabled(in:)` 纯读取"（前者还保留 `hint`/`maxCodeLength`，`CandidatePanelSnapshot` 与它的单测仍在用），这样不必重生成 XcodeGen 的 `project.pbxproj`。混拼的版本默认不再由 App Group 扛：`Preferences::for_edition` 在准备宿主时已把它写进第一份共享文档。
- 账号同步的键表两份都加这个键：Android 侧的 `crates/client-core/src/account/settings_sync.rs`（导出与应用），鸿蒙侧的 `platforms/harmony/entry/src/main/ets/account/AccountPreferencePlan.ts`（上传与下载）加 `input.wubi_auto_commit_unique`，与它的兄弟键 `input.wubi_code_hint` 保持一致，免得服务端声明该字段后只有 Android 会传。
- 删掉 macOS 的第二实现：`WubiCommitPolicy.h`、`WubiCommitPolicyTest.cpp`、`InputController.mm` 里那个不可达分支，以及 Tauri 侧 `load/save_macos_wubi_auto_commit_unique` 两个命令和 React 的一整套 `macosWubiAutoCommitUnique` 脏检查管道。共享路径接管后它们是重复的另一半。

macOS 的 defaults 键保留。原生外观面板仍读写它，值经 `merged[@"wubi_auto_commit_unique"]` 进共享文档；缺省值从 `@NO` 改成 `@YES`，并配一段迁移。这个键在接入共享偏好之前接不到实际行为，所以 defaults 里存下来的 `NO`（旧缺省、旧版本套用云快照时写回的）都是历史缺省而不是用户选择；云端 `platform.macos.wubi_auto_commit_unique` 的 `false` 同理。迁移用一个 `MSIMEClientWubiAutoCommitUniqueAdopted` 标记：只有本机留下过新语义下的真实选择之后，存下来的 `NO` 才作数；在那之前 getter 一律读作开。真实选择有两个入口——原生面板的 `setWubiAutoCommitUnique:`，和共享设置页写进共享文档的显式 `false`（`applySharedInputPreferences:` 读到文档里的 `false` 时也记标记；文档里的 `true` 不算选择，因为升级时把旧的原生 `NO` 合并进文档之前 getter 已经读作开）。没有这一条，用户在共享设置页关掉开关后，下一次账号同步的云快照（含旧客户端留下的历史 `false`）会被当成历史缺省、经 `sharedPreferencesByMerging:` 恢复成开。因此升级后、以及旧账号同步把 `false` 套到本机后，第四键照旧上屏，下一次上传还会用生效值把云端的历史 `NO` 改成 `true`；而用户在共享设置页或原生面板做出的关闭都能留住。配套两处：`cloudSettingsSnapshot` 的这个键改成取生效值（对齐 `shuangpin_preedit_uses_raw` 等共享文档字段，否则共享设置页只写文档不写 defaults 会导出旧值），`applyCloudSettingsSnapshot:` 套用时把这个共享文档字段的缓存一并清掉（它现在在云快照里）。

Linux 的 IBus 属性菜单在没有偏好目录时改走 `close()`/`open()` 重建会话：这个开关是由 runtime 判定的，override 只在会话建立或重读偏好时经 `apply_session_overrides` 交给它，照搬 `WubiCodeHint`（宿主渲染时读）那种「改状态 + render」的做法会让菜单显示关了、四码唯一仍在上屏。

## Alternatives considered

- **只修 macOS**：让 `MSIMEShouldAutoCommitWubi` 真正生效，其他平台维持恒定开启。改动最小，但「开关只在一半平台存在」正是这次要消灭的不一致；共享契约里仍然没有这个字段，Windows/Linux 用户照样关不掉。
- **把开关放进 Engine 的 `WubiInputOptions`**：偏好流经 Engine 的现成管道，不用动 `Runtime`。但上不上屏是宿主行为——Engine 只负责在快照里回答「这个码是不是四码唯一」，把提交与否塞回 Engine 会让「Engine 报告事实、宿主决定动作」的分工反过来，而且 Engine 重建要等组字空闲，开关的生效会滞后。
- **缺省关，与 macOS 的 `@NO` 和 README 一致**：文本上更「诚实」，但那是兑现一个从未生效的承诺，代价是全平台可见的行为回退。行为基线取实际发布的行为，文档改成跟行为一致。
- **保留 macOS 的 `MSIMEShouldAutoCommitWubi` 作为兜底**：共享路径关掉时由它兜住。两条路径写同一份输入法状态，谁先谁后、失败一半时归谁，都是新的边界；一处开关两处判定，正是这次的病灶。

## Consequences

- **收益**：设置页和实际行为在五个平台上一致；开关只有一个落点（`Runtime` 的一个字段），判定只有一处；macOS 少一份重复实现和一个不可达分支。
- **代价**：macOS 升级后 defaults 里没写过这个键的用户看到的状态从「关」变成「开」——但这只是显示追上行为，输入手感不变。`wubi_code_hint` 只影响显示，这个开关改变提交时机，所以 Linux 的 `CandidateAnnotationPreferencePolicy` 那套注释策略对它不适用，Fcitx5 侧不需要对应的展示位。`engine-wasm` 的 `host.rs` 仍有自己的无条件上屏（`wubi_letter` 读快照直接 `select_seat`），网页引擎没有设置面，这次没动——第三方接入方要关它得等它也有偏好通道。
- `packages/ui/src/settings/settings-dirty.ts` 原本就没有任何调用方，这次把它引用的已删字段换成仍然存在的双拼原生键，让文件保持自洽；它是死代码这件事不在本篇范围内。

## Verification

- `cargo test -p msime-input-runtime --lib`：新增 `turning_off_wubi_auto_commit_keeps_the_unique_four_code_in_the_candidate_list`——关掉后四码不上屏、词留在候选里；同一会话改回开后下一个四码照旧自动上屏。
- `cargo test -p msime-client-core`：`wubi_auto_commit_unique_defaults_on_and_roundtrips`（缺省开、旧文档缺键仍读出开、关掉后往返保持）；`settings_sync` 两条（导出键清单、逐键往返）。
- `cargo test -p msime-mcp-server`：`update_preferences` 用例补 `wubi_auto_commit_unique: Some(false)`。
- `pnpm --filter @msime/desktop typecheck`、`pnpm --filter @msime/desktop test`：`wubi-section` 三条（缺省开、走共享 patch 写入）、`settings` 的双平台共享保存用例取代原来那条「写原生域、共享文档不动」的用例。
- `platforms/linux/tests/core/settings_launcher_contract.py`、`fcitx5_contract.py` 各补对应断言；`platforms/macos/tests/settings/CloudAppearanceSettingsTest.mm` 的布尔缺省分组挪项、新增 `WubiAutoCommitPreferencesTest.mm`（存量 `NO` 与旧云快照的 `false` 都读作开，原生面板或共享设置页选过一次之后才作数）。
- `platforms/harmony/tests/run.sh`：`localAccountPreferences` / `applyAccountPreferences` 的用例覆盖新键（键表声明与 `wubi_code_hint` 一起摆在布尔声明组里）。
- iOS 三个五笔开关都写共享文档、从共享文档读；`KeyboardSkinTests` 去掉对 `WubiCodeHintPreference` App Group 键的强设，改由文档缺省承担。没有新增 App Group 键，也没有动 XcodeGen 的 `project.pbxproj`。
