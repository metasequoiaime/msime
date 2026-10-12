# Agent Note: Windows 和 Linux 用 Ctrl+Shift+J 在中文和日语之间切换

Status: implemented

## Problem

#7063：报告者希望在中、英、日三种语言之间切换，或者至少有快捷键做「中英」「日英」切换；现在中文和日文「只能二选一」，而且「必须到设置里面才行」。日语是 `InputScheme::Japanese`，和中文方案互斥，靠 `scheme` 和 `last_chinese_scheme` 两个偏好来回。

各平台的情况不同。macOS 把「中、双、五、日、한、英」声明成系统输入模式，Ctrl+空格和地球键会停在「日」上，已经满足。Android 由 [中英键轮换其他语言](2026-10-10-android-language-key-cycle.md) 覆盖（默认关）。Windows 和 Linux 只注册一个中文输入法，托盘、工具栏、IBus/Fcitx5 菜单都能一步换方案，但都要点鼠标，没有一个键盘快捷键能在中文方案和日语之间来回。

## Decision

- 共享偏好 `keybindings.toggle_japanese_ctrl_shift_j`，serde 缺省和 `Default` 都是关。`HostCapabilities::japanese_toggle_shortcut` 只在 Windows 和 Linux 上为真。共享设置页「快捷键 › 输入模式切换」在声明了它、并且本版本同时提供中文方案和日语（`edition.input_schemes` 缺省即 full）时多一行开关，判断在 `offersJapaneseToggle`；Windows 原生设置窗口同样只在 full 版显示这一行。说明写明它和浏览器开发者工具快捷键的冲突。
- 语义是中⇄日，不是轮换：中文状态下，Engine 实际跑的是中文方案时切到日语，并把这个中文方案记进 `last_chinese_scheme`；跑的是日语、韩语、越南语或藏语时回到 `last_chinese_scheme`，它跑不起来时依次退到版本的默认方案和第一个跑得起来的中文方案。规则两份实现、同一套写法：Windows 的 `common/InputSchemeTraits.h` `japanese_toggle_target`，Linux 的 `src/core/InputSchemes.h` `japanese_toggle_target`。
- 英文模式下组合键交给应用，和 Ctrl+Shift+F 一致。Engine 自己的英文模式（Ctrl+Shift+E，`dedicated_english`）里宿主仍报告中文，这时同样交给应用：Windows 的 TIP 读 `Global::DedicatedEnglish`，Server 再看一次视图；Linux 两个宿主读视图的 `dedicated_english`。
- Windows：
  - 新增可选协商位 `FanyImeProtocol::JapaneseToggleShortcut = 1u << 7`，不进 `Capabilities` 和 `RequiredCapabilities`。生产 Server 和 TIP 都宣告它，TIP 只在协商到它时才吃 Ctrl+Shift+J。判定 `IsJapaneseToggleShortcut` 照 `IsCharacterSetShortcut` 只认 Ctrl+Shift。
  - TIP 把它分成 `FUNCTION_TOGGLE_JAPANESE`，追加在 `FUNCTION_KOREAN_HANJA_KEY` 之后（38）。Ctrl+Shift+F 的四处特例它都照做：自动重复不再排队、Server 不可用时直接结束、能力在吃下之后丢失时不当普通按键发出、发出后不读回复。
  - Server 的 `production_key_handler` 多一个回调 `persist_japanese_toggle`，沿 `InputState` → `FocusedSession` → `ReplyComposer::toggle_japanese` 走到底：只看开关、中文状态和 Engine 英文模式，调用回调，回 `ReplyPath::NoReply`，不碰组字。回调把一次请求交给单次工作线程，线程上 `toggle_japanese_scheme` 和托盘的 `store_input_scheme` 一样从存储读出偏好、带 revision 写回，成功后更新托盘看到的方案；写回的 `scheme` 和 `last_chinese_scheme` 由纯函数 `japanese_toggle_write`（`common/InputSchemeTraits.h`）从存储里的两项算出，`server_main.cpp` 只管读写存储。工作线程还在处理上一次请求时，这一下照样吃掉、回 NoReply，只是不切换；方案经偏好监视器发布。
- Linux：
  - Fcitx5 的 `toggleJapanese` 走状态区菜单的 `selectScheme`，复用 `toggle_chord_held_` 让一次按下只切一次。
  - IBus 新增 `MenuPreference::JapaneseToggle`，经 `save_menu_preference` 一次写下 `scheme` 和 `last_chinese_scheme`，`japanese_chord_held` 吃掉同一次按下的自动重复和松开。上一次保存还没落盘时再按，这一下吃掉但不切换，和 Windows 工作线程忙时一样。没有偏好存储的预览实例不处理。
  - 两个宿主的菜单路径只在目标是中文方案时记 `last_chinese_scheme`，所以快捷键离开中文时自己补记当时跑的中文方案，否则回中文可能回到一个更早的方案。菜单路径本身不改。

### 组字中按下

- Windows 和 IBus 只写偏好。host-api 在组字结束、运行时空闲后才应用新偏好（`crates/host-api/src/lib.rs` 的 `preferences_pending` 检查），所以正在组的字照常由用户上屏，之后才换成日语，不丢字也不重复上屏；托盘和 IBus 属性菜单换方案本来就是这样。TIP 因此不需要一条带回复的路径。
- Fcitx5 沿用 `selectScheme`：先 `MSIME_FINISH_COMPOSITION` 上屏，再重建会话。按下的那一刻组字就结束了。
- 两种行为各自和该宿主的菜单换方案一致，没有为快捷键统一成一种。

## Alternatives considered

- **中、英、日三态轮换（Android 中英键的做法）**：最强的理由是报告者原话就是「中英日三种语言进行切换」，一个键走完三站。没有采用：桌面上中英已经有 Shift、Ctrl、Ctrl+Alt+Space 和 Caps Lock 可选，把英文也并进这个键会和那些键重叠，在中文和日语之间来回反而要按两次。桌面用户缺的只是中⇄日这一步。
- **在 full 版再注册一个 ja-JP 的 TSF 配置**：这样 Win+Space 就能轮到日语，系统输入指示器和 `GUID_PROP_LANGID` 也对。没有采用：同一个 TIP 的两个配置共用 compartment、Server 会话和偏好，切换配置不会换方案，要另写一套「配置 → 方案」的同步，并且和已经单独发布的水杉日语（ja-JP）在语言列表里重名。需要系统层面日语的用户另装水杉日语。
- **默认打开**：理由是不打开就没人发现。没有采用：Ctrl+Shift+J 是 Chrome、Edge 和 Firefox 打开开发者工具或下载页的快捷键，默认占用会让每个用户在浏览器里失去它；按约定新快捷键默认关。
- **Windows 组字中先上屏再切换（和 Fcitx5 一样）**：理由是按键的那一刻就换方案，反馈更直接。没有采用：Ctrl+Shift+F 的回复是 NoReply，TIP 发出后不读回复，要先上屏就得另开一条读回复的路径，并处理断线重连时不能重放；而 host-api 的延后应用已经保证不丢字，托盘换方案也是这样。
- **新建 `shared/input` 里的共享判定头文件**：理由是两个平台一份规则。没有采用：规则只有一句「中文方案去日语，否则回 last_chinese_scheme」，两边都有现成的 `IsChinese` 和 effective 工具，各写一个十几行的函数比新增一个跨平台头文件和它的构建接线更简单；两份函数用相同的回退顺序，各自有单测。
- **托盘「输入方案」一行写出 Ctrl + Shift + J**：繁体输出旁就写着 Ctrl + Shift + F。没有做：那一行右侧已经写着当前方案名，再塞一个快捷键会挤掉它。

## Consequences

- **收益**：Windows 和 Linux 用户不点鼠标就能在中文方案和日语之间来回，并且回到的是切走前的那个中文方案（例如双拼）；缺省关，现有用户无变化；新旧 TIP 与 Server 互配时靠协商位退回原来的行为。
- **代价与已知上限**：
  - Windows 只注册了一个 zh-CN 配置，切到日语后只有语言栏图标变成「日」，系统输入指示器仍是中文，上屏文字的 `GUID_PROP_LANGID` 也仍是中文。
  - 打开后，浏览器里的 Ctrl+Shift+J 在中文状态下被输入法占用；英文模式下照常交给浏览器。
  - Windows 和 IBus 在组字中按下时，方案要等这次组字结束才换；Fcitx5 立即上屏并切换。
  - 快速连按两下时，第二下若赶上第一次保存还没落盘（Windows 的工作线程还忙、IBus 的 `menu_save_pending`），只被吃掉、不切换，结果停在日语，要再按一次才回中文。没有像 Ctrl+Shift+F 在 IBus 上那样把这一下交给应用：Ctrl+Shift+J 在浏览器里会打开开发者工具，用户连按是想切回去，不是想用浏览器的快捷键。也没有把它排队等保存落盘后再切：那要在两个宿主各加一份排队状态，而再按一次就能补上。
  - `toggle_japanese_ctrl_shift_j` 在 `KeybindingPreferences` 里，结构带 `deny_unknown_fields`，早于它的设置应用或 `msime-mcp` 会拒读写过这个字段的文档；正式包里输入法和设置应用版本一致，和 [单按 Caps Lock 切换中英文](2026-10-11-caps-lock-language-switch.md) 的处理相同。
  - 后续项：Android 实体键盘（`HardwareShortcutPolicy`）和 HarmonyOS 2in1 实体键盘都读共享快捷键，但没有接上这个组合键，能力位按宿主分别声明，接上的那次改动再打开；iOS 和 HarmonyOS 的触屏中英键没有跟进 Android 的轮换；iOS 键盘扩展收不到实体键。

## Verification

- `client-core`：`japanese_toggle_defaults_off_and_persists_an_explicit_enable` 覆盖默认关、旧文档缺键、保存读回且不动其他快捷键；`capabilities_describe_each_host` 断言只有 Windows 和 Linux 声明能力。
- 共享设置页：`input-mode-shortcuts-section.test.tsx` 覆盖没有能力时不显示、缺省关、打开只写这一个键、说明里写出开发者工具冲突，以及 `offersJapaneseToggle` 在宿主没声明、版本缺日语或缺中文方案时为假。
- Windows：`windows-input-scheme-traits` 覆盖目标方案的计算、各版本是否提供，以及 `japanese_toggle_write` 写回的两项（从存储里的名字出发：离开中文时记下实际运行的中文方案而不是存储里跑不起来的那个、日语和韩语回到上次的中文方案、它跑不起来时退回全拼并记成全拼、不同时提供两头的版本不写）；`windows_ipc_contract` 覆盖新协商位不和已有位重叠、新旧 TIP 与 Server 互配、修饰键组合；`msime-tsf-japanese-toggle` 钉住组合键、TSF 分类条件、四处特例、枚举序号、偏好键和 Server 的宣告与工作线程。这三项本机 clang 运行通过。`session_smoke` 和 `session_pump` 的新用例覆盖开关、英文状态、Engine 英文模式、回调拒绝时都不改组字、生产路由只在开关打开时调用回调，`preference_monitor` 覆盖开关的热重载；它们和 `KeyEventSink.cpp`、`Ipc.cpp`、`server_main.cpp` 等一起用 MinGW 做过语法检查（`server_main.cpp` 补上 `msimeui/include` 和 curl 头文件目录后 `-Wall -Wextra` 无告警），没有完整交叉链接，也没有在 Wine 下运行。WinUI 3 的原生设置窗口只能交给 Windows CI 编译。
- Linux：`linux-input-schemes` 覆盖目标方案的计算；`fcitx5_contract.py` 静态钉住两个宿主的接线；`fcitx5-native-test --japanese-toggle` 和 `ibus-engine-smoke --japanese-toggle`（ctest 的 `fcitx5-japanese-toggle`、`ibus-japanese-toggle`，需要已校验的词库）从真实按键路径驱动：缺省关时交给应用、热重载打开和关闭、双拼去日语并记下双拼、再按回到双拼、按住只切一次、英文模式和 Engine 英文模式交给应用，Fcitx5 还覆盖组字中先上屏；IBus 还覆盖快速连按两下时两下都不交给应用（第二下是否赶上保存未落盘取决于保存线程的快慢，测试按存储实际的样子补按回双拼；临时把这一下改回交给应用，7 次运行里 6 次停在这条断言上）。在 `build-container.sh` 的镜像里编译了两个宿主和这两个测试程序，对着按 `resources/desktop-dictionary.lock.json` 下载并校验过的词库跑了这两个模式（IBus 改成保存未落盘时也吃掉之后又连跑四次）以及两个宿主原有的 `--caps-lock`、`--ctrl-space` 和 Fcitx5 的默认模式，全部通过；IBus 的默认模式停在「English-mode recording did not show streaming preedit」，`origin/develop` 的同一个程序同样停在这里，与本改动无关。
- 没有在任何平台上实机按键验证。
