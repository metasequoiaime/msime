# Agent Note: 鸿蒙手机和平板在「按键反馈」里选按键音效包

Status: implemented

## Problem

[移动端按键音](../bug-fix/2026-10-25-mobile-key-sound.md)让鸿蒙手机和平板的触屏键盘按共享偏好 `plugins.key_sound.pack` 播放音效包，但手机上没有任何地方能改这个值。共享设置页只在 2in1 上给出音效包：插件页和「声音与效果」靠 `key_sound`、`music`、`plugin_triggers` 三项能力打开，鸿蒙设置桥的 `SettingsFormFactorCapabilities` 只在 2in1 上声明它们，而且插件页在任何触屏设置里都不出现（`hasPlugins` 要求 `!mobilePlatform`）。结果手机用户永远听的是内置的「清脆键盘」，装机自带的 8 位游戏机、气泡、打字机、木鱼都用不上。

## Decision

屏幕键盘页「按键反馈」组在「按键音」开关正下方加一行「音效」，显示当前包名；「按键音」关着时整行禁用。点开是鸿蒙手机的选择面板（`SelectRow` 在 `settingsPlatform === "harmony"` 时自动换成 `PickerRow`），只列目录里能当按键音的包：`kind` 为 `sound` 且 `soundPackUse` 判为按键（`mode` 为 `keys` 或缺省，与宿主的读法一致），旋律包、音乐包和其他种类都不列。选中的 id 经设置表单的草稿写入 `plugins.key_sound.pack`，和其他共享偏好一样自动保存；`plugins.key_sound` 的开关、模式和音量原样保留，2in1 那边的设置不受影响。偏好里的包不在目录中（被删掉了）时，选择框多出一项「<id>（未找到）」照实显示，不显示成第一项。

页面靠一项新能力 `HostCapabilities.touch_key_sound_packs` 决定画不画这一行，鸿蒙设置桥按设备形态报告：`SettingsFormFactorCapabilities.touchKeySoundPacks` 在手机和平板上为真、2in1 上为假，`Settings.ets` 的 `hostCapabilities` 把它写进能力文档。它只在 TypeScript 的能力类型里，Rust 的 `client-core::host_surface::HostCapabilities` 不带这一项，与同样由鸿蒙设置桥自己填的 `harmony_shared_sandbox` 一样；其他宿主不报，页面不变。目录由新的 `useTouchKeySoundPacks` 读取，每次进入屏幕键盘页读一次；没有能力、没有插件客户端、还没读到或读取失败时不画这一行。

选中后试听一声。页面经 `MobileKeyboardFeedbackClient.previewKeySound(pack, volume)` 调设置桥 `keyboardFeedback` 的 `preview_sound` 操作，设置桥交给新的 `KeySoundPreview`：用 `msime_client_key_sound_pack` 取校验过的文件，`KeySoundPolicy.previewSample` 选出普通键样本（旋律包、没有普通键样本或不是 WAV 时为空，保持安静），经原生 `keySoundRenderNotes` 在包的时长上限内解码写到 `cacheDir/key-sound-preview`，再用一个只有一路的 `SoundPool`（`STREAM_USAGE_MUSIC`，与键盘一样跟随媒体音量）播一次，播完放掉声音池和写出的文件。新试听会先放掉上一次的。音量取 `plugins.key_sound.volume`。试听失败只记日志，页面不报错：选择已经写进去了。`KeySoundPlayer.openVerified` 保持私有，设置页经公开的 `openPreview` 包装入口复用同一套文件校验，避免试听绕过安全边界。

键盘一侧不用改：编辑框获得焦点时 `KeyboardSession.reloadChangedPreferences` 比较偏好修订号，变了就 `restartIdleSession` 重建会话，`start` 读入新的 `preferences.plugins` 后调 `configureSounds`，`KeySoundPlayer.configure` 发现包变了就重新加载。设置页的改动先由共享沙箱镜像发布、键盘在焦点进入时 `syncSharedState` 导入，和其他共享偏好走同一条路。

## Alternatives considered

- 在手机上也报 `key_sound`，复用这一项决定画不画「音效」：不用新加能力字段，安卓也已经报它。但 `key_sound` 的含义是「宿主播放按键音、旋律、上屏音和成就音」，手机只播按键样本；设置页其他地方（插件页、打字统计的成就开关）都按它打开，哪天有人放开触屏的插件页，手机就会冒出一串不起作用的开关。另起一项只说触屏这一件事，语义不越界。
- 把能力加进 Rust 的 `HostCapabilities`，按平台给 `Harmony` 为真、再由鸿蒙设置桥按形态收窄，像 `touch_toolbar_tools` 那样：能力集中在一处。但目前只有鸿蒙手机在共享设置页里用它，安卓和 iOS 的设置是原生界面，不读这份文档；放进 Rust 要多改一个 crate 和它的测试，换来的只是一个所有平台都填假的字段。等有第二个读共享设置页的触屏宿主需要它时再下沉。
- 试听复用键盘的 `KeySoundPlayer`：配置成只开按键音、等加载完再按一下。它没有「加载完成」的通知，要加一个只给试听用的回调；而且它为一个包准备四类样本，试听只要一声。单独一个几十行的 `KeySoundPreview` 更直接，安全上的约束（只播原生渲染出的 WAV、描述符核对 inode）照用同一套。
- 不做试听：最省事。但手机上没有别处能听到一个包的声音，用户只能切到别的应用里敲几下再回来，选音效这件事就变成盲选。代价只是设置进程里偶尔建一个一路的声音池。

## Consequences

鸿蒙手机和平板用户能在设置里换按键音效包，并当场听到；默认值没变，仍是内置 `default`（「清脆键盘」），按键音出厂仍是关。旋律、上屏音、成就音、连击音和背景音乐在手机上依旧不出现，2in1 的插件页和屏幕键盘页都不变。

手机上没有导入音效包的入口（插件页只在 2in1），所以列表通常就是装机自带的五个包；目录里有别的按键包时也会照样列出。换包后要等下一次编辑框获得焦点键盘才换声音，与其他共享偏好一致。试听用的是设置进程自己的声音池，不经过键盘，所以不受「按键音」开关以外的键盘状态（密码框、英文模式静音）影响。

## Verification

- `apps/desktop/tests/settings/harmony-key-sound-pack.test.tsx`：鸿蒙手机上「音效」紧跟「按键音」、缺省显示「清脆键盘」、面板只列五个按键包（不列旋律包和音乐包）、选中后试听一次并保存 `plugins.key_sound.pack` 且不动开关、模式和音量；手机页不出现旋律、上屏音、成就音和背景音乐；「按键音」关时整行禁用；目录里没有的包显示「（未找到）」；宿主不报能力时不画也不读目录；2in1 的屏幕键盘页没有这一行。
- `platforms/harmony/tests/run.sh`：新增 `previewSample`（按键包取普通键样本，旋律包、Ogg、缺普通键样本时为空）和 `touchKeySoundPacks`（手机、平板为真，2in1 为假）的检查。
- `scripts/test-harmony-audio-file-binding.py`：确认音乐和渲染音效都先校验目录项，再把同一文件描述符交给播放器；设置页的公开试听入口不暴露内部校验器。
- 编译机上 `build-native.sh arm64-v8a`、暂存和 `hvigorw assembleHap` 通过，新文件没有 ArkTS 告警。
- 没有覆盖：真机上的选择面板、试听出声、换包后键盘换声音都没有在设备上验证。
