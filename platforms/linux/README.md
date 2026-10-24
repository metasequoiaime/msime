# Linux 原生宿主（IBus 与 Fcitx5）

## 目录结构

Linux 平台代码按职责分层：`src/` 下按 `core/`、`candidates/`、`clipboard/`、`providers/`、`voice/`、`overlay/`、`system/` 和 `entrypoints/` 分层放置 C++ 实现及头文件，`tests/` 放置本地与容器测试，`scripts/` 放置运行时 Python/启动脚本，`data/` 放置 systemd、桌面入口和协议模板，`fcitx5/` 保留 Fcitx5 适配器，`cmake/` 保留安装辅助模块。平台根目录只保留构建入口和说明文档。

IBus 与 Fcitx5 是两条并列的系统入口，链接同一个 `msime-host-api` ABI，功能面一致。编译门禁、容器隔离验收、CMake 安装与 CPack 打包各有对应入口，分别写在下面的「构建与运行」「隔离验证」「生成 Linux 安装包」三节。

## Fcitx5

Linux 另有一个原生 Fcitx5 插件。它与 IBus 宿主并列，不是挂在后者下面的东西：在 Fcitx5 上，输入法**本身就是**被 `fcitx5` 进程加载的 addon，正如 IBus 的 engine 本身就是一个说 D-Bus 的独立进程，两者链接的是同一个 `msime-host-api` ABI。插件按每个 Fcitx 输入上下文保持一个 Host API 会话，不运行 IBus daemon，也不使用 Fcitx 的 IBus 兼容前端。普通的 Linux 构建只要机器上装了 Fcitx5 开发包就会配置它；传 `-DMSIME_ENABLE_FCITX5=OFF` 可以退出，没装开发包时配置阶段会把取得它的办法打出来。在进程内这一点有个后果值得预先考虑：Engine 和随包词库活在 `fcitx5` 进程里，Engine 一崩，所有应用同时失去输入法，而 IBus 宿主崩掉的只是它自己的进程，daemon 还在。把构建出的插件和 `msime.conf`/`inputmethod/msime.conf` 安装到 Fcitx5 前缀后，用 `fcitx5-configtool` 选择「MSIME」。用户配置不在通常的 XDG 位置时，用 `MSIME_FCITX5_OPTIONS` 指定绝对的 runtime-options JSON 路径。密码、数字和敏感上下文一律不处理，预编辑、Engine 拥有的候选页、翻页和候选身份则通过 Fcitx5 原生输入面板转发。

Fcitx5 的状态栏现在按共享 `floating_toolbar` 的八个组件开关提供一个「工具栏」子菜单。Windows 画的是一个悬浮窗口，IBus 把同一组开关映射到属性子菜单，Fcitx5 这里的对应面就是状态栏；在此之前这些开关在这个宿主上一个都不起作用，而设置页按能力位把它们全都显示着。子菜单里放的是已有的那些 action 本身而不是副本——一个行为和旁边状态栏项目略有不同的入口，等于同一个开关有了两份实现。中文输入项跟随 `english_mode`（它在 macOS 管的就是中/英按钮），其余各自跟随自己的开关，默认值也与那两个宿主相同，所以屏幕键盘是唯一默认隐藏的一项。偏好热重载时立即重建，不等下一次焦点变化。能连上 X 服务器时两个宿主另外画一条与 macOS 相同的悬浮工具栏（见下文「输入细节与平台差异」），这个子菜单照旧留着，画不出悬浮窗口的会话靠它。

Fcitx5 现在也消费共享 `keybindings` 的五个模式快捷键与 `default_ime_mode`。此前这个宿主只在状态栏上提供中英文开关：设置页按 `mode_switch_shortcuts` 能力位把四个开关全都显示出来，而它们在这里一个都不生效，选了「英文」启动的用户照样得到中文。裸 Shift 与裸 Ctrl 按 Windows 的手势判定——按下只是布防，松开才切换，且期间不能打其他键、不能带别的修饰键、按住不超过 500ms；Ctrl+Space（默认开启）与（开启时）Ctrl+Alt+Space 切换中英文，Ctrl+Shift+F 切换简繁；这些组合键和 Ctrl+Shift+Space 一样每按一次只切换一次，按住的自动重复与松开都被吞掉，Ctrl+Shift+F 切换时保留正在输入的拼音，只改写候选的简繁。中英文切换位于英文透传门禁之前，因此用快捷键切到英文后仍能用同一个快捷键切回中文；屏幕键盘与语音的宿主快捷键也不随拼音输入一起被停用。`default_ime_mode` 按输入上下文只套用一次，重新聚焦或重建 Engine 会话保留用户已经选择的状态，与 IBus 一致。契约由 `tests/core/fcitx5_contract.py` 静态钉住。

`msime-linux-settings` 的「快捷键 → 输入模式切换」下拉框提供 Shift、单击 Ctrl、Ctrl+Space、Ctrl+Alt+Space 和「不使用」。Ctrl+Space 对应 `keybindings.switch_language_ctrl_space`，旧配置缺少字段时仍默认开启；多项同时启用时按选项顺序显示第一项，打开页面不改写配置。用户选择时一次更新四个布尔值，仅保留选中的切换键；「不使用」全部关闭。关闭后两个宿主都不拦截新的 Ctrl+Space 按下、重复和松开，也不改变正在输入的拼音；保存后热重载。下拉框不改 IBus/Fcitx5 的全局快捷键，框架若占用同名组合键，需在框架设置中另行关闭。专项验收复用现有原生测试入口：`ibus-engine-smoke <已验证词库目录> --ctrl-space` 与 `fcitx5-native-test <已验证词库目录> --ctrl-space`。

同一组里的「单按 Caps Lock 切换中英文」（`keybindings.switch_language_caps_lock`，缺省关）与下拉框互不排斥。两个宿主按 `Caps_Lock` 符号认键（被 xkb 选项改成 Control、Escape 的 Caps Lock 不算，被 `shift:both_capslock` 一类选项改成 `Caps_Lock` 的 Shift 也不算）：按下前大写锁定是关的、没有和修饰键一起按、按着期间没有按别的键、300 毫秒内松开，就先用 `XkbLockModifiers` 撤回大写锁定，撤回成功再像裸 Shift 一样切换（组字中先上屏原文），松开事件照常交给应用。撤回只在 X11 会话里做得到（`src/system/CapsLockRelease.cpp`，构建时找到 libX11 才编进来）；`WAYLAND_DISPLAY` 有值或 `XDG_SESSION_TYPE=wayland` 时锁定状态归合成器，宿主不布防，轻点照常是大写锁定。

两个宿主还把大写锁定状态报告给会话（`msime_client_set_caps_lock`，IBus 的 `sync_caps_lock`、Fcitx5 的 `syncSessionCapsLock`），所以「表达」页的「大写锁定时使用英文标点」（`caps_lock_ascii_punctuation`）在 Linux 上也有效：打开后，大写锁定期间没有组字时按下的标点交给应用，即英文标点；组字中照旧由 Engine 决定，标点锁定为「始终中文标点」时仍是中文标点。锁定状态取自每个按键事件带的修饰键（与「⇪」指示同一来源），每个键分发前比较会话上次收到的值，变了才报告；新会话从关开始，IBus 在建会话时、Fcitx5 在打开会话的那个键分发之前报告。X11 与 Wayland 都一样，因为状态来自按键事件而不是撤回锁定的 XKB 调用。专项验收在 `ibus-engine-smoke <已验证词库目录> --caps-lock` 与 `fcitx5-native-test <已验证词库目录> --caps-lock` 的末尾。

同一组里还有「Ctrl+Shift+J 在中文和日语之间切换」（`keybindings.toggle_japanese_ctrl_shift_j`，缺省关，只在同时提供日语和中文方案的 full 版里出现）。中文状态下按它，当前跑的是中文方案就切到日语，并把这个中文方案写进 `last_chinese_scheme`；跑的是日语、韩语、越南语或藏语就回到 `last_chinese_scheme`，它跑不起来时按 `effective_input_scheme` 的规则退回（`src/core/InputSchemes.h` 的 `japanese_toggle_target`）。英文模式和 Engine 自己的英文模式（Ctrl+Shift+E）里组合键交给应用；浏览器里它是开发者工具的快捷键，所以缺省关。Fcitx5 走状态区换方案的同一条路，组字中先上屏再切换；IBus 和属性菜单一样经 `save_menu_preference` 写偏好库，正在组的字由宿主库等这次组字结束后才换方案。状态区和属性菜单换方案只在目标是中文方案时记 `last_chinese_scheme`，快捷键离开中文时自己补记。一次按下只切换一次，按住的自动重复和松开都归这次按下。专项验收：`ibus-engine-smoke <已验证词库目录> --japanese-toggle` 与 `fcitx5-native-test <已验证词库目录> --japanese-toggle`。

Fcitx5 状态栏切换输入方案、双拼方案和辅助码方案时，除了写共享偏好库，还给当前输入上下文留一个 override：会话按 `runtime-options.json` 建立，而这份文件只有设置页会同步。override 只在与偏好库一致时保留；设置页或别的窗口改过偏好库后，偏好热重载和新建会话都会丢掉它、改用偏好库的值，所以与 Windows 一样，设置页改方案所有窗口都跟着变。新建会话时这几项一律取偏好库的值，不取 `runtime-options.json` 里可能已过时的值，状态栏选辅助码方案时一并清掉的辅助码表插件也一样；状态栏切输入方案时与 IBus 一样清掉辅助码方案 override，全拼下选的辅助码不会带进双拼。保存失败的选择不回滚：与 IBus 的 `failed_menu_save` 一样，待重试的保存跨焦点保留，只在保存成功或偏好目录变更时清掉，「未落盘」标记记在各个 override 自己身上，偏好库真正存下这个值之前菜单不会跳回去。

Fcitx5 的 `ime_mode_scope` 也真正决定中英文状态的记忆范围：`app` 按输入上下文报告的程序名保存有界的应用级状态，`global` 在当前 Fcitx5 进程内共享一个状态；失焦时先记录（会话留给托盘菜单，到有输入上下文获得焦点时才关闭，见「状态区」的说明；托盘上的中英文切换会更新这份记录），重新聚焦并重建会话时恢复，而不是重新套用 `default_ime_mode`。匿名或缺失程序名使用共享的匿名槽位，不把状态写入偏好文件。

应用例外（共享偏好 `app_input_mode_rules`，设置页「中英文」组里的规则表）在两个宿主上按程序名执行，语义与 macOS、Windows 相同：焦点从别的程序进入有规则的程序时从规则里的中文或英文开始，优先于作用域、记忆和 `default_ime_mode`，「全局统一」下也生效且不改全局状态；在这个程序里手动切换中英文（快捷键、单按 Caps Lock、托盘或属性菜单）后，规则让位到焦点进入另一个程序为止，同一程序的其他窗口和输入框算同一次停留。程序名不分 ASCII 大小写比较（`src/core/AppInputModeRules.h`）。Fcitx5 取 `InputContext::program()`：经 Fcitx5 的 GTK、Qt 输入法模块或 D-Bus 前端接入的程序在 X11 和 Wayland 下都报告；没有程序名的前端（例如部分 Wayland text-input 客户端）不受规则影响，但它们获得焦点算换了程序，回到有规则的程序时规则重新生效，因为 Fcitx5 的上下文建起来时就带着程序名，空程序名一定是另一个客户端。IBus 只能尽力而为：1.5.27 起 `focus_in_id` 的客户端名只有 IBus 的 GTK 输入法模块带程序名（`gtk3-im:<g_get_prgname()>`，GTK 2、4 同理），Qt 插件（`QIBusInputContext`）、XIM 和更早的 IBus 都是匿名的，规则对它们不生效。GNOME Wayland（Ubuntu、Fedora 的默认桌面）下没有设置 `GTK_IM_MODULE` 的 GTK 程序走合成器的 text-input 协议，IBus 只看到 GNOME Shell 一个客户端（客户端名 `gnome-shell`），所以规则在那里只对设置了 `GTK_IM_MODULE=ibus` 的程序和 XWayland 里经 IBus GTK 模块输入的程序生效。有客户端名却没有程序名的焦点（`QIBusInputContext`、`xim`、`gnome-shell`）算换了程序；空客户端名（IBus 还在协商身份）和 IBus 自己的 `fake` 上下文认不出离没离开，停留保持不变。Linux 没有选应用的对话框，设置页手填程序名（粘贴 `/usr/bin/firefox` 这样的路径时只取文件名），并说明哪些程序会报告程序名。专项验收：`fcitx5-native-test <已验证词库目录> --app-rules` 与 `ibus-engine-smoke <已验证词库目录> --app-rules`（IBus 1.5.27 以前返回 77 跳过），两者都覆盖应用作用域、全局作用域和匿名客户端。

Fcitx5 候选操作通过原生候选 Action（新版本）和输入上下文 status action（旧版本兼容入口）提供置顶、固定到第 N 位、取消固定和删除动作；动作携带候选身份，适配 Linux 面板而不依赖 IBus 兼容前端或 Windows 原生窗口。

候选页显示时，Home 分两段：高亮不在首选时移回整份候选列表的首项，已在首选时把拼音光标移到开头（`MSIME_FIRST_CANDIDATE_OR_HOME`；打开的韩语汉字列表和注音列表里没有光标，一直是回到首项）；End 将高亮移动到末项，页码随之切换（End 会先展开按需加载的候选）；没有候选页时仍交给编辑器处理。

组字时 Shift+左右和 Ctrl+左右（只按这一个修饰键，小键盘的左右键也算）在所有方案里都按音节移动拼音光标（`MSIME_MOVE_LEFT_SEGMENT` / `MSIME_MOVE_RIGHT_SEGMENT`），全拼和双拼也一样；共享运行时从不把分段命令换成移动候选，所以高亮停在哪个候选上它们都只编辑拼音。不带修饰键的左右键在全拼和双拼里仍是整句改字的光标键。Ctrl+退格删除一段，Shift+退格没有额外含义。韩语音节、注音转换、越南语单词和藏语音节串没有分段，这些组合键照旧结束组字并交给应用；没有组字时它们是应用的按键。按住 Shift 再按方向键是组合键，松开 Shift 不会切换中英文。

### Wayland 成对标点光标转发

Fcitx5 的 Wayland 虚拟键盘会沿用当前修饰状态，因此用 Shift 输入标点时，补全后的左移和跳过闭标点的右移都等到 Shift 状态清零后才转发；Shift 仍正常参与标点输入和模式快捷键。失焦、reset 和会话关闭会取消待转发方向键。Fcitx5 5.1.22 起可回读实际修饰状态；更旧版本等下一次无 Shift 的按键，在处理该输入之前完成回移。

专项 E2E 复用隔离 daemon，需要 Node.js 22+、`wtype` 和 Chrome（默认 `google-chrome-stable`，可用 `MSIME_TEST_CHROME` 指定）。先启动支持输入法和虚拟键盘协议的独立 Wayland 合成器，下面的 socket **不能指向日常桌面**：

```sh
MSIME_ISOLATED_LINUX_TEST=1 WAYLAND_DISPLAY=/absolute/test-compositor/wayland-0 \
  dbus-run-session -- python3 platforms/linux/fcitx5/tests/daemon.py \
  /absolute/verified/resources "$PWD/target/debug/libmsime_host_api.so" \
  "$PWD/target/linux-ibus/fcitx5/libmsime-fcitx5.so" --wayland-punctuation
```

测试检查五种标点补全、继续输入保留闭符号、持续按住 Shift 连按开闭括号、快速输入和失焦取消，以及已有闭标点跳过路径的无 Shift 右移；事件记录和截图写到 `target/paired-punctuation-probe/`，不读取个人配置或页面。

### GNOME Wayland 下候选窗闪烁

在 GNOME 的 Wayland 会话里用 Fcitx5 时，GTK 程序**不要**设置 `GTK_IM_MODULE=fcitx`，否则候选窗会不停闪烁，也不显示水杉的候选窗皮肤。

原因：GNOME 的合成器不让输入法自己弹出候选窗。设了 `GTK_IM_MODULE=fcitx` 的 GTK 程序加载 fcitx5-gtk 输入法模块，Fcitx5 只能请它在程序里自己画候选窗（客户端输入面板，输入上下文带 `ClientSideInputPanel` 能力）。GTK3 没有实现移动已显示弹窗的那部分 xdg_popup 协议，fcitx5-gtk 每次挪动候选窗都要先隐藏再显示，这就是闪烁。这是上游的已知限制，见 Fcitx FAQ「Candidate window is blinking under wayland with Fcitx 5」和 fcitx/fcitx5#377。不设 `GTK_IM_MODULE` 时，GTK 程序走 Wayland 的 text-input，经 GNOME Shell 以 IBus 协议交给 Fcitx5（其 IBus 前端），候选窗由 Fcitx5 经典界面按水杉生成的主题绘制，不闪，皮肤也正常。X11 会话和 KDE Plasma 等支持输入法弹窗协议的桌面不受影响，照常设置即可。

Debian/Ubuntu 用 im-config 时，`im-config -n fcitx5` 会无条件设置 `GTK_IM_MODULE=fcitx`。在 `~/.xinputrc` 的 `run_im fcitx5` 之后加上下面几行，只对 GNOME 的 Wayland 会话去掉它：

```sh
case "$XDG_SESSION_TYPE:$XDG_CURRENT_DESKTOP" in
  wayland:*GNOME*)
    unset GTK_IM_MODULE
    systemctl --user unset-environment GTK_IM_MODULE >/dev/null 2>&1 || true
    ;;
esac
```

其他发行版的原则相同：只要会话是 GNOME Wayland，就不要为 GTK 程序设置 `GTK_IM_MODULE`；`XMODIFIERS=@im=fcitx` 照常保留。另外有两处要一并检查：

- `~/.config/environment.d/` 里如果写了 `GTK_IM_MODULE=fcitx`，要删掉这一行，再执行 `systemctl --user daemon-reload`。environment.d 的值在用户级 systemd 启动或 daemon-reload 时进入它的基础环境，`systemctl --user unset-environment` 删不掉。
- 开了 Linger（`loginctl show-user $USER -p Linger` 为 `yes`）时，用户级 systemd 跨登录常驻，会留着上一次会话的 `GTK_IM_MODULE`。D-Bus 激活的程序（例如 gnome-terminal）用的是它，所以上面的脚本里也执行了 `unset-environment`。

改完重新登录。程序是否已经绕开客户端面板，可以在 `busctl --user call org.fcitx.Fcitx5 /controller org.fcitx.Fcitx.Controller1 DebugInfo` 的输出里查：输入上下文只剩 `frontend:ibus`，不再有 `frontend:dbus` 且 cap 第 39 位置位的 GTK 程序。`msime-linux-setup` 和 `--register` 注册进 Fcitx5 时，如果检测到 GNOME Wayland 下设了 `GTK_IM_MODULE=fcitx`，会打印一句提示指向这一节，但不会替用户改文件。撤回时删掉 `~/.xinputrc` 里加的这几行，重新登录即可。

Qt 程序没有类似的做法：Qt5 不支持 text-input，只能靠 `QT_IM_MODULE=fcitx`，因此在 GNOME Wayland 下由客户端面板绘制，候选窗仍可能闪烁。

### deepin 玲珑应用里只能输入字母

在 deepin、统信 UOS 等带玲珑（Linglong，linyaps）的系统上，有用户遇到玲珑应用（Chrome、Firefox、文本编辑器）里切到中文也只能打出字母、非玲珑应用正常的问题（#6410、#8009）。需要分别确认正在运行的输入法框架、应用加载的模块和框架收到的上下文，不能只凭环境变量判断原因。

`GTK_IM_MODULE=ibus`、`QT_IM_MODULE=ibus` 不代表正在运行的是 `ibus-daemon`：Fcitx5 也提供 IBus 兼容前端。#8009 的系统只运行 Fcitx5，环境变量仍为 `ibus ibus @im=ibus`，`DebugInfo` 已出现 `frontend:ibus cap:52 focus:1`。这里的 `52` 是十六进制的普通预编辑与周边文本能力，并不表示密码字段。程序名为空也是该前端可能提供的正常结果，但这也意味着单凭这份输出还不能确认哪一个上下文属于失败的玲珑应用。若能确认该应用已经建立并聚焦了上下文，就不能再归因为它完全没有输入法模块；切换框架也不保证解决。

若确认使用的是 IBus daemon，并且应用运行时缺少 IBus 模块，可以考虑改用 Fcitx5；这只是针对模块缺失的排查路径：

1. 确认装了 5.0.20 及以上的 Fcitx5（`fcitx5 --version` 查看版本；没有时 `sudo apt install fcitx5`）。发布页的 `.deb`、`.rpm` 同时带 IBus 引擎和 Fcitx5 插件，不用重装水杉。只含 IBus 的 legacy 包没有 Fcitx5 插件，装的是它就不要照下面的步骤切换。
2. 用 `im-config -n fcitx5` 切换框架，注销并重新登录。应用使用 Fcitx 模块时环境变量通常是 `fcitx fcitx @im=fcitx`；发行版也可能让应用经 IBus 兼容前端连接 Fcitx5，以实际进程和上下文为准。
3. 运行 `msime-linux-setup --register`，或在 deepin 输入法设置、`fcitx5-configtool` 中把水杉加入当前输入法组并选中。
4. 不再用 IBus 时，可将它从开机自启动中去掉，避免同时启动两个框架。

`msime-linux-setup` 在 deepin/UOS 或带玲珑的系统上注册进 IBus、且本安装带 Fcitx5 插件时，会打印这条有条件的排查建议；legacy 包不提示，也不会自动切换框架。

使用 Fcitx5 后仍然失败，请附上 `fcitx5 --version`、相关 daemon 的进程信息、宿主与玲珑应用内 `echo $GTK_IM_MODULE $QT_IM_MODULE $XMODIFIERS` 的输出、预装拼音在同一应用里是否可用，以及焦点在该应用时 `busctl --user call org.fcitx.Fcitx5 /controller org.fcitx.Fcitx.Controller1 DebugInfo` 的结果。请延迟采样并在等待时切回失败应用，避免将执行命令的终端误当成目标上下文。仅保留复现用的合成编辑器上下文，不附真实输入、私人应用名或凭据。当前隔离验收已覆盖 Fcitx5 IBus 兼容前端和 GTK IBus 模块的中文上屏，但尚未证明 Deepin 玲珑场景的问题已解决，#8009 保持打开。

### 第三方输入源菜单集成

[Omarchy Input Menu](https://github.com/Ray0907/omarchy-input-menu) 一类精简输入源菜单需要区分 Fcitx5 的输入法列表和当前输入法的设置动作。水杉的输入方案、主题、云联想等菜单项都是设置；其中可勾选的动作通过 `fcitx::Action::setCheckable(true)` 保留当前状态。

Fcitx5 的 DBusMenu 导出会把所有可勾选动作写为 `toggle-type="radio"`（见 [Fcitx5 5.1.23 的导出实现](https://github.com/fcitx/fcitx5/blob/5.1.23/src/modules/notificationitem/dbusmenu.cpp#L338-L351)）。因此，`radio` 表示菜单的勾选行为，无法单独证明一个条目是输入源或中英文模式。子菜单设置项也可能没有图标；旧版本的多项设置可能共用键盘图标。

第三方菜单集成时应遵守以下边界：

- 输入源列表按 Fcitx5 当前组里的输入法生成，水杉作为一个输入法列出。
- 只有已明确支持某个引擎的模式协议时，才把它的子菜单动作展开为模式；不要把任意子菜单中的 `radio` 条目全部展开，也不要把空图标或共享图标当作模式标识。
- 缓存的模式列表在读取时也要按同样规则过滤。只修正新数据而继续读取旧缓存，会让错误条目在重启后再次出现。

如果菜单出现几十个重复的 `ABC` 或把云联想、主题等设置列成输入源，应先检查第三方菜单的筛选逻辑和缓存。水杉保留这些设置的勾选状态，Fcitx5 原生菜单仍用它们显示当前配置。

## 辅助码触发键

设置应用的「输入 → 辅助码」可分别选择全拼、双拼的触发键：Shift 加字母（默认），或分号、斜杠、反斜杠、反引号、等号。选择标点后，在拼音组字末尾按该键，再输入一到两个辅助码字母即可筛选；Shift 加字母仍然可用。未输入码时按退格只退出辅助码输入，保留已有拼音。没有组字、编辑中间的拼音、英文及快捷模式仍保留这些标点原本的作用。微软双拼未完成的半音节优先接收分号作为韵母。

IBus 与 Fcitx5 的专项验收分别是 `ibus-engine-smoke <已验证词库目录> --helpcode-trigger` 和 `fcitx5-native-test <已验证词库目录> --helpcode-trigger`，覆盖两族独立设置、双辅助码筛选、上屏、退格及微软分号韵母。

## 安装后首次使用

安装包的 Debian `postinst` 会为当前已登录且可联系到的用户自动注册本机匿名水杉账号；网络暂时不可用时不影响安装，在线 provider 会在之后重试。安装本身仍不准备词库和运行配置，也不会替你选中输入法。其余首次配置由随装的 `msime-linux-setup` 补齐：

```sh
msime-linux-setup              # 用已备齐的词库准备状态目录
msime-linux-setup --download   # 允许按随装的词库锁取回缺失词库（首次约 170 MB）
msime-linux-setup --list-downloads   # 只列出本版本词库的下载地址、大小、SHA-256 和保存目录
msime-linux-setup --update --download   # 升级之后只取回过期的那几项词库并切换过去
```

自动下载缓慢或超时时，可以在首次配置页展开「手动下载词库」，或运行 `msime-linux-setup --list-downloads`。清单来自当前安装包的词库锁，列出全部文件的原地址、当前国内镜像、大小、SHA-256 和默认保存目录；读取清单不联网、不创建目录，也不启动配置。将链接交给浏览器或其他下载工具，按清单中的文件名保存全部文件，再取消页面上的自动下载并点击「开始配置」，或在终端运行不带 `--download` 的 `msime-linux-setup`。如果保存在其他目录，用 `msime-linux-setup --resources /绝对路径` 指向它。已有状态目录仍可查看清单；`--list-downloads --json` 输出相同信息的 JSON，不能与下载、配置或注册选项混用。

首次配置清单的原生窗口回归是 `tests/tools/first-run-downloads-ui.py <CMake安装前缀> <证据目录>`，依赖 `tauri-driver`、`WebKitWebDriver`、`Xvfb` 和 `xclip`。在独立 Xvfb 与 D-Bus 会话中，为 `XDG_CONFIG_HOME`、`XDG_DATA_HOME`、`XDG_CACHE_HOME` 和 `XDG_RUNTIME_DIR` 指向专用测试目录，并把 `MSIME_CLIENT_HOST_OPTIONS` 设为尚不存在的测试状态目录内的 `runtime-options.json`，从安装好的 `msime-linux-settings` 打开窗口。该回归检查真实系统剪贴板，并拒绝已经配置的目录；不下载、配置或注册输入法。


它按 `desktop-dictionary.lock.json` 逐个核对词库的名称、大小和 SHA-256，再调用 `msime-linux-prepare` 在 `$XDG_CONFIG_HOME/msime-client`（默认 `~/.config/msime-client`）建立状态并发布 `runtime-options.json`，最后把输入法加入正在运行的 Fcitx5 或 IBus 的输入法列表（见下文）。词库目录按「随包安装的 `${CMAKE_INSTALL_DATADIR}/msime-client/resources` → `$XDG_DATA_HOME/msime-client/resources`」的顺序查找，`--resources` 显式指定优先；状态目录必须尚不存在，已存在时报错而不是覆盖，只有下文的 `--update` 例外。所有需要取回的词库先在同一文件系统的临时目录里下载并逐个校验，全部通过后才改名放进词库目录；校验不过或下载中断就中止，词库目录一个文件都不动，不会留下半份或新旧混杂的词库。锁之外的文件同样报出来，因为宿主会因此拒绝整份词库，而下载补不掉它们，需要用户自己移走。例外是 Engine 的 `helpcodes` 子目录（`crates/client-core/src/resources.rs` 的 `ResourceStore::verify`）。

`--download` 之外不发起任何网络请求。词库锁里每一项都带 HTTPS 下载地址；需要取回的某一项没有地址时，在发出第一个请求之前就中止并提示用 `--resources` 指向已备齐的目录，不会下出半份词库。词库锁无条件随装：此前它只在配置阶段传了 `MSIME_ENGINE_RESOURCES` 时才安装，也就是最需要它的那种安装里反而没有，只拿到安装包的用户没有任何办法把词库凑齐。

不想开终端也可以直接打开应用列表里的「水杉输入法」：还没有 `runtime-options.json` 时，`msime-linux-settings` 不再报错退出，而是打开首次配置页。页面运行的就是同一个随装的 `msime-linux-setup`（优先取桌面二进制旁边的那份，其次 `PATH`），状态目录固定为设置窗口读取的那个 `runtime-options.json` 所在目录，下载只在勾选「词库不完整时从固定地址下载」时才加 `--download`。页面的结构与 macOS 的安装窗口相同：设置窗口先以 500×640 的居中小窗口打开（进入设置时换回设置窗口的大小），有可拖动的标题栏和关闭按钮，选项在主按钮上方，进行中是带百分比的进度条，由脚本在设置应用运行时（环境变量 `MSIME_LINUX_SETUP_REPORT=1`）额外打印的 `msime-setup-stage: "<阶段>"` 行推进；脚本的原始输出收在「显示详细输出」里，失败时自动展开。结束时脚本再打印一行 `msime-setup-result: {"host": …, "registered": …}`，页面据此显示「加入输入法列表」结果卡片：已自动加入 Fcitx5 输入法组或 IBus 输入源时说明切换快捷键并配图，没能加入时给出分步说明，Fcitx5 附「打开 Fcitx5 配置」（启动 `fcitx5-configtool`），IBus 附「复制 ibus restart」。这两种机读行在终端里运行时不打印，页面也不把它们显示成输出。配置完成后直接进入设置，窗口每次读取都会重读这份文件，所以不需要重启。目录已存在但缺少 `runtime-options.json` 时，只有其中仅有安装流程创建的匿名账号文件这一种情况仍可继续；含其他文件时页面只说明原因、不提供按钮，与脚本拒绝覆盖的规则一致；已安装系统级配置（`MSIME_SETTINGS_SYSTEM_CONFIG`）时仍以它为准，不出现首次配置页。

升级安装包之后不需要重新准备状态。Windows 安装程序在升级时回放用户词库，Linux 没有这样一个按用户执行的安装步骤，所以改由输入法宿主在启动时完成：IBus 宿主和 Fcitx5 插件在建立任何会话之前调用 `msime_client_refresh_host`，它用库里编译进去的词库锁算出当前代次，与 `runtime-options.json` 中 `dictionaries` 指向的代次目录比较；不一致时校验新词库、由 Engine 复制出新代次的工作词库并把用户词库日志回放进去，再原子替换这份文件，只改 `resources` 和 `dictionaries` 两项，provider socket、模型路径等其他键原样保留。同一次刷新还让 `language_dictionaries` 跟上资源目录旁的 `language-dictionaries` 目录（粤语与注音词库，安装包带上时装在 `share/msime-client/language-dictionaries`，见下文构建一节）。两项都不需要改时只读一次文件。旧代次目录不会被改动，还在用它的另一个宿主重启前照常工作；准备失败时文件保持原样，输入法继续用旧词库，下次启动再试。符号链接、系统级 `/etc/msime-client/runtime-options.json` 以及路径不符合 `msime-linux-prepare` 布局的文件都不会被改写。

升级后两个宿主都要换到新程序才会用上新版本。Windows 安装程序先停掉输入法进程、装完再启动新的；dpkg 则把新文件改名覆盖旧文件，正在运行的 IBus 宿主和 Fcitx5 进程继续执行已被替换的旧程序。`.deb` 升级时 postinst 只重启正在运行的用户服务（见下文「生成 Linux 安装包」），两个宿主各自换到新程序。IBus 宿主在输入焦点变化时检查 `/proc/self/exe`，发现自身程序已被新文件替换、且当前没有组字、候选或语音录音时，在焦点事件处理完后以专用状态 78 退出，launcher 立即带 `--recovered` 启动新版本，不等待，也不计入崩溃退避（见下文「IBus 宿主崩溃后自动恢复」），所以通常切换一次窗口就换到了新版本，组字中的内容不会丢，有焦点的编辑器也不必重新选择输入法；正在组字时推迟到之后的焦点变化。换到新版本后 launcher 经 `msime-linux-first-run-guide --reason upgraded` 发一条「水杉输入法已更新」桌面通知，对应 macOS 设置应用的「水杉输入法已更新到 X」。程序被删除而不是替换（`apt remove`）时宿主不退出，因为没有可以启动的新程序，退出只会让输入法提前消失；它继续运行到 `ibus restart` 或注销，期间若崩溃，launcher 发现程序已不在就不再重启。Fcitx5 插件与 Fcitx5 同进程，自己重启会带走其他输入法，设置页的「重启输入法服务」对 Fcitx5 只在进程内重置水杉插件的会话和配置（见下文 Fcitx5 的 `Ctrl+Shift+Alt+R` 一段），不加载新程序，所以插件在激活时检查 `/proc/self/maps` 里自身代码所在的共享库：已被替换时在面板上提示「水杉输入法已升级：执行 fcitx5 -r 或注销后重新登录即可使用新版本」，已被删除时提示「水杉输入法已卸载：执行 fcitx5 -r 或注销后重新登录即可完成卸载」（Fcitx5 作为 systemd 用户服务运行时，例如 Omarchy 的 `omarchy-fcitx5.service`，提示里的 `fcitx5 -r` 换成 `systemctl --user restart <该服务>`，见下文「Omarchy」；KDE Plasma 的 Wayland 会话里 KWin 把 Fcitx5 当作虚拟键盘自己启动、并把输入法协议的连接交给它，别处起来的 `fcitx5 -r` 拿不到那条连接，Wayland 原生应用会打不了字，而 Fcitx5 继承了 KWin 的 cgroup，按服务重启又会重启 KWin，所以父进程是 `kwin_wayland` 时提示只说注销后重新登录），状态每变化一次只提示一次（卸载后没有重启 Fcitx5 就重新安装时，改为提示已升级），优先于配置相关的提示，下一次组字时被输入面板的内容替换。升级时插件另经同一个引导脚本发一条「水杉输入法已更新」桌面通知（标题与 IBus 的相同），写着同一条重启命令，并带「立即重启 Fcitx5」按钮（`notify-send --action`，libnotify 0.7.10 起支持；更早的版本退回不带按钮的通知），点了才重启：在 systemd 用户服务里运行时重启那个服务，否则执行 `fcitx5 -r -d`。KWin 启动的 Fcitx5 没有可用的重启命令，通知不带按钮，只请用户注销后重新登录，或在系统设置的「虚拟键盘」里重新选择 Fcitx 5。从源码树重新编译后仍在运行的旧宿主也会这样处理，换到的就是新编译的程序。设置应用也一样：关闭设置窗口只是把它隐藏、进程再留十分钟，升级后再打开设置时，单实例会把启动参数转给这个仍在执行旧程序的进程。它因此在收到转来的启动时检查 `/proc/self/exe`：程序已被替换（而不是删除）、且没有正在下载或删除的模型和资源包时，先隐藏所有窗口让页面把待保存的编辑写完，一秒后放开单实例的 D-Bus 名、以转来的参数启动新程序并退出，用户这次打开看到的就是新版本；新程序起不来时留在旧程序上照常打开界面。检测由 `linux_program_handover` 的单元测试（含真实内核的改名覆盖）钉住。检测由 `tests/core/replaced_program.cpp`（真实内核：改名覆盖和删除正在运行的程序与已映射的文件）钉住，launcher 的处理由 `tests/core/ibus_launcher_supervisor.py` 钉住。

随包提供词库的安装由包管理器在升级时换掉词库，宿主下次启动就切到新代次。用 `msime-linux-setup --download` 自己下载的词库没有人替换：升级抬高了词库版本，而记录的词库目录仍是旧的那份时，`msime_client_refresh_host` 的校验失败以 `dictionary_outdated:` 开头报告，与其他准备失败区分开。这时宿主照旧继续用旧代次，IBus 宿主在 stderr 写明原因，Fcitx5 写诊断 `reason=dictionary_outdated`，两者都以不阻塞的方式调用随装的 `msime-linux-first-run-guide --reason dictionary-outdated`。有图形会话时它在引导目录（`$XDG_RUNTIME_DIR/msime-client`，没有时是缓存目录下的 `msime-client`；其他版本是各自的目录，例如 `msime-client-wubi`，设置应用按自己所属的版本读同一个目录，并排安装时互不影响）留下标记文件 `dictionary-outdated`，并发一条桌面通知「水杉输入法词库需要更新」，指向设置里的「立即更新」（装了设置窗口时；没有设置窗口时仍指向 `msime-linux-setup --update --download`），不打开窗口；`notify-send` 支持动作时点通知上的「打开设置」即打开设置窗口。通知限流与首次配置引导相同，但标记是独立的 `dictionary-outdated.stamp`，两种引导互不抑制；`dictionary-outdated` 标记每次调用都写。设置窗口启动时看到这个标记，就先以首次配置的小窗口打开「词库需要更新」页，「立即更新」运行 `msime-linux-setup --update --download`，进度和输出与首次配置相同，成功后删掉标记；「稍后」直接进入设置。`msime-linux-setup --update`（状态目录已有 `runtime-options.json` 时给 `--download` 即视为 `--update`）对应 Windows 安装程序升级时的用户词库回放：它读出这份配置记录的 `resources`，按随装的词库锁核对；不符时加 `--download` 在它旁边另建一份按锁内容命名的 `resources-<摘要>` 目录，与锁一致的文件直接硬链接过去，只取回缺失或不符的几项。`msime-linux-setup` 不往正在使用的那份目录写任何文件：旧代次的会话每次查询都按路径读其中的 `msime-others.db` 等文件，原地替换会让它读到新旧混杂的一份，切换失败时也无法退回；旧目录保留，确认后由用户自行删除，锁之外的多余文件也随之留在旧目录里。随后它像设置窗口做词库维护那样写入 `.msime-dictionary-quiesce` 租约（在切换期间续期），并独占会话锁证明两个宿主都已关掉输入会话，再运行 `msime-linux-prepare --refresh`，走与宿主启动时同一条刷新路径切到新代次并回放用户词库。新目录先写进状态目录下随机临时子目录中的 `runtime-options.json` 副本交给刷新，刷新成功后这份副本才原子替换正式配置，失败时临时目录会清理。切换期间不开会话是为了回放完整：Engine 只在准备新代次时回放一次用户词库日志，之后一个仍开着的会话学到的词只会写进旧代次和日志，新代次再也拿不到。会话在 2.5 秒内未释放时不切换、配置不变，已下好的新目录留给下次重试。记录的词库目录是本次安装随包提供的那份（前缀下的 `share/msime-client/resources` 与 `$XDG_DATA_HOME/msime-client/resources` 重合时除外，例如装在 `~/.local` 下，那里的词库是首次配置时用户自己下载的），或不归当前用户所有、当前用户不可写时拒绝更新，说明它由包管理器负责；`--update` 不接受 `--resources`，没有 `runtime-options.json` 时报错并提示先做首次配置。`msime-linux-prepare --refresh` 输出 `refreshed` 或 `current`，词库仍与本版本不符时以 3 退出，其他失败以 1 退出，文件都保持原样。租约撤下后宿主重新打开的会话即使用新代次：Fcitx5 在建立会话时读取这份文件；IBus 宿主监视它并在 100 ms 内重读，监视不可用时每 5 秒重读一次，所以 IBus 在运行时租约会在切换后再保留 6 秒。

**状态准备好后，setup 把输入法加进正在运行的宿主的输入法列表**，对应 Windows 安装程序注册 TSF profile 后输入法直接出现在列表里。两个宿主都在跑时以 Fcitx5 为准。Fcitx5 经 D-Bus（`org.fcitx.Fcitx5` 的 `/controller`）先看 `AvailableInputMethods` 是否已列出 `msime`，没有才调用 `Restart`（Fcitx5 只在启动时加载插件，写入组里的未加载输入法会被它丢掉），然后读出当前输入法组，把 `msime`（显示为「水杉输入法」，英文界面为「MSIME」）追加到组末尾后写回，再读回核对。IBus 下先看 `ibus list-engine` 是否已列出 `msime-linux`，没有才执行 `ibus restart`（ibus-daemon 只在启动时读组件文件），然后把 `('ibus', 'msime-linux')` 追加到 GNOME 的 `org.gnome.desktop.input-sources sources`（`XDG_CURRENT_DESKTOP` 含 GNOME 且装有该 schema 时；GNOME Shell 不读 IBus 自己的列表），其他桌面追加到 `org.freedesktop.ibus.general preload-engines`，输入源里显示为「Metasequoia 水杉输入法」。已在列表里时只读不写，重复运行不会重复添加；IBus 列表为空说明桌面在用一份没写进该项的默认输入源，这时不写，免得把它替换掉；Fcitx5 当前组为空时同样不写，因为组里第一项是非激活状态下使用的输入法，应当是键盘布局，只放本输入法会让它无处可切回。任何一步失败（宿主没在跑、D-Bus 调用失败、宿主重启后仍未加载本输入法、列表或当前组为空）都只在 stderr 说明原因并打印手动步骤：Fcitx5 用 `fcitx5-configtool` 把「水杉输入法」加入当前输入法组，IBus 执行 `ibus restart` 后在输入源里添加「Metasequoia 水杉输入法」；已经准备好的状态不受影响，退出码仍为 0。`--no-register` 跳过这一步，只打印手动步骤。卸载时由 `msime-linux-setup --unregister` 把本输入法从这些列表里移除（见下文「卸载 CMake 安装」）。`msime-linux-setup --register` 是反过来的那一步：默认位置的状态目录已有 `runtime-options.json` 时，重新启用首次配置启用的在线、语音和剪贴板服务、链回 Omarchy 的钩子和插件，并按上面的规则把输入法加回正在运行的宿主的列表，不检查词库、不改状态；没配置过时什么也不做。发行版仓库的包替换另一个同样内容的包之后自动替每个用户运行它（见下文「包管理器」）。

**先选了输入法、还没做首次配置时，宿主会把用户引到首次配置页，而不是静默失效。** Windows 的安装程序在安装时就把数据目录准备好，输入法一能选中就能用；Linux 安装包不产生用户状态，此前在 IBus 里选中 MSIME 时启动器只往 stderr 写一行就退出，用户看到的是一个没反应的输入法，Fcitx5 则只显示笼统的「请检查运行配置」。现在只有「没有显式覆盖、用户的 `runtime-options.json` 与系统级配置都不存在」这一种状态被判为「尚未完成首次配置」；显式指定的 `MSIME_IBUS_OPTIONS`/`MSIME_FCITX5_OPTIONS` 无效、文件不可读或是悬空符号链接，仍按配置损坏处理，不做引导。判为尚未配置时，IBus 启动器照旧写 stderr 并以非零状态退出，Fcitx5 在面板上显示「水杉输入法尚未完成首次配置：请打开「水杉输入法」设置，或在终端运行 msime-linux-setup」，按键继续交给应用；两者随后都调用随装的 `msime-linux-first-run-guide`（Fcitx5 只在激活输入法时调用，按键只显示提示，免得打字途中弹出的窗口抢走键盘焦点）。这个脚本在有图形会话（设置了 `DISPLAY` 或 `WAYLAND_DISPLAY`）时以脱离调用方的方式打开 `msime-linux-settings`（窗口自己会进入首次配置页），并在装有 `notify-send` 时发一条桌面通知；没装设置窗口的最小安装只发通知，改为指向 `msime-linux-setup`。通知里的后续步骤按宿主区分（调用方传 `--host ibus|fcitx5`）：Fcitx5 下次按键或聚焦就会重读配置，写的是「完成后即可直接输入」；IBus 组件已经退出，写的是先切换到其他输入法再切回，仍不行就运行 `ibus restart`。ibus-daemon 每次选中都会重新拉起启动器、Fcitx5 每次聚焦都会激活输入法，所以引导有限流：标记放在 `$XDG_RUNTIME_DIR/msime-client/first-run-guide.stamp`，每个登录会话两个宿主合计只弹一次，标记不按时间过期，重新登录后才会再弹；会话没有 `XDG_RUNTIME_DIR` 时退到跨会话保留的 `$XDG_CACHE_HOME/msime-client/`，只能按 5 分钟冷却期限流。没有图形会话时既不弹窗也不写标记。脚本不创建状态目录（`msime-linux-setup` 拒绝准备已存在的目录），也不发起任何网络请求，下载仍只在用户勾选或传 `--download` 时发生。契约由 `tests/core/first_run_guide.py`（桩掉设置窗口与 `notify-send`，验证只调用一次、同一会话内不再调用、并发调用只引导一次、缓存目录下的冷却期、按宿主区分的通知文案、无图形会话时不调用，以及 `--reason dictionary-outdated` 只发通知不开窗口、与首次配置各用一个标记互不抑制）、`tests/core/first_run_guidance.cpp`（Fcitx5 的配置定位与「尚未配置」判定）和 `fcitx5/tests/native.cpp` 里的首次配置用例（真实插件：面板提示、按键不被拦截、按键不拉起引导、激活只拉起一次、不写失败诊断；词库过期时写 `reason=dictionary_outdated`、以 `--reason dictionary-outdated` 拉起引导且不改写配置）钉住。

**Fcitx5 的状态区入口需要桌面提供托盘宿主。** 中英文、简繁、工具栏这些动作挂在输入法状态区，由托盘（StatusNotifierItem）承载；部分发行版以 `fcitx5 --disable notificationitem` 启动，那些入口就不会出现。输入本身不受影响，设置也仍可从应用列表里的「水杉输入法」或 `msime-linux-settings` 打开；要让它们显示，需要在桌面侧恢复托盘。这属于发行版与桌面的配置，输入法不代劳。能连上 X 服务器（含 Wayland 桌面的 Xwayland）时悬浮工具栏照常显示，它不依赖托盘。

判定规则由 `tests/core/setup_resolution.py` 钉住，不需要词库也不联网：哪份词库算可用、锁里没有 URL 的那项归哪个归档、去哪里找锁和已有词库。加入输入法列表由 `tests/core/setup_registration.py` 钉住：用桩代替 `gdbus`、`gsettings`、`ibus` 与 `pgrep`，验证每种宿主写的是哪一项、`ibus restart` 只在引擎未被列出时执行、重复运行不重复写入、D-Bus 失败与 Fcitx5 丢弃写入时退回手动步骤、`--no-register` 不发起任何调用。升级更新由 `tests/core/setup_update.py` 钉住：词库从本机的 HTTP 桩取，验证只请求过期的那一项且下载进新目录、旧目录原样不动、一致的文件是硬链接，`msime-linux-prepare --refresh` 运行时租约有效且会话锁被独占、拿到的是新目录，配置随后指向新代次且不留副本与租约；会话不释放、刷新失败、下载中途失败时配置都不变，旧目录不动；装在与数据目录重合的前缀下时照常更新，随包词库目录以及不带 `--update` 的重复首次配置都被拒绝；门禁里它还直接运行构建出的 `msime-linux-prepare --refresh`，确认相对路径以 2 退出、与编译进去的锁不符的词库以 3 退出且不改写配置。

### 设置窗口闪烁、打不开或空白

设置窗口由 WebKitGTK 绘制。WebKitGTK 的 GPU 路径（合成模式与 DMA-BUF 渲染器）在部分驱动与合成器组合上有问题，设置启动器 `msime-linux-settings` 按下面的规则给两个环境变量定默认值：

- 加载了 NVIDIA 专有驱动（`/proc/driver/nvidia/version` 或 `/sys/module/nvidia` 存在，nvidia-open 也算）：默认 `WEBKIT_DISABLE_COMPOSITING_MODE=1` 和 `WEBKIT_DISABLE_DMABUF_RENDERER=1`，改用 CPU 绘制，规避窗口一启动就报 `Error 71 (Protocol error) dispatching to Wayland display` 退出（#3273、#5834）。
- 其他机器：两个变量都不设，由 WebKit 自己选择渲染路径。早先的版本对所有机器都设成 `1`，在 niri 下会让窗口持续闪烁（#6847），在其他桌面上会让滚动不跟手（#6403）。

自己设了的非空值一律优先，启动器不改；空值当作没设。所以默认判断不适合你的机器时，可以显式覆盖：

- 没有 NVIDIA 却打不开、空白或报 Error 71：两个变量都设成 `1`。
- 有 NVIDIA 但想试 GPU 绘制，或 NVIDIA 下窗口闪烁：两个变量都设成 `0`。只设其中一个不够：任一变量为 `1` 都会让 WebKit 退回 CPU 绘制。
- 想在 NVIDIA 上保留合成与 DMA-BUF 渲染器，只把缓冲改走共享内存：在前一条的基础上再加 `WEBKIT_DMABUF_RENDERER_FORCE_SHM=1`。NVIDIA 专有驱动（open 内核模块 615.71.09）加 Hyprland 0.56.2、webkit2gtk-4.1 2.54.1 上实测这样能避开 Error 71（#5834），但还没有量过它是否真的走了 GPU 合成路径；要长期用它，先确认这一点。

终端里临时试一次：先确认旧的设置进程已经完全退出（`pgrep -af msime-linux-desktop` 没有输出；设置外壳是单实例的，关掉窗口后进程还会留十分钟，再次打开会复用它和它启动时的环境），再运行

```sh
WEBKIT_DISABLE_COMPOSITING_MODE=0 WEBKIT_DISABLE_DMABUF_RENDERER=0 msime-linux-settings
```

从早先的版本升级上来后，同样要先让旧的设置进程完全退出，新的默认值才会生效。早先的启动器在所有机器上都把两个变量设成 `1`，这个值留在它拉起的设置进程的环境里。升级时这个进程如果还在（关掉窗口后的十分钟内都算），再打开设置会由它把启动交给新程序（见上文「安装后首次使用」里升级后设置应用的那一段）：新程序是旧进程直接启动的，没有经过新的启动器，继承的仍是两个 `1`，窗口照旧闪烁；它关掉窗口后同样会留十分钟，这期间再打开都复用它。所以升级后先关掉设置窗口，等到 `pgrep -af msime-linux-desktop` 没有输出（等十分钟，或关窗几秒后执行 `pkill -f msime-linux-desktop`）再打开设置。这一步只需做一次，之后的设置进程都由新的启动器拉起。

要让终端、应用菜单和 IBus/Fcitx5 菜单里的「设置」都用同样的值，把它们写进 `~/.config/environment.d/` 下的一个文件，例如 `~/.config/environment.d/msime-webkit.conf`：

```ini
WEBKIT_DISABLE_COMPOSITING_MODE=0
WEBKIT_DISABLE_DMABUF_RENDERER=0
```

然后重新登录。由 systemd 用户实例管理的会话（GNOME、KDE Plasma、用 `niri-session` 启动的 niri 等）会读这个目录；不经过它的会话，把这两行写进合成器自己的环境配置（例如 Hyprland 的 `env = WEBKIT_DISABLE_COMPOSITING_MODE,0`）。输入法菜单里的入口由 IBus 或 Fcitx5 进程拉起，用的是它们启动时的环境，只在 shell 配置（`~/.bashrc` 等）里设变量对它们无效。不必再用包装脚本加 `MSIME_CLIENT_SETTINGS_COMMAND`；那个变量仍然可用，设了时输入法菜单改为运行它指定的命令，环境由那条命令自己负责。

不要为此设 `GDK_BACKEND`：启动器不改它，设置窗口在 Wayland 会话里本来就用 Wayland 后端。报告问题时请附上显卡型号与驱动、合成器及版本、`pacman -Q webkit2gtk-4.1`（或发行版对应的包）的输出、输出是否用了分数缩放，以及上面两种覆盖各自的结果。

### Omarchy

[Omarchy](https://omarchy.org) 用 Hyprland 和 Fcitx5，Fcitx5 由用户服务 `omarchy-fcitx5.service` 以 `fcitx5 --disable notificationitem` 启动（`Restart=always`）。水杉在这里多做了四件事：

- **候选窗口跟随 Omarchy 主题。** `msime-linux-setup` 检测到 Omarchy（`~/.config/omarchy` 与 `~/.local/state/omarchy/current/theme` 都在）时，把随包安装的钩子 `<前缀>/share/msime-client/omarchy/theme-set` 链接为 `~/.config/omarchy/hooks/theme-set.d/msime`，并立即按当前主题生成一次皮肤。Omarchy 每次切换主题后运行这个钩子，钩子执行 `msime-linux-settings --sync-omarchy-theme`：不开窗口，用 Omarchy 自己的 `omarchy-theme-color --all` 读出新主题解析后的配色（旧式 `colorN` 主题也一样），写成皮肤目录下的 `omarchy` 皮肤包（直角、背景与前景取主题的 `background`/`foreground`，边框和强调取 `accent`，选中取 `selection`，悬停取 `lighter_background`，序号和翻译取 `dark_foreground`），再发布皮肤目录，宿主热重载后即生效。皮肤不会被自动选中：在输入法的「主题」菜单或设置页里选「Omarchy」之后，候选窗口才随 Omarchy 主题换色。`~/.config/omarchy/hooks/theme-set.d/msime` 已被别的文件占用时不覆盖，只打印提示；`msime-linux-setup --unregister`（`.deb` 卸载时由 prerm 调用）只移除指向水杉钩子的链接。包被删掉后留下的悬空链接会被 Omarchy 跳过，不会让切换主题出错。由 `tests/core/setup_omarchy.py` 和 `apps/desktop/src-tauri/src/shared/omarchy_skin.rs` 的单元测试钉住。
- **重启提示指向用户服务。** 在 `omarchy-fcitx5.service` 里执行 `fcitx5 -r` 会在服务之外再起一个 Fcitx5，它抢走总线名，服务自己的那份随即退出又被 `Restart=always` 拉起，两者来回抢。所以插件发现自己运行在某个 systemd 用户服务里（读 `/proc/self/cgroup`；桌面自启动项生成的 `app-*` 单元和作用域不算）时，升级、卸载提示改为 `systemctl --user restart <该服务>`。由 `tests/core/replaced_program.cpp` 钉住。
- **Hyprland 规则。** 模式角标和语音浮层是 layer-shell 表面，命名空间分别为 `msime-linux-mode-badge` 和 `msime-linux-wave-overlay`。Omarchy 对自己的浮层关掉了 layer 动画，水杉的也建议这样做，否则角标会从角落滑入；浮层边缘是半透明的，可以顺带打开背景模糊。把下面几行加进 `~/.config/hypr/looknfeel.lua`（或任意被 `hyprland.lua` require 的文件）：

  ```lua
  -- MSIME overlays: appear instantly like Omarchy's own, and blur what shows through their translucent edges.
  hl.layer_rule({ match = { namespace = "^msime-linux-(mode-badge|wave-overlay)$" }, no_anim = true, animation = "none", blur = true, ignore_alpha = 0.2 })

  -- MSIME panels on keys, through the same launcher the application menu uses.
  o.bind("SUPER + CTRL + ALT + V", "MSIME voice", "msime-linux-settings --panel voice")
  o.bind("SUPER + CTRL + ALT + H", "MSIME handwriting", "msime-linux-settings --panel handwriting")
  o.bind("SUPER + CTRL + ALT + I", "MSIME settings", "msime-linux-settings")
  ```

  按住说话不需要合成器绑定：输入法自己处理按住 RAlt（或 Ctrl+Win、RCtrl+RAlt）录音、松开结束，Ctrl+F9 开始/结束切换（见「语音输入」）。别把这些组合键绑给 Hyprland，否则按键到不了输入法。上面三个组合键在 Omarchy 默认绑定里空着，自定义过绑定的话先用 `omarchy menu keybindings --print` 确认。
- **状态栏插件代替托盘。** `--disable notificationitem` 让 Fcitx5 不注册托盘图标，水杉的中/英标签和托盘菜单在 Omarchy 上就都看不到了。`msime-linux-setup` 在 Omarchy 上把随包安装的插件目录 `<前缀>/share/msime-client/omarchy/plugin` 链接为 `~/.config/omarchy/plugins/metasequoia.msime`，并提示运行 `omarchy plugin enable metasequoia.msime` 把它放上状态栏（setup 不改用户的状态栏布局）。插件显示当前的 中/双/五/英/日/⇪，悬停显示当前方案；左键打开菜单（设置、词库、手写、屏幕键盘、表情、语音输入、云剪贴板、重载水杉输入法，`omarchy-fcitx5.service` 在运行时还有重启 Fcitx5），中键直接打开设置。数据来自 Fcitx5 插件写的 `$XDG_RUNTIME_DIR/msime-client/input-status.json`：`{"active":bool,"label":"中","scheme":"quanpin"}`，焦点上下文由水杉处理时 `active` 为真，切到别的输入法或失去焦点后为假，插件随之隐藏；内容不变时不重写。`--unregister` 只移除指向水杉插件的链接。由 `tests/core/input_status.cpp` 和 `tests/core/setup_omarchy.py` 钉住。

## 生成 Linux 安装包

发行版由 `.github/workflows/release-linux.yml` 手动触发，标签为 `linux-v<版本>`（版本默认取 `platforms/linux/version.txt`），附件按 x86_64 与 aarch64 两个架构各有一套（两者分别在 GitHub 的 x64 与 arm64 runner 上原生构建）：一个 Debian 包 `msime-linux_<版本>_<架构>.deb`（架构是 dpkg 的 `amd64`/`arm64`）、一个与它同一套文件、按 `/usr` 布局的归档 `msime-linux-<版本>-linux-<架构>.tar.gz`（`x86_64`/`aarch64`）、一个 Fedora 包 `msime-linux-<版本>-1.<架构>.rpm`（`x86_64`/`aarch64`），以及只列这些安装包的 `SHA256SUMS`。发布说明开头是 `scripts/edition_linux.py download-guide` 从版本表生成的下载指引：先按发行版选 `.deb`、`.rpm`、legacy `.deb` 还是 `.tar.gz`，再按 `uname -m` 选架构，最后每个版本一行。版本发布页只放用户会下载的安装包；发行版源码构建用的三个 tarball `msime-<版本>.tar.xz`、`msime-<版本>-vendor.tar.xz` 与 `msime-<版本>-frontend.tar.xz` 和只列它们的 `msime-<版本>-SHA256SUMS` 放在滚动的 `linux-sources` release 上（见下文「包管理器」），0.12.1 及更早版本的仍在各自的发布页上。设置页的检查更新按 `linux-v` 标签前缀挑选发行版并打开发行页；附件只用来显示校验值，按扩展名 `.deb`（没有时取 `.tar.gz`）识别，并按宿主报告的架构（`HostCapabilities.arch`）只看本机架构的那一份；宿主没报告架构时，一个发行版里有两份就不显示校验值。同一个标签下其他版本（见下文「多版本」）各有同样的一组附件，包名换成 `msime-linux-<id>`，设置页按本版本的包名挑选自己的那一份。两个架构另各有一个给 glibc 2.28 基线系统用的 `msime-linux-legacy_<版本>_<架构>.deb`（见下文「旧发行版的 legacy 包」），包内的包名仍是 `msime-linux`，附件名带 `legacy` 是为了不和上面那个 `.deb` 撞名，设置页挑 full 的包时也按 `msime-linux-<字母>` 开头把它排除。

安装前先核对校验值：`sha256sum -c SHA256SUMS --ignore-missing`。Debian/Ubuntu 用 `sudo apt install ./msime-linux_<版本>_<架构>.deb`，依赖由 apt 一并装好，卸载用 `sudo apt remove msime-linux`。对应 Windows 安装程序在卸载和升级时停止输入法进程：`apt remove` 删除文件前，包的 prerm 在每个已登录（或启用了 linger）用户的 systemd 用户实例里逐个 `disable --now` 与 CMake 卸载相同的那组在线、语音和剪贴板单元，免得它们指着已删除的程序反复重启，再在同一实例里以临时单元运行 `msime-linux-setup --unregister`，把本输入法从该用户的输入法列表里移除（见「卸载 CMake 安装」）；升级后 postinst 让这些实例重读单元文件，并重启其中正在运行的服务，使其换到新程序，socket 单元保持监听，输入法宿主自己换到新程序（见上文「安装后首次使用」里的升级一段）。联系不上的用户实例只打印该用户需要执行的命令；未登录的用户没有运行中的服务，但启用链接仍留在各自的 `~/.config/systemd/user`，需要时自行执行 `systemctl --user disable …`。没有 systemd 的环境（例如容器）两步都跳过，也都不会让 apt 失败。包里唯一不在 `/usr` 下的文件是剪贴板服务的 XDG 自启动项 `/etc/xdg/autostart/msime-linux-clipboard.desktop`（见「独立剪贴板采集」），它是 conffile：管理员修改或删除它之后，升级不会把它改回来；`apt remove` 留下它、`apt purge` 才删除，留下的自启动项在服务已被 prerm 停用后什么也不做。归档给不经 apt 安装的 Debian 系系统用，系统要求与 `.deb` 相同（Debian 12、Ubuntu 22.04 及更新的版本，见下一段），不是跨发行版的通用包：库目录是 Debian 的多架构布局 `usr/lib/<三元组>/`（例如 `usr/lib/x86_64-linux-gnu/`），Fcitx5 插件因此在 `usr/lib/<三元组>/fcitx5/`，Arch（`/usr/lib/fcitx5`）和 Fedora（`/usr/lib64/fcitx5`）上的 Fcitx5 不会去那里加载它。用法：`sudo tar -xzf msime-linux-<版本>-linux-<架构>.tar.gz --strip-components=1 -C /`，再执行 `sudo gtk-update-icon-cache -f -t /usr/share/icons/hicolor`；归档没有依赖声明，需由发行版提供 IBus 1.5.20+ 或 Fcitx5 5.0.20+、Python 3.9+，以及二进制链接的共享库（WebKitGTK 4.1、GTK 3、libsoup 3、ICU、libcurl、SQLite、D-Bus、Wayland、X11、xkbcommon 等，完整列表以同版本 `.deb` 的 Depends 为准）；它也没有卸载入口，删除时按归档内的文件列表（`tar -tzf`）逐个移除。两种方式装完都按上面的「安装后首次使用」执行 `msime-linux-setup`。

`.deb` 与归档在 Debian 12（bookworm）容器里构建，链接的是 bookworm 的库版本，Depends 因此要求 glibc 2.35、libstdc++ 12（包里的 ELF 文件最高要 `GLIBC_2.35` 与 `GLIBCXX_3.4.30`）、WebKitGTK 4.1 与 libsoup 3 及以上，只能装在提供这些包的发行版上（归档没有 Depends，但二进制是同一份，要求也一样）：Debian 12、Ubuntu 22.04 与更新的版本可以，Debian 11、UOS 20 这类 glibc 更旧或没有 WebKitGTK 4.1 的系统不行（#6311）；Debian 10 基线的系统装发布页上只含 IBus 的 legacy 包，见下文「旧发行版的 legacy 包（glibc 2.28）」。输入法框架在 Depends 里写成二选一 `ibus (>= 1.5.20) | fcitx5 (>= 5.0.20)`（#6401）：系统里已有哪个就算满足哪个，deepin 这类用 Fcitx5 的系统不会被拉进 IBus 守护进程和它的 GTK 模块，用 IBus 的系统也不会被拉进 Fcitx5；两个都没有时 apt 装写在前面的 IBus。Fcitx5 不进 Recommends，因为 apt 默认安装推荐包，那样 IBus 系统会被拉进 Fcitx5。Ubuntu 22.04 只有 Fcitx5 5.0.14，在那里由 IBus 满足这一项，整包照样装得上（#6305）；即使另装了系统的 Fcitx5 5.0.14，它也会按插件条目的 `core:5.0.20` 依赖拒绝加载插件。辅助程序链接 libibus，所以 `dpkg-shlibdeps` 仍会带上 `libibus-1.0-5`，那只是一个库，不会启动 IBus。`dpkg-shlibdeps` 从插件推出的 `libfcitx5*` 依赖由 `package-container.sh` 重新打包时从 Depends 去掉，`release-linux.yml` 逐包核对 Depends 里只以这个二选一提到两个框架。`.rpm` 同样写成 `(ibus >= 1.5.20 or fcitx5 >= 5.0.20)`，插件链接的 `libFcitx5*` 不进自动生成的 Requires；Arch 的两个 PKGBUILD 把 `ibus`、`fcitx5` 都放在 optdepends。

`.rpm` 在 Fedora 44 容器里单独构建（`tests/tools/Dockerfile.package-rpm`），不是由 `.deb` 转换来的：rpmbuild 按二进制实际链接的库生成 Requires，在 Debian 上链接的二进制会带上 Debian 独有的 soname 与符号版本（例如 libcurl 的 `CURL_OPENSSL_4`），Fedora 上没有包提供它们，已归档的 MSIME-Linux 0.9.1 的 rpm 就是因此装不上（#2095）。它面向 Fedora 44 及提供同样库版本的 DNF 系统，用 `sudo dnf install ./msime-linux-<版本>-1.<架构>.rpm` 安装，`sudo dnf remove msime-linux` 卸载；维护脚本就是 `.deb` 的 prerm 与 postinst，前面加一段把 RPM 的实例计数换算成对应的 dpkg 参数，所以卸载与升级的行为与上面 `.deb` 的描述相同，自启动项是 `%config(noreplace)`。包自带的 Host API 库与 sherpa-onnx 运行库既不作为依赖要求、也不对系统声明提供。发行流程在构建后把它装进一个干净的 Fedora 容器再卸掉（`tests/tools/check-rpm-install.sh`），依赖解析不了就在那一步失败。支持更多发行版仍需按目标发行版分别构建。

本地生成同一套附件：先 `pnpm install --frozen-lockfile && pnpm --filter @msime/desktop build`（桌面二进制在编译期嵌入前端产物），再 `bash platforms/linux/package-container.sh [版本]`。脚本在构建门禁镜像之上叠加 Tauri 与 Debian 打包依赖（`tests/tools/Dockerfile.package`），依次构建 Release 版 Host API 库、带 `tauri/custom-protocol` 的桌面二进制（报告的版本与包版本一致），用 `collect-notices.py` 收集这两个 Rust 产物静态链接的 crate 与前端打包进去的 npm 包的许可证文件，以 `-DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/usr -DMSIME_ENABLE_PACKAGING=ON -DMSIME_ENABLE_FCITX5=ON -DMSIME_BUNDLE_HANDWRITING_MODEL=OFF` 配置并构建原生宿主（发布页的包不带离线手写模型，由设置应用按需下载）、跑与门禁相同的 ctest，最后用 CPack 生成 DEB 与 TGZ，产物在 `target/linux-package/dist`。`MSIME_PACKAGE_FORMAT=rpm` 换成 Fedora 镜像走同样的步骤，CPack 生成 RPM，产物在 `target/linux-package-rpm/dist`，之后可用 `bash platforms/linux/tests/tools/check-rpm-install.sh <rpm>` 做安装检查。`MSIME_PACKAGE_DESKTOP=0` 可在没有前端产物时只打原生宿主，但那样的包没有设置窗口，不能作为发行版。`build-container.sh` 仍是 Debug 构建的合并前门禁，不产出安装包。

在 Linux 上手工打包时，同样显式传入 `-DMSIME_ENABLE_PACKAGING=ON -DCMAKE_INSTALL_PREFIX=/usr`，并提供 Release 版 Host API 库、桌面二进制和已固定来源的资源；`-DMSIME_PACKAGE_VERSION=<版本>` 指定包版本，不传时取 `platforms/linux/version.txt`（发布工作流读的同一个文件）；IBus 宿主在启动与崩溃上报里报告的也是这个版本。打包构建必定包含 Fcitx5 原生插件（不受开发机上是否装有 Fcitx5 开发包影响）；若只需 IBus 开发构建，可显式传入 `-DMSIME_ENABLE_FCITX5=OFF`。打包构建不得设置 `MSIME_RUNTIME_OPTIONS_FILE`，也不得启用安装开发测试程序的 `MSIME_LINUX_VOICE`。打包构建还必须传入 `-DMSIME_RUST_NOTICES=<文件>`（`python3 platforms/linux/collect-notices.py cargo <文件> msime-host-api msime-desktop:tauri/custom-protocol`，不打桌面二进制时去掉后一项），打包桌面二进制时再传 `-DMSIME_FRONTEND_NOTICES=<文件>`（`python3 platforms/linux/collect-notices.py npm <文件> apps/desktop`，需先装好 `node_modules`），缺哪一个配置就失败。构建完成后运行 `cpack --config <build-dir>/CPackConfig.cmake -G "TGZ;DEB"`；DEB 需要 `dpkg-shlibdeps`（`dpkg-dev`）与 `file`。Debian 包在手写的 IBus、Python、Fcitx5 依赖之外，由 `dpkg-shlibdeps` 从 ELF 文件生成共享库依赖。归档不是可任意搬移的便携包。

许可证与第三方声明装在 `${CMAKE_INSTALL_DATADIR}/doc/msime-client/`，普通 `cmake --install` 与安装包相同：`copyright`（本项目 GPL-3.0）、`THIRD_PARTY_NOTICES.txt`（本平台随附组件总览，源文件 `data/THIRD_PARTY_NOTICES.txt`），词库来源声明 `msime-engine-dictionary-NOTICE.md`（从 msime-engine 原样带过来，固定副本在 `resources/licenses/`）、辅助码声明 `msime-engine-helpcode-NOTICE.md`（`resources/helpcodes/ENGINE-NOTICE.md`）、`msime-helpcode-jiajia-NOTICE.md`（`resources/helpcodes/NOTICE.md`）与 `msime-helpcode-wubi86-NOTICE.md`（`resources/helpcodes/NOTICE-wubi86.md`）、离线手写识别所移植的 zinnia 的许可证 `Zinnia-LICENSE.txt`（`resources/licenses/`）、`@` 模式内置地名所取自的 modood/Administrative-divisions-of-China 的许可证 `Administrative-divisions-of-China-WTFPL.txt`（`resources/licenses/`）、韩语汉字转换内置汉字表所取自的 libhangul `data/hanja/hanja.txt` 的许可证 `libhangul-hanja-BSD-3-Clause.txt`（`resources/licenses/`）、拆字反查（C 模式）内置部件拼音表所改编自的 kfcd/chaizi（開放詞典《漢語拆字字典》，CC BY 3.0）的署名、改动说明与许可证 `kfcd-chaizi-CC-BY-3.0.txt` 及表中 Unihan 读音的 `Unicode-License-V3.txt`（`resources/licenses/`）、引擎粤语与注音方案的数据所取自的 rime-cantonese 与 libchewing-data 的许可证 `rime-cantonese-CC-BY-4.0.txt` 与 `libchewing-data-LGPL-2.1.txt`、笔画方案的笔顺数据所取自的 rime-stroke 的许可证 `rime-stroke-LGPL-3.0.txt`（`resources/licenses/`；装了这些词库时，词库旁还有随数据发布的 `msime-rime_cantonese_LICENSE.txt`、`msime-libchewing_data_LICENSE.txt` 与 `msime-rime_stroke_LICENSE.txt`）、藏文方案所用 ewts crate 的许可证 `ewts-MIT.txt`（`resources/licenses/`；这个 crate 没有附许可证文件，按包元数据收集的 `rust-crates-NOTICES.txt` 里只有它声明的许可证表达式）、装了手写模型时随模型的 `HandwritingModel-LICENSE.txt`、OpenCC 词典的许可证，编进原生宿主与 Fcitx5 插件的 nlohmann/json（`nlohmann_json-MIT.txt`，固定副本在 `data/licenses/`）与 Wayland 协议代码（配置时从实际编译的 `wlr-layer-shell-unstable-v1.xml`、`xdg-shell.xml` 取出 `<copyright>` 段，只在找到 `xdg-shell.xml`、协议代码确实编入时安装），以及打包时收集的 `rust-crates-NOTICES.txt` 与 `frontend-npm-NOTICES.txt`。输入引擎是本项目自己的 Rust 代码（`crates/engine`），由同为 GPL-3.0 的 msime-engine 移植而来，由 `copyright` 覆盖，不再单独附 Engine 的许可证；录音采集走 cpal，其许可证在 Rust 汇总里。本项目许可证在这里按 Debian 的要求叫 `copyright`（macOS 是 `GPL-3.0.txt`）。打包配置时任何一份声明的来源缺失（通常是没有传入 Rust/npm 汇总，或要随包却没有手写模型）都会直接失败；普通开发配置不要求 Rust/npm 汇总，其余缺失只给出警告并安装剩下的部分。

安装包不包含用户状态，不自动启用 provider 服务或切换输入法。首次使用由随装的 `msime-linux-setup` 备齐词库并准备运行配置（见上面的「安装后首次使用」）；语音录音、剪贴板、Wayland/X11 输入工具及可选模型按对应功能章节配置。包的内容取决于配置阶段传入了什么：没有传入桌面二进制或资源的构建只打包实际配置的部分，完整包需要同时提供二者。

### 旧发行版的 legacy 包（glibc 2.28）

该装哪个包，先看系统的 C 库版本（`ldd --version` 的第一行）和输入法框架：

| 系统 | 装哪个包 |
|---|---|
| glibc 2.35 及以上的 Debian 系（Debian 12、Ubuntu 22.04 及更新的版本），有 IBus 1.5.20+ 或 Fcitx5 5.0.20+ | 发布页的 `msime-linux_<版本>_<架构>.deb`，带设置窗口 |
| Fedora 等 RPM 系 | 发布页的 `.rpm`，或下文「包管理器」里的软件源 |
| glibc 2.28 到 2.34 的 Debian 系（UOS 20 专业版、Debian 10 这一类，Debian 11、Ubuntu 20.04 也是） | 发布页的 `msime-linux-legacy_<版本>_<架构>.deb`：只有 IBus 宿主，没有设置窗口，先装 IBus |
| 只有 fcitx 4 的系统（UOS 20、麒麟 V10 等默认就是 fcitx 4） | 没有 fcitx 4 前端。改用 IBus 后，按 glibc 版本装上面对应的包 |
| glibc 低于 2.28 | 没有能用的包 |

两个架构（`amd64`、`arm64`）都有 legacy 包。它由 `release-linux.yml` 调用 `.github/workflows/build-linux-legacy.yml` 在各自架构的原生 runner 上构建，发版时取消勾选 `legacy` 可以不带它；同一个 workflow 每周一在 develop 上定时跑一次，legacy 构建线自己的文件有改动的 Pull Request 上也会跑。

`package-legacy-container.sh` 在 Debian 10（buster）容器里构建这个只含 IBus 宿主的 `.deb`，给 UOS 20 专业版、Debian 10 这类 glibc 2.28 基线、装不上发布页 `.deb` 的系统用（#6311）。也可以在装有 Docker 的机器上自己构建：`bash platforms/linux/package-legacy-container.sh [版本]`，产物是 `target/linux-package-legacy-<架构>/dist/msime-linux_<版本>_<架构>.deb` 和它的 `SHA256SUMS`；发布工作流上传前把它改名为 `msime-linux-legacy_<版本>_<架构>.deb`，内容不变。`MSIME_LEGACY_ARCH=amd64|arm64` 选择架构，缺省是本机架构；另一种架构经 Docker 的 `--platform` 在模拟器里构建，要求 Docker 能运行该平台的容器（binfmt/qemu 或 Rosetta），比原生慢得多。

构建镜像是 `tests/tools/Dockerfile.legacy`：buster 自带的 GCC 8.3、IBus 1.5.19、Python 3.7 与 glibc 2.28，加上按 SHA-256 固定的 Kitware CMake 3.25.1、nlohmann-json 3.11.2 源码和 Rust 1.97.1（buster 的 CMake 3.13 与 nlohmann-json 3.5 低于 `CMakeLists.txt` 的要求，两者的版本与 bookworm 门禁镜像相同）。Host API 库、`msime-mcp` 和原生宿主都在这个容器里编译，所以只引用 glibc 2.28 及以下的符号版本；打包后脚本用 `tests/tools/check-elf-symbol-versions.sh` 逐个核对包里的 ELF 文件（含预编译的 sherpa-onnx 与 ONNX Runtime 库），任何一个要求的 glibc、libstdc++（`GLIBCXX`、`CXXABI`）或 libgcc（`GCC`）符号版本高于 buster 提供的就失败，上限从构建镜像里的库读出；发布工作流对改名后要上传的那个文件按固定上限 `GLIBC=2.28 GLIBCXX=3.4.25 CXXABI=1.3.11 GCC=7.0.0` 再核一次。这个脚本也可以单独拿来查任何一个 `.deb` 或解开的目录，例如 `bash platforms/linux/tests/tools/check-elf-symbol-versions.sh msime-linux_<版本>_arm64.deb GLIBC=2.28`，超出时逐个列出文件和它要求的版本。随后它在一个只有基础系统的 `debian:buster` 容器里用 apt 安装这个包，确认每个 ELF 文件的共享库和符号版本都能解析、没有设置窗口的文件，再在装好包的容器里跑构建树的 ctest：测试程序链接的库只能由包的 Depends 带进来。最后由 `tests/tools/legacy-runtime.sh` 做运行时验收：在 Python 3.7 上跑随包 Python 脚本的合约测试（与 `tests/tools/in-container.sh` 同一份清单 `python-contracts.list`，跳过只与设置窗口和豆包有关的两项），用第 1 步按词库锁取回的词库跑 `ibus-engine-smoke --page-number`，并让已安装的 `/usr/bin/msime-linux-ibus` 在 buster 自带的 ibus-daemon 1.5.19 下经合成输入上下文（`daemon_smoke.py`：打字、切换输入源、宿主崩溃后由监护进程重启、`ibus exit`）和 GTK 3 文本框（`gtk_smoke.py`）打字。词库只用于验收，不进包。

这个包与发布页的 `.deb` 包名相同（`msime-linux`），安装路径也相同，装上一个就替换掉另一个。区别是：

- **没有设置窗口。** 不带 Tauri 桌面二进制：它要 WebKitGTK 4.1 和 libsoup 3，buster 没有，Tauri 2 也不支持更旧的 WebKitGTK 4.0。于是没有 `msime-linux-settings`、应用列表里的「水杉输入法」和首次配置页；IBus 菜单里打开设置窗口的那些项（设置、词库、手写输入、屏幕键盘、表情面板、本地剪贴板、语音面板、云词库、云剪贴板、关于、使用帮助、问题反馈）点了没有反应。只能在设置窗口里做的事（词库管理、皮肤、账号与云同步、手写模型下载、检查更新）在这个包里没有替代入口。
- **没有 Fcitx5 插件。** buster 没有 5.0.20 以上的 Fcitx5；UOS 20 默认的 fcitx 4 也没有水杉的前端，只能走 IBus。
- **只有 full（水杉输入法，包名 `msime-linux`）一个版本**，全拼、双拼、五笔等方案都在里面；没有水杉拼音（`msime-linux-pinyin`）、水杉五笔等其他版本的 legacy 包。其他版本配置时要用 `scripts/edition_linux.py` 改写脚本，它需要 Python 3.10。
- **依赖更低。** Depends 是 `ibus (>= 1.5.19), python3 (>= 3.7), procps`，加上 `dpkg-shlibdeps` 按 buster 的库算出的共享库依赖（`libc6 (>= 2.28)` 等）。这两个下限来自 CMake 缓存变量 `MSIME_IBUS_MIN_VERSION` 和 `MSIME_PYTHON_MIN_VERSION`，它们同时决定配置时检查的 IBus 版本和 `.deb`/`.rpm` 的依赖声明，发布页的包用缺省值 1.5.20 和 3.9。宿主用到的 IBus 接口里只有 `IBUS_INPUT_HINT_PRIVATE` 晚于 1.5.19（1.5.26 才进头文件），旧头文件上按协议的固定值补上（`src/core/ClientEngine.h`）；1.5.27 的 `focus_in_id` 与 `has-focus-id` 原本就按 `IBUS_CHECK_VERSION` 条件编译，旧版 IBus 不报告客户端身份，`ime_mode_scope = app` 因此退化为所有窗口共用一个中英文状态（`src/core/ClientInputModeMemory.h`）。
- **豆包实时识别不可用。** 它要 `websockets` 15，而 `websockets` 15 需要 Python 3.9；语音服务照常启动，选用豆包时报告依赖不满足。Recommends 里的 `python3-websockets (>= 15)` 在 buster 上满足不了，apt 会跳过它。
- **升级和卸载时不会替已登录用户处理用户服务。** postinst 和 prerm 用 `systemctl --user -M <uid>@` 联系每个用户的 systemd 管理器，这要 systemd 248，buster 是 241；联系不上时它们只打印该用户要执行的命令，安装和卸载本身不受影响。postinst 顺带为每个用户注册匿名账号的那一步也因此跳过，由在线服务之后重试。

在 UOS 20 上的用法：

1. UOS 20 默认的输入法框架是 fcitx 4，系统里没有 IBus。先装 IBus，再装这个包：`sudo apt install ibus`，然后 `sudo apt install ./msime-linux-legacy_<版本>_<架构>.deb`。卸载时包名仍是 `msime-linux`：`sudo apt remove msime-linux`。
2. 把输入法框架切到 IBus 并重新登录。Debian 10 上是 `im-config -n ibus`；UOS 20 上的切换方式我们没有在真机上验证过。
3. 在终端运行 `msime-linux-setup --download`，按上面「安装后首次使用」取回词库、准备状态并把输入法加进 IBus 的列表；加不上时执行 `ibus restart`，再在 `ibus-setup` 的「输入法」页添加「Metasequoia 水杉输入法」。没有设置窗口，所以先选中输入法、还没做首次配置时不会打开首次配置页，装有 `notify-send` 时桌面通知直接指向 `msime-linux-setup`。

没有设置窗口时改偏好有两条路：

- IBus 面板上的输入法菜单与发布页的包相同，中英文、全角、中文标点、简繁、输入方案、双拼方案、辅助码方案和主题都在那里切换。
- 其余偏好用随包的 `msime` 命令（`msime-mcp` 的命令行形式，见上文「包管理器」）：`msime config` 列出当前偏好，`msime config get <键>` 读取，`msime config set <键>=<值> …` 修改，例如 `msime config set candidate_page_size=7 fuzzy_pinyin=true`，输入法在几秒内热重载。能改的键是 MCP 工具 `update_preferences` 接受的那一组（`crates/mcp-server/src/preferences.rs` 的 `PreferencesChange`），包括输入方案与双拼方案、候选个数、字号、缩放、透明度、圆角与排列、候选跟随光标、数字行选词、模式提示、模糊音、默认中英文、全半角、中文标点、智能标点、繁体输出、五笔方案、五笔混输拼音、五笔编码提示、诊断日志，以及全局主题和浅色、深色模式各用哪款候选窗皮肤（`global_theme`、`candidate_skin`、`candidate_skin_dark`；Linux 宿主从运行时选项里的 `candidate_skin_catalog` 读皮肤，设置页重扫皮肤、保存偏好时发布它，`msime` 装上或删掉皮肤、`config set` 改偏好时也重新发布，所以手工拷进皮肤目录的皮肤在下一次 `config set` 之后也画得出来）；`msime --help` 列出全部命令。

验证范围：amd64 与 arm64 的构建、符号版本检查、buster 里的 apt 安装、ctest 和上面的运行时验收都由 `build-linux-legacy.yml` 在各自架构的原生 runner 上跑过，IBus 引擎在 ibus-daemon 1.5.19 与 Xvfb 上的 GTK 3 程序里打字通过。arm64 的包另在 Debian 11、Ubuntu 20.04 和 Ubuntu 22.04 的容器里用 apt 装上过，`ldd` 没有缺库或缺符号版本，但没有在这些系统上跑运行时验收。两种架构都没有在 UOS 20 真机或真实桌面会话里打过字，Qt 程序与 Wayland 下的行为也没有在 IBus 1.5.19 上测过。

### Nix 与 NixOS

仓库根目录的 `flake.nix` 提供 NixOS 模块和各个包，构建方式、包的组成与开发命令见 [nix/README.md](nix/README.md)。

在 NixOS 上用模块：

```nix
# flake.nix 的 inputs
msime.url = "github:metasequoiaime/msime";

# NixOS 配置（imports 里加 inputs.msime.nixosModules.default）
programs.msime.enable = true;
```

`programs.msime.enable` 接入 Fcitx5 插件，放上 `msime-linux-setup`、`msime-linux-settings` 等命令和设置窗口的桌面入口，并注册 provider 的用户单元。包按本系统的 nixpkgs 构建，与系统上的 Fcitx5 出自同一份，不需要另加 overlay。

用 IBus 时设 `i18n.inputMethod.type = "ibus";`（GNOME 已默认如此，不用另设），模块改用不带 Fcitx5 插件的 `msime-ibus`，把它的 IBus engine 加进 `i18n.inputMethod.ibus.engines`，其余不变。`i18n.inputMethod.type` 是其他框架时会给出警告。

在 GNOME、KDE 以外的 Wayland 混成器上（Hyprland、Sway 等），nixpkgs 的 ibus 模块经 XDG autostart 以 `ibus-daemon --daemonize --xim` 启动 IBus，并导出 `GTK_IM_MODULE=ibus`、`QT_IM_MODULE=ibus`。这样的 IBus 不提供 Wayland 的输入法协议，只认 text-input 协议的原生 Wayland 程序（如 WezTerm）连不上它；经输入法模块接入的 GTK、Qt 程序，IBus 面板把候选和「中/英」提示画进 X11 候选窗，在 Hyprland 上那是一个抢焦点的新窗口，聚焦输入框时提示反复弹出。两处要一起改：

- 设 `i18n.inputMethod.ibus.waylandFrontend = true;`，不再导出这两个变量（IBus 也要求 Wayland 方式下不设），改后重新登录。
- 以 `ibus start --type wayland` 启动（IBus 已在运行时用 `ibus restart --type wayland`），由 `ibus-ui-gtk3 --enable-wayland-im` 经 `zwp_input_method_v2` 接入混成器。每次登录都这样启动，需要在混成器的启动项里执行它，并用 `~/.config/autostart/ibus-daemon.desktop`（写 `Hidden=true`）盖住系统的那一条。

| 选项 | 默认 | 作用 |
|---|---|---|
| `programs.msime.services.online.enable` | 开 | 按 socket 激活的在线候选与翻译服务 |
| `programs.msime.services.voice.enable` | 开 | 按 socket 激活的语音输入服务，本地与云端识别都经过它 |
| `programs.msime.services.clipboard.enable` | 开 | 剪贴板历史监视器，只在偏好里开启剪贴板历史时才采集 |
| `programs.msime.package` | `msime-fcitx5`，用 IBus 时 `msime-ibus` | 换成 `override` 过的包，见下 |

三个服务的默认与 `msime-linux-setup` 首次配置时为用户启用的一致。包默认带着设置窗口、离线手写模型和本地语音识别的运行库，与各发行版的包相同；不要哪一样就在 `package` 里去掉它（经 overlay 取包，仍按本系统的 nixpkgs 构建）：

```nix
programs.msime.package = (pkgs.extend inputs.msime.overlays.default).msime-fcitx5.override {
  settingsWindow = null;     # 不带设置窗口；插件菜单里打开设置和各个面板的项随之失效
  handwritingModel = null;   # 不带离线手写模型
  voiceRuntime = null;       # 不带本地语音识别，云端识别不受影响
};
```

用 IBus 时把上面的 `msime-fcitx5` 换成 `msime-ibus`；`type` 是 `fcitx5` 时却给了 `msime-ibus`，求值会以断言失败。

设置窗口的前端用 nixpkgs 的 `pnpm_11` 和 `nodejs_24` 构建。系统的 nixpkgs 较旧、还没有它们时，包不带设置窗口，求值时给出一条警告，其余部分照常可用。

录音、提示音和静音用的音频工具不随包，用系统的音频栈（例如 `services.pipewire`）。

**首次使用。** 切换配置并重新登录后，运行 `msime-linux-setup --download`（见「安装后首次使用」），也可以打开设置窗口在首次配置页里完成；它会把水杉输入法加进当前的 Fcitx5 输入法组，用 IBus 时加进输入源列表（GNOME 的输入源，或 IBus 的预载引擎）。包不带词库，与 `.deb` 一致：词库下载到 `$XDG_DATA_HOME/msime-client/resources`，配置里记录的也是这个用户目录。本地语音识别的模型在设置窗口的语音页下载。

**每次切换配置之后**，要让 Fcitx5 从新的会话环境启动：注销后重新登录，或在新开的终端里执行 `fcitx5 -rd`。从 Fcitx5 内部重启（托盘菜单的「重新启动」、`fcitx5-configtool`）沿用旧进程的环境，加载的仍是上一次构建的插件；新旧版本的词库不一致时，表现是能切到水杉输入法但打字没有候选。可以用 `grep msime-fcitx5 /proc/$(pgrep -x fcitx5)/maps` 核对正在运行的插件是否来自当前系统（`readlink -f /run/current-system/sw/bin/fcitx5` 所在的那份 `fcitx5-with-addons`）。IBus 同理：正在运行的 `ibus-daemon` 只读它自己那份 `ibus-with-plugins` 里的组件，切换配置后要重新登录，引擎才会换成新构建的宿主。

**从按路径启用的单元迁移。** 以前用 `systemctl --user enable /nix/store/…/msime-linux-online.socket` 之类启用过这些单元的话，`~/.config/systemd/user` 里会留着指向旧 store 路径的链接，它们优先于模块注册的单元，旧路径被垃圾回收后单元就加载不了。换到模块后执行一次 `systemctl --user disable msime-linux-online.socket msime-linux-voice.socket msime-linux-clipboard.service`（会提示这些单元仍在全局范围启用，即由模块拉起）和 `systemctl --user daemon-reload`，再用 `systemctl --user show -p FragmentPath <单元>` 确认它们来自 `/etc/systemd/user`。

**不用模块**、经 `overlays.default` 自己写配置时，把 `pkgs.msime-fcitx5` 加进 `i18n.inputMethod.fcitx5.addons` 和 `environment.systemPackages`，并加上 `systemd.packages = [ pkgs.msime-fcitx5 ];`，否则 provider 单元不会注册到 systemd。只用 IBus 时换成不带 Fcitx5 插件的 `pkgs.msime-ibus`，并把 `pkgs.msime-ibus.ibusEngine`（不是整个包）加进 `i18n.inputMethod.ibus.engines`：

```nix
i18n.inputMethod = { enable = true; type = "ibus"; ibus.engines = [ pkgs.msime-ibus.ibusEngine ]; };
environment.systemPackages = [ pkgs.msime-ibus ];
systemd.packages = [ pkgs.msime-ibus ];
```

### 包管理器

除了发布页上的 `.deb`/`.rpm` 和上面的 Nix，仓库还维护这几个发行版仓库的包定义，都在 `platforms/linux/packaging/` 下，都只打完整版（full），内容与发布页的 `msime-linux` 包基本相同：Fcitx5 插件、IBus engine、`msime-linux-setup`、provider 服务、`msime-mcp`、设置窗口、语音运行库、离线释义与粤语/注音/笔画词库；唯一的区别是这些发行版包随包带手写模型（构建时离线，模型在 vendor 包里），发布页的 deb/rpm 不带，由设置应用按需下载；主词库同样不随包，装完每个用户运行一次 `msime-linux-setup --download`。

| 仓库 | 定义 | 构建方式 |
| --- | --- | --- |
| Arch AUR `msime` | `arch/msime/PKGBUILD` | 从 `linux-v<版本>` 标签的源码构建，Rust 经 `rustup` 用 `rust-toolchain.toml` 钉住的版本，资源由 `scripts/fetch_*.py` 按锁文件下载 |
| Arch AUR `msime-bin` | `arch/msime-bin/PKGBUILD` | 把发布页的 `.rpm` 改成 Arch 的目录布局重新打包，不编译 |
| Fedora COPR、openSUSE OBS | `rpm/msime.spec` | 离线源码构建，依赖全部来自 `msime-<版本>-vendor.tar.xz`，编译器用发行版自己的 rust（≥ 1.90，锁定依赖里最高的 `rust-version`） |
| Debian/Ubuntu（Launchpad PPA、OBS） | `debian/` | 同上，vendor 包作为 `orig-vendor` 组件 tarball |
| Gentoo overlay | `gentoo/`（`msime-9999.ebuild` 与 `msime.ebuild.in`） | crate 由 `pycargoebuild` 逐个列进 `SRC_URI`，资源按锁文件地址列出，前端取 `msime-<版本>-frontend.tar.xz` |

完整版的包（发布页的 `.deb`/`.rpm` 和上面这些发行版的包）把 `msime-mcp` 装进 `/usr/bin`，另装一个指向它的符号链接 `msime`，供手动测试输入法：`msime expand nihao` 按当前方案每行输出一个候选，`msime config` 列出当前偏好，`msime config set scheme=shuangpin` 修改偏好，`msime --help` 列出全部命令；其他版本装在 `/opt/msime-linux-<id>` 下，不带 `msime`。Nix 包目前不带 `msime-mcp`。

所有定义的构建步骤都照搬 `package-container.sh`（同样的 cargo 目标、同样的 `-DMSIME_*` 选项、`-DMSIME_EDITION=full`；唯一的有意差别是发行版传 `MSIME_HANDWRITING_MODEL_DIR` 随包带手写模型，`package-container.sh` 传 `-DMSIME_BUNDLE_HANDWRITING_MODEL=OFF`），跑与门禁相同的 ctest，装完核对插件按 RUNPATH 找到的是本包里的 Host API；包描述和主页 `https://github.com/metasequoiaime/msime` 在各定义里一致；许可证除了项目自己的 `GPL-3.0-only`，还列出随包的第三方代码与数据（静态链接的 Rust crate 与 npm 包、sherpa-onnx 与 ONNX Runtime、手写模型、方言词库、离线释义等，涉及 Apache-2.0、MIT、LGPL、CC-BY-4.0、CC-BY-SA-4.0 等），AUR 的 `license` 与 RPM 的 `License` 是同一份 SPDX 清单，Gentoo 用它自己的许可证名，Debian 写在 `debian/copyright`。资源的哈希只记在 `resources/*.lock.json`，各定义不另抄：构建时由同一批 fetch 脚本核对。`scripts/test-linux-distro-packaging.py` 核对五份定义传给 CMake 的选项与 `package-container.sh` 相同（手写模型那一项除外），`scripts/test-arch-gentoo-packaging.py` 核对 AUR 与 Gentoo 的维护脚本、单元列表、`.SRCINFO` 与 Rust 版本，两者都由 `scripts/run-checks.sh` 自动运行。各目录的 README（`arch/README.md`、`gentoo/README.md`、`debian/README.source`）写了更细的取舍，`arch/check-in-container.sh` 与 `gentoo/check-in-container.sh` 在容器里做完整构建或检查。

**每次发布自动产出、不自动发布。** `release-linux.yml` 的 `distro-sources` job 用 `packaging/make-source-tarballs.sh` 生成上面三个 tarball 和只列它们的 `msime-<版本>-SHA256SUMS`，main 上的发布由 `packaging/publish-sources.sh` 在版本发布页上线之前把它们传到滚动的 `linux-sources` release（预发布、不设为 latest、每个版本一组、长期保留，上传后按 GitHub 记录的摘要核对），不放在版本发布页上；beta 与 alpha 的只留在 workflow artifact 里。发布之后 `package-definitions` job 在同一提交上运行 `packaging/render-definitions.sh <版本> <目录>`，在各发行版的官方容器里渲染出 `rpm/`（`msime.spec`、`msime-rpmlintrc`、`msime-<版本>-1.src.rpm`）、`debian/`（`.dsc` 与 `.debian.tar.xz`）、`arch/`（两个包各自的 `PKGBUILD`、`.SRCINFO`、`msime.install`）和 `gentoo/`（带 Manifest 的完整 overlay），作为构建产物 `msime-package-definitions-linux-<版本>` 上传。版本号只来自 `platforms/linux/version.txt`（或手动触发时填的版本），`.rpm` 的校验值只来自版本发布页的 `SHA256SUMS`，三个 tarball 的只来自 `linux-sources` 上的 `msime-<版本>-SHA256SUMS`；COPR、OBS 与 PPA 的 workflow 用 `packaging/download-sources.sh` 取 tarball，`linux-sources` 上没有这个版本（0.12.1 及更早）时退回它自己的发布页；AUR 的 `msime` 与 Gentoo 的版本 ebuild 还要 GitHub 为 `linux-v<版本>` 标签生成的源码归档，它不是发布资产、不在 `SHA256SUMS` 里，校验值由 `arch/render.py` 与 `ebuild manifest` 下载后现算，GitHub 改变归档的生成方式时这两个包的校验会失败。本地也可以对任何一个已发布的版本跑同一个脚本（需要 docker），`MSIME_DEFINITIONS=arch,gentoo` 只渲染其中几部分。

**上架之后的安装方式：**

```sh
yay -S msime            # 或 yay -S msime-bin
sudo dnf copr enable <owner>/msime && sudo dnf install msime
sudo zypper addrepo https://download.opensuse.org/repositories/home:/<user>/openSUSE_Tumbleweed/home:<user>.repo && sudo zypper install msime
sudo add-apt-repository ppa:<owner>/msime && sudo apt install msime
sudo eselect repository add msime git https://github.com/metasequoiaime/gentoo-overlay.git && sudo emaint sync -r msime
echo 'app-i18n/msime ~amd64' | sudo tee /etc/portage/package.accept_keywords/msime   # arm64 上写 ~arm64
sudo emerge app-i18n/msime
```

Gentoo 的版本 ebuild 只有 `~amd64 ~arm64` 关键字，稳定分支的系统要先像上面那样放行，否则 Portage 报 `masked by: ~amd64 keyword`。

发行版仓库里的包名是 `msime`；它与发布页的 `msime-linux` 文件完全重合，RPM 以 `Provides`/`Obsoletes`、Debian 以 `Conflicts`/`Replaces` 替换掉后者，AUR 的 `msime` 与 `msime-bin` 也互相冲突。被替换的包按卸载处理：它的卸载脚本对每个已登录用户停用 MSIME 的用户单元并运行 `msime-linux-setup --unregister`，输入法从 IBus 与 Fcitx5 的列表里消失。所以新包在替换完成后替每个已登录用户以临时单元运行 `msime-linux-setup --register` 恢复两者：RPM 在 `%triggerpostun -- msime-linux`（msime-linux 移除之后），Debian 在本包每次安装（不含升级）后的 postinst（`debian/postinst-register`，由 `debian/msime.preinst` 留下的标记区分安装与升级：从 config-files 状态重装时 postinst 拿到的参数和升级一样），AUR 在 `post_install`。Debian 与 AUR 分不出这次安装替换了谁，所以卸载时保留了配置、之后重新安装的用户同样被恢复（包括 `apt remove msime` 后停在 config-files 状态再装回来）；没配置过的用户什么也不会发生。替换时没有登录的用户不受影响，被替换的包的卸载脚本也够不到他们；systemd 用户实例连不上的用户会在包管理器的输出里看到提示，登录后自己运行一次 `msime-linux-setup --register`。其他版本（`msime-linux-<id>`）装在 `/opt` 下，与它不冲突。

#### 发布到各仓库

仓库不向任何外部仓库推送，下面每一步都由维护者手动执行。以版本 `V` 为例，先从 `release-linux.yml` 那次运行下载构建产物 `msime-package-definitions-linux-V` 并解开到 `defs/`。

**AUR**（先在 aur.archlinux.org 注册 `msime` 与 `msime-bin` 两个包并上传 SSH 公钥）：

```sh
git clone ssh://aur@aur.archlinux.org/msime.git aur-msime
cp defs/arch/msime/{PKGBUILD,.SRCINFO,msime.install} aur-msime/
cd aur-msime && git add PKGBUILD .SRCINFO msime.install && git commit -m "Update to V" && git push origin master
```

`msime-bin` 同理，仓库换成 `ssh://aur@aur.archlinux.org/msime-bin.git`，文件取 `defs/arch/msime-bin/`。再把 `defs/arch/` 下的两个目录拷回 `platforms/linux/packaging/arch/`，经普通 PR 合入 `develop`，让仓库里的副本与 AUR 一致。

**Fedora COPR**：项目 `msime/msime`（<https://copr.fedorainfracloud.org/coprs/msime/msime/>），chroot 为 Fedora 43/44 的 x86_64 与 aarch64，构建不联网。Fedora 用户执行 `sudo dnf copr enable msime/msime && sudo dnf install msime`。Release Linux 在发布页生成后自动调用 `.github/workflows/publish-linux-copr.yml`：从 `linux-vV` 的 spec 打出 SRPM，`copr-cli build` 提交并等待构建结束，失败会显示在这次运行上（需要仓库 secret `COPR_CONFIG`，即 <https://copr.fedorainfracloud.org/api/> 给出的整段 `~/.config/copr`；缺少时跳过并告警）。COPR 的 API token 有效期 180 天，过期时工作流报错并指向重新生成的页面，剩不到 30 天时告警。补发或重建某个版本用 `gh workflow run publish-linux-copr.yml -f version=V`。新增 Fedora 版本时在项目设置里勾选对应 chroot，或用 `copr-cli modify msime --chroot <每个要保留和新增的 chroot>`（这个选项给的是完整列表）。

**openSUSE OBS** 是官方的发行版仓库：项目 `home:msime`（<https://build.opensuse.org/project/show/home:msime>），同时构建 RPM 与 Debian 包，目前覆盖 Fedora 43/44（x86_64、aarch64）、openSUSE Tumbleweed、Ubuntu 24.04/26.04、Debian testing/unstable 与 Arch Linux（含 Omarchy 等衍生版）（x86_64：OBS 的 Arch 基础仓库 `Arch:Extra` 只有 x86_64，Arch Linux ARM 不在 OBS 上）。Arch 仓库用 `arch/msime-bin` 的 PKGBUILD 重新打包发布页的 `.rpm`（OBS 构建不联网，`publish.sh` 把 PKGBUILD 列出的每个架构的 `.rpm` 原名放进包里，makepkg 找到同名文件就不下载；虽然只构建 x86_64，OBS 构建完还会用 `makepkg --allsource` 打源码包，要取齐全部架构的 source，0.11.0 起的 PKGBUILD 有 `source_aarch64`，缺 aarch64 的那个就会失败），用户把 `[home_msime_Arch]` 加进 `/etc/pacman.conf` 后 `pacman -S msime-bin`。aarch64 的 Arch（Arch Linux ARM、Asahi）没有这个仓库，从 AUR 装 `msime-bin`（它按架构取发布页的 x86_64 或 aarch64 `.rpm`）或从源码构建的 `msime`；OBS 自己解析 PKGBUILD 求依赖，不认数组里的注释，所以两个 PKGBUILD 的数组里不写注释。用户经 `curl -fsSL https://msime.app/install.sh | sh` 安装（脚本在 msime-web 仓库的 `public/install.sh`），它按发行版添加 `https://download.opensuse.org/repositories/home:/msime/<仓库>/` 和签名公钥，再用 dnf、zypper 或 apt 安装 `msime`；之后的升级随系统更新到来。Release Linux 在发布页生成后自动调用 `.github/workflows/publish-linux-obs.yml`，把这个版本的打包定义和源码包提交到 OBS（需要仓库 secret `OBS_USER`/`OBS_PASSWORD`，缺少时跳过并告警）；补发或重发某个版本用 `gh workflow run publish-linux-obs.yml -f version=V`。项目配置在 `packaging/obs/`：`repositories.txt` 列出构建哪些仓库（Ubuntu 要显式列出 `universe-update`、`update`、`universe`、`standard` 四个源，rustc 1.91 只在 `universe-update` 里），`prjconf` 固定 Ubuntu 上 cargo/rustc 的候选包与 Debian 上 libselinux 的提供者，`_constraints` 要求 8 GB 内存与 40 GB 磁盘，`publish.sh VERSION DEFS RELEASE` 据此写入项目 meta 并提交，本地运行时用 `osc` 当前登录的账号。Debian 12/13、Ubuntu 22.04 和 openSUSE Leap 不在其中：前两者的 rustc 低于 1.90，Ubuntu 22.04 的 Fcitx5 低于 5.0.20，Leap 不在 OBS 的发行版列表里（见 `repositories.txt` 的注释）。

**Launchpad PPA**：`ppa:msime/ppa`（<https://launchpad.net/~msime/+archive/ubuntu/ppa>），目前上传 Ubuntu 24.04（noble）与 26.04（resolute）。Ubuntu 用户执行 `sudo add-apt-repository ppa:msime/ppa && sudo apt install msime`。Launchpad 只收签名的源码上传，每个代号要单独的 `debian/changelog`；Release Linux 在发布页生成后自动调用 `.github/workflows/publish-linux-ppa.yml`，按 `debian/README.source` 的「上传到 PPA」为每个代号生成 `1~ppa1~ubuntu<版本>` 的源码包并签名上传（第一个代号带上两个 orig tarball，之后的复用），给 noble 的那份把 Build-Depends 的 cargo、rustc 直接换成 cargo-1.91、rustc-1.91，因为 Launchpad 只取第一个候选。签名密钥是仓库 secret `LAUNCHPAD_GPG_KEY`（`MSIME Release Signing <admin@msime.app>`，指纹 `B9C96EE4B38BF864233BFA1CF25F85719A49653E`，2028-10-04 到期，公钥在 keyserver.ubuntu.com 并登记在 Launchpad 账号 msime 上）；缺少时跳过并告警。Launchpad 不接受同一版本号重传，重新上传已被接受的版本用 `gh workflow run publish-linux-ppa.yml -f version=V -f ppa_revision=2`。构建在 Launchpad 上进行，结果看 PPA 的 +packages 页。Ubuntu 22.04 的 Fcitx5 过旧、Debian 13（rustc 1.85）不能作为目标。

**Gentoo**（建议单独的 overlay 仓库，例如 `metasequoiaime/gentoo-overlay`）：

```sh
git clone git@github.com:metasequoiaime/gentoo-overlay.git
cp -r defs/gentoo/. gentoo-overlay/
cd gentoo-overlay && git add -A && git commit -m "app-i18n/msime: add V" && git push
```

`defs/gentoo/` 已经是带 Manifest、通过 `pkgcheck scan --exit error` 的完整 overlay；版本 ebuild 只有 `~amd64 ~arm64` 关键字。

## 多版本（水杉拼音、水杉五笔、水杉日语、水杉越南语、水杉藏文）

除了现有的水杉输入法（full），版本表 `shared/contracts/editions.json` 还定义了水杉拼音（`pinyin`，全拼与双拼，带临时日文）、水杉五笔（`wubi`，只有五笔，混拼默认打开，不带临时日文），以及只有一个方案的水杉日语（`japanese`）、水杉越南语（`vietnamese`）和水杉藏文（`tibetan`）。三个语言版本只带英文词库及其 SCOWL 许可声明、`msime-others.db` 和清单（日文版另带日文词典和两份 Mozc 许可文本），不带中文主词库、n-gram 和落定重排模型；它们也没有手写和非英文离线释义（版本表 `features.handwriting`、`features.offline_glosses` 为 false），安装包里没有 `handwriting-zh_CN.model` 和 `offline-glosses/`，IBus 与 Fcitx5 菜单里没有「手写输入…」。几个版本是各自独立的包，可以同时安装、同时启用，互不覆盖：

- full 仍是今天的 `msime-linux`，装在 `/usr` 下，包结构、文件、IBus 引擎 `msime-linux`、Fcitx5 条目 `msime`、状态目录 `~/.config/msime-client` 和用户服务都与引入版本之前相同；
- 其他版本是 `msime-linux-<id>`，程序、词库、宿主库和文档整个装在 `/opt/msime-linux-<id>` 下；系统按名字查找的那几样装到系统目录，名字都带版本：IBus 组件 `/usr/share/ibus/component/msime-linux-<id>.xml`（引擎 `msime-linux-<id>`）、Fcitx5 插件 `libmsime-<id>-fcitx5.so` 与条目 `msime-<id>`、systemd 用户单元 `msime-linux-<id>-{online,voice}.socket` 等、桌面入口与自启动项、图标 `msime-linux-<id>`（与 full 同一张图），以及 `/usr/bin/msime-linux-<id>-setup` 和 `/usr/bin/msime-linux-<id>-settings`；
- 每个版本登记在默认方案所属的语言下：中文的版本是 IBus `zh`、Fcitx5 `zh_CN`，日文、越南文、藏文版是 `ja`、`vi`、`bo`（IBus 组件与引擎的 `<language>`、Fcitx5 输入法条目的 `LangCode`），GNOME 的输入源对话框和 Fcitx5 的配置工具把它们列在日语、越南语、藏语下；
- 每个用户的状态目录、运行时目录（`panel-input.sock`、`candidate-panel.json`、各 provider socket）、自行下载的词库和缓存都在 `msime-client-<id>` 下，宿主生成的 Fcitx5 主题、Omarchy 钩子与插件、使用统计目录和设置窗口的 Tauri identifier（`app.msime.linux.<id>`）也按版本分开；随装的词库锁是本版本的锁（`resources/editions/<id>.lock.json`，装成 `<前缀>/share/msime-client-<id>/desktop-dictionary.lock.json`），首次配置只取回本版本需要的词库。

装好之后运行该版本自己的首次配置命令，例如 `msime-linux-wubi-setup --download`；任何一个版本的 `msime-linux-setup` 也可以加 `--edition wubi`，它会转交给 `/usr/bin` 下那个版本的命令，那个版本没装时报错。卸载一个版本只停用、注销它自己的服务和输入法条目，其他版本不受影响。Fcitx5 下两个版本的插件被同一个 fcitx5 进程加载：插件以 `RTLD_LOCAL` 加载，非 full 的插件隐藏全部符号；宿主库在非 full 的版本里改名为 `libmsime_host_api_<id>.so`（full 仍是 `libmsime_host_api.so`），因为动态链接器按插件 DT_NEEDED 里的文件名复用已经加载的库，同名时两个插件会共用先加载的那个版本的宿主库，改名后各用各的那一份；每个插件注册的输入上下文属性也随插件名（full 仍是 `msimeState`），否则后加载的插件拿不到属性槽位；`crates/host-api` 的测试 `two_editions_with_their_own_state_roots_share_one_process` 另外确认，即使两份宿主库被合并成一份，两个状态目录的会话也互不影响。

构建某个版本时给 CMake 传 `-DMSIME_EDITION=<id>`（缺省 `full`），打包时前缀必须是该版本的前缀（`-DCMAKE_INSTALL_PREFIX=/opt/msime-linux-<id>`，`packaging.cmake` 检查）；换成别的前缀的开发安装把所有文件都留在前缀里，不碰系统目录。脚本和数据文件由 `scripts/edition_linux.py` 在配置阶段按版本改写，C++ 侧的名字来自它生成的 `src/core/LinuxEdition.h`；改了版本表之后运行 `python3 platforms/linux/scripts/edition_linux.py gen`。`scripts/test-linux-editions.py` 检查生成文件没有漂移、每个版本都能改写全部文件且不留下 full 的名字、各版本的 Fcitx5 动作名两两不撞、每个版本登记在正确的语言下。`package-container.sh` 缺省按版本表给每个版本各打一个包（`MSIME_PACKAGE_EDITIONS=full,wubi` 可以只打其中几个），单测只在 full 的构建上跑；发布流程逐个版本核对包名与版本，再用 `tests/tools/check-deb-coexistence.sh` 和 `tests/tools/check-rpm-install.sh` 把全部版本一起装进干净容器、逐个卸载，确认卸载一个不带走另一个的文件。

## 卸载 CMake 安装

保留执行安装的构建目录，可用 `cmake --build <build-dir> --target uninstall` 删除该构建的 `install_manifest.txt` 中记录的程序、资源和桌面入口。卸载前先切换到其他输入法并关闭 MSIME 面板。执行卸载所需权限与原安装相同。对应 Windows 卸载时停止输入法进程并删除登录任务：以普通用户身份卸载时，删除文件前会在当前用户的 systemd 用户实例里逐个 `disable --now` `msime-linux-setup` 启用过的在线、语音和剪贴板服务，免得它们指着已删除的程序反复重启；以 root 或带 `DESTDIR` 卸载时够不到各用户的实例，只打印每个用户需要执行的那条 `systemctl --user disable --now …` 命令。卸载不停止输入法宿主：正在运行的 IBus 宿主继续运行到 `ibus restart` 或注销，不会因程序被删而自行重启；Fcitx5 在下次激活 MSIME 时提示执行 `fcitx5 -r` 或注销后重新登录。

暂存安装使用相同的 `DESTDIR`，例如 `DESTDIR=/absolute/staging cmake --build <build-dir> --target uninstall`。若安装时用 `cmake --install` 的 `--prefix` 覆盖了配置前缀，使用 `cmake -DMSIME_UNINSTALL_PREFIX=/actual/prefix -P <build-dir>/uninstall.cmake`。程序文件必须位于该前缀内，唯一的例外是前缀为 `/usr` 时装到 `/etc/xdg/autostart/` 的剪贴板自启动项；前缀外的其他自定义绝对安装目录会使卸载在删除前中止，需要按原安装布局单独处理。

卸载时（`.deb` 的 prerm 对每个已登录用户、CMake 卸载对执行卸载的用户）在删除文件之前运行 `msime-linux-setup --unregister`，把水杉从 GNOME 输入源、IBus 预载引擎和当前 Fcitx5 输入法组中移除，对应 Windows 卸载程序注销 TSF profile；它两份 IBus 列表都检查，不看当前桌面，Fcitx5 没在该用户的会话总线上时不去唤起它；若移除会让列表变空则保持原样，已不在列表里时什么都不写，缺少 `gdbus`、`gsettings` 或会话总线时只说明原因，不会让卸载失败。环境里没有会话总线地址时（用户 systemd 实例未导入桌面环境）按 `$XDG_RUNTIME_DIR/bus` 补上。移除之后它还原宿主改过的桌面候选面板设置，对应 Windows 卸载程序删除水杉留下的全部状态：宿主第一次改写 Fcitx5 classicui 的 `Theme`、`DarkTheme`、`Font`、`WheelForPaging` 或 IBus 面板的 `custom-font`、`use-custom-font` 之前，先把原值和写入值记进 `$XDG_STATE_HOME/msime-client/panel-restore.json`（默认 `~/.local/state/`），之后每次写入更新写入值；若用户在两次写入之间自己改过，记下的原值换成用户的那个。卸载时仍等于水杉写入值的项恢复原值，用户之后改过的项保持不变。Fcitx5 在会话总线上时经 `org.fcitx.Fcitx.Controller1.SetConfig`（`fcitx://config/addon/classicui`）写回，由它自己生效并保存，调用失败或 Fcitx5 没在运行时直接编辑 `~/.config/fcitx5/conf/classicui.conf`；IBus 用 `gsettings set`，原本从未设置过的键用 `gsettings reset` 回到默认值。记录的原值可能是用户原来的自带主题或第三方主题（用户主动选了水杉主题、从第三方主题手里接管时），只把「原值已经是水杉自己的主题」这一种（早先的版本写入时没有记录）记成 `default`、`default-dark`，所以记录里放回的恢复值从不是水杉自己的主题；卸载时仍指向 `msime` 的这两项，无论有没有记录，也一并改回 `default`、`default-dark`，随后删除 `~/.local/share/fcitx5/themes/msime`；但只在这个目录仍是宿主生成的原样时才这样做：`theme.conf` 的 `[Metadata]` 仍是 `Description=Generated from the MSIME candidate settings…`，目录里也只有 `theme.conf` 和宿主写的 `shape-`、`decoration-`、`background-`、`canvas-` 图片（及写入中断留下的同名临时文件）。改过这行说明、放进别的文件或子目录、目录本身是符号链接时，按用户维护的主题处理：目录原样保留，仍指向它的 `Theme`／`DarkTheme` 也不动，其他设置照常恢复。卸载脚本在用户的 systemd 实例里运行，看不到 Fcitx5 进程的 `MSIME_FCITX5_THEME_AUTOGENERATE`，所以只能看目录内容判断；AUR 的 `msime` 与 `msime-bin` 互换、`.deb` 被同名包替换、RPM 的 Obsoletes 都会经过这一步，之后的 `--register` 也不会再改主题设置。最后删除记录文件；记录在宿主共用的 `panel-restore.json.lock` 锁下删除，且只在它仍是读出的那一份时删除，恢复期间仍在运行的宿主又记下的改动留给下一次 `--unregister`，锁文件本身保留，保证所有写入者锁的是同一个文件。某项没能恢复（例如缺少 `gsettings`）时记录保留，稍后可再运行一次 `--unregister`。这一步由 `tests/core/setup_panel_restore.py` 钉住。无法连到某个用户的会话时（以 root 或带 `DESTDIR` 的 CMake 卸载、联系不上的用户实例），打印该用户需要手动移除的项：桌面输入源里的「Metasequoia 水杉输入法」（IBus），或 `fcitx5-configtool` 当前输入法组里的「MSIME」（Fcitx5）——程序随即被删除，事后已无法再执行 `--unregister`。

卸载保留 `msime-client/runtime-options.json`、用户配置及学习数据，不递归删除目录，也不停止用户服务以外的进程。这一点与 Windows 不同：Windows 卸载程序删除它自己管理的数据目录，而 CMake 卸载无法得知哪些用户用这份安装准备过状态，所以不碰任何人的数据；需要清除时手动删除各自的 `$XDG_CONFIG_HOME/msime-client`。通过发行版包管理器安装的文件应由原包管理器卸载。

本目录只处理 IBus 系统入口，使用同一个 `msime-host-api` 动态库；不直接创建 Rust 引擎 `crates/engine` 的会话、不复制候选分页、数字选词或配置持久化逻辑，不依赖 Tauri 常驻。按键与焦点在 GLib 主线程调用线程绑定会话，系统候选点击取当前共享视图中的代次和全局索引。预编辑采用 Engine 的 ASCII editing_text，避免把字节光标用于中文显示串。失焦、禁用和 reset 清除组合；修饰键与 key-up 透传，快捷键取消组合后透传。密码、PIN、数字与电话字段不处理输入，private/no-spellcheck 文本会话关闭学习。

共享运行时只返回当前候选页；IBus lookup table 显示该页，auxiliary text 标示共享页码。宿主读取共享 navigation 设置，支持减号/等号、逗号/句号、方括号、Tab/Shift+Tab、PageUp/Down 翻页及上下候选移动，也支持小键盘导航键。启用“鼠标滚轮”后，IBus 面板上的滚轮（面板发出的 `cursor_up/down` 信号和候选点击的按钮 4/5）分别翻到上一页/下一页；关闭时滚轮不做任何事，与 Windows 一致。键盘上下键走按键路径，不经过这两个信号。Fcitx5 classic UI 自己把滚轮换成翻页请求，与翻页按钮走同一个接口，宿主无法区分，所以这个开关写进 classicui 自己的 `WheelForPaging` 配置；它属于整个桌面，与候选字体同一条规则：设置仍是默认值时不写，之后每次修改写一次，卸载时恢复接管前的值（见「卸载 CMake 安装」）。X11 上默认的原生画布自己处理滚轮，但翻不翻页照同一个 `WheelForPaging`（每次主题同步时从 `conf/classicui.conf` 读，缺省为开），所以换成原生画布后滚轮与原先 classicui 画候选窗时一样：水杉的开关动过就照它，从没动过就沿用桌面原有设置。设置通过验证后立即更新按键分派，不等待 Engine 组合结束；按键路径不读文件。按当前键盘布局的字符映射，Shift 符号不当作未按 Shift 的物理键，保留 Unicode `U+`。关闭的标点绑定交回 Engine，关闭的 Tab/Page/上下键先完成组合再交还编辑器；空闲时透传。IBus“候选操作”菜单在候选页存在时提供上一页/下一页，动作仍调用共享分页命令并受已渲染 session/generation 栅栏保护；不把当前页伪装成完整候选集自行分页。

智能标点使用 IBus 提供的 surrounding text：逗号、句号和冒号前若是 ASCII 字母或数字，则保留 ASCII；否则交由 Engine 的中文标点表转换。正在组合时优先使用当前高亮候选的末字符，候选提交和标点在同一运行时转换中完成。重复输入的 ASCII 标点在短时间内可按设置替换为中文标点；失焦、删除或其他编辑动作会使该状态失效。无法取得有效 surrounding text 时按中文标点处理，不读取或记录完整编辑器内容。

`smart_punctuation_space_convert` 现在也在 IBus 生效：Engine 刚单独上屏 `。，！？；：、`、单双引号的一侧、方括号、书名号或圆括号，且当前没有组合时，紧接着的一次无修饰空格把该标点改回半角 ASCII 并消费空格；全角输出模式不改变这次手势的替换结果。自动补全的成对标点不会武装改写，避免只替换左半边后留下混合对子。判据是文档指纹而不是计时器——同一个标点在文档里通常不止一处，窗口内移动光标又不是焦点变化，所以上屏时记下标点前面的那个字符，改写前回读两个字符核对：标点必须仍是刚上屏的那个，且仍跟在同一个字符后面。上屏时取不到指纹（标点开在文档首，或宿主没有可读的 surrounding text）按匹配处理，与来源一致；其间任何其他按键都会解除该状态。完整映射和指纹判定在 `src/core/SmartPunctuationSpace.h` 由两个宿主共用并带纯 C++ 单测（`linux-smart-punctuation-space`）。

设置页的「数字后直出」「字母后直出」两个智能标点开关现在在 IBus 上也生效：IBus 自己判断标点前面的字符，此前一律按字母和数字都保留 ASCII 处理，关掉任何一个都没有效果；Fcitx5 走共享路由，本来就遵守这两个开关。

Fcitx5 现在也走同一条路径：它此前只在状态栏带着智能标点和重复标点两个开关，却没有任何改写实现，`smart_punctuation_space_convert` 在这个宿主上同样什么都不做。现改为共用 `SmartPunctuationSpace.h` 的指纹判定、直接智能按键集合与完整空格改写映射，通过 `InputContext::deleteSurroundingText` 原位替换并消费成功手势的空格，沿用同样的解除点（任何其他按键、会话关闭）。ASCII→中文的三键重复转换与中文→ASCII 的完整空格转换明确分开，避免两个不同产品规则再次漂移。

Fcitx5 的「重复标点回切中文」同样从只有开关变为真的生效：智能标点把某个标点保留为 ASCII 后，两秒内再按同一个键会通过 `deleteSurroundingText` 换成中文标点，该按键被消费、不再交给 Engine。与 Windows/IBus 一致，退格删掉那个 ASCII 标点后重新按同一个键会走中文路径——Fcitx5 的这个判断在共享 Rust 路由里做，因此实现方式是不再把前一个字符交给路由，让它按「前面不是 ASCII 字母数字」处理。上屏识别用共享的 `ascii_mark_from_text`，按 Engine 实际输出的宽度匹配，宿主不另留一份按键清单；没有组合时路由把保留的 ASCII 标点交还编辑器，没有上屏可供识别，宿主在调用路由前按同样的条件（跟随锁定、中文与智能标点开启、前一个字符是 ASCII 字母或数字、无组合）判定并记下这个键。

IBus 属性菜单和 Fcitx5 状态栏切换「智能标点」时，与设置页一样连同「数字后直出」「字母后直出」一起写成同一个值（`src/core/SmartPunctuationChoice.h`），Fcitx5 的三个键在同一次保存里落盘。只写总开关的话，设置页关过一次之后两半已是假，从菜单再打开的只是一个不起作用的总开关。

Fcitx5 下 `Ctrl+Shift+Alt+R`、状态菜单「重载输入法服务」和设置页「重启」按钮都会重置水杉插件：结束所有输入上下文的组合、关闭 Engine 会话并重新读取 `runtime-options.json`，当前焦点立即获得新会话；不重启 Fcitx5 进程，其他输入法不受影响。快捷键和菜单只在输入上下文仍有焦点、不是受限或隐私字段时生效，重置在插件内直接完成，不启动任何外部程序；快捷键的重复按下和对应抬键在一次手势内由插件吞掉，不会把 `R` 泄漏给编辑器。设置页按钮经 `gdbus` 调用 Fcitx5 controller 的 `ReloadAddonConfig`（参数 `msime`）触发同一重置；`fcitx5-remote -r` 发送的 `ReloadConfig` 只重载 Fcitx5 的全局配置、不会传到插件，所以不用它。重置不会换到升级后的新程序，那仍需 `fcitx5 -r` 或重新登录。

候选快照同时携带 Engine 的来源编号，并与候选顺序绑定；GTK/Qt 或其他 Linux panel 可以据此区分词库、英文、Emoji、颜文字及在线候选，不需要从显示文本反推来源。来源只用于展示和交互提示，不改变候选身份、分页或提交文本。

候选注释也由 Engine 快照按候选顺序提供：启用帮助码时使用当前方案和帮助码表生成，无法生成时保留纠错提示。Linux 宿主只显示该注释，不重复实现帮助码计算。

可选的 `online_provider_socket` 顶层启动配置指定用户管理的绝对 Unix socket。宿主复制在线查询后在 GLib worker 中请求该服务，再通过 Host API 的代次校验回填候选；未配置时不发起在线请求。请求带有 `kind:"online"`，启用 AI 联想时还携带已校验的 provider、model、候选数量和提示词配置，但不携带 token；socket 服务负责凭据、网络和 provider 策略。独立入口 `msime-linux-online /absolute/provider.sock` 从标准输入读取同一 OnlineQuery JSON 并输出受界限的 JSON 响应，供 GTK/Qt 面板或其他 Linux 宿主复用。也可用 `translation_provider_socket` 或 `MSIME_TRANSLATION_PROVIDER_SOCKET` 指定独立的候选翻译服务；未指定时翻译继续复用在线 socket。`MSIME_ONLINE_PROVIDER_SOCKET` 可作为在线 socket 的环境变量回退。

候选翻译服务选择「水杉账号」时，Linux provider 只在这个选项被保存且没有其他可用翻译服务时问账号释义：它经 host 库的 `msime_client_account` 发 `translate`，会话选择（设置应用登录的账号优先，没有登录时用本机匿名账号）、令牌刷新、`/v1/translate` 请求和逐项规范化都在 Rust 里，provider 自己不取令牌、不发这个请求。两种会话都不可用时（Rust 回 `account_unauthorized`，例如安装时的注册没成功，或匿名会话被服务端吊销后已被清掉）provider 补注册一次匿名账号再问，同一个进程 10 分钟内最多补一次。安装包的 Debian `postinst` 会为每个可联系到的登录用户调用 provider 的 `--ensure-anonymous-account`，`msime-linux-setup` 也会在首次配置时执行同一步；它先生成本机匿名身份，再通过 `https://api.msime.app/v1/auth/challenges` 与 `/v1/auth/login` 换取翻译令牌。网络失败只会提示并留待下一次重试。身份和轮换后的令牌分别保存在 `$XDG_CONFIG_HOME/msime-client/anonymous-account.json` 与 `anonymous-session.json`，文件由当前用户独占读写（0600），不传给设置页或输入法进程。新装与恢复默认设置时默认选中「水杉账号」；已有配置里没有 `translation_account` 字段的按未选择处理。未选择「水杉账号」时不会发送候选翻译，但账号注册仍按安装流程完成。

当 JSON 和环境变量都没有指定 provider socket 时，宿主仅在 socket 已存在的前提下尝试 `$XDG_RUNTIME_DIR/msime-client/online.sock` 与 `voice.sock`；显式 JSON 路径和环境变量始终优先，不会自动启动服务或连接不存在的路径。

运行中的 IBus 会话会在配置文件热重载时同步读取新的在线、翻译和语音 provider socket；在线请求立即失效，正在使用旧语音服务的录音会被取消。未聚焦或尚未创建会话时，新焦点直接使用最新配置。

`preferences.cloud_candidates` 会随 OnlineQuery 传给 provider；关闭后宿主不发起仅云候选请求，并拒绝返回的云来源候选，但仍保留符合条件的 AI 联想。

IBus 属性菜单中的“云联想”在配置绝对共享偏好目录时按 revision 持久化，保存成功后使正在进行的 provider 请求失效；未配置目录的直接预览只覆盖当前会话。

IBus 属性菜单中的“显示译文”（候选翻译开关）在配置绝对共享偏好目录时按 revision 持久化，保存成功后使正在进行的请求失效；未配置目录时只覆盖当前会话，关闭后不会发起翻译 provider 请求。

“翻译目标语言”菜单可选择英语、法语、日语、西班牙语、俄语、德语或韩语；配置绝对共享偏好目录时按 revision 持久化，保存成功后使旧语言请求失效并按当前候选重新请求，未配置目录时只覆盖当前会话。

语音输入通过可选的 `voice_provider_socket` 顶层绝对 Unix socket 接入，也可用 `MSIME_VOICE_PROVIDER_SOCKET` 作为环境回退。IBus 属性中的“语音输入”首次点击启动录音，再次点击结束录音并等待识别结果上屏，与 Windows 托盘语音操作一致；识别和润色期间该属性不可重复操作，Esc 仍可取消当前语音代次。IBus 的语音输入与 Windows 一样不随中英文模式停用：英文模式下语音快捷键（包括本来会交给应用的单独 RAlt）、“语音输入”属性和 Esc 取消照常工作，录音途中无论用快捷键还是属性菜单切换中英文都不会取消录音。英文模式下通常没有 Engine 会话，这时录音代次由宿主自己编号（最高位置 1，与 Engine 代次不会相撞），识别结果不经 Engine 确认代次，直接按 provider 原文上屏，与 Windows 英文模式下语音不经过组字一致；繁体输出和全角设置仍照常套用，与中文模式上屏的文字相同。用户管理的 socket 服务收到 `{"version":1,"kind":"voice","query":{"language":"zh-cn","generation":1,"stream":true,"options":{"sound_enabled":true,"mute_system_audio":false,"polish_enabled":false,"stream_inline_preedit":true,"doubao_boosting_table_id":""}}}` 后负责 PipeWire/ALSA 录音、提示音、静音、ASR 凭据、网络和结果润色，并按行返回 `{"text":"中间结果","type":"partial","generation":1}` 以及最终的 `{"text":"识别结果","type":"final","generation":1}`；每个事件都必须带 `type` 和与请求相同的 `generation`，缺少任一项或 `generation` 不匹配的事件会被丢弃。取消时输入法另发 `{"version":1,"kind":"voice_cancel","query":{"generation":1}}`，provider 应停止对应录音并忽略后续结果；按住 RAlt、Ctrl+Win 或 RCtrl+RAlt 松开时则发送 `{"version":1,"kind":"voice_stop","query":{"generation":1}}`，provider 应停止录音并让原连接返回最终结果，Ctrl+F9 和再次点击 IBus“语音输入”属性也使用该完成路径。豆包流式识别且 `preferences.voice_input.stream_inline_preedit` 开启时，中间文本更新 IBus 预编辑；关闭该选项或使用其他识别服务时只在辅助区域显示实时转写并提交最终文本。Linux 上语音结果只经 IBus（或 Fcitx5）提交这一条路径，设置页不提供「结果提交策略」，偏好里存着的 `commit_mode` 在这里不起作用；设置应用里的独立语音面板同样把文本交给当前的输入法宿主，不按它改用粘贴。`options` 只包含非敏感行为配置（包括有长度上限的润色提示词和 Doubao boosting table ID），输入法不会转发 token、app key 或其他凭据；provider 可以忽略不支持的字段。结果回到 GLib 主线程后再次校验会话和代次；最终文本为空但已有有效中间转写时，输入法保留该转写提交，只有没有任何可提交文本的空结果、过期结果和取消结果才不会上屏。若 Engine 应用结果失败，宿主也会把同一有界文本直接提交到 IBus。每条响应文本最多 4096 字节，服务调用最长等待 730 秒（包含录音及识别），每 100ms 检查取消，整行响应最多 16 KiB。`preferences.voice_input.enabled` 和 `preferences.voice_input.language` 控制属性是否可用及识别语言。独立入口 `msime-linux-voice /absolute/provider.sock` 从标准输入读取同一查询 JSON 并输出受界限的 JSON 响应；加上 `--stream` 参数时按行输出 `partial`/`final` 事件，供 GTK/Qt 面板或其他 Linux 宿主复用，不在输入法进程内保存凭据或原始音频。

语音波形展示通过 `WaveOverlaySurface` 注入宿主，外观与交互对齐 macOS 的语音浮层（`platforms/macos/src/voice/VoiceWaveOverlay.mm`），几何、计时、波形运动和文案集中在 `src/overlay/WaveOverlayLayout.h`，X11 与 Wayland 两个后端共用 `src/overlay/WaveOverlayPainter.h` 的 Cairo/Pango 画法，只是像素去处不同。浮层按状态取四种尺寸（逻辑像素）：按住录音时是 78×32 的紧凑圆角条，只有波形；有实时转写时是 420×112 的卡片，上方一小段波形、下方 15 px 字号按 22 px 行距显示最新的三行，较早的文字收在开头的“…”后面，按字形簇边界截断，不会切开 emoji 或带附加符号的字母；空格锁定录音后，以及识别、润色进行中，是 142×40 的胶囊，两端各有一个中性色的圆形取消（×）和完成（✓）按钮，录音时中间是波形，识别中显示“识别中...”、润色中显示“处理中...”（14 px 字号）。普通按住录音、菜单或 Ctrl+F9 开始的录音都不显示按钮，与 macOS 相同。识别进行中点完成按钮只收起浮层，识别结果到达后照常上屏；点取消则放弃本次语音。面板为圆角（紧凑 10 px、卡片 16 px、胶囊取高度的一半），填充不透明度 0.90，带 1 px 描边；颜色取候选窗解析出的主题（内置主题是 Adwaita 风格的配色，皮肤用它自己的颜色），波形、文字和按钮符号都用主题的文字色（macOS 用同一个前景色画柱子和文字），没有主题描边时不画描边（内置配色用文字色的淡描边）。浮层显示期间宿主每 16 ms 推进一帧波形：12 根细胶囊按 macOS 的多谐波模型随麦克风电平起伏，中间高、两边低，安静时退回圆点；IBus 用 GLib 定时器，Fcitx5 用事件循环定时器并在每帧读取最新电平。宿主同时在自己的主循环上监听浮层显示连接的文件描述符，按钮点击和窗口重绘请求随到随处理，不再等下一次重绘。

Wayland 且系统提供 `wayland-client`、`wayland-scanner`、`xdg-shell` 协议文件和 `pangocairo` 时，宿主优先使用 `wlr-layer-shell` 底部居中的 overlay layer：尺寸随布局变化，exclusive zone 为 0，因此停在其他面板保留区之上 10 px 处（与 X11、macOS 的工作区底部之上 10 px 一致），按所在输出的整数缩放（`wl_output.scale`）绘制并设置 buffer scale，在 HiDPI 屏上不再被放大发虚；具体放在哪块输出仍由 compositor 决定。不支持该协议的 compositor 会回退到 IBus 辅助栏。X11 且同时提供 `x11`、`xfixes`、`xrandr`、`cairo-xlib` 和 `pangocairo` 开发模块时使用不抢焦点的原生浮层，按前台窗口所在显示器（取不到时用指针所在或主显示器，经 XRandR 枚举）的工作区底部之上 10 px 居中定位，尺寸按 `Xft.dpi`/`GDK_SCALE` 缩放，每次显示时重新计算；没有 XRandR 时回退到 EWMH 工作区或 root 屏幕尺寸并保留多屏负坐标。有合成管理器（`_NET_WM_CM_S<n>` 有属主）时用 32 位 ARGB visual 画出半透明圆角，没有时面板不透明，用 Shape 扩展裁成圆角；合成管理器在两次录音之间启停时，下次显示会按新状态重建窗口。浮层只有两个圆形按钮区域接收鼠标输入，其余区域保持输入透明；文字用系统的 Sans 字体经 Pango 排版，不再依赖 misc-fixed 点阵字体。连接失败也会回退到 IBus。Wayland 后端同样不请求键盘焦点、不占用工作区；没有可用指针设备时仍可通过 IBus 菜单完成动作。可用 `MSIME_WAVE_OVERLAY_BACKEND=wayland`、`x11` 或 `ibus` 请求指定后端。构建时缺少 `pangocairo`（或 X11 的 `cairo-xlib`）就不编译对应后端，语音状态改由辅助文字显示；这两项与 `x11`/`xfixes`/`xrandr` 分开检查，所以缺少它们时 X11 的 Caps Lock 撤回照样编进来。实时转写里的 NUL 等控制字符（换行和制表符除外）在进入浮层之前就被丢掉。

Fcitx5 对同一组流式语音设置采用同样语义：豆包流式识别且 `stream_inline_preedit=true` 时把 partial 写入原生预编辑，与存着的 `commit_mode` 无关；关闭开关或使用其他识别服务时只在辅助区域显示。最终响应为空但已经收到有效 partial 时，保留最后一份有界中间转写作为最终提交；完成、取消、失焦和会话关闭都会清除预编辑与缓存，避免旧代次重新出现。

Fcitx5 宿主复用同一套 X11/Wayland 原生浮层和取消、完成按钮；没有可用的显示后端、浮层无法显示（例如 GNOME Wayland 没有 `wlr-layer-shell`）或 `MSIME_WAVE_OVERLAY_BACKEND=ibus`（也接受 `auxiliary`）时，实时状态回退到 Fcitx5 输入面板上方的辅助文字。辅助文字与 IBus 回退时完全相同，由 `src/overlay/WaveOverlayFeedback.h` 生成：第一行是状态（“正在录音…”“识别中...”“处理中...”，空格锁定后是“录音已锁定 · 可松开快捷键 · 再按快捷键或点击语音菜单结束 · Esc 取消”）和录音时的十格麦克风电平，第二行是最近 160 个字符的转写，更早的文字以“…”表示。空格锁定在按下空格时立即反映到浮层或辅助文字上，不等下一次轮询。浮层显示失败后本次录音都走辅助文字，下次录音再尝试浮层，与 IBus 的回退一致。Fcitx5 的浮层同样不请求键盘焦点，完成、取消、异常和焦点关闭都会清理浮层；识别进行中再次点完成、再按快捷键或点语音菜单不会重复向语音服务发送结束请求。

语音失败时两个宿主都会告诉用户，对应 Windows 语音服务弹出的提示框：浮层（没有浮层时是辅助栏）显示固定的一句话 6 秒（与 macOS 相同），按 Esc、切换焦点或开始下一次录音会提前收起；浮层上的提示在继续打字时不收起，与 macOS 一致（IBus 以前每次重画候选都会把它收掉），只显示在辅助栏时则把那一行让给候选；浮层上的失败提示是一个按文字加宽的胶囊，只有这句话，没有波形和按钮，分别说明未识别到文字、语音服务或提供商出错、结束录音被拒（本次语音随之取消）以及没有配置语音服务；语音服务报告缺少 websockets 或录音工具时改为说明该装什么，报告本地识别组件（`msime-voice-local` 或 sherpa-onnx 运行时）缺失时提示“本地语音识别组件无法加载，请重新安装输入法”，因为重新录音无法解决。语音服务没有给出结果（包括无法连接或返回失败）算作服务出错，不会提示未识别到文字。文字是固定的，不透传 provider 的错误信息，以免其中带出凭据等私人内容；流式 C ABI 只在 `detail` 是 `websockets`、`recorder` 或 `local_asr` 时以 `voice_dependency_missing:<detail>` 错误返回，其他失败仍返回空值。Fcitx5 此前在这些情况下只是收起浮层，看上去像按键没有反应。Fcitx5 浮层也和 IBus 一样区分“识别中...”与“处理中...”两种收尾状态，并在按住空格锁定录音时显示取消、完成按钮。

两个宿主向 provider 转发的语音选项由共享的 `src/voice/VoiceProviderOptions.h` 生成，不再各维护一份：Fcitx5 以前那份漏掉了提示词，选了自定义润色方案的用户在这里得到的其实是默认的整理提示词。内置方案只以 `polish_prompt_id` 发送方案名，provider 使用该方案的内置提示词；自定义方案另以 `polish_prompt_custom_1`、`polish_prompt_custom_2` 或 `polish_prompt_custom_3` 发送所选槽位的内容，槽位为空时 provider 使用默认的整理提示词。超过 8 KiB 的提示词直接拒绝，不截断，以免被截断的指令改变润色的意思。

Fcitx5 每 5 秒通过 freedesktop Settings portal 读取 `org.freedesktop.appearance/color-scheme`，语音浮层跟随颜色模式（存下的 `voice_theme` 不再读取），因此全局颜色模式为 `system` 时，已显示的语音浮层会跟随系统明暗变化；portal 不可用时保留上一次主题，不阻塞输入。

切换中英文时（`input_mode_hud`，默认开启），两个宿主短暂提示当前的「中」或「英」。进入输入框时是否提示由独立的 `input_mode_focus_hud` 控制，默认关闭；Linux 设置页「输入 → 中英模式」可分别调整「中英文切换提示」和「聚焦输入框时提示」，两者互不依赖。缺字段的旧文档同样关闭聚焦提示。提示的显示规则与 macOS 的紧凑切换提示一致：一次切换只出一处提示，显示 0.6 秒后在 0.18 秒内淡出。X11 会话里（Fcitx5 按输入上下文是否为 X11 客户端判断，IBus 按会话里没有 `WAYLAND_DISPLAY` 判断）自绘一块徽章放在光标正下方 10 px、水平居中，下方放不下就翻到光标上方，离所在显示器工作区边缘至少 8 px；输入框没报光标时放在工作区水平居中、下沿离底边四分之一高的位置。这时不再发面板的文字提示。焦点进入输入框时 GTK、Qt 的输入法模块要过一会儿才补报光标位置，所以这时的徽章先等光标报来（Fcitx5 等光标矩形变化事件，IBus 等 `set_cursor_location`），最多等 150 ms；等完仍没有光标就改用面板的文字提示，不把徽章放到屏幕下方。每个宿主只有一块徽章，新的提示替换屏幕上那块，不会在几个输入框的光标下同时留着几块；徽章在第一次真要画时才连接显示服务器，Wayland 客户端且没打开标志时一直不连。其余情况（Wayland 客户端的光标矩形是它自己表面里的局部坐标，徽章放不过去）由面板的文字提示跟随光标：Fcitx5 是面板自带的输入法信息弹窗，IBus 是辅助区域。从别的输入法切到水杉时由 Fcitx5 自己弹输入法名，不再叠加这个提示。IBus 协商客户端身份时重放的同一次焦点不会再显示一遍；密码框和私密输入中都不显示。焦点不在任何输入框时（切换窗口、落在文件列表上），开着全局引擎的 IBus 把引擎挂到它自己的 `fake` 上下文上，这时没有输入框可提示，焦点进入和切换中英文都不显示。徽章与 macOS 同尺寸：18 px 常规字重的字，32 px 高，左右各 8 px 内边距，8 px 圆角，有合成器时带一圈柔和阴影并淡出（没有合成器的 X11 上不画阴影，到时直接收起）；产品标志只在打开「显示水杉图标」（`show_app_logo`，默认关）时以 16 px 画在字前。打开了标志而徽章放不到光标旁时，带标志的徽章另显示在屏幕右下角（Wayland 经 `wlr-layer-shell`，没有 layer-shell 的会话如 GNOME 经 Xwayland 用 X11 窗口），因为面板的文字提示带不了图。X11 徽章是不抢焦点、点击穿透的原生窗口，按光标所在的显示器（没有光标时按前台窗口或指针所在，经 XRandR 枚举）定位，尺寸、标志和边距按 `Xft.dpi`/`GDK_SCALE` 缩放。徽章有自己的固定大小，不读共享偏好里的悬浮工具栏大小，与 macOS 的紧凑提示一样。徽章深浅跟随颜色模式（`theme`），颜色模式为「跟随系统」时跟随上述 portal 报告的系统明暗；存下的 `toolbar_theme` 和 `candidate_theme` 都不读，固定明暗的全局主题（水杉是深色，纸白是浅色）照样决定明暗。Linux 可选的聚焦提示保留 #2589 的需求；默认行为与 macOS 相同，只在切换时提示。

设置 → 输入 → 中英文中的「屏幕角落模式提示」（`input_mode_corner_hud`，默认开启）独立控制屏幕角落的附加徽章。关闭后保留光标旁的提示，也保留候选窗和光标徽章上的水杉图标；它不修改「中英文切换提示」或「聚焦输入框时提示」。角落徽章仍要求开启「显示水杉图标」，且当前后端无法把徽章放到光标旁。

两个宿主都按共享偏好 `floating_toolbar` 画一条悬浮工具栏，对应 macOS 的 `MetasequoiaFloatingToolbarPanel`（`src/overlay/FloatingToolbar*`）。它是一块不抢焦点的 override-redirect 窗口，用 cairo 和 pango 画：左端是水杉标志（`show_app_logo` 关着时是一条两列三行的握把），按住它拖动；分隔线之后依次是中/英（显示方案徽标：中、双、五、日、한、粤、注、越、ཀ、笔，直接输入时「英」，英文候选模式「En」，大写锁定「A」；与 macOS 一样，「En」时点它退出英文候选模式回到中文，其余时候切换中文输入与直接输入）、切换输入方案（弹出可用方案的菜单，正在用的打勾）、标点（。/.）、全半角（半/全）、简繁（简/繁）、表情、手写、屏幕键盘、语音和设置，各按钮跟随自己的开关，默认只开中/英、切换方案、标点、全半角、简繁和设置六个。几何照搬 macOS：按钮宽高是字号加 8、相隔 2，标志 34（握把 10）加 4，整条高字号加 20、圆角 10，按 `scale_percent`（75/100/125/150）和 `font_size`（16 到 28 的偶数）缩放，再乘桌面的 `Xft.dpi`/`GDK_SCALE`；默认的一行是 255 × 44。指针停在按钮上 0.6 秒出提示（文字与 macOS 的辅助功能标签相同），悬停和按下时按钮下垫一块圆角底；设置按钮左键打开设置，右键弹出 macOS 齿轮菜单的那些项（表情面板、设置、宣传、官网、帮助、关于、反馈和「隐藏悬浮状态栏」，Linux 没有系统字符面板，也由包管理器更新，所以不列「检查更新」）。按钮的动作走「工具栏」子菜单里同一项的路径。IBus 的这条路径只对获得焦点的引擎生效，所以属主失焦后工具栏留在屏幕上时，中/英、切换方案、标点、全半角、简繁、语音这些要输入框的按钮和工具菜单里的「宣传」画成淡色、点了不做事，打开面板、设置、官网和隐藏照样能用，回到输入框就恢复；macOS 在失焦后这些按钮仍然生效，IBus 引擎在失焦时改输入状态会让会话以为自己有焦点，所以不这样做。Fcitx5 把失焦的上下文留给状态区菜单，按钮照样生效。颜色按 THEME_CONTRACT §3 从候选调色板推出：系统主题是 Adwaita 的白或 #303030 底，菜单里的勾和选中色是 GNOME 的蓝；皮肤画自己的候选配色；明暗跟随颜色模式，与切换徽章相同。皮肤包里单独的 `[toolbar]` 段（工具栏自己的底色、描边、分隔线、图标、悬停色和圆角，macOS 经 ToolbarSkinTokens 叠在候选配色上）Linux 还不读：共享层发给宿主的皮肤目录（`host_candidate_catalog`）为控制文档大小不带工具栏段，所以 Linux 的工具栏用候选配色和固定的 10 圆角，与 macOS 在这些皮肤下的工具栏颜色会有差别。有合成器时带一圈柔和阴影，没有时按画出来的形状裁出外形。

工具栏的显示规则除全屏一条外与 macOS 相同：一个宿主进程只有一条，跟着当前获得焦点的输入上下文（IBus 的引擎对象、Fcitx5 的输入上下文）；失焦不收起，换到别的输入法或上下文销毁时收起（Fcitx5 默认每个上下文各记各的输入法，焦点移到用别的输入法的上下文时插件收不到切换，只收到失焦，所以那里在任何上下文获得焦点时检查它的输入法，不是本输入法就收起）；连续 10 秒没有按键也没有点它就自动隐藏，下一次按键或点击再出现，只是换个输入框不唤醒它。前台窗口全屏（EWMH 的 `_NET_WM_STATE_FULLSCREEN`）时不显示，这一条跟 Windows 走、与 macOS 不同：macOS 的面板带 FullScreenAuxiliary，盖在全屏应用上面，而 X11 的 override-redirect 窗口会盖住全屏的视频和游戏。前台窗口一换（根窗口的 `_NET_ACTIVE_WINDOW`），还在屏幕上的工具栏重新抬到所有窗口之上，有的窗口管理器用 XRaiseWindow 抬起被点中的窗口时会把它压下去；不维护 `_NET_ACTIVE_WINDOW` 的窗口管理器（例如 twm）上没有这个时机，工具栏可能被盖住，直到下一次隐藏再出现。第一次出现在焦点窗口所在显示器工作区的右下角、离边 20；拖过之后的位置记在 `$XDG_STATE_HOME/<客户端目录>/floating-toolbar-position.json`（与 Windows 一样只属于这台机器，不进同步的共享偏好），下次显示时恢复并收进工作区四边 12 以内。窗口画在 X11 上：GNOME、KDE 和 wlroots 的 Wayland 会话经 Xwayland 显示同一块窗口，但那里看不到 Wayland 原生应用是否全屏，所以输入框失焦时工具栏也收起，获得焦点再出现。连不上 X 服务器（没有 `DISPLAY`）或构建里没有 x11、xext、xrandr、cairo-xlib 和 pangocairo 时没有悬浮窗口，工具栏只是菜单里的「工具栏」子菜单，开关和按钮选择在两处同样生效。工具栏在开关开着、第一次有输入框获得焦点时连 `DISPLAY`，之后一直连着；按需启动 Xwayland 的合成器会因此启动 Xwayland。这条连接（以及键位图的连接）断开时（X 服务器退出，Xwayland 崩溃或被合成器重启），Xlib 默认会让整个 IBus 引擎或 fcitx5 守护进程退出；用 libX11 1.7 及以上构建时（CMake 按 pkg-config 的 x11 版本打开 `MSIME_X11_IO_ERROR_EXIT_HANDLER`）只记下断开，下一轮主循环关掉它，下一次要显示时重新连：这两条连接各有自己的 `XSetIOErrorExitHandler`，另有一个进程级的 I/O 错误处理只对它们打印一行并返回（Xlib 默认的那个打印完就退出，走不到每条连接的退出处理），别的连接照旧交给之前的处理。legacy 包所在的 Debian 10 只有 libX11 1.6.7，那里断开仍会结束进程。切换徽章、语音浮层和 Fcitx5 原生画布各自的 Xlib 连接不在这一条里，断开时仍是 Xlib 的默认行为。GNOME Shell 下 IBus 的输入源菜单没有「工具栏」子菜单，悬浮工具栏是那里唯一的工具栏。

双拼组字时（共享偏好 `shuangpin_keymap_hint`，默认关，设置页「输入时显示双拼键位提示」）两个宿主在候选窗旁画当前方案的键位图，对应 macOS 的 `MSIMEShuangpinKeymapPanel`（`src/overlay/ShuangpinKeymap*`）：620 × 203 的卡片、13 圆角，标题行左边是「<方案>键位」、右边是「当前按键会高亮 · 上屏后自动隐藏」，三排键帽（键名用等宽粗体，键帽下沿是这个键代表的声母、韵母），刚按下的键与 macOS 一样填主题的强调色、白字（系统主题是 Adwaita 的蓝 #3584E4，皮肤用自己的 `accent`），最下面一行是零声母的写法；键位表经 host-api 从 Engine 自己的方案表读出，自定义双拼方案不画（它的表在偏好里，按小鹤画会标错键）。键位图放在光标下方、让开候选窗再隔 8（Fcitx5 自己画候选窗、即 X11 上默认的原生画布时按画布里卡片的真实矩形让开，不算窗口四周的投影边距、装饰图和外留白，与 macOS 按候选窗几何让开相同；候选窗由 IBus 面板或 Fcitx5 经典界面画时宿主拿不到它的矩形，按候选字号、横竖排和条数估计它的高度，与 macOS 算精确几何之前的基线相同，经典界面的字体由插件按同一个字号偏好写入，所以估计用的字号与它画的一致；皮肤装饰很高或面板字体被另行改大时可能略有重叠），下面放不下就翻到光标上方，左边对齐光标，收进工作区四边 16 以内；上屏、退格删空、失焦或切到别的方案就收起。它只画给报根窗口坐标光标的客户端：Fcitx5 按输入上下文是否为 X11 客户端判断，IBus 按会话里没有 `WAYLAND_DISPLAY` 判断，与切换徽章相同；Wayland 原生客户端的光标是它自己表面里的局部坐标，layer-shell 也不能把表面放到光标旁，那里不画键位图，输入照常。设置页在 Linux 上把这一点写进这个开关的说明。

同一个 socket 也承载候选翻译请求。候选视图更新后，宿主发送一行 JSON：

```json
{"version":1,"kind":"translation","query":{"generation":9,"target_language":"en","candidates":["你好","世界"],"provider":"tencent"}}
```

服务应在一行内返回 `{"translations":[{"text":"你好","translation":"hello"}]}`；未知候选可以省略。独立入口 `msime-linux-translation /absolute/provider.sock` 从标准输入读取同一 TranslationQuery JSON 并输出受界限的 JSON 响应，供 GTK/Qt 面板或其他 Linux 宿主复用。`provider` 是设置页「翻译服务」当前的选择（`none`、`account`、`tencent`、`niutrans`、`custom`），即使所选服务配置不全也照实填写，是必填字段，缺少它的请求按格式错误拒绝；服务只能询问这一家，选「关闭」或所选服务不可用时不发出任何请求，不得退回腾讯。腾讯密钥只存在于服务自己的 `tencent-provider.json`，协议里不传。启用自定义翻译时，请求还会携带已验证的 `custom_translation` endpoint 和 API Key，provider 可据此调用兼容 DeepLX 的服务或 DeepL 官方 API。宿主只接受最多 9 个候选、每项最多 4096 字节、每次完整响应最多 8 秒、128 KiB，并把返回的 generation 原样交给 Host API 校验；过期视图不会被更新。服务必须由用户管理绝对 Unix socket，负责所有凭据、网络访问和日志策略，输入法不会记录原始输入或 API Key。关闭 `preferences.candidate_translations` 后不会发起该请求。候选翻译会按当前候选布局附加到 IBus 候选行。

“翻译当前句子”是显式动作，不受 `candidate_translations` 自动开关控制。它发送 `sentence: true` 的单项请求，最多 512 个 Unicode 字符；普通候选翻译仍最多 9 项、每项 40 个字符。结果复用当前候选代次写回候选区，过期结果会丢弃。

英文候选的自定义释义沿用 Engine 的 `custom_translations.txt` sidecar。Linux 从 HostOptions 的 `user_data` 目录读取该文件，用户覆盖优先；没有用户文件时回退到已验证资源目录中的内置文件，再复制到当前可写词典代次供 Engine 加载。这样资源目录可以保持只读，用户只需在状态目录的 `user/custom_translations.txt` 中按“源词<Tab>释义”维护覆盖，重新建立输入会话后生效。

Linux 独立手写面板使用同一类用户管理 Unix socket，不把 GTK、Wayland 或某个桌面环境绑定进 IBus Engine。面板采集归一化坐标笔画后，按一行 JSON 请求发送：

```json
{"version":1,"kind":"handwriting","query":{"language":"zh-CN","strokes":[[{"x":0.2,"y":0.3},{"x":0.7,"y":0.8}]]}}
```

识别服务返回 `{"candidates":["你","好"]}`，最多 12 个候选，每项最多 4096 字节；请求和响应各自限时 500ms。模型、凭据和平台识别器由该服务负责，面板可以用 `msime-linux-handwriting /absolute/socket` 复用 Host API 契约。服务不可用或响应过期时面板保留笔画，不向 IBus 会话伪造提交；候选点击应由面板在当前手写请求代次内完成。

装有离线手写模型时，面板也可执行 `msime-linux-handwriting --local /absolute/handwriting-zh_CN.model`。安装后的工具省略模型参数时会读取绝对路径环境变量 `MSIME_HANDWRITING_MODEL`，否则按自身安装前缀查找 `share/msime-client/handwriting/handwriting-zh_CN.model`；发布页的 deb/rpm 不在这里放模型，见下文「离线中文手写模型」一段。该入口把归一化笔画交给输入引擎里移植自 zinnia 的识别器，模型路径必须是受信任的绝对路径；没有模型或识别失败时返回错误，不回退为伪造候选。

独立 Emoji 面板也可通过该 socket 查询目录。请求使用 `kind:"emoji"`，查询包含 `search`、`category` 和 `limit`；服务返回 `{"items":[{"text":"😀","annotation":"grinning face"}]}`。搜索最多 256 字节、分类最多 128 字节、结果最多 96 项，每项文本最多 64 字节、注释最多 256 字节，调用限时 500ms。面板使用 `msime-linux-emoji /absolute/socket` 获取结果；没有 provider 时可用 `msime-linux-emoji --local /absolute/resource-generation` 直接查询已验证的 `msime-others.db`。Linux 桌面打开面板时保存当前输入目标，点击项目优先用 `xdotool type` 或 `wtype` 回填当前编辑器，目标已失效时回退到剪贴板；IBus Engine 仍只负责组合中的本地 Emoji 模式，不读取系统剪贴板。

桌面 Tauri Emoji 面板在 Linux 上直接读取 HostOptions `resources` 下 Engine 提供的 `msime-others.db`，通过输入引擎分页读取完整 Emoji、颜文字和符号目录，并按数据库分类聚合后交给共享 UI；读取失败时 UI 保留内置目录。面板只接收资源目录中的目录数据，不读取用户输入、凭据或私人资料。

Fcitx5 插件状态栏里的表情菜单同样显示已安装的符号集插件：在切换表情类别、开始表情搜索和读取分组列表时，经 `msime_client_emoji_catalog_request` 的 `list_plugin_symbol_groups` 读取 `<preferences_directory>/plugins` 下的符号集。符号组排在内置符号之后，分组循环里显示为「插件名 / 组名」；颜文字组排在内置的 All 之后。「全部」和搜索结果在内置条目读完后接上插件条目，搜索匹配组关键词或条目原文；不与内置目录去重。插件读取失败时菜单只显示内置目录。合并与分页规则在 `src/core/EmojiPluginGroups.h`，由 `tests/core/emoji_plugin_groups.cpp` 钉住。

桌面 Tauri 面板的系统剪贴板按 Linux 会话能力选择后端：优先使用 Wayland 的 `wl-paste` / `wl-copy`，不可用时回退到 X11 的 `xclip`；合成器不提供 data-control 协议时（GNOME/Mutter，进程内用 `wl-paste --watch` 是否立即退出探测一次），读取不用 `wl-paste`，直接经 XWayland 的 `xclip`，避免每次轮询都抢焦点打断输入法；剪贴板历史仍只在用户开启设置后写入本地受限存储。桌面宿主运行期间以低频轮询捕获新的文本剪贴板内容，设置关闭后立即停止记录并清除本轮监视状态，读取失败不会伪造同步结果。

桌面 Tauri 面板在 Linux 上也接入了屏幕键盘、手写和语音提交。打开面板时宿主先保存当前输入目标：X11 使用 `xdotool getactivewindow`，Sway 使用 `swaymsg -t get_tree`；按键通过目标窗口的 `xdotool key`、Sway 的 `wtype` 或通用 Wayland 的 `ydotool` 发送。`ydotool` 仅在其 daemon 可用时启用，以 `/dev/uinput` 注入，不依赖面板重新夺取焦点；没有全局注入能力时回退到 `wtype`。手写候选和语音识别结果通过同一目标提交文本，语音面板消费 provider 的 partial/final 事件并实时显示转写，关闭面板时发送当前 generation 的取消消息。手写识别服务的绝对 Unix socket 由 `MSIME_HANDWRITING_PROVIDER_SOCKET` 提供，语音服务使用 HostOptions 的 `voice_provider_socket` 或 `MSIME_VOICE_PROVIDER_SOCKET`；服务仍负责录音、模型和凭据。缺少注入工具或服务时面板保留可见状态并返回宿主错误，不伪造提交。

IBus 和 Fcitx5 宿主还提供一条比上述工具更直接的提交路径，对应 Windows 面板经 TSF 回填当前编辑器的行为：输入法进程在 `$XDG_RUNTIME_DIR/msime-client/panel-input.sock`（目录 0700、socket 0600，只接受同一 uid 的对端）监听，面板的按键、手写候选、语音结果和 Ctrl+V 优先通过它由输入法自身提交或转发，不需要 `xdotool`、`wtype` 或 `ydotool`，Wayland 下也不依赖 compositor 支持虚拟键盘协议。请求为一行 JSON：`{"op":"generation"}` 取当前焦点代次，`{"op":"text","text":"…","after_generation":N}` 提交单行文本，`{"op":"key","key":"BackSpace","keycode":14,"shift":false,"control":false,"alt":false,"super":false}` 以 X keysym 名和 evdev 键码转发按键；应答为 `{"ok":true}` 或 `{"ok":false,"error":"no_focus|restricted|invalid"}`。面板获得焦点时先取代次、释放焦点，再要求输入法只在更新的焦点上投递，所以文本不会落回面板自己的 webview；700ms 内没有新的焦点则应答 `no_focus` 并丢弃请求，不会迟到上屏。密码等受限字段应答 `restricted`，面板不再改用注入工具绕过。含换行或制表符的文本仍走剪贴板或注入工具。两个宿主同时运行时先绑定的一方提供服务，不抢占仍存活的 socket。应答缺失或无法解析时桌面宿主报告错误而不回退，避免同一文本被提交两次。

`key` 请求与 Windows 屏幕键盘的 `SendInput` 一样先交给输入法：按下、松开各送一次，只有按下没被输入法消费时，才把这一对事件转发给编辑器，编辑器因此不会只收到半次按键；松开事件总会交给输入法，退格长按和已消费的快捷键都靠它收尾。这个顺序集中在 `PanelInputChannel.h` 的 `deliver_panel_key_stroke`，IBus 调引擎自己的按键处理，Fcitx5 构造 `fcitx::KeyEvent` 交给 `InputContext::keyEvent`，走该输入上下文当前的输入法。因此中文模式下屏幕键盘打出的字母起拼音组合，数字键、空格、退格作用于已有的组合串（包括物理键盘起的组合）；要打英文先切到英文模式。面板请求不带 CapsLock 状态，宿主沿用最近一次真实按键报告的锁定位，锁定时字母按物理键的规则变大写（同时按 Shift 则小写）并直接交给编辑器，不开始组字。`text` 请求（手写、Emoji、语音）仍原样上屏，不经过组字。

桌面手写和语音面板跟随颜色模式（`HostCapabilities::surface_themes_follow`），存下的 `preferences.handwriting_theme` 与 `preferences.voice_theme` 不再读取，Fcitx5 的模式角标同样不读 `toolbar_theme`。设置保存后，已打开的面板通过偏好变更事件立即更新外观。

桌面 Emoji 面板（包括颜文字、符号和剪贴板页）同样跟随颜色模式，存下的 `preferences.emoji_theme` 不再读取；设置保存后已打开的面板实时同步主题。

面板的落点与 macOS 对齐：屏幕键盘贴在输入窗口所在显示器工作区的底部居中（macOS 的屏幕键盘同样如此），表情和本地剪贴板面板在工作区居中（macOS 不给位置，由系统居中），手写、语音、云剪贴板等其余面板仍定位到输入窗口下方并水平居中。X11 上读取活动窗口矩形、用它找到所在的显示器，Sway 上读取 focused container 的 `rect` 和所在 workspace；窗口矩形不可用时（从设置窗口打开、只有输入法通道等），屏幕键盘和居中的面板按窗口当前（或主）显示器的工作区摆放，其余面板回退到屏幕默认位置。GNOME、KDE 等 Wayland 合成器不让客户端自定窗口位置，那里一律由合成器决定，不伪造坐标。

屏幕键盘使用共享的 `touch_key_spacing_tenths` 和 `touch_row_spacing_tenths` 设置实时调整键位与行间距；启用 `touch_voice_shortcut` 时，键盘标题栏提供“语音”入口并复用已保存的输入目标打开语音面板。普通字符、编辑和候选提交键在按住 450 毫秒后以 75 毫秒间隔自动重复；粘滞修饰键与 Num Lock 只执行一次，松开、取消、失焦、关闭面板或宿主投递失败都会停止重复，不自动重放可能已部分发送的按键。设置变化只影响当前面板布局，不改变 IBus Engine 组合状态。键盘标题栏与 macOS 一样先显示「先点击目标应用的输入框，再点击这里的按键」，Win 键在 Linux 上标作 Super（与打字统计的热力图一致，发送的仍是同一个键）；按键送不出去时按宿主的错误码说明原因：找不到能接收按键的输入位置时提示 X11 需要 xdotool、Wayland 需要 wtype、ydotool 或正在使用的水杉输入法。

IBus 属性面板提供 `EnglishCandidates`、`SymbolCandidates`、`EmojiCandidates` 和 `KaomojiCandidates` 四个混输开关（Fcitx5 对应「混合符号」「混合 Emoji」「混合颜文字」等状态动作）。配置绝对共享偏好目录时，切换先按 revision 保存，再由共享运行时决定活动组合后的应用时机；未配置目录时会结束当前组合并重建本会话的 Engine，避免把新旧混输候选规则混在同一代视图中。

IBus 属性菜单与 Fcitx5 状态区的顶层有「英文候选模式（Ctrl+Shift+E）」（IBus 的 `EnglishMode`），与 macOS 输入法菜单同名同位；它不是 macOS 菜单里的「英文输入」，后者就是顶层「中文输入」开关的未勾选状态。Ctrl+Shift+E 或这个开关调用 Engine 的 dedicated English 模式，保留中文输入法会话和 IBus 输入源边界；它与「输入选项」里的「中英混输」（`EnglishCandidates`，混输英文候选）相互独立。状态按当前 IBus 会话保留，切换时由 Engine 清理正在进行的组合。

Linux IBus 会话支持 `Ctrl+Shift+Super+K` 打开屏幕键盘面板。宿主只在当前输入上下文获得焦点且不是密码等受限字段时消费该组合，并通过现有桌面面板启动器打开键盘；客户端把 Super 报告为 `MOD4`、`SUPER` 或两者同时置位时都能识别；Super 组合是否能到达 IBus 仍由桌面环境的全局快捷键策略决定。

IBus 属性面板还提供 `TraditionalOutput`。开启后，中文方案的候选显示和提交文本经 `msime-host-api` 导出的 `msime_client_simplified_to_traditional` 转换为繁体，与 Windows 共用同一份 OpenCC 词级表（「头发」→「頭髮」而不是逐字的「頭發」），不再依赖系统 ICU。用哪套配置看共享偏好 `traditional_chinese_variant`：缺省 `s2t`，设置页「中文输出」选台湾正体或台湾用词时是 `s2tw`、`s2twp`，两个宿主读偏好时一并读它，经 `msime_client_convert_to_traditional` 转换；Unicode 直接输入、日语方案和英文/Emoji 文本保持原样。配置绝对共享偏好目录时开关按 revision 保存 `traditional_chinese_output`，未配置目录时只覆盖当前会话。

`Ctrl+Shift+F` 使用同一简繁输出路径：配置了共享偏好目录时通过 revision 保存 `traditional_chinese_output`，保存成功后更新当前会话；没有可写偏好目录时保留会话级切换。持久化写入进行中，新的一次按下不切换也不吞掉该快捷键，避免重复操作覆盖较新的 revision；已经切换过的那次按下，按住时的自动重复和松开都归这个快捷键，不会再切换一次，与 Windows 一致。

IBus 与 Fcitx5 会话都支持 `Ctrl+Shift+Alt+1` 到 `Ctrl+Shift+Alt+8` 删除候选页对应的可编辑词条。宿主只传递候选快照中的会话、代次和全局索引，由 Host API 校验来源和执行词库删除；没有对应候选或不可编辑候选时按键交回应用。`Ctrl+Shift+Alt+C` 清除当前输入法会话的 Engine 候选缓存并刷新当前视图，不会结束正在进行的组合。`Ctrl+Shift+Alt+R` 重启或重载输入法：IBus 下执行用户会话的 `ibus restart` 重启服务，Fcitx5 下在进程内重置水杉插件、不影响其他输入法（见上文 Fcitx5 的 `Ctrl+Shift+Alt+R` 一段），设置页也提供同一动作的按钮。

`Ctrl+Shift+Alt+T` 立即退出当前 Linux IBus 宿主进程，快捷键由宿主消费，不会停止用户正在运行的其他 IBus 服务；宿主以专用退出状态结束，launcher 的崩溃守护据此不会重启它。Fcitx5 插件与 Fcitx5 同进程，退出就会带走其他输入法，因此 Fcitx5 不提供这个快捷键，设置页也按此注明；需要恢复时使用 `Ctrl+Shift+Alt+R` 重置水杉插件（见上文 Fcitx5 的 `Ctrl+Shift+Alt+R` 一段）。`Ctrl+.` 在两个宿主上都切换中英文标点，与 Windows 相同；标点锁定为「跟随」时，Fcitx5 把中文模式下的切换结果保存到共享偏好，和状态菜单里的同一开关一致；锁定为中文或英文时不保存（见下文英文模式一段）。英文模式下 `Ctrl+.` 同样生效（见下文英文模式一段），只改本次会话、不写偏好，到下一次中英切换为止；Fcitx5 状态栏的「中文标点」在英文模式下显示和切换的也是这一状态。菜单主题不在 Linux 设置页出现：IBus 属性菜单由桌面面板按自己的主题绘制，Fcitx5 classicui 的状态菜单跟随全局主题的候选调色板（见下文 Fcitx5 候选表一段），kimpanel 与 GNOME Shell 面板下同样由面板自己绘制。

Windows 配置中的 `candidate_arrow_navigation` 兼容名称也会映射到共享导航的 `arrows` 开关，保证迁移配置在 Linux 上保持一致。

Linux 的 `floating_toolbar` 偏好既是能连上 X 服务器时画的悬浮工具栏（见「输入细节与平台差异」），也映射为 IBus 属性菜单与 Fcitx5 状态区里的“工具栏”子菜单，画不出悬浮窗口的会话靠后者。启用后，子菜单按 macOS 悬浮工具栏的顺序和默认开关列出中文输入（由 `english_mode` 控制，与 macOS 的中/英按钮相同）、输入方案（与顶层「输入方案」同样内容的子菜单）、中文标点、全角字符、繁体输出、表情面板、手写、屏幕键盘、语音和设置；表情、手写、屏幕键盘和语音与共享默认值一样默认不列。「英文候选模式」只在顶层，macOS 的工具栏也没有它。两个宿主的条目文案与顶层相同，都取自 `src/core/MenuLabels.h`。Fcitx5 的托盘在失焦后仍列最近的上下文，这时子菜单不列语音和「宣传水杉输入法」，它们要往有焦点的输入框里写；IBus 失焦时整个子菜单不可用。其后是 macOS 工具栏齿轮菜单里的「宣传水杉输入法」（在光标处插入产品介绍；IBus 宿主原有的 Ctrl+Shift+P 也做同一件事，Fcitx5 不占用这个组合键，因为 Linux 应用普遍把它用作命令面板或隐私窗口，macOS 也只有菜单项）、「访问 msime.app」、「使用帮助…」「关于水杉输入法…」「问题反馈…」。模式动作复用当前会话，面板动作通过 `msime-linux-settings` 启动已有 Tauri 面板，并把当前输入目标交给面板保存。关闭工具栏或单独关闭组件后，入口会在配置热重载时同步隐藏。

## 构建与运行

IBus 提交也接入共享的聚合打字统计。统计在文本成功提交到 IBus 后异步写入 Host API，按当前方案、本地模式、英文模式或语音来源计数；只保留字符类别、来源和日期的聚合数据，不保存输入文本。输入法没有消费、交还给应用的可打印字符也计入统计（英文模式记为 `english` 来源），判据与 Windows `ShouldCountPassthroughChar` 相同，写在 `TypingStatistics.h` 的 `should_count_passthrough_character`：只算按下、有焦点、非密码与隐私输入、不带 Ctrl/Alt/Super 的键，不算控制字符和 DEL；Fcitx5 同样如此。未配置绝对的 `preferences_directory` 时跳过统计，统计写入失败不会影响输入。统计开关（默认关闭）由宿主缓存在 `TypingStatistics.h` 的 `TypingStatisticsSwitch` 里：启动时和每次偏好热重载时刷新，统计文件没变时只做一次 stat，变了才经 `msime_client_typing_statistics_enabled` 重读；关闭时上屏和透传按键在入口处直接返回，不取日期、不序列化请求、不起 GTask 或线程，也不碰统计文件和锁。在设置里打开统计后，要到下一次热重载（IBus 约 1 秒、Fcitx5 约 250 毫秒）才开始计入，这之间的上屏不记录，与 macOS 的做法一致。

英文模式与 Windows 一样仍处理中文标点和全角：标点锁定为「始终中文标点」，或锁定为「跟随」时在英文模式下按过 `Ctrl+.`，先把 ASCII 标点换成中文标点（引号交替、书名号嵌套，按 `shared/contracts/punctuation/policy.h` 的正向表），全角开着时再把可打印 ASCII 换成全角（空格为 U+3000），其余键交给应用；小键盘不转中文标点，带 Ctrl/Alt/Super/Hyper 的组合键照旧透传，`Ctrl+.` 和已启用的语音快捷键除外（见「语音输入」段）。中文和英文模式下的 `Ctrl+.` 都由宿主消费；标点锁定为「始终中文标点」或「始终英文标点」时，它和 IBus 属性菜单、Fcitx5 状态栏里的「中文标点」都不改变标点状态、也不写偏好，与 Windows 经 `ResolvePunctuationOpen` 按锁定值解析标点状态一致。IBus 与 Fcitx5 共用 `shared/input/EnglishModeOutput.h` 的 `english_mode_output`（macOS 宿主也用它）。每次中英切换后（包括焦点切换时按应用记忆恢复出另一种模式）按标点锁定重设会话标点：「跟随」时中文模式用中文标点、英文模式用英文标点（英文模式下 `Ctrl+.` 的选择随之作废），锁定为中文或英文时保持锁定值；这只改本次会话，不写偏好文件。裸 Shift/Ctrl 松开时切换模式，但这次松开仍交给应用，跟踪修饰键状态的程序不会以为它一直按着。

候选辅助文本在页码后展示 Engine 快照提供的本地模式标签（U+、日期时间、短语、Emoji、颜文字、简拼、EN、日文）。普通或未知模式不附加标签，取消组合或没有候选时隐藏辅助文本；不从预编辑前缀推断模式。

按 Windows 基线，全拼辅助码默认启用自然码且不在候选窗显示，双拼辅助码默认启用蓝天且显示；旧共享偏好或 Linux 运行选项缺少对应对象时使用相同的分方案默认值。当全拼或双拼启用辅助码候选显示时，Linux 也为 Engine 生成的整句候选按当前方案和词库映射计算辅助码；不再把整段原始预编辑拼音当作候选注释。辅助码仍是展示文本，不进入候选身份或提交内容。

全拼纠错按字母错位和邻键误触分别启用，两个字段缺失时均默认开启；已有明确保存的 `false` 仍保持关闭。纠错由 Engine 自动执行，不在设置页、IBus 属性或 Fcitx 状态栏显示开关。

候选行保留 Engine 的来源身份：本地词库和用户词库不额外标记，云候选和 AI 候选与 macOS 候选窗一样在行文字后隔一个空格加 ☁️、🤖 角标。注释（辅助码、五笔编码提示、`/` 和 `@` 模式的命令标题或省市）也照 macOS 包在半角括号里、紧贴候选字，画成「你好(wqvb)」，已带括号的原样保留，角标跟在注释后面；两个前端共用 `src/candidates/CandidateRowText.h`。序号与选它的数字键一致、不带句点，第十个标 0：IBus 是 `1`…`0`，Fcitx5 是 `1 `…`0 `（classicui 序号和候选之间没有间距，用一个空格代替 macOS 的 1.5pt），主题的 `LabelTextSizeFactor=80` 让 5.1.18 起的 classicui 按候选字号的 80% 画序号，与 macOS 的 0.8 倍相同，更早的版本忽略这个键。来源标签只用于 IBus panel 展示，不进入提交文本、候选索引或异步结果校验。

候选颜色来自共享层解析的全局主题（ABI 3 的 `msime_client_resolve_theme`）：宿主以 `global_theme`、`custom_theme`、候选明暗和当前候选布局请求 ResolvedTheme，自定义主题在所绘明暗下的槽位引用已安装的外部皮肤时（深色取 `candidate_skin_dark`，没设时取 `candidate_skin`；浅色取 `candidate_skin`），再带上运行配置里该皮肤的目录条目；共享层拒绝该条目时宿主去掉它重新解析，仍失败才退回原生 token。解析出的 surface、正文、序号与选中行颜色映射为 IBus 候选文字、编号标签前景和行背景属性，以及 Fcitx5 classicui 主题；IBus 上固定了排位的候选未选中时画 macOS 和 Windows 的固定色 `#379AD3`，行尾另有「固定 N」文字，GNOME Shell 等不认颜色属性的面板上也看得出来；主题未给出的槽位（`system` 主题全部如此）使用 Linux 原生 token：浅色白底、深色 `#303030` 底，选中行 `#3584E4` 实色填充、白色正文。固定了排位的行与 macOS 一样画固定色 `#379AD3`，不随皮肤的 accent 变化（IBus 和 Fcitx5 原生画布画颜色，classicui 改在正文后加「  固定N」），序号颜色在 IBus 和带 `CandidateLabelColor` 的 Fcitx5 版本上生效；边框、圆角与阴影只由 Fcitx5 绘制（X11 上的原生画布，或 classicui 主题），IBus 的候选属性无法描边；hover 状态和 `show_selected_bar` 只有 Fcitx5 在 X11 上的原生画布画得出来，IBus 面板和 classicui 主题都没有；编号色和边框色跟随主题：`candidate_number_color` 与 `candidate_border_color` 在 Linux 为假，设置页不给这两个取色器，宿主请求解析时也去掉自定义主题里存下的这两个槽位，编号色由文字颜色推出，边框取主题和皮肤的发丝线，`system` 底和留空边框槽位的自定义主题画 Linux 原生的发丝线（`CandidateColors.h`）；hover 等选中外观随 `candidate_selection_appearance` 隐藏；圆角能力位按平台给出，说的是 Fcitx5，IBus 宿主则在下面的面板状态文件里报告 `ibus_panel`，外观与主题页据此说明 IBus 下整体大小只缩放字号，圆角、内边距、不透明度、水杉 logo 和皮肤的装饰图都不会生效。候选字体族、回退字体和字号由宿主写进桌面 panel 自己读取的那一个字体描述：IBus 写 `org.freedesktop.ibus.panel` 的 `custom-font` 并打开 `use-custom-font`（与 ibus-setup 写的是同一对键），Fcitx5 通过 classicui 插件自己的配置写 `Font`，立即生效并保存在 `classicui.conf`。描述按 Pango 格式把主字体和回退字体依次列出、去重。字号在 IBus 上以像素写出。Fcitx5 按实际运行版本选择单位，不受打包或本机构建时开发库版本限制；运行版本从 5.1.18 起换算成磅（像素乘 3/4，例如 14px 写成 `10.5`）：这些版本的 classicui 画候选序号时把描述里的字号乘上主题的 `LabelTextSizeFactor` 再按磅设回去，写像素会让序号比候选大三分之一。5.1.22 起 classicui 在 X11 上按 DPI 整窗缩放、字体 DPI 固定为 96，Wayland 上除非设了 `ForceWaylandDPI` 也是 96，换算前后候选大小相同；5.1.18 到 5.1.21 的 X11 以 `Xft.dpi`（没有时取不低于 96 的屏幕 DPI）作字体 DPI，DPI 不是 96 时候选文字随之放大，与 5.1.22 起整窗缩放后的大小一致。更早的版本序号与候选共用同一个描述，本来就一样大，仍写像素。这个字体归整个桌面所有，所以设置从未改过、仍是默认值时宿主不写，之后用户每次修改都会写一次，回到默认值也算一次修改；例外是升级：偏好仍是默认值，而 `classicui.conf` 里的 `Font` 正是旧版本按像素写下的同一个字体时，宿主按磅重写一次。第一次写入之前宿主记下被替换的原值，卸载时只要这一项仍是水杉写入的值就恢复原值，用户之后自己改过的保持不变（见「卸载 CMake 安装」）。GNOME Shell 自带的候选弹窗跟随 Shell 主题、Plasma 的 kimpanel 跟随桌面字体，这两种面板上宿主不写任何东西：IBus 宿主在 GNOME Shell 会话（`XDG_CURRENT_DESKTOP` 含 `GNOME` 且会话总线上存在 `org.gnome.Shell`）中不写 `use-custom-font`。当前绘制候选的面板是否忽略这些设置由宿主写进会话运行目录的 `$XDG_RUNTIME_DIR/msime-client/candidate-panel.json`（`{"host":"ibus"|"fcitx5","limit":"gnome_shell"|"fcitx_theme"|"kimpanel"|"ibus_panel"|null}`，仅在内容变化时原子替换），设置页的外观与皮肤页据此提示哪些设置不会生效（IBus 宿主在 GNOME Shell 以外总是报告 `ibus_panel`，它的面板只接收字体和颜色，从不报告 `null`）；文件以最后写入的宿主为准。预编辑由应用自己绘制，IBus 面板和 classicui 主题的拼音行也只有候选字体一种字号，所以 Linux 默认不提供单独的预编辑字号，设置页按 `candidate_preedit_font` 能力位隐藏它；Fcitx5 在 X11 会话里用原生画布画候选窗时，状态文件另带 `"renderer":"native_canvas"`，设置页据此给出拼音字号（首行的拼音按它画）、展开候选和选中外观三项（`HostCapabilities::apply_linux_native_canvas`）；英文字体同理不提供。

中英混输默认在预编辑达到 5 个字母后显示英文候选，Emoji 与颜文字混输默认关闭；符号混输（打出符号的名称出符号，如 `zhengfu` → ±，`mixed_input.symbols`）默认开启，共享文档只在关掉时写这个键，两个前端读到缺键都按开显示。旧宿主选项缺少这些字段时使用相同默认值。用户仍可在设置中选择 1–8 个字符并分别切换符号/Emoji/颜文字，显式配置优先于默认值。

新建共享偏好使用 Windows `develop` 的候选外观基线：跟随系统明暗、`system` 全局主题、每页 6 项、18px 候选文字、15px 候选预编辑，以及 `Noto Sans SC` / `Microsoft YaHei` 字体回退栈。Linux IBus 只应用 panel 协议可表达的主题色和页大小；字体继续由桌面 panel 管理，但 Tauri 设置与预览保留完整共享配置。已有偏好文件和显式宿主选项不被默认值覆盖。

Linux IBus 候选表画的是共享层解析出的全局主题（水杉、浅色、纸白、夜青、墨，或自定义主题及其外部皮肤）的 surface、正文、序号、accent 与选中行颜色。IBus 的候选属性只携带 RGB 前景/背景，不能表达原生窗口的 alpha、圆角、阴影、边框、hover、选中条或布局间距，因此半透明槽位先按设计合成到候选底色上再发布，其余装饰无法呈现；外部皮肤声明的顶部装饰图同样无法经 panel 协议显示，IBus 忽略它。主题没有选中填充时，以 accent 实色作为选中行，再没有 accent 时使用原生 `#3584E4`；选中行正文取主题的选中正文色，没有时取自定义文字色，再没有时按选中底色取对比度更高的黑或白。

Fcitx5 在 X11 上默认由宿主自己的原生画布画候选窗（见下文「原生 X11 画布」）；Wayland、kimpanel、逃生口 `MSIME_FCITX5_CANDIDATE_RENDERER=classicui` 和原生窗口准备失败时，候选表由 classicui 插件按主题绘制。宿主把同一份 ResolvedTheme（全局主题、自定义主题及其外部皮肤、`follow` 跟随颜色模式）写成用户数据目录下的主题 `$XDG_DATA_HOME/fcitx5/themes/msime/theme.conf`（默认 `~/.local/share/fcitx5/themes/msime/`），再通过 classicui 自己的配置把 `Theme` 和 `DarkTheme` 指向它；配置一写入插件就重新读取主题，改皮肤或系统明暗切换后无需重启。有效 `system` 解析也提供默认候选样式：细边框、卡片和水杉品牌标志（按共享偏好 `show_app_logo` 显示，与 macOS、Windows 相同，新装默认关闭，已存偏好缺这个键时按共享层的读法显示。Linux 以前不读这个键、总是画 logo，而共享层从 #5631 起把默认值 `false` 写进每一份保存过的偏好，所以升级后大多数现有安装的 logo 会消失，要看到它得在设置页重新打开；这是有意接受的，与 macOS 新装默认隐藏一致，没有为此做迁移），配色使用 Linux 原生 token 跟随系统明暗；空调色板不代表退出接管。其他候选覆盖来自内置主题或有皮肤、颜色的自定义主题；解析失败不视为有效 `system`。只有用户主动选主题——主题菜单里点选，或设置页把新的主题选择写进存储——才会从第三方主题手里接管；首次观察偏好、主题选择未变的焦点进入/后台同步和系统明暗变化都不把用户选的第三方主题换回来。设置页在没有会话时保存的新主题不会被下一次聚焦吞掉：会话创建成功后与插件已有的共享基线比较，尚未处理的新存储选择接管一次；首次建立基线、已处理的选择和没有存储快照的 runtime options 更新都不算主动选择。用户在 fcitx5-configtool 里选过的第三方主题保持不变，此时 MSIME 的候选颜色不生效，第三方 `DarkTheme` 则在深色模式下生效。主动切回「跟随系统」仍保留水杉样式，也可以重新接管第三方主题；若要使用第三方主题，在 fcitx5-configtool 中选择，后台同步不会夺回。选中没有实际候选覆盖的自定义主题、皮肤不支持当前明暗或解析失败时，宿主停用自己的主题，把仍是水杉写的 `Theme`/`DarkTheme` 按接管记录恢复：记录里留有原值就用原值，没有（旧版本写入、记录被删）用自带主题 `default`/`default-dark`；用户在 fcitx5-configtool 里改过的项一项都不动，恢复也不写接管记录、不动字体等其他选项。`Theme` 与 `DarkTheme` 分别判断、分别记录。第一次接管之前宿主记下原来的 `Theme` 和 `DarkTheme`，卸载时仍指向 `msime` 的项恢复原值、主题目录随之删除，用户之后改过的保持不变；主题目录不再是宿主生成的原样时目录和这两项都保留（见「卸载 CMake 安装」）。接管不随切换输入法撤销：切到别的输入法后，classicui 为它绘制的候选窗仍使用 `msime` 主题和水杉写入的字体。有焦点的输入框每 250 ms 同步一次主题，每一拍先比对主题输入（解析请求、圆角、装饰图的大小与修改时间），这些输入已经接管写好过就什么也不做：不调 classicui 的 `getConfig()`（它每次都扫描并解析所有已装的 Fcitx5 主题，装的主题越多越慢，曾让打字和删除明显卡顿，#5988），也不重新生成图片，接管之后用户手改或卸载时还原的主题也不会被改回去；唯一的例外是用户在主题菜单里重新选择水杉主题，此时即使主题输入没变也重新接管。还没接管成功时再看 classicui 落盘在 `conf/classicui.conf` 里的 `Theme`、`DarkTheme`，输入或这份选择变了才按 classicui 的现值判断能否接管，能接管才原子替换主题文件；用户在 fcitx5-configtool 里把第三方主题改回默认后，下一拍恢复接管。写主题失败时不逐拍重试，每 10 秒再试一次，主题输入或 classicui 的选择变化、或「重启输入法服务」后也会立即再试。classicui 主题只能把背景画成纯色矩形或九宫格图片，所以圆角与阴影由宿主画成图片：宿主按 macOS 候选窗的几何 token（`CandidateSkin.cpp` 的 SkinTokens、`CandidateChrome.h`）生成 PNG（候选卡片 10px 圆角、1px 细边、下方阴影，内容离卡片外沿 4px，细边画在这 4px 里，与 macOS 的 `CandidateInset` 取描边宽度与内边距中较大的一个相同；选中行 6px 圆角实色，行内文字左右各 8px、上下各 6px；菜单 12px 圆角与 1px 描边、6px 圆角 hover、分隔线、勾选与子菜单箭头；候选翻页按钮 ‹ › 按序号颜色绘制，没有序号颜色时跟随正文），各带一份 `@2x`，按内容哈希命名为 `shape-<hash>.png` 与主题一起原子写入，颜色变化后旧图片随即删除；每一节同时保留纯色 `Color`，classicui 读不到图片时退回纯色矩形。PNG 由宿主自己编码（未压缩的 deflate 块），不新增依赖，cairo 在所有 Fcitx5 版本上都能直接读取。阴影所占的透明边写进 `ShadowMargin`，X11 上 classicui 定位时扣除它；Wayland 上 classicui 不读这个值，候选窗相对光标多偏移阴影边的宽度（左 12、上 8 像素），因此阴影比设计的 `0 6px 18px` 收紧为下移 4px、标准差 5px。没有合成器的 X11 会话里 classicui 没有 alpha 通道，透明的圆角外和阴影显示为黑色，这对所有带圆角图片的 Fcitx5 主题都一样。菜单没有阴影边可用，只画圆角不画阴影，宽度随内容而定，不是设计的 260px。选中行的九宫格固定切片就是行的文字边距，所以行圆角最多等于上下文字边距（6px），再圆就画不出来；macOS 那样折行、宽度上限和最小宽度 classicui 都没有对应项。classicui 只在主题同时给出 `PrevPage` 与 `NextPage` 两张图片时才画翻页按钮，位置由它自己决定：内容区右侧，无页可翻的一侧变淡。主题写入 `PageButtonAlignment=Top`，2023 年 4 月之后的 Fcitx5 据此把按钮放到预编辑所在的顶行，对应设计中的标题行；更早的版本（如测试所用的 5.0.21）忽略这个键，按钮画在右下角。classicui 的页码在辅助行里，照 macOS 写成「1 / 3」，按钮旁没有；原生画布把它画在首行的 ‹ › 左边。整张图片都是点击区域。关掉 `show_candidate_page_number` 时主题不生成这两张图，classicui 就不画 ‹ ›，与 macOS 一起隐藏页码和箭头相同；翻页键和滚轮照常。classicui 的候选文字没有颜色属性，也没有与选中分开的 hover 状态，因此固定了排位的候选画不出 macOS 的固定色 `#379AD3`，改在正文后加只供显示（DontCommit）的「  固定N」，与 IBus 行尾的文字相同；原生 X11 画布能画单行颜色，未选中的固定行正文直接画 `#379AD3`，不加这段文字；序号颜色写进 `CandidateLabelColor`、`HighlightCandidateLabelColor`，不认识这两个键的旧版本忽略它们，序号跟随正文颜色。菜单（托盘和状态区菜单）按 THEME_CONTRACT §3 从同一份 ResolvedTheme 的候选调色板派生；所画皮肤包写了 `[toolbar]` 配色时，当前明暗那一组盖过候选调色板推出的槽位（background 作菜单底色，icon 作条目文字，hover 作悬停条目，border 作外描边，divider 作分隔线）：工具栏的功能也都在输入法菜单里，这里对应 macOS 用这组颜色画悬浮工具栏（`ToolbarSkinTokens`）的位置；两个前端在 X11/Xwayland 上另画的悬浮工具栏用的是候选调色板，还不读 `[toolbar]` 段（见上文按共享偏好 `floating_toolbar` 画悬浮工具栏的一段）。其余槽位：surface 作菜单底色，text 作条目文字，hover 作悬停条目，border 作分隔线和外描边，半透明的颜色合成在菜单底色上；主题留空的槽位（`system` 全部留空）取 Linux 菜单 token（浅色白底、深色 `#383838`，文字 82% 黑或纯白，hover 为 6% 黑或 8% 白，外描边 8% 黑），按解析出的明暗选取。border 全透明的主题菜单也不画外描边，分隔线仍用原生细线。契约里勾选条目的 selected 底色与 selected_text 在 classicui 菜单里没有对应项：勾选只由 CheckBox 图片表示，图片按条目文字颜色绘制。外部皮肤声明了顶部装饰（清单的 `decoration.top_inset_dip`、`decoration.width_dip` 与通过校验的 `decoration.image`，没有时取预览图）时，共享层发布给宿主的皮肤目录带上这两个尺寸和图片的绝对路径，宿主按内容哈希把图片原子写进主题目录，仅在内容变化时重写（PNG 重新编码成 `decoration-<hash>.png` 与 `@2x` 两份，其他格式原样沿用扩展名），再以 `Overlay=`、按清单 `decoration.align` 取的 `Gravity=Top Right`（默认）、`Top Left` 或 `Top Center` 和 `HideOverlayIfOversize=False` 写进主题；切换到别的皮肤或无装饰的皮肤后，旧图片随即删除。几何与各平台宿主一致：候选窗比卡片高出 `top_inset_dip`（这条带），带本身透明，不画底色、边框和阴影，卡片从带下方开始；装饰图片画在带里，底边压进卡片顶边以下一个卡片内边距（4px，边框更宽时取边框宽度，即候选内容开始的位置），横向按对齐方式在卡片内收同样的内边距（居中时不留偏移），画在卡片之上。宿主生成的卡片九宫格图片顶部多出 `top_inset_dip` 行全透明像素，并计入九宫格上边距，这几行永不拉伸；带下面的阴影边与无装饰时相同。`ShadowMargin` 与内容上边距同样加上带高，所以 X11 上 classicui 把卡片（而不是带）对到光标下方，候选文字仍从卡片顶边加内边距处开始；Wayland 上 classicui 不读 `ShadowMargin`，整个窗口（带在内）从光标处开始，卡片比 X11 多下移带高。没有合成器的 X11 会话里这条带和阴影一样显示为黑色。classicui 在绘制背景时一并绘制 overlay，早于候选文字，所以压进卡片的部分在预编辑和选中行之下；它恰好止于内容开始处，不会盖住文字。classicui 按图片自身的像素尺寸绘制 overlay，自己不做任何缩放，所以宿主在写入前就把 PNG 装饰图缩放到皮肤声明的 `width_dip`，再与预留高度（带高加卡片内边距）取小，保证整张图落在带里，偏移按缩放后的高度计算。宿主只从 PNG 头读尺寸（签名、`IHDR` 与 1–2048 的宽高，同一处解析不再重复），非 PNG、读不出、声明尺寸超限或缩放失败都退回原图：退回的图读得出高度时按底边定位，高于「带高加内边距」的部分在带顶被 `OverlayClipMargin` 裁掉；读不出高度时从带顶开始按原尺寸画，过高时会画到候选上。水杉主题带装饰图时，classicui 在候选窗顶行显示的中/英模式提示会被补宽到装饰完整显示所需的宽度，装饰不会被 `OverlayClipMargin` 从两侧切掉；宽度在写主题时按主题自己的 `Gravity`/`OverlayOffsetX`/`OverlayClipMargin` 与暂存装饰图的宽度算好并缓存，中英切换不重读主题文件，只在字体或 Wayland 字体 DPI（`ForceWaylandDPI`）变化时重测，第三方主题、没有装饰和 kimpanel 保持普通提示。候选卡片的圆角按优先级取值：设置页「圆角大小」写入的 `candidate_corner_radius`（0–32，未设置时不写入）优先，其次是皮肤的 `candidate_window.corner_radius_dip`（0–32），两者都没有时保持设计的 10px；取到的值替换卡片圆角，九宫格的角随之变化。选中行的圆角照 macOS 推导：用户自己设了卡片圆角时取卡片圆角减去内边距，与卡片同心；只调了内边距时至少大到高亮的角不伸出卡片；都没调时用皮肤包的 `item_corner_radius_dip`（共享目录对写了它的包直接发布，Windows 外观和 canvas 底的包取 `resolvedLayout.itemCornerRadiusDip`），没有就是 6px；最后都不超过上面说的 6px 文字边距。被这个上限截短时 macOS 靠同心保证的「高亮不伸出卡片」不再成立，宿主就把内边距加大到选中行高亮的圆角在对角线上不越过卡片圆角（例如用户把圆角设成 32 时内边距从 4px 变成 8px），默认的 10px 卡片不受影响。设置页「窗口样式」的整体大小、不透明度和内边距照 macOS 的 `StyledCandidateSkin` 写进主题（`FcitxCandidateStyle`）：整体大小（`candidate_scale_percent`，50–200%）除了乘进面板字体描述，还乘到圆角、内边距、行的文字边距、翻页按钮、logo 和装饰图的预留尺寸上，logo 从更大一档的图标缩放出来；阴影的边距、偏移和模糊不缩放，因为 Wayland 上候选窗离光标的偏移就是阴影边距，放大它会让候选窗离光标越来越远（macOS 的投影由系统画，也不随候选窗缩放）；内边距（`candidate_padding`，0–16）代替 4px；不透明度（`candidate_opacity_percent`，50–100%）只淡化卡片底色和描边，文字、序号和选中高亮保持不透明，卡片底下的阴影被擦掉，桌面才透得出来，同时打开 classicui 的 `EnableBlur` 并给出卡片形状的 `BlurMask` 和到卡片外沿的 `BlurMargin`，KWin 据此模糊卡片后面的内容，GNOME 等其他合成器忽略这几个键，较旧的 Fcitx5 不认 `BlurMask` 时模糊整个矩形；没有合成器的 X11 会把半透明的卡片画得发黑。canvas 包的九宫格贴图是包自己的外观，不跟着缩放和淡化。清单里的背景图（`candidate_window.background`）由共享目录发布为包内图片的绝对路径、`fit` 和不透明度（背景图和 `[toolbar]` 配色只发布给自定义主题选中的皮肤，宿主只画它；其余条目带上这几百字节只会占用 16 KiB 的运行时选项预算，把已装的皮肤挤出菜单），宿主用共享层的解码器读出图片（PNG、JPEG、GIF、WebP，动图取首帧），按 `fit` 画进一个按布局估的卡片大小（竖排约 240×280、横排约 560×80，随整体大小缩放），乘上背景图与用户两层不透明度，写成 `background-<hash>.png` 与 `@2x`，作为 classicui 背景的叠图居中贴在卡片上、裁到卡片里（四边各收进描边宽度和圆角处的弧高，矩形裁剪才不会从圆角伸出去）。classicui 画主题时不知道卡片会多大，背景图只能是这样一张固定大小的图：卡片比估的大时四周露出卡片底色，比它小时只看得到中间一块，与 macOS 按实际卡片尺寸铺排不完全一样。背景和装饰共用 classicui 唯一的一张叠图，装饰图画得出来时背景图让给它；canvas 包不画背景图。桌面面板路径中，皮肤的翻译色（`translation`，在解析结果里是与序号不同的 `secondary`）由 IBus 以区段前景属性画在释义上，是否支持区段属性取决于桌面；classicui 的一条候选只能是一种颜色。下文的 Fcitx5 原生 X11 画布直接绘制释义及其词性、分隔符的独立配色。Plasma 的 kimpanel 和 GNOME Shell 面板不使用 classicui 主题。Fcitx5 宿主在当前界面是 kimpanel 时报告 `kimpanel`，在 classicui 的 `Theme` 或 `DarkTheme` 是用户所选的第三方主题时报告 `fcitx_theme`，设置页据此提示颜色与皮肤不会生效。

要手工维护水杉的 classicui 主题，可设置 `MSIME_FCITX5_THEME_AUTOGENERATE=0`。先正常启动一次生成主题，退出 Fcitx5，再带此变量启动，例如 `MSIME_FCITX5_THEME_AUTOGENERATE=0 fcitx5`；自动启动 Fcitx5 时也需给它的进程设置同一变量。随后可编辑 `themes/msime/theme.conf` 和同目录图片，重启或通过配置工具重载主题。要让卸载和换包保留这份主题，把 `[Metadata]` 的 `Description` 改成自己的说明（或放进一张不以 `shape-`、`decoration-`、`background-`、`canvas-` 开头的图片）：`msime-linux-setup --unregister` 只删除仍是生成原样的主题目录，见「卸载 CMake 安装」。设置页此时提示候选配色和皮肤不生效，与选了第三方 classicui 主题时相同。关闭后插件不生成或清理这些文件，不接管 `Theme`／`DarkTheme`，X11 也由经典界面绘制人工主题；缺失的主题目录不会被创建。候选配色、皮肤、标志和整体大小等偏好不再写入这份主题，字体同步仍独立生效。取消该变量或改为 `1` 并重启，恢复自动生成；恢复前请先备份定制文件，因为下一次主题同步会按偏好重建。

默认自动生成并非逐键写盘：每次 Fcitx5 启动（或「重启输入法服务」）后的第一次主题同步都会按当前偏好重建一次，之后只在主题解析输入变化（配色、系统明暗、布局、候选大小／内边距／不透明度、标志、所选皮肤及其图片文件戳）时才重建；文件内容相同时不重写。主动选择水杉主题可重新接管经典界面，输入未变化时复用已经生成的文件；写入失败按 10 秒间隔重试。普通形状主题的左、上切片向上对齐到 4 逻辑像素，并在标识之后留至少 2 像素的纯色安全带：这两条切线从面板原点量起，对齐后在 1.25／1.5 倍显示缩放下落在整设备像素上；右、下切线由 classicui 从按内容决定的面板宽高往回量，对齐不起作用，保持只容下阴影和圆角，避免抬高面板最小尺寸。源 PNG 最小尺寸可能增加，但内容、标识和阴影的定位不变。外部 canvas 包仍使用包自己的切片。

`tsf_preedit_style` 在 Linux IBus 中映射为：`raw` 显示 Engine 的 ASCII `editing_text`，`pinyin` 显示 Engine 的 `preedit`，`empty` 隐藏预编辑；设置热重载会更新当前会话的显示样式。IBus 预编辑（包括语音的流式预编辑）与 Fcitx5 一样整段加单下划线，是 Windows 组合串点状下划线（`TF_LS_DOT`）在 Linux 上的对应形式；`empty` 样式和清除预编辑时不带下划线。正在组词（`phrase_prefix` 非空）时，已选的那一段和还在输入的读音分成两个子句，对应 macOS 的细、粗两种下划线：IBus 是单下划线加双下划线，和整句改字的焦点子句同一种画法，Fcitx5 没有粗下划线这种格式（`Bold` 在 fcitx5-gtk、fcitx5-qt 里是字形加粗，会把字母画粗），所以是下划线加「下划线并高亮」，同样和整句改字的焦点子句一致；不认这些属性的客户端仍只画一条下划线。候选与上屏仍由 Engine 的共享状态决定。

`candidate_preedit_style` 在 Linux 中映射为候选面板辅助文本：`pinyin` 显示当前拼音，`empty` 隐藏拼音。`show_candidate_page_number` 独立控制当前页/总页数，默认 `true` 保持旧版行为；设置页「候选窗口 → 预编辑 → 显示页码」可以关闭它。两者都隐藏且没有模式标签或连击提示时，辅助行不显示。IBus 与 Fcitx5 都支持此设置，设置热重载立即更新现有会话，翻页快捷键和候选选择不受影响。

双拼方案提供 IBus 属性“双拼原始预编辑”，对应共享 `shuangpin_preedit_uses_raw`：开启时预编辑保留原始双拼编码，关闭时显示 Engine 展开的拼音。该选项仅在双拼方案下可用；有共享偏好目录时按 revision 持久化并由 Engine 在组合空闲后应用，没有偏好目录时切换会结束当前组合并重建当前会话。

共享设置页的“输入 → 双拼预编辑”在 Linux 上同样可见，对应 `HostCapabilities::shuangpin_preedit`。IBus 与 Fcitx5 都是自己把快照的 `preedit` 写进平台预编辑的，因此这个选择归宿主展示；此前该控件按平台名只给 macOS，Linux 用户只能在双拼方案生效时从原生状态菜单里找到它。

五笔方案提供 IBus 属性“五笔剩余编码”，对应共享 `wubi_code_hint`，默认开启；关闭后候选仍按 Engine 原文显示，但隐藏候选后的剩余五笔编码提示。该设置仅影响展示，不改变候选身份或提交文本；配置共享偏好目录时持久化，未配置时保留在当前会话。同一菜单还有“五笔四码唯一自动上屏”，对应共享 `wubi_auto_commit_unique`（默认开启），关闭后四码唯一的词停在候选列表里等空格或数字键选择；提交判定在共享 `crates/input-runtime`，两个前端都只是把值写进共享偏好文档。

Windows 的 `clipboard_history` 依赖独立剪贴板监听器和候选历史 UI；IBus Engine API 不提供剪贴板事件。Linux IBus 宿主只读取用户明确配置的历史文件，并通过属性菜单提供最近条目、删除和清空操作，不读取系统剪贴板，也不在输入线程监听剪贴板。Linux 桌面面板的剪贴板同步仍由独立 Tauri 服务承载。独立工具的 `get INDEX` 操作会将已存储条目写到标准输出，`remove-index INDEX` 按历史位置删除单个条目，供桌面服务或 compositor 显式接管粘贴和删除动作；它不会写入或读取系统剪贴板。

`candidate_theme` 是 Windows 候选窗口的整体深浅主题覆盖。IBus Engine 只提交 lookup table 内容与文本属性，候选 panel 的边框、间距和主题切换由桌面环境控制；Linux 保留共享设置，但不伪造 panel 主题覆盖。显式文字和表面色按 IBus 属性传递，编号色由文字颜色推出后同样传递。

个人词典维护使用共享 Host API 的独立 `msime-linux-dictionary` 原生入口，不由 IBus 输入线程执行。它从标准输入读取一个不超过 65536 字节的 JSON 请求并输出 JSON 响应；请求格式和 `list`/`edit` 操作见 `msime_client.h`。`list` 不带词库类型和编码前缀时只列出用户自己添加的词；指定类型并给出编码前缀（快捷短语不需要前缀）时，同时按前缀查到随输入法附带的内置词条，用户词排在前面，每条带 `source`（`user` 或 `bundled`）。内置词条只能调整权重或删除：`edit` 的替换项必须保持类型、编码和词不变，否则返回 `bundled dictionary entry is read-only`；改动与 Windows 一样记入用户词库日志，词库升级重放后仍然生效。拼音词库的 `export` 除用户词外还带出对内置词学到或设置过的权重，并与 Windows 一样略去单字；五笔、英文和快捷短语只导出用户词。调用方必须在编辑前停止使用相关词典的会话，API 负责共享访问锁、请求幂等和 Engine 原子写入；错误输出不包含词条内容。该入口不替代桌面设置页，便于 GTK/Qt 前端复用同一契约。

该入口也支持本地词库批量迁移：`import` 接受不超过 64 KiB、最多 1000 行的 UTF-8 文本，`standard` 格式为 `词条<TAB>编码<TAB>权重`，`windows` 格式为 `编码<TAB>词条<TAB>权重`，`rime` 格式兼容 `userdb.txt/dict.yaml` 的 `词条<TAB>编码[<TAB>权重]`、YAML 头和 `c=… d=…` 元数据；拼音词库的 `hans` 格式则每行接受纯汉字词条，由 Engine 从已验证主词典解析最高权重的规范拼音并以 10000 导入。省略权重时使用 10000。空行和 `#` 注释会跳过。调用方提供请求 ID 前缀，入口为每行生成稳定回执，重复提交同一请求安全。`export` 按页返回相同两种格式的文本和 `has_more`；设置窗口把拼好的文件交给 Tauri `save_export` 写进用户的「下载」文件夹（重名时另起 `name (2).txt`），并在页面上提示写入的完整路径。这一入口契约不变；桌面设置页导入不超过 32 MB 的文件时（更大的文件在读取前就被拒绝，提示拆分后分别导入），会在行边界把它拆成若干满足上述限制的请求（请求 ID 为 `<ID>-<序号>`，Rime YAML 头以空行代替），按整份文件的列顺序逐批导入，并把各批结果合并成一份报告，失败行号按整份文件计算；整份文件因控制字符、没有可用行等原因会被入口拒绝时，设置页在写入任何一批之前就拒绝。导入取得独占维护锁。活动 IBus / Fcitx5 会话持有共享锁时，设置窗口会在用户数据目录写入带过期时间的 `.msime-dictionary-quiesce` 租约（最长 30 秒）：两个宿主在偏好轮询时（Fcitx5 每 250 ms、IBus 每秒）上屏当前组合、关闭会话，租约存在期间不再打开新会话；设置窗口在约 2.5 秒内重试获取锁，分批导入时租约在各批之间保持并在每批前续期，整次操作结束后立即删除租约，仍取不到锁才返回 busy。导出使用共享锁，不会中断用户组合。

设置窗口的云词库面板（设置页「云词库」分类、IBus 菜单和 Fcitx5 状态栏打开的独立面板、`--panel cloud-dictionary`）默认经设置应用登录的水杉账号完成请求，与 Windows 和没有原生会话的 macOS 设置进程同一条路（`apps/desktop/src-tauri/src/platform/desktop/desktop_cloud_dictionary.rs`）：列表、目录、增删改、导入导出、候选调频和固定排位，以及完整云词库备份（导出、选文件预览、恢复到云端）和「应用到本机」。完整快照的下载预览和中转文件放在设置应用自己的本地数据目录（`$XDG_DATA_HOME/<identifier>/dictionary-snapshots`）；「应用到本机」把校验过的快照放进状态目录下的持久队列 `dictionary-snapshots`，挂上 IBus 和 Fcitx5 都认的词库维护租约让输入会话放开词库，在设置进程里准备并激活新的一代词库（暂存在 `dictionary-snapshot-staging`），再交还会话。两个宿主都不自己处理这个队列，所以和 Windows 一样由面板当场应用、开着时每分钟重试，面板不说「输入法空闲时应用」。导出和完整备份经宿主写进「下载」目录（`save_export`，单个文件上限 64 MiB），因为 WebKitGTK 和 macOS 的 WKWebView 一样没有下载处理器；恢复用页面自己的文件选择。配置了 provider socket（下文）或在默认位置发现 `cloud-dictionary.sock` 时仍先走 provider，provider 失败时直接报错，不改走账号。

账号云词库也可以使用独立的 `msime-linux-cloud-dictionary /absolute/provider.sock` 入口。它验证 `list`、`changes`、`add`、`update`、`delete`、`import` 和 `export` 请求后，经用户管理的 Unix socket 转发一行 `{"version":1,"kind":"cloud_dictionary","request":...}`；provider 负责登录态、凭据、网络和冲突同步，入口只输出有界 JSON 响应，不保存账号信息。Tauri 设置页通过 `cloud_dictionary_provider_socket` 或 `MSIME_CLOUD_DICTIONARY_PROVIDER_SOCKET` 接入同一 provider，提供词库选择、搜索分页、词条 CRUD，以及标准 TSV、Windows TSV 和拼音汉字自动注音导入。

### 账号

Linux 桌面外壳现在提供与 Windows、macOS 同样的九个账号命令（`account_status`、`account_providers`、`account_request_code`、`account_login`、`account_profile`、`account_rename`、`account_logout`、`account_delete`、`account_forget`），共享设置页的账号分类因此在 Linux 上可用。此前这些命令只为 Windows、macOS、Android、iOS 注册，`main.tsx` 也只在前两者注入 `account` 客户端，Linux 上整个账号界面没有宿主可调。

登录后，账号页还有和 Windows 相同的「设置同步」卡片（四个 `account_preferences_*` 命令，`platform/desktop/desktop_settings_sync.rs` 与 Windows 共用）：上传、应用的只有各平台共有的 `input.*` 键（输入方案、繁简、双拼和五笔版本、词库学习、调频和标点），应用走与设置页保存相同的路径，同步进 `runtime-options.json` 后两个宿主当场读到。账号文档里没有 Linux 自己的键，候选窗口、快捷键和皮肤这些桌面设置不随设置同步。社区页的「词包与回复模板」也与 Windows 一样接在这个会话上（`community_resource_*` 命令）：浏览、收藏、评分、举报和发布，词包可以导入本机个人词库或账号云词库；桌面没有「高情商回复」键盘，回复模板不能存进本机。

命令本体与 Windows 完全一致，差别只在会话文件所在的目录：Linux 没有一个所有目标桌面都保证在跑的密钥服务，因此会话保存在共享状态目录下的 `account-session.json`，以 0600 创建、按临时文件加原子重命名发布，读取前核对是普通文件（不跟随符号链接）、属主是当前用户、其他用户无任何权限、且大小不超过 64 KiB；不满足时报告存储错误而不是伪装成「未登录」。这与 Linux provider 私有配置文件一直声明的规则相同，也沿用 `msime-linux-prepare` 建立的 0700 状态目录。**它不加密静态数据**；macOS 与 Windows 桌面端现在用的是同一个文件存储（`crates/client-services/src/account/file_storage.rs`），不再用 Keychain 或凭据管理器。令牌只在桌面宿主进程内，交给 WebView 的仍是与其他平台相同的脱敏 DTO。

云剪贴板使用独立的 `msime-linux-cloud-clipboard /absolute/provider.sock` 入口。它验证列表、明确添加、删除和启停请求后，经同一类用户管理服务转发 `{"version":1,"kind":"cloud_clipboard","request":...}`；服务负责账号凭据、云端保留和冲突处理，不自动读取本地剪贴板。

桌面设置可在 HostOptions 中配置 `cloud_dictionary_provider_socket` 和 `cloud_clipboard_provider_socket` 两个绝对 Unix socket；未配置时分别回退到 `MSIME_CLOUD_DICTIONARY_PROVIDER_SOCKET` 和 `MSIME_CLOUD_CLIPBOARD_PROVIDER_SOCKET`。这两个字段会随 runtime-options 原样保留，但不会进入 Engine 选项或输入会话。

共享设置页「剪贴板」的「云剪贴板同步方式」可以改成自建中继（地址和令牌存在共享偏好的 `cloud_clipboard_relay`，协议与部署见 `docs/cloud-clipboard-relay.md`）。设置窗口和云剪贴板面板此后按这个顺序选路：配置的 `cloud_clipboard_provider_socket`（或环境变量），然后是中继，然后是自动发现的 socket，最后是水杉账号。中继的本机密钥串 `cloud-clipboard-relay-keystore` 与 `account-session.json` 在同一个状态目录。Fcitx5 键盘里的云剪贴板栏也按显式 socket、中继、自动发现的 socket 选路；没有 socket 时可直接读取中继列表，后台请求失败不转发到其他服务。键盘栏与设置面板共用中继密钥串，地址、令牌或状态目录变化会清空旧列表；新设备的批准仍在云剪贴板面板完成。

IBus 面板注册 `InputMode` 开关：选中时按当前输入方案转换，关闭时直接透传编辑器输入。面板符号与 macOS 的输入模式图标同一套字：全拼为「中」，双拼为「双」，五笔为「五」，直接输入为「英」，日语方案开启转换时为「日」，韩语方案为谚文「한」，粤拼、注音、笔画、越南文、藏文方案分别为「粤」「注」「笔」「越」「藏」，CapsLock 打开时无论哪种模式都显示「⇪」（与 Windows 语言栏图标的优先级一致，macOS 没有这一项），不把日语、韩语等已配置方案误标为中文。Fcitx5 宿主通过输入法子模式标签在托盘和面板上显示同样的状态：「中」「双」「五」「英」「粤」「注」「笔」「日」「한」「越」「藏」「⇪」。CapsLock 状态取自按键事件（松开 CapsLock 时即更新）。切到直接输入前通过 Engine 完成高亮组合；再次聚焦保留当前实例的选择，关闭期间的候选点击和翻页无效。密码等受限字段及失焦时开关不可用，恢复正常字段后继续使用原选择。按 `ime_mode_scope` 分别记忆客户端或共享全局状态，不写回偏好文件或重启持久化，也不占用桌面已有的输入源切换快捷键。

`ime_mode_scope` 可设为 `app` 或 `global`。按应用时按 IBus 提供的客户端身份分别记忆每个编辑器的中英文状态；没有客户端身份时回退到 `default_ime_mode`。状态表有界且只保存在当前 IBus 进程内。设为全局时，当前 IBus 进程的输入上下文在获得焦点时同步同一个中英文状态；该状态不写回偏好文件，也不干预桌面环境已有的输入源切换。

`default_ime_mode=chinese` 在宿主实例初始化时选择中文输入，普通字母进入 Engine 组合；新建共享偏好或字段缺失时也使用 Windows 基线的中文状态。显式设置为 `english` 时则直接透传普通字母，切回中文后使用中文候选。英文透传不启用独立的“英文输入模式”（英文候选），后者仍由菜单或快捷键控制。重新聚焦或重建 Engine 会话保留用户已经选择的中英文状态，不重新套用启动默认值。切换到其他输入源时，IBus 的 Disable 会清除全局中英文记忆并恢复宿主默认状态；切回后按当前配置默认模式开始，普通 FocusOut/FocusIn 不触发这一重置。

英文透传且尚未创建 Engine 会话时，宿主仍加载中英文切换快捷键等宿主偏好，并应用偏好文件热重载。配置的 Ctrl 切换键可直接启用中文输入；关闭的 Shift 切换键不会因没有输入会话而回退为启用。

共享 `word_character` 支持方括号或减号/等号选择高亮候选的首/末汉字，按 Windows 基线默认启用方括号；与同组翻页配置互斥，由共享配置校验拒绝冲突。旧偏好或 Linux 宿主选项缺少该对象时使用相同默认值。启动及实时设置发布均更新绑定，活动组合保留；宿主将当前候选代次和全局索引传给 Engine，不自行切分汉字。无汉字候选走共享组合完成与标点路径，关闭功能后恢复通常的标点输入；Shift 符号不触发以词定字。

需要 Linux、Rust 1.97.1、CMake 3.25+、C++17 编译器、IBus 1.5.20+ 开发包、nlohmann-json 3.11+、ALSA 开发包（`libasound2-dev`，Host API 的录音采集经 cpal 链接它，由 pkg-config 查找）。SQLite 由 rusqlite 的 bundled 特性编进 Host API，不需要系统开发包。通用 Wayland 的全局面板注入可选安装 `ydotool` 并运行 `ydotoold`；没有它时仍尝试 `wtype`。原生构建：

```sh
cargo build -p msime-host-api --locked
cmake -S platforms/linux -B target/linux-ibus
cmake --build target/linux-ibus
cargo run -p msime-host-api --example prepare_host --locked -- /absolute/verified-resources /absolute/new-state
target/linux-ibus/msime-linux-ibus /absolute/new-state/runtime-options.json
```

`BUILD_TESTING` 默认开启，构建原生单测和 `ibus-engine-smoke` 等验收工具。只构建产品时，在 CMake 配置命令中传入 `-DBUILD_TESTING=OFF`，测试目标不再生成；IBus 宿主、Fcitx5 插件和 provider 入口仍构建、安装。`MSIME_LINUX_VOICE=ON` 的 worker 测试也要求开启 `BUILD_TESTING`。容器门禁保留 full 的完整测试构建，五笔版关闭测试，并通过 CMake 实际目标图核对两边的生产目标一致。

准备配置必须在没有会话使用该状态目录时执行。运行入口动态注册独立的 `msime-linux`，不安装系统组件、不修改旧 Linux 产品或自动切换用户输入法；关闭进程即结束本次注册。安装后的 component 通过 `msime-linux-ibus-launcher` 启动，默认读取 `~/.config/msime-client/runtime-options.json`；也可用 `MSIME_IBUS_OPTIONS` 指向已准备好的绝对路径。launcher 按自身目录定位 Engine，支持自定义安装前缀。上面这条命令直接跑构建目录里的二进制，路径指向源码树，面向开发调试；面向发行的包见「生成 Linux 安装包」。宿主监听配置 JSON 的写入和原子替换事件；后续新焦点会话使用新配置，正在组合的会话保持原设置直到结束。

安装产物提供 `msime-linux-prepare`，首次准备状态无需 Cargo 或源码目录：

```sh
msime-linux-prepare /absolute/verified-resources /absolute/new-state
export MSIME_IBUS_OPTIONS=/absolute/new-state/runtime-options.json
msime-linux-ibus-launcher
```

两个参数必须是绝对路径，状态目录的父目录必须已存在；全新状态目录不能已有文件，但安装流程预先写入的 `anonymous-account.json` 或 `anonymous-session.json` 可以保留在其中。命令通过 Host API 按固定的 `desktop-dictionary.lock.json` 校验资源目录及其资源，再准备 Engine 用户数据、缓存和偏好配置；以 0700 创建状态目录，以 0600 原子发布 `runtime-options.json`，成功时输出配置路径。失败时保留已准备的数据，不覆盖已有目录或配置；重试需另选全新目录。若希望启动器自动发现配置，可将新状态目录选为 `$XDG_CONFIG_HOME/msime-client`（未设置时为 `$HOME/.config/msime-client`），并事先准备其父目录。自定义位置的 `MSIME_IBUS_OPTIONS` 需传入实际启动 IBus 的会话环境。

若安装时已在 CMake 配置阶段传入 `-DMSIME_ENGINE_RESOURCES=/absolute/verified-resources`，CMake 会按仓库内固定的 `resources/desktop-dictionary.lock.json` 校验每个词库文件的名称、大小和 SHA-256，并将这些文件安装到 `${CMAKE_INSTALL_DATADIR}/msime-client/resources`；仓库自带的七套辅助码表（`resources/helpcodes`，不在词库发布里）装进其中的 `helpcodes/`，Engine 从资源目录下的这个子目录读辅助码表，宿主校验放行它；锁文件作为同级的来源元数据安装，不会混入 Engine 运行目录。辅助码表无条件随安装包提供；`msime-linux-setup` 首次配置时复制到下载或自备词库目录，已有安装运行 `msime-linux-setup --update` 即可补齐，不需要重新下载词库；`helpcodes/custom` 中的自定义表保留。这样可以直接使用已安装资源准备状态：

```sh
msime-linux-prepare --installed /absolute/new-state
```

非英文翻译目标（法、日、西、俄、德、韩）的离线释义词典是可选的：用 `scripts/build_offline_glosses.py` 生成到 `target/offline-glosses` 后，配置时传 `-DMSIME_OFFLINE_GLOSSES=/absolute/target/offline-glosses`，CMake 把其中的 `zh-*.db` 连同必需的 `offline-glosses-NOTICE.txt` 装到资源目录的同级 `${CMAKE_INSTALL_DATADIR}/msime-client/offline-glosses`。使用显式资源目录时，把它们放在该目录同级的 `offline-glosses/` 下即可。IBus 与 Fcitx5 在主翻译目标装有词典时先显示词典释义，开启候选翻译且配置了自己的翻译服务时再逐个询问所有候选，服务的回答替换词典的，没回答的保留词典释义。没有这些文件时候选释义仍只有英文。

背单词的内置词书（中考到 GRE 八本）由 `python3 scripts/fetch_wordbooks.py` 从锁定的 ECDICT 提交生成到 `target/wordbooks`；配置时 CMake 自动从那里取，也可以传 `-DMSIME_WORDBOOKS=/absolute/path`，把八本 `*.json` 连同必需的 `wordbooks-NOTICE.txt` 装到资源目录的同级 `${CMAKE_INSTALL_DATADIR}/msime-client/wordbooks`。设置窗口在 HostOptions 里 `resources` 的上一级找词书（词库在线更新后 `resources` 指向下载代次，这时回到原安装包的资源目录），所以使用显式资源目录时，把它们放在该目录同级的 `wordbooks/` 下即可。`-DMSIME_REQUIRE_WORDBOOKS=ON`（发版和各发行版定义都这样设）让缺了 `resources/wordbook.lock.json` 列出的任何一本的配置失败。没有它们时背单词只列用户导入的词表。

粤拼、注音与笔画的词库同样是可选的：`scripts/fetch_language_dictionaries.py` 按 `resources/language-dictionaries.lock.json` 下载到 `target/language-dictionaries`（锁还没有发布时只打印 skipped 并成功退出），配置时自动使用该目录，也可以传 `-DMSIME_LANGUAGE_DICTIONARIES=/absolute/dir`。CMake 把其中的 `msime-cantonese.db` 连同 `msime-rime_cantonese_LICENSE.txt`、`msime-zhuyin.db` 连同 `msime-libchewing_data_LICENSE.txt`、`msime-stroke.db` 连同 `msime-rime_stroke_LICENSE.txt` 装到资源目录的同级 `${CMAKE_INSTALL_DATADIR}/msime-client/language-dictionaries`；词库旁缺少许可证时配置直接失败，不会只装数据。`-DMSIME_REQUIRE_LANGUAGE_DICTIONARIES=ON`（`package-container.sh` 下是环境变量 `MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1`）要求 `resources/language-dictionaries.lock.json` 固定的每一份都在，否则配置失败；锁固定 `msime-stroke.db` 之前笔画词库不在必需之列。用 `msime-linux-setup --download` 自己下载到 `$XDG_DATA_HOME/msime-client/resources` 的资源目录旁没有这份目录，打包时没有备齐这几份词库的安装也是如此。这时设置应用的资源包列表会提供「粤语、注音与笔画词库」，与 macOS 同一个资源包，下载、校验后装到状态目录的 `resource-packs/language-dictionaries/`；宿主下一次获得焦点时就能用上，不需要重启。随包带齐三份词库时不列出它。也可以手动把词库放到资源目录同级的 `language-dictionaries/` 下。越南文和藏文不需要任何数据。

`--installed` 通过 `/proc/self/exe` 的实际路径和配置时的数据目录相对位置定位同一安装前缀下的资源目录，状态目录仍必须是绝对路径且不存在。它不会修改输入法选择、启动服务或创建用户状态，只有显式执行命令才会准备新状态；未配置资源包时请继续使用显式资源目录形式。

Linux 桌面设置保存时会先按 `PreferencesStore` 的 revision 规则写入 `preferences.json`，随后以原子替换同步同一 HostOptions 的 `preferences` 到 `MSIME_IBUS_OPTIONS`，或 `MSIME_CLIENT_HOST_OPTIONS` 指向的 `runtime-options.json`；未设置前者时，桌面应用也可直接用 `MSIME_IBUS_OPTIONS` 作为 HostOptions 来源。这样正在运行的 IBus 宿主可以通过已有文件监听接收新设置；同步失败会把保存命令报告为存储错误，避免界面误报已同步。发布给宿主的是一份去掉屏幕键盘自定义皮肤照片（`custom_theme.keyboard.photo`）的副本：IBus 和 Fcitx5 宿主不画屏幕键盘，设置应用自己的屏幕键盘读的是 `preferences.json`，照片仍在那里；整份文件不超过两个宿主读取的 16 KiB 上限，超出时（例如提示词过长）不写入，原文件保持可读，这次保存也整体撤回并报错（`runtime_options_too_large`），而不是写出一份让两个宿主都无法启动的配置。

Linux Tauri 设置窗口也会监视同一 `PreferencesStore` 的 revision。其他窗口或 IBus 侧写入新 revision 后，未编辑的设置页自动刷新；若当前有未保存草稿，只提示外部变更并保留草稿，用户通过“重新读取”显式解决冲突。事件只携带已验证的偏好快照，不携带输入内容或凭据。

设置页「关于 → 数据目录」可以把词库、学习记录、缓存、偏好、皮肤和剪贴板历史移到另一个空目录（例如另一块磁盘）。目录用桌面自带的选择器挑选：KDE 下优先 `kdialog`，其他桌面优先 `zenity`，两者都没有时设置页提示安装。Linux 上默认状态目录 `$XDG_CONFIG_HOME/msime-client` 同时是 IBus 启动器、Fcitx5 插件、剪贴板监视服务和设置启动器读取 `runtime-options.json` 的固定位置，在线服务和语音服务也从这里读取凭据，所以移动时这个目录本身不动：`runtime-options.json` 和三个 provider 凭据文件留在原处，复制开始前设置窗口先在旧的 `user` 目录写入词库维护用的 quiesce lease，并等到拿到词库的独占锁：两个宿主都在计时器上看到 lease 后结束组字、关闭会话，锁拿到才说明它们确实放手了，此后直到迁移结束都不会有会话在旧目录打开（lease 过期也挡得住），翻译词义缓存的写入也要同一把锁，一并被挡住；两个宿主的菜单偏好保存在 lease 生效期间同样暂缓（按保存失败处理，可重试），旧 `user` 目录被移走后，仍按旧配置发起的保存直接放弃，不会在旧目录里重建 `preferences.json`，Fcitx5 每次保存前都重读 locator，保存到它当前指向的目录；约 2.5 秒内还有会话不放手就以“输入法仍在使用数据目录”失败，什么都不复制。没有运行的宿主不持有锁，不会让迁移一直忙。其余状态条目先复制到目标目录内的临时目录（lease 文件不复制），再逐个 rename 到位，然后只把 `runtime-options.json` 里位于旧目录下的绝对路径改写到新目录，provider socket、模型等其他键原样保留。任一步失败都会回滚已放置的条目和 locator，原数据不动。发布成功后才删除旧目录中的这些条目，先整体 rename 进旧目录内的临时目录再删除，仍按旧路径打开会话的宿主只会找不到目录而打不开，不会在半空的旧目录里重建空词库；删除只成功一部分时，删不掉的条目放回原来的名字，设置页提示旧数据已保留，用户能在原处找到它们，以 `.msime-data-migration-` 开头的遗留临时目录以后既不会被当作状态复制，也不会让目标目录被判为非空；目标目录写入 `.metasequoiaime-data` 标记，以后再移走时整个目录只在确认归水杉所有时才删除，否则保留并告知用户。移回默认目录同样可行。完成后仍在持有 lease 和锁的时候重启当前输入法框架（Fcitx5 经 `gdbus` 调用 controller 的 `ReloadAddonConfig`（参数 `msime`）重置水杉插件，见上文 Fcitx5 的 `Ctrl+Shift+Alt+R` 一段；否则 `ibus restart`），再放开旧目录，宿主按改写后的 locator 在新目录重新打开会话；重启失败不会撤销已完成的迁移，设置页会提示手动重启输入法，窗口多停留几秒再关闭。剪贴板监视服务每轮都重读 locator，无需重启；设置窗口随后关闭。

安装时可使用 `cmake --install target/linux-ibus`。安装产物包含 IBus 主程序、`msime-linux-online` 在线候选请求入口、`msime-linux-translation` 候选翻译请求入口、`msime-linux-dictionary` 个人词典请求入口、`msime-linux-cloud-dictionary` 云词库请求入口、`msime-linux-cloud-clipboard` 云剪贴板请求入口、`msime-linux-clipboard` 剪贴板历史工具、`msime-linux-handwriting` 手写识别请求入口、`msime-linux-voice` 语音识别请求入口和 `msime-linux-emoji` Emoji 目录请求入口；工具与主程序使用相同的安装前缀。需要预置系统配置时，在 CMake 配置阶段传入 `-DMSIME_RUNTIME_OPTIONS_FILE=/absolute/runtime-options.json`，安装到 `${CMAKE_INSTALL_SYSCONFDIR}/msime-client/runtime-options.json`。该文件必须来自已准备且匹配安装环境的状态目录，不能直接分发开发机上的私人状态。

CMake 配置时可传入 `-DMSIME_EMOJI_RESOURCES=/absolute/emoji-resources`，安装会将该受信任目录复制到 `${CMAKE_INSTALL_DATADIR}/msime-client/emoji`，供 `msime-linux-emoji --local` 自动发现；未提供时不会从未验证的相邻仓库或网络下载资源。

Emoji 本地 CLI 的 `msime-linux-emoji --local` 会按显式资源目录、其中包含 `msime-others.db` 的 `MSIME_EMOJI_RESOURCES`、`$XDG_DATA_HOME/msime-client/emoji`、`$XDG_DATA_DIRS/*/msime-client/emoji`、安装前缀和系统数据目录顺序查找资源。显式传入路径优先；未找到时返回错误，不访问网络。这样发行版安装后的 Emoji 面板不要求用户手工复制 Windows 风格资源路径。

`msime-linux-handwriting --local` 也会按显式模型路径、`MSIME_HANDWRITING_MODEL`、`$XDG_DATA_HOME`、`$XDG_DATA_DIRS`、安装前缀和系统目录自动查找模型，都没有时再看设置应用下载到默认数据目录的那份（`$XDG_CONFIG_HOME/<客户端目录>/resource-packs/handwriting/`；数据目录移到别处后要显式指定）；未找到模型时不访问网络。

离线中文手写模型（zinnia 格式，26.8 MB，LGPL-2.1）不进版本库：`resources/handwriting-model.lock.json` 按 msime-engine 固定提交的 HTTPS 地址、字节数和 SHA-256 锁定模型与许可证，`python3 scripts/fetch_handwriting_model.py` 把两者下载到 `target/handwriting-model`（`--out` 可改），不符即丢弃、已符合则跳过。CMake 在该目录存在时自动使用（也可 `-DMSIME_HANDWRITING_MODEL_DIR=/absolute/dir` 指定），逐个按锁校验后装到 `${CMAKE_INSTALL_DATADIR}/msime-client/handwriting`，锁本身装到 `${CMAKE_INSTALL_DATADIR}/msime-client` 作来源记录。随不随包由 `MSIME_BUNDLE_HANDWRITING_MODEL` 决定：默认 `ON`，打包时缺少模型直接失败，`packaging/` 下各发行版的定义和 Nix 都按这个默认随包；发布页的 deb/rpm 由 `package-container.sh` 以 `-DMSIME_BUNDLE_HANDWRITING_MODEL=OFF` 配置，不下载也不安装模型和它的许可证，用户需要手写时由设置应用按同一份锁把两者下载到自己的状态目录（手写资源包）。面板应只引用受信任的安装路径或这份按锁校验过的下载。

若要把 Tauri 设置窗口一并安装，可先用 `pnpm --filter @msime/desktop tauri build --no-bundle` 生成 Linux 二进制，再在 CMake 配置阶段传入 `-DMSIME_DESKTOP_BINARY=/absolute/path/to/msime-desktop`。安装会增加 `msime-linux-desktop`、`msime-linux-settings` 和桌面菜单项；设置启动器按 `MSIME_CLIENT_HOST_OPTIONS`、`MSIME_IBUS_OPTIONS`、用户配置路径、安装时配置的系统配置路径的顺序选择绝对 runtime-options，并把它传给 Tauri 宿主，不把开发机路径写入桌面文件。加载了 NVIDIA 专有驱动内核模块（`/proc/driver/nvidia/version` 或 `/sys/module/nvidia` 存在，nvidia-open 也算）时，启动器默认设置 `WEBKIT_DISABLE_COMPOSITING_MODE=1` 与 `WEBKIT_DISABLE_DMABUF_RENDERER=1`，规避 NVIDIA 上的 GBM 缓冲分配失败与 Wayland 显式同步协议错误（NVIDIA 驱动加 KWin 时 DMA-BUF 渲染器会让窗口一启动就报 Error 71 退出，只禁用合成不够）；其他机器上两个变量都不设，由 WebKit 自己决定渲染路径。终端、桌面菜单及输入法菜单入口都经过这个启动器，得到同样的默认值；不强制切换 GTK 后端。调用方显式给出的非空值原样保留，空值按未设置处理；遇到设置窗口闪烁、打不开或空白时怎么覆盖，见上文「设置窗口闪烁、打不开或空白」。两个变量都让设置页退回 CPU 绘制：右侧内容区每滚一步都整块重绘，带模糊的阴影每次都要重新模糊一遍，所以 Linux 下的分组和皮肤卡片只用不带模糊的边框与硬阴影（#6403），NVIDIA 用户仍走这条路径；往设置页里加带模糊的 `box-shadow`、`filter: blur` 或 `backdrop-filter` 之前，先在关掉合成的 WebKitGTK 里量一下滚动。设置页的“语音输入”分类可打开独立语音面板，面板调用同一 provider 并把识别结果提交到打开前捕获的编辑器。Linux IBus 与 Fcitx5 菜单顶层的“词库…”“水杉输入法设置…”“关于水杉输入法…”以及“桌面工具”中的“使用帮助…”“问题反馈…”分别路由到共享 Tauri 的对应设置页（`msime-linux-settings --panel dictionary|settings|about|help|feedback` 同样如此，`--panel account` 打开账号页）；“快捷键”分类提供“重启输入法服务”按钮：先用 `fcitx5-remote --check` 探测当前会话，Fcitx5 正在运行时经 `gdbus` 调用它的 `ReloadAddonConfig`（参数 `msime`）重置水杉插件，否则调用当前用户的 `ibus restart`。探测不会通过 D-Bus 启动一个原本未运行的 Fcitx5，也不会为了刷新 MSIME 杀掉承载其他输入法的整个 Fcitx5 进程。普通配置保存仍通过 runtime-options 文件热重载，不需要为了设置变更重启服务。

## 隔离验证

候选操作也通过 IBus 属性菜单提供：对当前候选页的 1–9 槽位分别注册固定和删除动作，动作携带当前视图代次调用共享 Host API；菜单只对可写入用户词库的中文/英文候选显示这些动作，云端、AI、Emoji、颜文字、快捷短语和日文候选保持只读。桌面 panel 不支持 Windows 式右键候选窗时仍可使用该菜单路径。

候选操作菜单还提供将当前词库候选固定到第 1–5 位及取消固定。固定位置由 Engine 持久化并随候选快照返回；Linux 宿主只传递候选身份和目标槽位，不复制词典写入或排序逻辑，并在 IBus 候选行显示“固定 N”状态。

`bash build-container.sh` 在容器里编译整个 Linux 原生宿主（IBus engine、Fcitx5 插件、全部 provider 入口和单测）并运行 `ctest`，不需要词库、不启动任何 daemon（只有 `ibus-mode-hint-x11-badge*` 两项、`ibus-mode-hint-x11-toolbar` 和 `linux-floating-panels-x11` 经 `xvfb-run` 在测试里起停一个 Xvfb，验证 IBus 在 X11 会话里画的切换徽章、悬浮工具栏的接线，以及工具栏的位置、按钮、提示、菜单、拖动和双拼键位图的摆放；后者设置了 `MSIME_TEST_CANVAS_ARTIFACTS` 时把截屏存成 PNG），也不需要本机是 Linux；它由 `scripts/verify-local.sh` 作为编译门禁自动调用，Linux 主机上则直接用系统 ibus 开发包跑同一套配置。镜像定义在 `tests/tools/Dockerfile.build-gate`，与隔离验收镜像分开，以免给后者加上会改变其构建内容的 X11/XFixes/Fcitx5 开发包。

`scripts/verify-local.sh` 的「compile: linux desktop shell」阶段在非 Linux 主机上用 `tests/tools/Dockerfile.desktop-check` 构建的镜像跑 `cargo check -p msime-desktop --locked --all-targets`：与编译门禁同一个固定摘要的 `rust:1.97.1-bookworm`，预装 Tauri 外壳需要的 webkit2gtk、gtk3、libsoup、javascriptcoregtk 和 cpal 需要的 ALSA 开发包，apt 只在 Dockerfile 变化后的第一次运行时执行，`--quick` 和 pre-push 钩子不再每次重装。镜像构建日志留在 `target/linux-desktop-check/image.log`，apt 失败时阶段打印其末尾并 FAIL。

原生门禁和桌面检查分别把 `CARGO_HOME` 放在已有挂载内的 `/build/cargo-home`、`/ctarget/cargo-home`，对应宿主的 `target/linux-build-gate/cargo-home`、`target/linux-desktop-check/cargo-home`。registry 和 git 下载缓存随构建目录保留，后续临时容器复用；首次运行或依赖变化仍可联网准备，日常命令继续使用 `--locked`。两个缓存按 checkout 与门禁隔离，不借用主机 Cargo home，删除 worktree 时一并清理。

**重复构建安装包。** 更新源码后重新执行 `bash platforms/linux/package-container.sh` 即可（命令从仓库根目录运行；legacy 用 `package-legacy-container.sh`）。Deb、RPM 和 legacy 镜像按 Dockerfile、平台、构建参数及 `rust-toolchain.toml` 的内容打标签；Deb 子镜像还纳入父镜像 ID。镜像已存在时直接复用，跳过 `docker build`；源码通过挂载读取，不进入镜像，源码或包版本更新不触发镜像重建。运行容器仍在退出时删除，构建所需状态保存在镜像和原有 target 挂载里。

打包的 Cargo 下载缓存放在 `target/linux-package/cargo-home`、`target/linux-package-rpm/cargo-home` 或 `target/linux-package-legacy-<arch>/cargo-home`，Rust 产物仍在各目录的 `cargo/`，各版本 CMake 对象文件留在原有 `cmake`、`cmake-<edition>` 目录。每轮重新生成 `CMakeCache.txt` 并清空 `dist/`，所以切换包版本、桌面开关或可选资源不会继承上一轮配置，也不会混入旧发行附件。`.package-image` 记录实际打包镜像 ID；首次使用新布局或镜像变化时清掉 Rust/CMake 编译产物，保留下载缓存，防止工具链和系统库变化后复用旧二进制。首次运行仍需联网准备依赖；缓存齐全后才可离线复用，不把 `--offline` 设为默认。

打包缓存随 worktree 移除；要单独重编译可删除对应目录下的 `.package-image`。内容标签镜像可跨 worktree 共用，不随目录删除自动回收；用 `docker image ls 'msime-linux-*'` 查找并按明确标签删除不再需要的打包镜像，下次打包会重建。Deb 父镜像另有 `msime-linux-build-gate:id-<镜像摘要>` 标签供 BuildKit 的 `FROM` 使用；它与配方标签指向同一镜像，不复制镜像层。日常原生门禁仍使用下面按 checkout 隔离的旧标签规则。

这两个门禁镜像、隔离验收基镜像（`tests/tools/Dockerfile`）和 RPM 打包镜像（`tests/tools/Dockerfile.package-rpm`）同时预装 `rust-toolchain.toml` 要求的 `rustfmt` 和 `clippy`，临时容器启动时不再逐次补装。Fcitx5 和现代 IBus 验收镜像继承验收基镜像，Deb 打包镜像继承原生门禁镜像。组件层放在已有构建层之后，修改组件准备步骤时可复用系统依赖、Wayland 驱动或 RPM 编译器安装缓存。构建或升级相应镜像后，在仓库根目录验证工具链就绪：

```sh
linux_gate_hash=$(printf %s "$PWD" | shasum | cut -c1-12)
bash platforms/linux/tests/tools/check-image-toolchain.sh \
  "msime-linux-build-gate:${linux_gate_hash}" \
  "msime-linux-desktop-check:${linux_gate_hash}"
# 验收和 RPM 镜像构建后，用各自按同一 checkout 哈希打出的 tag 检查。
bash platforms/linux/tests/tools/check-image-toolchain.sh \
  "msime-linux-test:${linux_gate_hash}" \
  "msime-linux-package-rpm:${linux_gate_hash}"
```

检查只读挂载真实仓库，在断网的新容器里执行 `cargo`、`cargo fmt` 和 `cargo clippy` 的版本命令。组件缺失或镜像工具链与仓库声明不匹配时无法临时下载补齐，检查失败。该检查只验证工具链准备；项目构建、真实输入法框架和发行包仍通过各自原有流程验证。

这两个日常门禁镜像都按 checkout 路径打 tag（`msime-linux-build-gate:<哈希>`、`msime-linux-desktop-check:<哈希>`，哈希取仓库绝对路径的 SHA-1 前 12 位），每个跑过门禁的 worktree 各留一份，单个占 2.4–3.3 GB，worktree 删除后不会自动回收。清理只删这两类 tag，不要 `docker system prune`（会连带别的项目和并发会话在用的镜像）：先 `docker images 'msime-linux-*'` 看有哪些，再 `docker image rm <tag>` 删掉已不存在的 worktree 对应的那些，最后 `docker image prune` 回收失去 tag 的悬空层。当前 checkout 的哈希可用 `printf %s "$PWD" | shasum | cut -c1-12` 在仓库根目录算出；删错了也无妨，下次运行会重建。

隔离验收脚本只读挂载源码，输入引擎是仓库里的 Rust crate，不再需要预先准备或借用任何 Engine 树，因此在 worktree 里也能直接跑。它的测试镜像与编译门禁一样按 checkout 路径打 tag，并发的 worktree 不会互相覆盖镜像。随包在线/语音/剪贴板 provider、凭据、豆包鉴权、翻译缓存、录音设备这一整片 Python 测试都在容器内执行。

`ibus-mode-hint-focus-preferences`／`-x11` 在真实 IBus 宿主中检查缺字段及显式关闭不提示、关闭切换提示后仍可单独开启聚焦提示，分别观察私有 D-Bus 辅助信号与 Xvfb 徽章。`fcitx5-mode-hint-preferences` 只在 Fcitx5 开发库不低于 5.1.11、编入自定义模式提示路径时登记，通过真实 GTK 编辑器反复聚焦、按 Shift 切换中英文并检查实际窗口像素，验证默认行为、两个开关的独立热更新及继续组字上屏。它们不代表 QQ 或 GNOME／niri Wayland 的现场验收。

Fcitx5 的 `tests/tools/check-container.sh /absolute/verified-resources --fcitx5` 还运行 `fcitx5-gtk-canvas-horizontal` 与 `fcitx5-gtk-canvas-vertical`，以及各自的 `-gif`、`-webp` 用例（合成图片需要 Pillow，专用验收镜像已包含）：真实 GTK 编辑器通过本次构建的插件唤起 classicui 候选，读取隔离 X11 显示像素，验证合成九宫格的明暗切换、坏图回退和修复恢复。期间不发送刷新按键，最终检查焦点保持、空格上屏与面板消失。在验收容器内直接运行 CTest 时，设置 `MSIME_TEST_CANVAS_ARTIFACTS` 为可写目录可保留各阶段截图。它覆盖 Xvfb／GTK／classicui，不包含 Wayland 合成器、真实字体或系统安装验收。

`fcitx5-candidate-font-pixels-horizontal`／`vertical` 在固定 96 DPI 的 Xvfb 中测量 classicui 序号像素，覆盖 20／32 像素字号、150% 整体缩放、偏好热更新、焦点和空格上屏。参考字形按绝对像素字号由 Pango 单独测量；测试核对 daemon 实际加载的 core 与 classicui 均来自 `pkg-config Fcitx5Core` 指向的库目录。跨版本验收时先用旧开发库构建插件，再令 `PATH`、`LD_LIBRARY_PATH` 与 `PKG_CONFIG_PATH` 指向同一份新运行库前缀，运行这两项 CTest。默认容器覆盖其实际安装的版本，不把替换版本入口当作新运行库验收；X11 像素测试也不替代 Arch／niri 的安装与 Wayland 现场验收。

同一验收还包含 `fcitx5-native-canvas-horizontal`／`vertical` 及各自的 `-scale2`、`-composited`：真实 GTK 编辑器检查原生 X11 窗口的挂件像素、独立行距热更新、随框架坐标移动、明暗、坏图回退与恢复、过期鼠标释放、滚轮翻页、左键上屏、右键置顶写入合成词库，以及失焦和切换输入法时隐藏。`-composited` 用专用 `xcompmgr` 检查实际 ARGB 窗口，其他用例覆盖无合成器的 Shape 回退。`linux-candidate-native-canvas` 在内存中检查几何、锚点、层级与透明中心，`linux-candidate-native-canvas-chrome` 检查 macOS 结构的首行、行与释义、网格、缩放、装饰带、调色板、定位和动效采样；它们不能替代这些真实窗口用例。`fcitx5-native-chrome-{horizontal,vertical}` 用 `system` 主题在真实 GTK 编辑器里验收默认画法：原生窗口出现、首行 › 与 ‹ 翻页、悬停不改变选中、横排 ↓ 展开网格和 Esc 收起、点选上屏，以及非 canvas 皮肤包的顶带装饰（带待机动效）画在卡片上方的透明带里、不压到候选行。另有 `fcitx5-native-canvas-{horizontal,vertical}-animation-{apng,gif,webp}` 六项真实 GTK 图片播放验收，覆盖连续输入、隐藏重显、同路径资源替换和九宫格边框动画。

`bash tests/tools/check-container.sh /absolute/verified-resources` 创建专用 Linux 容器，源码与词库只读挂载，构建缓存仅写入本仓 target/linux。基础 Rust 镜像固定摘要，apt 开发依赖来自 Debian bookworm 仓库；不声称所有系统包字节级可复现。容器内创建独立 D-Bus 和 IBus daemon，不连接宿主桌面，不修改现有输入源，结束后移除容器并保留构建缓存。

`engine_smoke` 使用真实共享库与固定 Release 词库，通过 D-Bus 调用实际 IBusEngine：验证预编辑与候选信号、上屏、第二页全局索引点击、标点、修饰键/key-up、快捷键取消、失焦、密码隔离与私密文本恢复。另启动实际宿主可执行文件，由独立 Python IBus 输入上下文通过 daemon/factory 输入合成拼音并接收提交。共享核心/运行时/宿主 25 项 Rust 测试纳入本地脚本。

嵌套偏好对象整体可以省略（共享 `Preferences` 会给默认值），但它的成员一个都不能少：宿主若把单个键补进一个 runtime-options 文档里本来没有的 `mixed_input` / `local_modes`，写出的就是残缺对象，Host API 会判为 invalid options document。这些默认值因此由 `msime_client_default_preferences` 从共享层发布，宿主据此补全缺失成员，不在 C++ 里另写一份契约。

已验证的环境包括 Debian bookworm arm64、IBus 1.5.27 的容器链路，以及 Arch x86_64（Hyprland/Wayland、Fcitx5 5.1.22、IBus 1.5.34）上的真实安装：`cmake --install` 到 `/usr` 后，Fcitx5 加载 addon（日志中的 `Loaded addon msime`）、输入法出现在可用列表与当前输入法组、`msime-linux-settings` 拉起的设置窗口实际映射。CI 在固定容器里构建并跑 ctest；发行附件 `.deb`/`.tar.gz` 的包内文件、Depends 与动态链接在打包容器里检查过，尚未在真实系统上用 apt 安装验收；换发行版、换架构或改动桌面环境时，按本节的容器脚本和上面的安装步骤各跑一遍即可确认。

系统行为依据 [IBus Engine API](https://ibus.github.io/docs/ibus-1.5/IBusEngine.html) 和 [IBus InputContext API](https://ibus.github.io/docs/ibus-1.5/IBusInputContext.html)。

`candidate_follow_cursor` 是 Windows 候选窗口的定位选项。IBus Engine API 只提供候选表和输入上下文光标位置的通知，不提供由输入法宿主固定 panel 锚点的接口；候选 panel 的定位由桌面 panel 自己决定。因此 Linux 会读取并透传该共享配置，但不伪造 Windows 的固定候选窗口行为：在 Linux 上候选表始终交给 IBus panel 按当前输入上下文位置呈现。该限制属于 IBus/桌面环境边界，不影响候选内容、分页或选词。

Linux 关于页的“输入法宿主日志”对应共享偏好中的 `diagnostic_log.server`。开启后，IBus 与 Fcitx5 宿主在偏好目录写入同一个仅用户可读的 `diagnostic.log`，记录焦点会话、偏好应用、菜单保存、词库维护时释放会话和固定操作失败阶段；文件达到 1 MiB 时保留一个 `.1` 轮转副本。记录经过长度和 ASCII 控制字符限制，不包含按键、输入文本、候选文本、凭据、路径或 provider 响应；关闭开关后不再写入。开关变更随偏好热重载立即生效，IBus 与 Fcitx5 都不需要切换焦点。「日志文件 › 在文件管理器中显示」与 macOS 的「在 Finder 中显示」相同：文件已存在时经 `org.freedesktop.FileManager1.ShowItems`（`gdbus`）在文件管理器里选中它，没有实现该接口的文件管理器、没有会话总线或文件还不存在时用 `xdg-open` 打开所在目录。Windows 专用的 `diagnostic_log.tsf` 在 Linux 设置页隐藏，旧配置字段仍原样保存以保持跨平台同步。

Linux 关于页的“检查更新”读取水杉输入法仓库的 GitHub 发行版列表，只取 `linux-v` 标签下非草稿、非预发布的版本，按版本号取最新，不复用只发布 Windows 安装程序的 `msime.app/update.json`。发行页地址必须属于固定的 `metasequoiaime/msime` releases 路径才会显示；仓库尚无 Linux 发行版时显示正常的“暂无可用发行版”状态，网络错误或无效响应才报告检查失败。打开设置窗口时（设置应用启动时，或已在运行的设置进程被叫到前台时；只为面板启动的设置进程不检查），设置应用每天在后台按同样的规则检查一次，与 macOS 输入法的每日检查对应：上次成功检查和上次尝试的时间、已提醒过的版本和找到的新版本记在状态目录的 `update-check.json`，成功后未满一天不再联网，连不上时记下尝试时间、六小时后再试，不会每次打开都重试；发现新版本时发一条桌面通知「水杉输入法有新版本 vX」（应用名和图标随本安装包所属的版本，例如五笔版是「水杉五笔」和 `msime-linux-wubi`），正文说明由安装它的包管理器更新。通知还在、发通知的设置进程也还在时，点通知上的「查看」打开设置的「关于」页；进程退出以后点它不再有反应，从设置窗口打开「关于」页即可。关于页打开时从记录里读出找到的新版本直接显示，直到运行的版本追上它；正在后台检查时打开的关于页要重新打开才会看到这次的结果。同一个版本只提醒一次。安装包归包管理器管，所以不做应用内安装；检查只在设置应用里进行，Fcitx5 插件和 IBus 宿主都不联网；开发构建不做后台检查。发行版打包时在构建环境里设置 `MSIME_DISABLE_UPDATE_CHECK=1`（编译 `msime-linux-desktop` 时读入），或者用户在运行设置应用的环境里设置它，都会关掉后台检查，关于页的「检查更新」按钮照常可用。更新提示注明软件包未签名，并给出 GitHub 为 `.deb`（没有时为 `.tar.gz`）附件计算的 SHA256 和 `sha256sum <文件名>` 核对命令；GitHub 没有返回该附件的摘要、或同一发行版带有多个架构的包时不显示校验值，改为提示下载发行版里的 `SHA256SUMS`，用 `sha256sum -c SHA256SUMS --ignore-missing` 核对。

## 输入细节与平台差异

Linux 桌面设置页通过宿主能力显示共享的模糊音配置。总开关首次从关闭切换为开启时，偏好存储会一次性选中 11 条规则；用户之后删减规则、暂时关闭再恢复时保留删减结果，并用内部播种标记避免空规则集被再次填充。规则计算仍由 Engine 完成。

词库管理可从 IBus 与 Fcitx5 菜单顶层的“词库…”、桌面启动器的“词库”动作或执行 `msime-linux-settings --panel dictionary` 打开，与 Windows 桌面工具使用同一设置宿主和词典状态。

**键盘布局跟随系统设置**（#6365）。两个宿主收到的都是按当前布局翻译后的按键符号（IBus 的 keyval、Fcitx5 的 `key.sym()`），字母、标点和 Ctrl+Shift+F 这类字母快捷键都按符号匹配，所以 Dvorak、Colemak 等布局打出来的字母就是拼音字母；只有候选序号键按物理数字行取键（AZERTY 上也能选词）。IBus 组件里引擎的 `<layout>` 写的是 `default`，切到本输入法时 IBus 面板和 GNOME Shell 都不改当前布局；此前写的是 `us`，二者会在切换时把键盘换成美式 QWERTY。Fcitx5 的输入法条目不带 `LayoutHint`，用的是输入法组的默认布局：在 `fcitx5-configtool` 的「输入法」页把当前组的键盘布局（或单独给「水杉输入法」选的布局）设成 Dvorak 等即可。

全角/半角输出与 Windows 模式面板对应：`CharacterWidth` 由 `input-runtime` 和 `msime-host-api` 按会话携带，IBus 属性菜单和 Fcitx5 状态栏都提供该开关，可打印 ASCII 在上屏时完成全角转换。配置了共享偏好目录时，模式按 `character_width` 持久化；没有该目录的直接预览配置保持会话级。Fcitx5 与 IBus 都在会话建立时按 `character_width` 设置全角，共享偏好热重载、属性/状态菜单和快捷键切换都会立即同步到正在运行的会话；焦点切换不会丢失全角状态。Fcitx5 新会话以偏好存储中的 `character_width` 为准，另一个窗口在状态栏切换的宽度也会带过来；状态栏切换后尚未写入存储的宽度（保存失败待重试，或隐私输入窗口中本不保存的切换）不会被热重载改回，下一个会话再以存储为准。IBus 冒烟夹具和 Fcitx5 原生上下文测试覆盖全角与半角 ASCII 上屏。

IBus 注册入口通过 launcher 启动，配置优先级为 `MSIME_IBUS_OPTIONS`、用户的 `$XDG_CONFIG_HOME/msime-client/runtime-options.json`（默认 `~/.config`）、安装时配置的系统 runtime-options。IBus 与桌面启动器仅在用户配置不存在时回退；显式覆盖、已存在但不可读的用户配置、悬空符号链接或相对用户配置目录会报错，不会悄悄改用系统配置。系统配置的写入权限沿用安装权限，启动器不会自动复制或改写配置。直接运行 launcher 时可用第一个参数指定系统配置回退路径。

**IBus 宿主崩溃后自动恢复。** ibus-daemon 不会重新拉起退出的 component，宿主一旦崩溃，用户只能切换输入法或 `ibus restart` 才能继续打字。launcher 因此不 `exec` 宿主，而是留在它前面做守护：`msime-linux-ibus` 以非 0 状态退出或被信号杀死（段错误、`SIGKILL` 等）时，按 Windows watchdog 的方式退避重启：第一次等 2 秒，之后每次翻倍、封顶 30 秒，宿主连续运行满 30 秒算健康，此后再崩溃又从 2 秒开始。重启的宿主带 `--recovered` 启动，重新注册 factory 后，若全局引擎为空或仍是 `msime-linux`，就把 MSIME 重新设为全局引擎，有焦点的编辑器不必重新选择输入法即可继续输入；全局引擎已是其他输入法时不干预。IBus 在动态注册的 component 消失时也会清掉来自其他 component XML 的全局引擎（例如 xkb 布局），所以用户切到这类输入法期间宿主恰好崩溃时，恢复后回到的是 MSIME 而不是原来的布局。四种退出不重启：`Ctrl+Shift+Alt+T` 维护退出（宿主以专用状态 77 退出，守护照此退出）；宿主以 0 退出，表示总线断开，即 ibus-daemon 自身在退出或重启：`ibus restart` 后由新的 daemon 在选中引擎时再拉起 launcher，`ibus exit` 后不再运行，这是预期行为；重启的宿主连不上总线时同样以 0 退出，说明 daemon 在退避期间已经消失（被 `SIGKILL` 或自身崩溃，没来得及停止守护），守护随之结束，不会成为孤儿反复重启，也不会在之后启动的新 daemon 上多注册一个 component；准备重启时配置文件已不可读；宿主程序已被删除或不再可执行（`apt remove` 后仍在运行的宿主又崩溃了），这种情况在退避之前就退出，不会每 30 秒对着缺失的文件重试。宿主以专用状态 78 退出表示程序已被升级替换（见「安装后首次使用」里的升级一段），launcher 立即带 `--recovered` 重启，不等待，退避状态保持原样。ibus-daemon 用 `SIGTERM` 停止 component，launcher 把它连同 `SIGINT`、`SIGHUP` 一律作为 `SIGTERM` 转给宿主，等宿主退出后自己也退出，退避等待中收到时立即退出。契约由 `tests/core/ibus_launcher_supervisor.py`（桩宿主：退避间隔、`--recovered` 参数、升级退出立即重启且不影响退避、各种不重启的退出与停止请求，含重启后以 0 退出）和 `tests/runtime/daemon_smoke.py` 的崩溃用例（真实 daemon：`SIGSEGV`/`SIGKILL` 后重新注册、有焦点的上下文不重新选择即可输入、维护退出不重启、`ibus exit` 结束守护）钉住。

**使用上报：IBus 宿主和 Fcitx5 插件都上报，默认开启，读共享偏好 `usage_reporting`，都不拖住输入。** 两个入口都经 `platforms/common/Telemetry.cpp` 调用 Host API 的 `msime_client_telemetry_*`，队列、本机随机安装 id、按 UTC 日去重的 `active`、会话和投递都在 Rust 里（见 `crates/client-services/src/telemetry.rs`）。一个宿主进程就是一次会话：`msime-linux-ibus` 每次启动（含崩溃守护带 `--recovered` 的重启）在连 ibus-daemon 之前开始会话，只做文件 I/O；主循环正常退出时排进一条 `session`。ibus-daemon 停止组件、注销和关机时宿主收到的是启动器转来的 SIGTERM，宿主先退出主循环、走完收尾并排进 `session`，再按 SIGTERM 的默认动作退出，所以启动器和 ibus-daemon 看到的退出状态不变；主循环卡住时 5 秒后由 `alarm()` 的 SIGALRM 结束进程。结束会话和 terminate 路径的崩溃记录都按本进程的会话 id 进行，同一用户在同一目录下同时跑着的另一个宿主（例如另一个图形会话）不会被当成自己结束或记成崩溃。Fcitx5 插件以插件实例的生命周期为一次会话，文件放在单独的 `$XDG_STATE_HOME/msime/fcitx5`，不和同一用户的 IBus 宿主共用会话标记。崩溃时 `std::terminate` 回调和 SIGSEGV、SIGBUS、SIGILL、SIGFPE、SIGABRT 的 sigaction 处理函数只把这次会话的崩溃记录写到磁盘（信号或异常摘要加 `backtrace_symbols_fd` 的调用栈），然后照原样把信号交还：IBus 宿主走默认动作，Fcitx5 进程里交给 Fcitx5 自己先装的处理函数。下次启动时这条记录变成 `crash` 和 `session_crash`，模块路径只保留文件名；只留下会话标记而没有崩溃记录（被杀、注销、关机）不算崩溃。投递在注册 component 之后的后台线程里进行，之后每 30 分钟一轮；端点慢或不可达都不影响注册和输入，宿主退出时也不等它，没发出去的留在队列里下次再发。关掉 `usage_reporting` 后下一轮就清空队列、会话标记和崩溃记录，不再发送。字段、落盘位置和与其他平台的差异见 [PRIVACY.md](../../PRIVACY.md#使用统计与崩溃上报默认开启可关闭)。`tests/core/ibus_startup_telemetry.py` 用普通 dbus-daemon 加只应答 `RegisterComponent` 的桩代替 ibus-daemon，把 HTTPS 经 `HTTPS_PROXY` 指到一个只 accept 不回应的本地端点，钉住：注册先于投递且不等端点、端点挂起期间宿主主循环仍能应答 IBusFactory 的 `CreateEngine`、事件是带安装 id 的 `active`、`--recovered` 启动补报上一会话的 `session_crash` 和去掉目录的 `crash`、宿主收到 SIGTERM 时排进本会话的 `session` 并仍以 SIGTERM 退出、宿主收到 SIGSEGV 时只写崩溃记录不联网。`platforms/common/tests/telemetry.cpp` 在子进程里真实触发信号和 `std::terminate`，另外覆盖链式交还给原处理函数、偏好关闭时清空，以及同目录里有别的进程的会话时 terminate 只记一份、只结束自己的会话。

数字选词：IBus 属性菜单中的“数字选词”控制主键盘和小键盘 `1–0` 对当前候选页的选择，默认开启；状态按输入上下文保留，候选分页仍使用 Engine 提供的全局候选身份。候选表支持左键或中键选词，面板附带的修饰键状态（如 NumLock）不影响选词；右键不改动词典，只在辅助区域提示通过 IBus「候选操作」菜单固定、固定排位或删除。操作会校验会话、代次和全局索引。

两个宿主都按物理数字行选词（Windows 用虚拟键码，与布局无关），AZERTY 等未按 Shift 时打出 `&é"…` 的布局同样能选词；Unicode 模式下裸数字是十六进制输入，选词改用 Shift 加数字行，与 Windows 一致。Fcitx5 以前只认按键符号，这两种情况都选不了词，`0` 也不选第十项。AltGr 打出的布局字符（德语的 `@`、`[` 等）在两个宿主上都先完成组合再交给编辑器，不再被 Fcitx5 当作中文标点或翻页键。

九键输入：IBus 属性菜单中的“九键输入”只在全拼方案下可用。开启后数字键交给 Engine 组成九键拼音，候选视图中的数字选词自动让位；切换会先结束当前组合并重建会话，九键拼音候选和代次由 Engine 返回。关闭后恢复普通数字选词，设置只作用于当前 IBus 输入上下文。

九键歧义拼音：Engine 返回 `nine_key_spellings` 时，IBus 属性菜单显示当前代次的拼音选项（如 `ni`、`mi`）。选择菜单项通过 Host API 携带会话和 generation 调用 `choose_nine_key_spelling`；组合已变化或失焦后，旧菜单项会被忽略，不会改写新组合。

快捷短语编码也和 macOS 一样直接参与普通候选：两个宿主建会话时请求 `quick_phrase_candidates`（与 `phrase_preedit` 一样是宿主能力，不写进偏好），中文模式下直接打快捷短语的编码，展开就排在普通候选后面；仍可按 Shift+K 进入 K 模式只查快捷短语。设置页「快捷模式」里快捷短语一行的说明据 `HostCapabilities.quick_phrase_candidates` 写成「直接输入编码即可调用」。

快捷模式：属性菜单中的“快捷模式”提供 Unicode、日期时间、快捷短语、Emoji、颜文字、超级简拼、临时英文/日文、拆字（C），以及计算与数字（V）、指令（/）和名单（@）模式的会话级开关。后三个默认关闭，偏好文件在它们关闭时不写这几个键；拆字在桌面默认关闭，偏好文件只在用户动过开关后才有这个键。两个宿主的菜单因此把这四个的缺省读作关闭，其余模式缺省读作开启。切换会结束当前组合并重建 Engine 会话，开关只覆盖当前输入上下文；共享 Preferences 和设置页中的持久化开关仍作为新会话默认值。

小键盘标点：`KP_Decimal` 始终提交 ASCII `.`；`KP_Separator` 按逗号标点处理；`KP_Subtract`、`KP_Add`、`KP_Divide`、`KP_Multiply` 和 `KP_Equal` 映射为 `-`、`+`、`/`、`*`、`=`。候选或组合活动时，宿主先通过 Host API 提交高亮候选，再追加对应 ASCII 标点；空闲时算术键仍遵循 Engine 的标点策略，且不会触发减号/等号候选翻页绑定。

Fcitx5 采用同一规则：此前小键盘句点会按普通句号变成「。」，数字里的小数点因此被改掉。

Microsoft 双拼：当当前方案使用 Microsoft 键位且光标所在分音节已有奇数个按键时，未修饰的分号按键作为 `ing` 输入键交给 Engine，不会被中文标点路径提前消费；其他分号仍遵循普通标点处理。

Unicode 输入：进入 Unicode 本地模式后，`Shift++` 作为 Engine 的 `+` 输入继续组成 `U+` 前缀，不会被候选标点或减号/等号翻页路径拦截。

计算、指令与名单模式：空组合时 `Shift+V` 进入计算与数字模式，中文标点下空组合时 `/`、`@` 进入指令和名单模式，都由 Engine 决定，宿主不做模式判断。Engine 在视图的 `spelling_symbols` 里列出当前状态下它当输入收的非字母字符（计算模式是数字和 `+-*/.()%^`，Unicode 模式是数字，网址模式是数字和网址符号，空闲时是开启了的 `/`、`@`，全拼、双拼、五笔组字原文是 `www`、`http`、`https`、`ftp` 时是打开网址模式的 `.` 或 `:`），两个宿主按这份清单路由（共享的 `src/core/SpellingSymbols.h`）：本地模式正在拼写的字符在任何宿主绑定之前作为字符交给 Engine，所以计算模式里 `-`、`=`、`,`、`.` 不翻页，`(` 不补成对括号，`.` 不走智能标点，数字不选词；清单里有数字时选词改用 Shift 加数字行，但 Shift 打出的符号若也在清单里（美式布局上的 `%^*()`）仍是输入。空闲的 `/`、`@` 仍走普通标点路径，由共享 runtime 决定开不开模式（例如还有未完成的词组时不开）。组字中清单里的键同样先交给 Engine，所以 `www` 后按 `.` 打开网址模式，而不是翻页或上屏「。」；网址模式里键入网址用的字母、数字和符号，空格或回车上屏整串网址（全角模式下仍是半角），网址外的符号（如 `"`、`<`）先上屏网址再处理该键，网址不进入学习。这三个模式上屏的文本（算式结果、日期、名单里的名字）是 Engine 生成的，不计入打字统计。

按键音、上屏音与背景音乐：两个宿主每建一个会话就调用 `msime_client_set_lifecycle(MSIME_LIFECYCLE_FEEDBACK)`，把按键音、上屏音、打字特效和背景音乐交给 Host API 的生命周期核心，并立刻用 `msime_client_set_field_traits` 报告输入框属性（`src/system/LifecycleFeedback.h`）。属性逐个标志映射：IBus 的 `IBUS_INPUT_HINT_PRIVATE`、Fcitx5 的 `Sensitive` 报成 SECURE 加 PRIVATE，核心在这样的输入框里不出任何声音、不画特效、不放音乐；IBus 的 `IBUS_INPUT_HINT_NO_SPELLCHECK`、Fcitx5 的 `NoSpellCheck` 只报 NO_SPELLCHECK，这类输入框（终端、代码编辑器常这样标）照常出声、照常放音乐。属性变了而会话不重建时当场重新报告；每次调用 `msime_client_focus(true)` 之前，属性没变也再报告一次，脚本运行时要先看到这次确认才往脚本投递事件。每个打字键在交给引擎之前先报告属性、再调用 `msime_client_key`（空格、回车、退格各有一类，其余按键一类）；按住不放产生的自动重复与 macOS 一样不出声、不画特效、不计连击，按住退格或空格只在第一次按下响一声；重复按硬件键码认（先松开 Shift 再松开数字键时松开事件的 keysym 和按下时不同，按 keysym 认会把下一次 Shift+数字当成重复），每次重复仍调用 `msime_client_key(MSIME_KEY_POINTER)` 开新的一轮，脚本插件的 fuel 因此按每次按下给，按住退格时一连串派发不会把一轮的配额花光、让插件在这次聚焦里停掉；所以上屏的那个键先响按键音、再响上屏音；上屏音由核心在派发里发出，回复的 `feedback` 带回连击数。按键音只在中文输入开启时响，Ctrl/Alt/Super 组合键、单独的修饰键、松开和录音期间的键（包括按 Esc 取消录音的那个键）都不调用 `msime_client_key`，不响；失焦时写出的组字（Fcitx5 在不支持预编辑的客户端里为韩文、注音、越南文、藏文写出的组字，IBus 暂存的分段选词）不出上屏音。点选候选和翻页先调用 `msime_client_key(MSIME_KEY_POINTER)`，不出声。背景音乐经 `msime_client_set_active` 只说本会话是否激活（获得焦点且没有在录音），核心记着最后激活的会话是音乐的所有者，由它按字段属性决定放不放；设置里的声音变了而本会话正激活着时，宿主再声明一次激活，刚打开的背景音乐不用等下一次换焦点。密码、PIN、数字类输入框两个宿主都不建会话，一律无声。音效开关、音色包和音量都来自共享偏好 `plugins`，由 Host API 在进程内的播放器里解码和混音，宿主只投递请求：什么都没开时不起线程也不打开音频设备，音频设备出错或播放器 panic 时本进程内音效关闭并在 stderr 留一行，不会影响打字。这一点对 Fcitx5 尤其重要，插件跑在 `fcitx5` 进程里，关着音效的用户不承担任何开销。成就音由打字统计的 record 触发，不需要宿主调用。内置音色包安装在 `share/msime-client/sound-packs`，与资源目录并列；IBus 宿主按自身可执行文件的位置找到它，Fcitx5 插件用编译期的安装路径，两者都在运行时配置没写 `sound_packs` 时把它交给 Host API，因为用户下载了新词库后资源目录已不在安装目录里，库自己按资源目录旁边去找会落空。随桌面安装包分发的官方插件包（uuid 脚本插件、latex-symbols 短语表，`plugins/official-packs.json` 登记的那些）装在它旁边的 `share/msime-client/plugin-packs/<kind>/<id>/`，Host API 和设置窗口从 `sound_packs` 推出这个目录，宿主不另外传；它们默认停用，脚本插件要在授权页授权，短语表要在插件页启用。CMake 选项 `MSIME_PLUGIN_PACKS` 给了才装（`scripts/build-official-plugins.sh` 构建到 `target/plugin-packs`）：发布页的 deb、rpm 和 tar.gz 由 `package-container.sh` 传它（发版设 `MSIME_REQUIRE_OFFICIAL_PLUGINS=1`，缺了就失败），`build-container.sh` 在 `target/plugin-packs` 存在时装一遍并核对两个包都到了。从源码构建的发行版定义（`packaging/` 下的 Arch `msime`、Gentoo、OBS、Debian 源码包和 nix）不跑 wasm 构建，不带官方包；`msime-bin` 重新打包发布页的 deb，所以带。

成对标点：启用成对标点后，`(`、`[` 和 `<` 的中文开标点会由宿主追加对应闭标点，`{` 追加 ASCII `}`，并向编辑器转发一次左移以把光标留在标点中间；`<` 的嵌套层级仍由 Engine 决定，宿主自动补出 `》` 后通过专用接口通知 Engine 抵消本组深度，连续独立输入保持为 `《》《》`，显式嵌套仍为 `《〈〉》`。候选活动时先完成高亮候选再补成对标点；候选导航绑定优先于成对标点。宿主按焦点右侧的 surrounding text 校验闭标点，使用有界配对栈避免重复输入；带 Ctrl、Alt、Super、Meta、Hyper 或 Mod5 的组合不触发跳过，Shift 仍可作为 `)`、`}`、`>` 和引号的物理按键修饰键，且被拒绝的组合不会破坏配对栈。IBus 提供的表格客户端名为 `scalc`、`gnumeric` 或 `calligrasheets` 时跳过成对补全与闭标点移位，以适配表格单元格编辑；共享 `soffice.bin` 不列入排除，避免影响 Writer 等其他文档。

引号样式与方头括号样式（共享偏好 `quote_style`、`bracket_style`，在共享设置页「标点」组）下 Engine 让 `'` 出「」、`"` 出『』，`[` `]` 出〖〗。两个宿主的补全（`normalize_punctuation_pair` 按提交末尾实际是哪一组补哪一组）、跨过（`paired_closing_for_key` 按配对栈栈顶在两组后半个里挑）、空格转换的反查（`SmartPunctuationSpace.h`）都认这两组字；英文模式下固定中文标点时的输出（`shared/input/EnglishModeOutput.h`）按 View 的 `punctuation_marks` 出字。IBus 属性菜单不列这两项，它们不是需要随手切换的开关。

Fcitx5 现在也做同样的成对补全：此前它只有状态栏上的开关，不补闭标点、不回移光标，也不跳过已有的闭标点。补全、配对栈和表格客户端排除都来自共享的 `src/candidates/PairedPunctuation.h`，客户端名取 Fcitx5 输入上下文的程序名；光标移动通过 `InputContext::forwardKey` 转发左/右方向键，闭标点校验读 surrounding text。Fcitx5 的标点先经共享路由，智能标点决定保留 ASCII 时不会补对子。

AI 联想请求可携带 `ai_context`：当前焦点会话最近经共享提交路径上屏的文本，最多 1024 个 UTF-8 字节，按字符边界截断。分段选词和整句候选各追加本次提交，不重复追加已提交前缀；使用最终简繁/全角转换后的文本。仅启用 AI 联想且查询符合条件时发送，私密字段不记录；private/no-spellcheck 输入框在 IBus 与 Fcitx5 中都不发云候选、AI 和候选翻译请求，切出私密框时会话重建，其中上屏的文字不会进入之后的 AI 上下文。上下文在失焦、reset 和会话关闭时清空；不落盘、不写日志。provider 应将该字段仅用于 AI 联想上下文。

IBus 的语音识别、剪贴板历史选取、空闲全角输入和直接标点提交也通过统一上屏入口更新 AI 上下文。重复智能标点替换会先移除上下文中的旧字符，再记录替换文本，避免把已删除标点重复发送给联想服务。

在线候选在输入停顿 500ms 后请求；Google 响应严格按 Windows 的 `SUCCESS` 首候选协议解析，与 Windows 的云候选空闲延迟一致。GLib 主循环定时器随连续输入重新计时，到期时才读取最新 Engine 查询；失焦、会话关闭或 provider 配置变化时取消待发请求。网络工作继续在后台线程执行，结果仍按会话和代次校验。

在线 provider 支持批量响应 `{"candidates":[{"text":"云候选","source":0},{"text":"AI候选","source":1}]}`，没有 `candidates` 数组的响应按失败处理。每次最多两个候选，每个来源各一个，对应 Engine 当前的云/AI 槽位；Linux 依次用原查询身份回填后统一刷新显示。传输层限制整行 16 KiB、单项 4096 字节，并过滤已禁用或不符合查询条件的来源。Windows `ai_assistant.cpp` 同样只回填模型结果中的第一条，`candidate_limit` 用于模型请求。

启用且符合查询条件的 AI 请求允许 provider 在 8 秒内返回完整结果，与 Windows AI 网络请求的等待时间一致；仅云候选请求保留 500ms 等待。计时覆盖整行响应，分段发送不会续期，16 KiB 上限继续生效；请求写入也设置 500ms 超时，所有等待仍在后台线程。

## 随包在线候选服务

安装包含 Python 3.9+ 标准库实现的 `msime-linux-online-provider`，为 IBus 提供 Google 云候选和 OpenAI 兼容 AI 联想。使用当前用户的私有运行目录启动：

```sh
mkdir -p "$XDG_RUNTIME_DIR/msime-client"
chmod 700 "$XDG_RUNTIME_DIR/msime-client"
msime-linux-online-provider "$XDG_RUNTIME_DIR/msime-client/online.sock"
```

将该 socket 的绝对路径填入 runtime-options 的 `online_provider_socket`。仅提供云候选时无需凭据；AI 服务可增加 `--ai-config /absolute/private-ai.json`，文件仅允许所有者读写，包含 `provider`、`endpoint`、`model`、`token` 四个字符串字段。前三项须与共享 AI 设置一致，endpoint 使用 HTTPS，token 只留在服务配置中，不进入 IBus 查询。AI 私有配置在每次符合条件的请求中重新加载，修改凭据无需重启；可用 `profiles` 按 provider 保存多组配置，选择与共享设置一致的 provider、endpoint、model。不会自动启用系统服务或 CI。

服务只接受同一用户连接，同时最多处理四个请求；云候选与 AI 并行请求，AI 失败时仍可返回云候选。HTTP 响应最多 64 KiB，拒绝 HTTP 重定向以保持凭据与端点绑定。AI 沿用 Windows 的 JSON 请求、上下文、candidate_limit 和 DeepSeek thinking 禁用设置；所选提示词槽位为空（含空白）时，system 消息使用与 Windows 默认 `[ai_assistant].prompt` 相同的内置联想提示词，与 client-providers 的 `DEFAULT_CANDIDATE_PROMPT` 一致，因此默认配置和手动清空过提示词的配置都能得到可解析的候选。按配置最多保留 10 条有效且不重复的模型候选，再由 Engine 批量缓存和排序。成功的 AI 结果按 provider、endpoint、model、候选数量、提示词摘要、近期上下文摘要和拼音分段保存在有界进程内缓存中，可跨候选 generation 复用；缓存键不包含凭据、上下文或提示词原文、会话或原始输入，空响应和失败不会缓存。候选翻译同样按单项缓存，最多 4096 项；成功结果在当前 provider scope 下持续复用，直到配置 scope 变化或容量淘汰，失败项使用 8 分钟负缓存且在 TTL 内不会重复请求，独立候选不会互相抑制。服务不打印输入或网络错误正文，退出时仅删除自己创建的 socket。该入口同时实现候选翻译；语音由独立的随包 voice provider 提供；账号同步服务仍按各自契约接入。

设置页的「获取模型列表」也接进了同一个 provider。这个按钮此前在 Linux 上不可用：持有 token 的宿主是自己发 HTTP 去取的，而这个平台按设计把 token 留在 provider 的所有者专用配置文件里，外壳手上没有可用于鉴权的东西。现在 provider 有 `ai_models` 与 `ai_test` 两种请求：请求只给出 provider 名、接口地址、模型、提示词和待润色文字，provider 先核对这些与私有配置中的 provider/接口地址（润色再加模型）一致，再用自己的 token 发请求；不一致就什么都不发。模型名最多 128 个、每个 256 字节，去重并保序；润色结果最多 16 KiB。设置页现在只用 `ai_models`：桌面上的 AI 辅助页撤掉了「AI 润色测试」（这份配置在桌面上只给拼音联想用，测出的结果和用户实际得到的没有关系），`ai_test` 请求和 Tauri 的 `ai_test` 命令仍然保留。新增能力位 `ai_provider_credentials` 表示「凭据归宿主的 provider」，设置页据此不显示 API Token 输入框，也不再因为没有 token 而禁用「获取模型列表」——此前模型列表那一段是按平台名藏掉的。Linux 的 AI 候选数量固定为 3（client-core 的 `HostPlatform::fixed_ai_candidate_limit`），fcitx5 和 IBus 转发的在线查询已经带着这个值，provider 照它请求。IBus 转发前会把读到的偏好快照和在线查询的 AI 配置逐键比较、不一致就不问 AI（防推迟应用的偏好改动带着旧设置发请求），候选数量不在比较之列：快照里是文档存的原值，查询里是固定值，存的不是 3 时一比就永远不等（`scripts/test-linux-ibus-ai-candidate-limit.py` 钉住这一点）。

共享设置页可对当前腾讯翻译、NiuTrans、自定义翻译、AI 辅助、语音识别和语音润色选项执行“测试配置”。Linux Tauri 只把服务名和当前非敏感选项发送到已有 provider socket；AI、腾讯和语音 token 仍由 provider 从所有者专用配置文件读取，不返回 WebView。NiuTrans 与自定义翻译继续沿用本来就会进入候选翻译请求的设置字段。provider 使用固定的合成文本或静音 WAV 发出最小请求，并只返回有界的成功/失败文案，不转发服务端正文、URL、凭据或异常细节。测试不会保存草稿，也不采集真实输入；provider 不可用时设置页明确提示先启动服务。

### 随包候选翻译

IBus 宿主在后台通过共享 Host API 查询随包 `msime-english.db`。目标语言为英语且启用 `preferences.candidate_english_gloss` 时，先显示离线中英双向释义，再把未命中的候选发送给在线 provider；关闭 `preferences.candidate_translations` 也不会关闭这项独立的离线释义。未配置在线服务也能使用离线命中。其他目标语言直接使用在线 provider。日语方案和临时日语模式不请求候选翻译。离线与在线结果均校验会话、候选代次和配置代次，释义只用于显示，不进入选词提交文本。

英语目标的在线短释义还会通过 Engine 写入用户数据目录的 `translation-glosses.db`，后续离线查询先查用户释义，再查发布词库。 两个词库独立检查可用性：发布词库缺失或损坏时仍可读取用户释义；用户词库不可用时回退发布词库；两者均不可用时返回错误。保存沿用 Windows 的规则：仅中英候选、源文本不超过 40 字符、格式化后译文不超过 32 字符且不与原文 ASCII 大小写等价；其他目标语言不保存。读写在后台线程进行，不修改发布资源。重建 IBus 宿主且关闭在线 socket 后仍可读取已保存的释义；候选显示继续检查会话和代次。共享 C API `msime_client_translation_gloss_save` 接受 `{target_language,translations:[{text,translation}]}`，离线候选查询可传入 `user_data` 读取相同用户词库。IBus 在 Ctrl+Enter 遇到多个以分号分隔的译义时显示临时副候选页，空格、数字键、上下键、PageUp/PageDown 和鼠标点击只作用于译义，选择后直接上屏并恢复原 Engine 候选；单译义仍直接提交，译义页不会写入词库。IBus 的点击只带槽位号，宿主按面板上实际显示的那一页解释它：译义页的每一行在渲染时记下全局序号，点选从那一行取；别的视图画上来（异步到达的译文或云候选、偏好更新、菜单改每页候选数）时临时页随之结束，不再恢复保存的候选，之后的点选、数字键和 Esc 作用于面板上真正显示的候选。普通候选页的点选与滚轮同样先核对面板快照的会话和代次，通过之后才报告 `MSIME_KEY_POINTER` 并派发。

同一服务接受 `kind:"translation"`，沿用 Windows 默认腾讯 TMT、自定义 DeepLX 的选择顺序。默认翻译增加 `--tencent-config /absolute/private-tencent.json`，配置文件必须是当前用户所有、其他用户无权限的普通文件，包含 `secret_id`、`secret_key`，以及可选 `region`（默认 `ap-guangzhou`）。凭据只在 provider 中读取，TC3 签名请求固定发送到腾讯 TMT HTTPS 地址，不接受请求覆盖地址。未提供腾讯配置时仍可使用云候选、AI 和自定义翻译。腾讯私有配置在每次腾讯翻译请求开始时重新读取，无需重启服务；建议通过原子替换更新文件。读取使用非阻塞文件描述符，只接受不超过 16 KiB 的 UTF-8 普通文件，并核对读取前后的文件信息。文件删除、权限变宽或内容无效时跳过腾讯翻译，恢复有效配置后下次请求重新使用；自定义翻译不会读取腾讯配置。每批请求使用同一份凭据和区域快照，缓存按该快照摘要隔离。 腾讯凭据与 Windows 一致，先清除首尾 ASCII 空格、制表符和换行，再进行校验和签名；仅空白、占位符及含内部空白或控制字符的凭据仍会被拒绝。仅首尾空白变化不会使缓存失效。

启用共享设置中的 `custom_translation` 后，服务使用请求中的 endpoint 和可选 API Key 调用 DeepLX 兼容接口，支持本机 HTTP 服务或 HTTPS，禁止重定向。endpoint 的主机名恰好是 `api.deepl.com` 或 `api-free.deepl.com` 时改按 DeepL 官方 API 发请求：`Authorization: DeepL-Auth-Key <key>`，正文 `text` 为数组，`target_lang` 中文用 `ZH-HANS`、英文用 `EN-US`，发往 `/v2/translate`，主机按密钥是否以 `:fx` 结尾选 Free 或 Pro；没有 API Key 时不发请求。规则与共享 Rust 实现相同，用例在 `shared/contracts/custom-translation/cases.json`，`tests/dictionary/custom_translation_contract.py` 逐条核对。 与 Windows 一致，请求前清除 endpoint 和 API Key 的首尾 ASCII 空格、制表符及换行；归一化后的快照同时用于两个翻译方向和缓存标识。API Key 归一化后为空时不发送 Authorization。自定义服务失败不会回退到腾讯。英文候选译为中文；中文候选译为所选英语、法语、日语、西班牙语、俄语、德语或韩语。源文本超过 256 个字符或不符合中英文候选规则时跳过。中文筛选沿用 Windows 的汉字范围，包含兼容汉字和扩展区，允许带汉字的中英数字混排；包含 Emoji、符号图形、零宽连接符、Emoji 变体选择符或键帽组合标记时跳过。纯英文仍只接受字母、空格、连字符和撇号。IBus 在候选更新后等待 500 毫秒停顿再读取 Engine 当前查询并发出翻译请求；连续输入会重置定时器，失焦、禁用或切换配置会取消待发请求。腾讯按翻译方向批量请求，自定义服务逐条请求，每次网络操作超时 2.5 秒，批次在 6 秒预算用尽后停止发起新请求；宿主在后台最多等待 8 秒，过期代次仍由现有 Host API 拒绝。

翻译结果压平换行并过滤控制字符，内存缓存最多 2048 项，成功结果保留 480 秒、失败保留 30 秒。缓存按 provider、凭据摘要和语言方向隔离，不写入磁盘。

## 随包语音服务

`msime-linux-voice-provider` 接收现有 `voice`、`voice_stop` 和 `voice_cancel` 请求，实现 OpenAI、Groq、SiliconFlow、EveryAPI、Mistral、百炼的 HTTP 语音识别及可选润色。需要 Python 3.9+，录音使用 `pulseaudio-utils` 的 `parec`（也适用于 PipeWire 的 PulseAudio 兼容服务），或 `alsa-utils` 的 `arecord`。默认优先使用已安装的 `parec`，可用 `--capture alsa` 显式选择 ALSA；选定后设备打开失败会返回失败，不会偷偷改用另一麦克风。

```sh
msime-linux-voice-provider "$XDG_RUNTIME_DIR/msime-client/voice.sock" \
  --config /absolute/private-voice.json --capture pulse
```

先按在线服务章节创建当前用户专用的运行目录，再把 socket 绝对路径填入 `voice_provider_socket`。配置文件必须是当前用户所有、其他用户无权限的普通 JSON 文件，至少含 `asr` 与 `polish` 对象之一；文件不存在时服务照常启动，只有本地识别可用，云端识别请求在文件写好后的下一次录音生效。批量识别及润色对象包含 `provider`、`token` 两个非空字符串，以及可选的 `endpoint` 和 `model`。HTTP ASR provider 支持 `openai`、`groq`、`siliconflow`、`everyapi`、`mistral`、`bailian`；润色支持 `openai`、`groq`、`siliconflow`、`deepseek`。这些批量接口须为 HTTPS 且不允许重定向，凭据不通过 socket 查询或命令行参数传递。设置中的 provider/model 须与服务配置一致；私有配置在每次录音开始时重新加载，更换凭据或端点无需重启服务。

服务集合、默认地址/模型与 HTTP 请求格式由共享 Rust 语音目录生成到 `scripts/msime_voice_provider_catalog.py`；语音脚本同步导入，私有配置和 profile 的校验、自定义值优先及本地/豆包的处理保持由宿主管理。修改共享目录后，在仓库根目录运行 `cargo run --quiet --locked -p msime-client-providers --example export_voice_provider_catalog -- --write`，再用 `--check` 检查。生成模块与润色提示词模块一起安装到各版本的 `bin` 目录；`tests/voice/provider_catalog.py` 固定原目录并用合成模块验证实际配置和请求读取生成值，可用 `--provider <已安装的语音脚本>` 验证安装产物。

服务通过 `parec`、`pw-cat` 或 `arecord` 捕获 16kHz 单声道 PCM（.deb 以 Recommends 声明 `pulseaudio-utils | pipewire-bin | alsa-utils`），在内存中封装 WAV，按共享格式发送 multipart 上传或带 `input_audio` 的 Chat Completions 请求。所选后端的录音工具不存在时服务照常启动并在日志中写明，录音请求返回 `detail` 为 `recorder` 的 `voice_dependency_missing`，IBus 与 Fcitx5 宿主都提示“未找到录音工具，请安装 pulseaudio-utils、pipewire-bin 或 alsa-utils”，装上工具后下一次录音即可使用。配置文件、socket 目录、录音设备名或录音上限无效，命令行参数有误，或服务已在运行、socket 无法恢复时，服务以状态 2 退出，systemd 不再重启。默认录音上限 300 秒，可用 `--max-recording-seconds` 设置为 1–600 秒；到时自动停止并识别。松开录音快捷键也走同一完成路径，取消则丢弃结果。小于 250ms 的录音不上传，上传音频不超过 20 MiB；SiliconFlow 按 Windows 行为补静音、省略 language 字段，并在网络或服务端错误后最多重试一次。每次 ASR 网络操作超时 60 秒，可选润色超时 3 秒，润色失败保留原转写。正在发送的 HTTP 请求不能撤回，但取消后其结果不会交给输入目标。

润色保留 Windows 的精炼整理、忠实校对、中翻英、口语整理及三个自定义提示词选择，并沿用 `<asr_text>` 包装和 provider thinking 设置。批量识别没有录音中的实时转写；启用润色时可先返回原始转写的 partial，再返回整理后的 final。Doubao 实时识别使用下述 WSS 配置。

提示音尊重 `sound_enabled`，通过 `paplay` 或 `aplay` 播放短提示音。输入法宿主和桌面语音面板只转发总开关 `sound_enabled`，不再转发开始、结束提示音各自的开关；provider 仍接受 `start_sound`、`end_sound`（缺省都为开），供自行接入的客户端使用。`mute_system_audio` 在支持 JSON 输出的 `pactl` 上暂时静音最多 32 个现有播放流，结束或取消后恢复被本服务改变的流；已有静音、已消失或被替换的流不会强制改写。缺少这些可选工具时继续录音。单次仅占用一个麦克风，最多四条语音任务等待网络，控制请求另留容量；会话按连接进程及 generation 区分。客户端断开录音连接或服务收到终止信号时停止采集并恢复播放流。原始音频、转写和凭据均不写入磁盘或日志。

### Doubao 实时识别

`asr.provider` 设为 `doubao` 时，语音服务使用相同录音和控制入口流式上传，无需先录完整段音频。流式识别需要 websockets 15.0 或更高版本的同步客户端（15.0 起才接受连接所用的保活参数，旧版要到连接时才报错，所以服务在启动和每次 Doubao 请求前同时检查版本号和这些参数）。.deb 以 Recommends 声明 `python3-websockets (>= 15)`，Debian 13、Ubuntu 25.10 及更新版本的发行版包满足要求；Debian 12、Ubuntu 24.04 和 25.04 的发行版包过旧，须按随包安装的 `share/msime-client/requirements-voice.txt`（`websockets>=15,<16`）用 `pip install --user` 为系统 Python 安装（这些版本的系统 Python 受 PEP 668 保护，需加 `--break-system-packages`，只写入当前用户目录），装好后重启语音服务。缺少或版本过旧时语音服务照常启动并在日志中写明，只有 Doubao 请求在录音前返回 `{"ok":false,"error":"voice_dependency_missing","detail":"websockets"}`，IBus 与 Fcitx5 宿主都提示“豆包语音需要 websockets 15 或更高版本，请安装 python3-websockets”，设置中的豆包连接测试同样直接说明缺少 websockets；批量识别和其他 provider 照常可用。批量识别仍只依赖 Python 标准库。同步 WebSocket 客户端参数参考其[官方文档](https://websockets.readthedocs.io/en/15.0.1/reference/sync/client.html)。

Doubao 的 `asr` 配置包含 `provider:"doubao"`、`endpoint`（WSS，如 Windows 使用的 `wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async`）及 `token`；`doubao_auth_mode` 可设为 `api_key`（新版控制台，发送单个 `X-Api-Key`）或 `legacy`（旧版控制台，发送 `X-Api-App-Key` 与 `X-Api-Access-Key`）。新版 API Key 放入 `token`；旧版还必须配置 `app_key`，并把 Access Token 放入 `token`。省略、留空或填写未知模式时按 `api_key` 处理，并忽略残留的 `app_key`。`resource_id` 默认 `volc.seedasr.sauc.duration`。`model` 可省略，协议固定使用 `bigmodel`。凭据仍保存在所有者专用 JSON 文件中；查询只能提供非敏感选项，不能改写已配置端点或凭据，非空 `asr_resource_id` 必须与服务配置一致。

实现沿用 Windows 固定提交 `b21a1671` 的二进制协议：16kHz、16-bit、单声道 PCM，每 200ms 一帧，gzip 压缩，递增序列号，结束帧使用负序列号。`doubao_enable_itn`、`doubao_enable_punc`、`doubao_enable_ddc` 和 `doubao_boosting_table_id` 进入首帧选项。录音中的变更转写以 partial 事件返回；宿主继续根据 `stream_inline_preedit` 决定是否更新预编辑。松开或达到录音上限后发送结束帧，最多等待 30 秒获取最终结果，再进行可选润色。服务错误、超时和取消不会把中间结果冒充最终结果上屏。

音频发送队列最多容纳 10 秒音频，满时终止该请求；单个 WebSocket 响应和解压后的正文分别限制为 1 MiB。识别连接启用 TLS 证书检查、禁用 WebSocket 扩展压缩、拒绝重定向，并关闭该连接日志；Doubao 协议自身仍使用 gzip。取消会停止录音、丢弃结果并中断已建立的连接；建立连接阶段最多等待 10 秒。流式上传在录音时即发送音频，取消不能撤回已经发送的数据，短录音虽不会上屏也可能已有音频发出。

### 本地识别

`asr_provider` 为 `local` 时不读取任何凭据，也不联网：服务为录音启动随包安装的 `msime-voice-local`（与 `libmsime_host_api.so` 同在 `msime-client` 库目录），按行交换 JSON，把录音按 100ms 一段交给它，并把它返回的中间转写作为 partial 事件转发。`asr_model_path` 选项给出设置页下载的模型目录，目录中必须有普通文件 `msime-model.json`（不跟随符号链接，最大 1 MiB）；路径必须是不超过 4096 字节的绝对目录，否则该次请求返回 `ok:false`，服务不启动 helper。识别器由 `libsherpa-onnx-c-api.so` 与 `libonnxruntime.so` 提供，二者装在 helper 旁边，helper 先从自己所在目录加载它们；缺少 helper 或运行时时请求在录音前返回 `{"ok":false,"error":"voice_dependency_missing","detail":"local_asr"}`，服务照常为其他请求运行，设置页的「测试」同样检查模型与组件。服务在两次录音之间保留一个空闲 helper，使模型不必每次重新加载；helper 空闲 600 秒自行退出，服务在 540 秒后不再复用它，异常退出或未确认取消的 helper 不会复用。离线模型在录音结束后才完成解码，首次录音还需加载模型，因此最终结果最多等待 120 秒。开发时可用 `MSIME_VOICE_LOCAL_HELPER=/absolute/msime-voice-local` 指向构建目录中的 helper。

用户词库中的词作为热词：IBus 与 Fcitx5 宿主在语音工作线程上调用 `msime_client_voice_hotwords`（语音 provider 拿不到词库路径和偏好，无法自己构造 HostOptions），把结果按每行 `词\t拼音` 放进 `voice_hotwords` 选项并限制整条查询不超过 15872 字节；设置页（Tauri 面板）发起的本地录音用同一个选项，读取设置页编辑的词库，选项总长不超过 15872 字节，`asr_model_path` 不像其他名称类选项那样截断到 512 字节。服务把词交给 helper；模型清单声明 `"hotwords":"pinyin"`（模型不支持热词偏置）时，改为在最终转写上通过已安装的 `libmsime_host_api.so` 调用 `msime_client_voice_hotword_correct` 做拼音纠正，中间转写不纠正。本地识别仍可按 `polish` 配置润色。

打包时 `package-container.sh` 用 `scripts/fetch_voice_runtime.py` 下载并校验对应架构的 sherpa-onnx 与 ONNX Runtime 共享库，以 `-DMSIME_VOICE_RUNTIME_DIR` 传给 CMake；生成安装包而不提供该目录时配置失败。

Linux provider 请求工具可省略 socket 参数，依次使用对应的 `MSIME_*_PROVIDER_SOCKET` 环境变量和 `$XDG_RUNTIME_DIR/msime-client/` 下的默认 socket：`online.sock`、`translation.sock`、`voice.sock`、`cloud-dictionary.sock`、`cloud-clipboard.sock`、`handwriting.sock`、`emoji.sock`。语音的 `--stream` 同样支持省略 socket；手写和 Emoji 的 `--local` 仍使用本地资源发现。IBus 在配置热重载时重新发现在线和语音 socket，候选翻译继续按独立配置、环境变量、在线 socket 的顺序选择服务。

IBus 属性菜单和 Fcitx5 状态区顶层可直接打开水杉表情面板、云剪贴板、云词库、水杉屏幕键盘和手写输入（菜单里的名字与 macOS 输入法菜单相同，带省略号：「水杉表情面板…」「云剪贴板…」「云词库…」「水杉屏幕键盘…」「手写输入…」），「桌面工具」子菜单里还有本地剪贴板和语音面板；「水杉输入法设置…」与「关于水杉输入法…」在菜单顶层。这些入口独立于可配置工具栏，通过 `msime-linux-settings` 启动已有 Tauri 面板；需要安装桌面二进制，也支持 `MSIME_CLIENT_SETTINGS_COMMAND` 自定义启动器。密码等受限输入上下文禁用这些入口。

安装桌面宿主后，支持 Desktop Actions 的应用菜单或任务栏可直接打开手写、屏幕键盘、表情、语音、云词库与云剪贴板。也可把 `msime-linux-settings --panel handwriting` 等命令绑定到桌面环境快捷键；`--panel` 支持 `settings`、`handwriting`、`keyboard`、`emoji`、`voice`、`cloud-dictionary`、`cloud-clipboard`，继续使用同一 runtime-options 配置及桌面面板输入目标捕获流程。

剪贴板工具支持 `add-stdin`，例如 `wl-paste --no-newline | msime-linux-clipboard "$XDG_STATE_HOME/msime-client/clipboard.json" add-stdin`（需将 `XDG_STATE_HOME` 设为绝对目录，未设置时使用 `$HOME/.local/state`）。X11 可将管道上游换为 `xclip -selection clipboard -o`。文本通过标准输入传递，首次写入自动创建历史目录；最多读取 1 MiB，保存时保留完整 UTF-8 字符并限制为 4000 个 UTF-16 单元（与 Windows 规则一致），继续去重并保留最近 50 项。此命令仅执行一次明确采集，不注册后台剪贴板监听。

剪贴板历史菜单的粘贴操作使用展示时缓存的文本；删除在文件锁内按文本内容匹配，后台新增历史不会导致误删另一行。菜单行绑定加载代次，忽略已过期的行操作；清空也使用同一历史文件锁。条目与删除按钮可见，预览按完整 UTF-8 字符截断。

IBus 剪贴板历史按每组 10 条显示最近 50 条，较早条目也可粘贴和删除。“刷新历史”异步重新加载文件，不必切换输入焦点；刷新会使旧菜单行失效，正在运行的旧加载完成后会重新调度最新加载。

IBus 在可输入的焦点会话中监听历史文件所在目录，外部工具新增、删除或原子替换历史文件后自动异步刷新菜单。失焦或受限上下文停止监听；目录尚不存在时利用既有宿主定时器重试接入。仅监听已配置历史文件，不采集系统剪贴板。

语音 provider 支持 `--capture pipewire`，使用原生 `pw-cat` 录制 16 kHz 单声道 PCM；提示音也可通过 `pw-cat` 播放。`auto` 保持优先 `parec`，其次 `pw-cat`，再尝试 `arecord`。`--capture-device` 可指定 PulseAudio source、PipeWire node name/object.serial 或 ALSA PCM 名称，建议与明确的 `--capture` 后端配合使用。原生 PipeWire 录音无需 PulseAudio 兼容录音工具；系统音频静音优先使用 `pactl`，不可用时回退到 `pw-dump` 与 `wpctl`。参数依据 [PipeWire pw-cat 官方手册](https://docs.pipewire.org/page_man_pw-cat_1.html)。

录音期间静音支持原生 PipeWire/WirePlumber：`pactl` 缺失或无法枚举时，使用 `pw-dump` 查找应用播放流并通过 `wpctl` 静音，录音结束后仅恢复本次静音且节点序列号一致的流。原本已静音、已退出或被新节点复用的流不会被恢复。最多处理 32 个播放流，不更改麦克风静音或默认输出设备音量。参考 [WirePlumber wpctl](https://pipewire.pages.freedesktop.org/wireplumber/man/wpctl.html) 与 [PipeWire pw-dump](https://docs.pipewire.org/page_man_pw-dump_1.html)。

语音服务启动时持有同路径 `.lock` 进程锁。异常终止留下的 socket 在确认属于当前用户、连接被拒绝且 inode 未变化后自动清理，使服务可重新启动；活跃服务、普通文件、符号链接及无法确定状态的端点不会被替换。锁文件保留并由内核在进程退出时释放锁。

在线候选/翻译 provider 与语音 provider 共用 socket 所有权和异常重启恢复逻辑：持有独立 `.lock` 进程锁，清理已确认无监听的残留 socket，并拒绝覆盖活跃服务或其他文件。共用模块 `msime_provider_runtime.py` 随服务一起安装。由 systemd socket 激活启动时（`LISTEN_PID`/`LISTEN_FDS` 指向本进程且只传入一个 socket），provider 直接在继承的监听 socket 上服务：socket 必须是 Unix stream 且地址与参数路径一致，否则以状态码 2 退出；此时不取 `.lock`、不绑定，退出时也不删除 socket 文件，路径由 systemd 持有并跨 provider 重启保留。

### 用户服务启动

安装包提供 `msime-linux-online.service` 和 `msime-linux-voice.service`，不自动启用。服务通过 `msime-linux-provider-session` 使用 `$XDG_RUNTIME_DIR/msime-client/online.sock` 和 `voice.sock`，与 IBus 自动发现路径一致。运行目录首次启动时创建为仅当前用户可访问；已有目录权限不合要求时直接报错。

配置放在 `$XDG_CONFIG_HOME/msime-client/`（默认 `~/.config/msime-client/`）：在线服务可选 `ai-provider.json`、`tencent-provider.json`，语音服务的云端识别与润色需要 `voice-provider.json`，本地识别不需要。格式和 owner-only 权限要求与对应 provider 参数一致。仅云候选可不提供私有配置。

`ai-provider.json` 和 `tencent-provider.json` 不必手写：设置页「AI 辅助」和「输入 → 在线翻译服务」里的凭据输入框会由 Tauri 宿主直接写入这两个文件（目录 0700、文件 0600、先写临时文件再 rename），AI 凭据按服务商存到 `profiles` 下并绑定当时的接口地址和模型。凭据只从设置页流向宿主进程，设置页只能读到哪些服务商已有凭据及其绑定的接口和模型。已存在但不合规的文件（权限过宽、符号链接、JSON 无效）不会被覆盖，设置页会提示修复或删除。

`voice-provider.json` 同样可以在设置页「语音输入」里写入：识别和润色各有一组凭据输入框，按上方选中的服务商、模型、接口地址（豆包还有资源 ID、鉴权方式和旧式鉴权的 App Key）写进 provider 要求的 `asr`/`polish` 与 `asr_profiles`/`polish_profiles` 布局。语音 provider 没有该文件也能启动，本地识别（`local`）不读取任何凭据，所以识别与润色凭据都可以单独保存：只用本地模型并开启润色时文件里只有 `polish`，手写的 `{"provider": "local"}` 识别条目也会原样保留。每次保存凭据后宿主执行 `systemctl --user enable --now msime-linux-voice.socket`（并先 `reset-failed` 之前失败的服务），在设置页下载本地模型后同样执行一次，因此选择「本地模型（离线）」并选用已下载的模型即可使用，不需要任何云端凭据。清除凭据只改文件，清除最后一个凭据时删除文件，但不停用 socket，本地识别仍然可用。用户服务管理器不可达时文件照常保存，设置页给出需要手动执行的命令。豆包识别凭据上方与 Windows 一样提供「流式接口」选择：整句流式（`bigmodel_nostream`）或双向流式（`bigmodel_async`），选中后写入凭据的接口地址；地址留空时 provider 默认使用双向流式。

随包在线服务启动器通过 `--config-directory` 固定配置目录，即使启动时尚无 `ai-provider.json` 或 `tencent-provider.json`，后续创建或修复文件也会在下次请求生效，无需重启服务。目录模式下缺失、损坏或权限不合规的配置只会停用相应功能；每次请求仍执行 owner-only 文件校验。手动传入 `--ai-config` 或 `--tencent-config` 时保留原有启动校验，并优先于配置目录中的默认文件。

Windows 上在线、语音和剪贴板功能随常驻的服务进程一直可用；Linux 对应的做法是 systemd 用户 socket 激活。安装提供 `msime-linux-online.socket` 和 `msime-linux-voice.socket`，登录后 `$XDG_RUNTIME_DIR/msime-client/online.sock` 与 `voice.sock` 即存在（目录 0700、socket 0600），输入法和面板按原有路径发现服务，首个请求到达时 systemd 才启动 provider 进程。`msime-linux-setup` 准备好状态目录后会执行 `systemctl --user enable --now`：在线 socket 总是启用（没有私有配置也能提供云候选）；语音 socket 也总是启用，语音 provider 没有 `voice-provider.json` 也能启动并提供本地识别；状态目录位于默认的 `$XDG_CONFIG_HOME/msime-client` 时同时启用剪贴板监视器。没有 systemctl 或启用失败时，脚本打印可手动执行的命令，不影响首次配置本身。

手动启用或补启用语音：

```sh
systemctl --user daemon-reload
systemctl --user enable --now msime-linux-online.socket
systemctl --user enable --now msime-linux-voice.socket
```

仍可像以前一样直接启用 `.service` 让 provider 登录即常驻。

修改配置后使用 `systemctl --user restart msime-linux-voice.service`；停止并取消登录自启使用 `systemctl --user disable --now msime-linux-voice.socket msime-linux-voice.service`（只停 service 时 socket 仍会在下一个请求到达时重新启动它）。在线服务同理。异常退出会重启，配置错误不会循环重启。语音停止时留出录音退出和恢复静音的时间。

可通过 `systemctl --user edit msime-linux-voice.service` 的 `[Service]` 段设置 `Environment=MSIME_VOICE_CAPTURE=pipewire`、`Environment=MSIME_VOICE_CAPTURE_DEVICE=设备名` 和 `Environment=MSIME_VOICE_MAX_RECORDING_SECONDS=300`；凭据仍放在私有 JSON 中。无 systemd 的桌面可直接运行 `msime-linux-provider-session online` 或 `voice`。自定义安装前缀可用 CMake 的 `MSIME_SYSTEMD_USER_UNIT_DIR` 指定用户服务搜索目录。

在线和语音默认 socket 的发现由宿主每秒独立刷新，不依赖偏好目录是否配置或偏好文件能否成功读取。服务晚启动后会更新菜单可用状态并重新调度当前在线查询；默认路径仅接受实际 socket，缺失或不可访问的路径不会抛出文件系统异常。活动语音端点切换时先向旧端点取消录音，再保存新端点。

未配置偏好存储目录时，活动 IBus 会话每秒应用 runtime-options 中更新的 preferences，无需切换焦点。配置偏好目录时仍使用持久化快照。两种来源共用会话内递增版本号，菜单覆盖也可形成新版本；相同有效设置不重复提交。runtime-options 更新后忽略此前发出的旧配置读取结果。资源目录等需重建会话的配置仍在下次会话打开时使用。

`clipboard_history_path` 变更会切换当前 IBus 会话的历史来源，并使旧菜单与旧加载任务失效。关闭 `preferences.clipboard_history` 会停止读取和监听、清除会话缓存并禁用菜单操作；重新开启后重新读取配置的历史文件。宿主不会因切换路径或关闭展示而删除历史文件。

候选翻译关闭、目标语言切换或 provider 端点变更会清除当前旧译文；设置热更新后立即调度当前候选翻译。菜单翻译开关和目标语言覆盖也会提交给共享运行时。自定义翻译配置变化时使旧请求失效，避免旧服务结果回填。

云候选菜单开关会传入共享运行时设置，偏好更新后重新调度在线查询。AI 配置变化会使旧请求失效并清除旧上下文；若组合期间 Engine 延后应用新配置，宿主暂不发送旧 AI 配置的请求，仍可按当前开关查询云候选，新配置生效后恢复 AI 请求。

### 独立剪贴板采集

`msime-linux-clipboard-monitor /absolute/runtime-options.json` 在没有 Tauri 设置窗口时也可采集文本历史。它读取 runtime-options 的 `preferences_directory`，仅在该目录已保存的 `preferences.json` 中明确开启 `clipboard_history` 时工作；如果指定 `clipboard_history_path`，它必须指向同目录的 `clipboard_history.json`。配置读取失败或关闭开关时停止采集并忘记本轮去重状态。

Wayland 使用 `wl-paste --type text --watch`；X11 构建环境提供 `x11` 和 `xfixes` pkg-config 模块时，安装 `msime-linux-clipboard-watch-x11`，通过 XFixes 监听 CLIPBOARD 所有权变化，并优先使用同一工具的 `--read` 原生读取路径，无需额外安装 `xclip`/`xsel`。读取支持 UTF8_STRING、STRING 编码回退和 INCR 分块传输，限制累计数据量并使用统一超时；原生读取不可用时仍可回退 `xclip` 或 `xsel`。再次复制相同文本也会触发捕获。监听模式只输出事件标记；读取模式将有界文本经标准输出管道交给监控器，不写日志。不支持事件监听的环境继续以 750ms 间隔轮询，轮询只经 X11 读取，从不调用 `wl-paste`。Wayland 的 `wl-paste --watch` 一启动就报告合成器不提供 data-control 协议（stderr 里写 “... data-control protocol”、以状态 1 退出，GNOME/Mutter 就是这样）：在这种合成器上 `wl-paste` 只能临时建窗口抢走键盘焦点才读得到剪贴板，每读一次都会打断前台应用里的拼音预编辑，并让 gnome-shell 刷 `meta_window_set_stack_position_no_sync` 断言。监视器因此改用 `msime-linux-clipboard-watch-x11` 经 XWayland 监听和读取（Mutter 会把 Wayland 剪贴板同步到 XWayland），在日志里记一行原因，关闭再开启剪贴板历史前不再尝试 `wl-paste --watch`；没有 `DISPLAY` 时暂停采集。`wl-paste --watch` 因为别的原因退出（崩溃、被信号杀掉、连不上合成器）不算不支持 data-control，仍按下文隔 30 秒重试。每次文本读取限时 1 秒、最多 12000 个 UTF-8 字节，并保留最多 4000 个完整 UTF-16 单元。文本经标准输入交给 `msime-linux-clipboard-capture`，由 Host API 在偏好锁内重新检查开关并持有历史锁写入，避免关闭设置与写入竞态。监视器不打印剪贴板文本。

可按需执行 `systemctl --user enable --now msime-linux-clipboard.service`，使用默认 XDG runtime-options 路径。`make install` 本身不启用服务，由 `msime-linux-setup` 在首次配置时启用；语音和在线服务不依赖它。启用后服务随 `graphical-session.target` 启动，桌面会话需向用户服务管理器提供 `WAYLAND_DISPLAY` 或 `DISPLAY`。没有这个 target 的会话（Sway、i3、未用 uwsm 的 Hyprland 等）由随包安装的 XDG 自启动项 `msime-linux-clipboard.desktop` 在登录时启动，这对应 Windows 上剪贴板监视随输入法在每次登录时启动：它先把本次会话的 `WAYLAND_DISPLAY`、`DISPLAY`、`XAUTHORITY` 导入用户服务管理器，再重启服务，使 linger 下遗留的旧实例也换到本次会话的显示。从未启用服务的用户不受影响，自启动项什么都不做；`graphical-session.target` 已在运行时也交给它，不重复启动。安装包把它放在 `/etc/xdg/autostart/`；CMake 安装在前缀为 `/usr` 时同样放在那里，其他前缀放在 `<前缀>/etc/xdg/autostart/`，随 `cmake --install --prefix` 一起移动，但只有这个目录在会话的 `XDG_CONFIG_DIRS` 里时才会被读到。安装位置、卸载与启动条件由 `tests/core/clipboard_autostart.py` 钉住。

未显式指定 `clipboard_history_path` 时，IBus 使用 `preferences_directory/clipboard_history.json`，与共享设置存储及独立采集服务一致。显式历史路径仍优先；切换偏好目录时默认历史来源随之更新。监视器遇到非对象 JSON 或无效偏好结构时停止本轮采集并等待下次有效配置。

“候选操作”按当前页候选分组，一级菜单显示候选序号与完整 UTF-8 字符预览，子菜单按 macOS「候选操作」菜单的顺序列出置顶、固定到第 N 位、取消固定和删除（Linux 的菜单没有子菜单，「固定排位」平铺成「固定到第 N 位」；macOS 的「取消置顶」来自它本机记的置顶表，Linux 的置顶交给 Engine，始终显示「置顶」）。Fcitx5 的候选右键操作、状态区「候选维护」菜单和原生画布的面板内列表用同样的文案和顺序。操作仍绑定会话与候选代次。九键拼音分支及「主题」子菜单在原生 IBus 菜单中可见并沿用既有选择回调。

外部皮肤目录只由 Linux 展示层消费，不传给严格校验的 HostOptions。目录由桌面设置写入运行配置的 `candidate_skin_catalog`：保存设置，或设置的外观页、皮肤页扫描皮肤目录时，读取状态目录下的 `skins/`，每个包保留 id、标题（清单 `name`）、清单的 `base` 与 `layouts`，以及清单声明的明暗主题下的调色板（未设任何颜色的模式是空对象）；宿主只列出 id 安全、标题非空、`base` 不是 `custom` 且布局只含 `horizontal`/`vertical` 的条目，调色板原样交给共享层解析，最多 32 个，浅色槽位 `candidate_skin` 与深色槽位 `candidate_skin_dark` 当前选中的皮肤总在其中；IBus 与 Fcitx5 整读运行配置，上限 16 KiB，写入时整份文件超过 15 KiB 就从目录末尾删包，两个槽位选中的皮肤保留。只把包拷进 `skins/` 而不打开设置时，宿主看不到它。启动、菜单主题选择及设置热更新均按最终选择重新解析；目录内容变化也会刷新当前展示。自定义主题的颜色选择器叠在外部皮肤之上，由共享层按设置页同一规则合成，外部皮肤的颜色不写入共享 preferences。

Fcitx5 的 classicui 还可绘制 `base = "canvas"` 的静态 PNG／APNG、GIF 和 WebP 九宫格面板：支持 `source`／`light_source`、`stretch_dip`（缺省取 `slice_dip`）、独立的 `width_dip`、`fill`、内容与外留白；非整数边距向上取整。资源通过共享包 API 校验，共享 Rust 解码器成功返回画布后才启用贴图配色，用户文字取色器仍由共享解析器覆盖；坏图回到原生卡片与可读颜色，修复后自动重试。APNG 只显示默认图，GIF／WebP 只显示首帧；整个落在角切片里（左或右边距列、上或下边距行）的自由挂件按 `z_index` 画进面板图（`z_index < 1` 在面板下），Wayland 上因此也看得到角上的挂件，跨过拉伸区的挂件和帧动画只有原生画布画得出来；编码输入最多 8 MiB、边长最多 2048。JPEG 按上游规则不能用于 canvas 面板或挂件。

在具备 X11／Pango 的构建中，X11 输入上下文在 classicui 的明暗主题均归水杉接管时一律由插件的 Cairo／Pango 原生候选窗绘制：有效 canvas 包按清单的 Windows 版式画（九种锚点的自由挂件、前后层、外伸留白与独立 `line_gap_dip`），其余主题（`system`、内置主题、自定义主题的取色器和非 canvas 皮肤包）按下一段的 macOS 结构画。窗口不获取焦点，使用框架上报的根坐标定位，GTK 3 移动整个窗口后在下一次光标更新时上报新位置；跨屏编辑器以光标所在屏幕约束位置。指针选择、翻页与右键操作复用原候选身份和运行时围栏，失焦、清空及切换输入法时隐藏。成功显示后只保留客户端内嵌预编辑并清空桌面面板候选；资源或窗口准备失败时保留原桌面面板。第三方主题、其他活动 UI、Wayland 原生客户端与 IBus 不进入此路径；Wayland 会话里走 XWayland 的程序只有 canvas 包由原生画布画，其余主题照旧交给 classicui（会话判断与 `candidate-panel.json` 报告设置页能力用的是同一个：`XDG_SESSION_TYPE` 是 `x11` 或 `wayland` 时照它，其他值如控制台登录后 `startx` 的 `tty` 或没设时看有 `DISPLAY` 而没有 `WAYLAND_DISPLAY`），所以设置页隐藏的展开候选、拼音字号和选中外观在那里也确实不生效；逃生口 `MSIME_FCITX5_CANDIDATE_RENDERER=classicui`（Fcitx5 进程的环境变量，例如写进 `~/.config/environment.d/`）整套退回 classicui，canvas 包也画成 classicui 的静态面板。有合成器时使用 ARGB，无合成器时使用二值透明轮廓并将半透明像素合到主题底色上；挂件和留白不截获鼠标。macOS 结构的卡片在有合成器时自己画投影（X11 上没有人替 override-redirect 窗口画）：与 classicui 主题那一层相同，向下 4px、sigma 5、22% 黑，窗口四周放大左右 12、上 8、下 16 逻辑像素（装饰带够高时上边不多留），投影不截获鼠标，定位时贴着光标的仍是卡片；无合成器时不画投影。候选窗不透明度（`candidate_opacity_percent`）照 macOS 只淡化面板贴图、卡片底色和描边，文字、序号、选中高亮和面板内菜单不变；没有合成器时它不起作用。canvas 包的整体大小只经面板字体放大文字，留白、挂件和贴图边宽不随之缩放；macOS 结构的卡片按整体大小缩放全部长度。

原生画布的 macOS 结构（`NativeCanvasChrome.h`）逐条对照 macOS 的 `renderCandidates` 与 `candidatePageGeometry`，行排版、尺寸常量、装饰摆放、定位与动效数值都与 macOS 共用 `shared/candidate`（`CandidateItemLayout.h`、`CandidateChromeMetrics.h`、`CandidateDecoration.h`、`CandidatePlacement.h`、`CandidateMotionRules.h`、`CandidateExpandedGrid.h`、`CandidateWheelRouting.h`），配色仍是 Linux 的 Adwaita token（`system` 主题选中行 `#3584E4`）。首行 26pt：水杉 logo 16pt 加 6pt 间距（`show_app_logo`）、半粗体拼音（字号取 `candidate_preedit_font_size`，画成皮肤的预编辑色、没有时取强调色，插入光标是 1.25pt 竖线并在光标处让出空隙，一行放不下时向左滚动让光标留在框里），右端是 13pt 等宽数字的「1 / 3」、两个 28pt 宽的 ‹ ›（无页可翻的一侧淡到 40%）和展开键；页码与箭头跟着 `show_candidate_page_number` 一起显示或隐藏，首行什么都不画时不占高度、展开键放在第一行候选的右端。候选行上下各留 6pt、文字左右 8pt（有选中条时左边再让 6pt），序号 0.8 倍字号、与正文隔 1.5pt；释义固定 12pt（乘整体大小），竖排放得下时与正文同一行，横排总在正文下方，放不下时在列内折行；韩文汉字的训音是释义行第一行，插件注释是最后一行、斜体；横排开了释义时为还没到的释义预留高度，按配置的目标语言（主语言与不同的第二语言）预留一到两行，卡片不在眼前变高。卡片宽度以工作区一半封顶（横排一页放得下一行时可以更宽），至少 7 倍字号、有翻页时至少 76pt，横排在同一次显示期间只变宽不变窄。底部一行保留本地模式和打字连击，macOS 没有这一行。悬停填 macOS 的悬停色（浅色黑 6%、深色白 10%），指针在候选、翻页和展开键上变成手形；非 canvas 皮肤包的顶带装饰图按 `DecorationPlacement` 画在卡片上方的透明带里并播放 APNG、GIF、WebP 帧动画；它的背景图（`[candidate_window.background]`）照 macOS 画在卡片底色之上、描边与候选之下，裁在圆角里，按 `fit`（cover、contain、stretch）和不透明度乘候选窗不透明度绘制，动图取第一帧。选中竖条没设颜色时跟随主题的 accent，`system` 主题是 `#3584E4`。候选窗摆在光标下方 4pt，放不下时翻到光标上方，翻不翻按这次显示以来竖排最高的一页决定，装饰带朝上伸、不占光标间隙。皮肤清单 `[animation]` 显式写了的进场、退场与选中预设（fade、fade_scale、slide_up、spring、glow）按 macOS 的时长、曲线、错峰上限和连续输入规则播放：进场（连同它的逐行错峰）和退场要合成器（没有合成器的 Shape 窗口只有二值透明），选中装饰在两种窗口上都画；换进密码、数字或敏感字段时候选窗立即收起，不播退场；装饰图的待机动效（`decoration.motion`：float、sway、breathe、bounce）以 30 帧重画。GNOME 的 `org.gnome.desktop.interface enable-animations` 关闭或 KDE 的 `AnimationDurationFactor=0` 时不播任何动效，帧动画停在首帧；屏幕经 DPMS 熄灭时画布不再定时唤醒。读屏朗读和悬停提示（tooltip）还没有接入。

展开候选在横排上有两种样式，跟着画法走，共用 `FcitxState` 里同一份展开状态（`expanded_*`）：完整候选经 `msime_client_all_candidates` 复制一次，它会放出引擎扣着的尾部并推进代次，所以复制后重读当前页、按重读后的这一代整份核对会话、代次和严格递增的全局下标（`NativeCanvasSnapshot.h`），混代、重复或身份不合法时一个也不认；选择用 `msime_client_select_any_candidate` 带着复制时的全局身份与一代，宿主再核对焦点、会话和代次，画出来的那一代已被重载替换时不选；候选在原地换代（云候选、释义到达）时按新的一代重载。失焦、上屏、关掉面板、换皮肤、换竖排、进释义页、打开韩文汉字或注音的候选列表时收起；展开后原生画布画不下时收起并由原生画布画普通的一页。两种样式展开时滚轮都滚动内容、不翻页，不看「滚轮翻页」开关。只有 X11 的原生画布提供展开，classicui、Wayland 与 IBus 不提供。

- macOS 结构的网格：「展开候选」（`expand_candidates`）打开、横排且不止一页时，↓ 或 Tab（或点首行的展开键，`show_candidate_expand_button` 关掉时没有这个键）把这一代的全部候选排成网格，Tab 打开网格不看「翻页方式」里 Tab 翻页开没开（与 macOS 相同）：一行一页，最多看见 5 行（皮肤包的共享版式写了 `expand_rows` 时是它加 1，2–8 之间），只有高亮所在的行有序号，首行是「2 / 7」和收起键；←/→ 行内移动、↑/↓ 换行、翻页键和 Tab 滚过可见的几行（字符翻页键的缺省与普通候选窗相同；日语的 - = 和以词定字占着的那一组不翻页，照常输入）、空格和回车上屏、数字键选这一行里的第几个、Esc 或第一行的 ↑ 收起，其他键先收起再照常处理，滚轮一格滚一行。组字变了就收起。
- canvas 包的卷轴（清单 `[candidate_window] expand_panel = true`，与 Windows 的卷轴相同）：由清单打开，不看「展开候选」开关和「显示展开键」，设置页的这两项管不到 canvas 包。横排右侧固定一个展开／收起键（它代替翻页标记；释义页这类不提供展开的横排仍画翻页），点开后首行仍是当前页，下面是完整候选里当前页之后的那些（跳过联网占位和空文字，不画序号和释义），至少 2.6em 的等宽列、长候选跨格；视口是首行加 `expand_rows` 行（2–8，没写时 4 行，与网格的缺省一致），窗口连同挂件和外留白限制在工作区的一半以内，放不下时视口变矮。首行和网格一起按候选行高滚动，有细滚动条可拖滑块、点轨道；滚出视口的候选不画也不接收点击，网格项没有右键菜单。首批排 300 项，滚到接近底部时再加 300 项。卷轴不接管键盘，也不由键盘打开；继续输入时留着并按新的一代重载，滚动随继续输入、翻页或简繁投影变化回到顶部，后台换代时留在原位。

原生 X11 画布优先使用共享 `resolvedLayout`：出厂公共、出厂排列、皮肤公共、皮肤排列依次覆盖，未声明的 `canvas` 几何继承 Windows 的 5 DIP padding、2 DIP 行距与额外行高、0.8 倍序号和 4 DIP 高亮圆角；原始声明留给编辑器，显式零值与 `false` 不丢失。旧 catalog 按共享 `geometry` 再按当前 `horizontal`／`vertical` 表消费 `padding`、`item_gap`、`item_extra_height`、`number_inset`、`item_padding_right`、`item_min_width`、`number_gap`、`number_scale`、`preedit_margin_dip` 与 `justify_rows`，并遵守 `min_width_dip`。em 按实际 DIP 字号乘后取整，Pango point 字号先换算；皮肤 `font_family` 在用户字体与回退链之前，字号仍来自用户设置。显式 content insets 优先于 padding，`line_gap_dip` 优先于拼音下边距；隐藏拼音不留额外行距。横排按工作区宽度折行，各行分别撑高，均分只分给本行实际候选，`fit_content` 保持内容收缩并使用对称尾部留白。文字测量、绘制和命中共用布局；热更新同步改变三者。原生画布还消费 `item_corner_radius_dip`、`item_corners_follow_card`、`selected_bar_width`、`selected_bar_height_em` 与 `selected_bar_inside`；卡片零圆角仍保持贴角，选中条宽至少 3 DIP、高至少两倍条宽。鼠标悬停和按下只改变绘制，保持键盘选择与候选身份。共享层的 `candidate_details` 提供预编辑文字、光标、背景、分隔线和菜单颜色；文字取色器覆盖预编辑文字，透明选中条／光标色跟随 accent，透明选中文字跟随普通文字。原生右键菜单使用独立底色，不叠候选九宫格与挂件。光标位置必须与显示文字同源，未知映射时不猜。canvas 包的卷轴展开（`expand_panel`、`expand_rows`）与 macOS 结构的展开网格见上面「展开候选」一段。

原生画布把候选正文、释义与插件后缀分开传递，classicui 的原有文本和上屏行为保持一致。canvas 包的释义用正文字号的 0.78 倍（macOS 结构固定 12pt，见上文），竖排只有正文和整段释义能放下时并列，否则放在下方，横排始终放在正文下方；正文和释义按工作区宽度完整换行，Pango 对整段文字统一测量与绘制，新增行高参与高亮和命中。均分背景后保留测量时的文字宽度，避免绘制重排；预编辑、辅助行和预编辑光标也使用同一受限布局。九宫格、外留白和挂件计入最终窗口尺寸，必要时收紧面板后重新排字；超高或不可拆字形、序号栏及装饰无法容纳时仍回退桌面面板。单独移动光标也会复核工作区宽高，变化时重新排字并取消旧指针格位；无法容纳时恢复完整桌面候选。共享运行时的可选 `translation_runs` 给出同一 `translation` 字符串的 UTF-8 字节范围，识别词性和 ASCII／全角义项分号，宿主忽略越界、编码错位和未知角色。共享主题同时提供普通与选中的释义、词性、分隔符颜色；透明释义色跟随本行正文并乘 0.62，透明分段色跟随本行释义，按下使用选中配色，悬停不改变选中语义。

原生 X11 画布的面板与挂件可播放 APNG、GIF、WebP：APNG/GIF 不超过 10ms 的延时按 100ms，有限播放停在末帧，GIF 无循环声明时只播放一次；WebP 与固定 Windows WIC 回退一致，按 100ms 无限循环。候选刷新、菜单和坐标变化不重启，换肤或重新显示从首帧计时，隐藏和有限末帧不再唤醒。共享动画接口只在换肤时解码，按不同资源累计最多 800 万像素，每图最多 120 帧；旧单帧接口仍保留默认图语义。列表内翻页消费 `page_arrows` 与 `page_arrow_style`：横排独立右列、竖排下方对齐序号，支持折线／圆角三角、横排三角分隔线和禁用／悬停／按下状态；缺省关闭，开关不影响滚轮翻页偏好；共享偏好 `show_candidate_page_number` 关掉时皮肤要的翻页箭头也不画，与 classicui 主题和 macOS 相同。完整皮肤几何、声明式进退场动画与系统材质尚未补齐；Wayland 的自由画布、IBus、GNOME Shell 与 kimpanel 仍受各自面板协议和能力限制，不能据此当作 Linux 或全平台完整视觉兼容。

皮肤资源变化后，主题成功套用或退出覆盖会立即请求当前候选窗口重绘，明暗切换、坏图回退和修复恢复均无需再次按键。不变输入不重复刷新，组合和编辑器焦点保持。

候选明暗 `follow` 跟随颜色模式（与 Windows 的 `theme_cand` 跟随 `theme_mode` 一致）：全局为浅色或深色时直接取用；全局为“跟随系统”（缺省值）时，才通过桌面门户的 `org.freedesktop.appearance/color-scheme` 获取系统明暗偏好，并监听后续变化；异步读取不阻塞 IBus，门户重启后重新接入。明确的 `light`/`dark` 设置优先，门户缺失或未表达偏好时使用浅色。全局主题、自定义主题与外部皮肤共用此解析；水杉、浅色、纸白、夜青、墨自带明暗，选中它们时候选按主题自身的明暗绘制。切换不会重建输入组合。接口依据 [XDG Desktop Portal Settings](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Settings.html)。

候选配色没有明确文字色时，会根据实际背景的相对亮度选择对比度更高的黑色或白色，避免系统主题与 IBus 面板主题不一致时出现深底深字或浅底浅字。有效的用户文字色和外部皮肤文字色仍优先。

Tauri 设置页按 HostCapabilities 分开暴露候选行颜色与原生卡片装饰：Linux IBus 显示候选强调色和选中行颜色，字体、字号、悬停色和边框色仍隐藏，因为 lookup table 没有对应能力。这样共享设置不会把 Linux 能消费的 RGB 颜色误判为不可用，也不会展示保存后无效的装饰选项。

运行配置通过父目录事件监听重载，连续写入合并为 100ms 后的一次读取，并每 5 秒进行低频回退读取，覆盖原子替换、删除重建、父目录替换及目录外符号链接目标更新。每次最多读取 16 KiB；相同内容不重复解析，无效中间内容保留上一次配置，后续有效保存会继续生效。

Linux 桌面未显式设置 `MSIME_CLIENT_STATE_DIR` 时，优先使用 runtime-options 的 `preferences_directory` 作为共享状态目录，保持 IBus、独立采集和桌面历史一致。面板列出历史时重新读取文件；桌面自动采集、手动同步和复制记录均通过共享偏好锁复查开关后写入，避免关闭历史后因旧检查结果继续记录。

### 按次录音选择设备

Linux 设置页的“语音输入 → 录音设备”可选择 PulseAudio、PipeWire、ALSA 或自动选择，并填写对应的设备名称。配置保存为 `preferences.voice_input.capture_backend` 和 `capture_device`；桌面语音面板与 IBus 在下一次请求中传递这两个字段，provider 为每次录音单独构造采集命令，不修改正在录制的会话。从其他平台带来的后端（如 `macos`、`windows`、`harmony`）provider 不认识，宿主连同设备一起不转发，provider 改用自己的录音设置照常录音。

后端和设备都留空时沿用服务 `--capture` / `--capture-device`；明确选择后端而设备留空时使用该后端的系统默认设备。只填写设备则沿用服务后端。选择不存在的后端工具或非法设备会返回失败，不悄悄切换到其他麦克风。设备字段最多 128 个字符、512 UTF-8 字节，禁止控制字符；设备通过独立命令参数传递。无需重启 provider，原始音频及设备配置不会写入日志。

设置页提供“刷新设备”与“可用录音设备”选择器。桌面宿主分别通过 `pactl --format=json list sources`、`pw-dump`、`arecord -L` 读取设备，选中一项时同时填入对应后端和设备名；刷新不会改写当前设置。每个工具最多等待 2 秒、读取 1 MiB，最多返回 256 项，不启动录音，不记录原始工具输出。缺少工具或会话不可访问时仍可手动填写；PulseAudio 的 `.monitor` 播放监视源不列为麦克风。

设备目录依据 [PulseAudio pactl 实现](https://github.com/pulseaudio/pulseaudio/blob/master/src/utils/pactl.c)、[PipeWire pw-dump 文档](https://docs.pipewire.org/page_man_pw-dump_1.html) 和 [ALSA arecord 实现](https://github.com/alsa-project/alsa-utils/blob/master/aplay/aplay.c)。列表反映发现时的设备信息，不保证设备之后仍连接或可用于指定采样格式。

录音采集在连续 5 秒未收到 PCM 字节时终止本次请求并恢复被服务静音的播放流，释放麦克风供下一次重试。正常静音仍有 PCM 数据，不会被当作设备停滞。停止录音后最多收取 1 秒的管道尾部数据，并遵守录音长度及音频字节上限；取消或客户端断开后不再向识别服务补发尾部音频。16-bit PCM 被管道拆开的单字节会与下一块合并，只向流式识别器提交完整采样点。

服务退出时先取消所有会话，并等待各会话完成采集清理与音频恢复，再关闭 socket；不会因为固定 20 秒等待结束就中断仍在恢复的播放流。录音子进程清理失败仍执行音频恢复，识别连接关闭失败仍移除对应会话并释放语音并发槽位。所有工具调用继续使用各自的超时边界，退出等待不包含已经结束采集后的网络识别请求。

剪贴板历史在新配置及缺失开关时默认关闭，沿用 Windows 产品的主动启用行为；已有明确保存的开关值保留。设置页编辑开关不会立即删除历史，保存关闭设置后才清空。清空操作在共享偏好锁下再次确认仍然关闭，避免并发重新启用后误删新记录；保存冲突不会触发清空。IBus 缺失开关时也按关闭处理。

语音语言接受常见的两字母语言代码和区域写法，例如 `zh-CN` / `zh_CN`、`en-US`、`ja-JP`。服务规范化为 `zh-cn`、`en`、`ja`，空值和 `auto` 表示自动识别；OpenAI 兼容的非 SiliconFlow 请求使用基础语言代码，自动识别时不发送 language 字段。设置页和语音面板均提供中文、英文、日文和自动识别建议值。豆包及当前 SiliconFlow 路径继续由模型自动判断语言；规范化不表示模型新增了语言支持。

### 语音服务独立配置

私有语音配置继续要求 `asr` 默认对象，并允许 `polish` 默认对象；可以增加 `asr_profiles`、`polish_profiles` 对象，以服务名作为键，值采用对应默认对象的字段（`endpoint`、`token`、`model`，豆包另有 `app_key`、`doubao_auth_mode`、`resource_id`）。值内的 `provider` 可以省略；填写时必须与键一致。一个角色内每个服务只配置一次，默认对象的服务不应再次出现在 profiles 中。ASR 支持 doubao/openai/siliconflow/groq，润色支持 openai/siliconflow/groq/deepseek。所有配置均沿用私有文件权限、大小、HTTPS/WSS 地址与凭据校验。

设置页选择已配置的服务后，下一次录音在对应角色的配置中选取独立 endpoint、token 和模型，无需重启 provider，也不会把凭据传到输入法或 UI。Linux 切换服务时清空旧模型约束及识别服务专属热词表标识，空模型使用对应私有配置的模型；没有配置所选识别服务时请求失败，不改用其他服务。润色服务未配置时保留原转写。profiles 中配置豆包时也会在启动时检查其网络库依赖。新增或编辑私有配置从下一次录音开始加载，无需重启服务。

每次新录音会重新读取 `--config` 指定的私有文件，识别与后续润色使用同一份已校验快照；进行中的录音不被新配置替换。支持原子替换文件，读取期间原地修改文件会拒绝本次请求。文件格式错误、权限不符合要求或选中的服务不可用时不复用启动时的旧配置，修复后下一次请求重新读取。通过非阻塞文件描述符读取并确认是普通文件，避免配置路径被替换成 FIFO 后占住请求线程；仍限制文件为 16 KiB。

语音润色设置支持独立选择 `custom_1`、`custom_2`、`custom_3`。选择内置清理口语、忠实原文、中译英或自然口语方案时使用该方案的内置提示词；自定义槽位为空时使用清理口语。

AI 联想设置提供三个独立自定义槽位选择，online provider 根据 `prompt_id` 读取所选 `prompt_custom_*`，留空时使用内置联想提示词。缺失的方案值按第一槽位处理，不认识的方案（包括 `custom`）不发起 AI 请求。切换方案只切换选择，不改写其他槽位内容。

### AI 联想的独立服务配置与热更新

`--ai-config` 保留原有顶层 `provider`、`endpoint`、`model`、`token` 格式，也支持 `profiles` 对象：键为服务名，值包含独立的 `endpoint`、`model`、`token`，可省略与键相同的 `provider`。顶层默认服务不能在 profiles 中重复；可以只提供 profiles，最多 16 项，整个私有文件仍限制为 16 KiB。

请求按设置中的 provider 选择对应配置，并继续严格匹配 endpoint 和 model，不能用请求中的地址替换凭据绑定地址。每次符合 AI 条件的请求重读并验证文件，支持原子替换；修改服务配置或凭据无需重启，已经发出的请求使用原快照。配置损坏、权限不合规或所选服务未配置时仅忽略 AI 结果，普通云候选继续独立处理。读取不输出文件内容或凭据。

### 语音处理阶段

Linux 桌面语音面板区分“正在录音”“正在识别”和“正在润色”。支持阶段通知的宿主在语音查询中发送 `events:["status"]`，服务才发送 `type:"status"`、`phase:"recording"|"recognizing"|"polishing"` 和当前 `generation`。状态不包含转写文本，不会清空已有转写或触发提交；进入识别、润色后停止录音按钮禁用，取消仍可用。旧宿主不协商此能力时继续只收到 partial/final 文本，旧服务不返回阶段时面板仍按原流程完成识别。

IBus 快捷键语音输入也通过可选阶段回调接入以上协议，在辅助文本和“语音输入”属性中显示录音、识别、润色状态。阶段通知回到 GLib 主线程后校验会话代次、焦点与启用状态；取消或失焦后的通知不会重新显示。状态与转写预编辑分离，关闭行内预编辑仍可看到处理阶段。旧 C ABI `msime_client_voice_provider_stream` 保持文本回调行为；新宿主可使用 `msime_client_voice_provider_stream_events` 接收独立阶段回调。

### 自定义语音润色提示词

IBus 与桌面语音面板按 `polish_prompt_id` 只发送当前选中的自定义提示词，最多 8192 UTF-8 字节，完整保留内容。所选槽为空时不发送，由服务使用默认整理提示词。内置预设不发送自定义槽内容。超限提示词在启动语音请求前拒绝，不再静默截断；请求仍受整体 16 KiB JSON 限制。

### 麦克风音量反馈

Linux 桌面语音面板录音时显示实时麦克风音量。服务对 16 位 PCM 计算 RMS，沿用 Windows 的噪声底限和视觉压缩，将结果归一化为 0–1；最多每 100 毫秒发送一次，不传输原始音频。宿主通过查询 `events:["status","level"]` 协商，服务返回带当前 generation 的 `type:"level"`、`level` 数值。运行时拒绝非有限或越界值，面板只接收当前请求的更新；进入识别、润色或取消后隐藏音量条。旧服务未提供音量事件时不显示模拟音量。

IBus 快捷键录音通过 `msime_client_voice_provider_stream_feedback` 接收同一音量协议，在候选辅助栏显示十格麦克风音量。只在收到真实音量事件后显示，格数未变化时不重绘；停止录音、进入识别或润色后隐藏。音量事件在 GLib 主线程按录音代次、焦点和启用状态过滤，取消后不恢复显示。原有文本与阶段 C ABI 保持兼容。

启用录音时静音其他应用后，语音服务每 500 毫秒在后台检查新出现的 PulseAudio/PipeWire 播放流，覆盖录音期间新启动的播放器。已见过的流不会反复静音，保留用户手动调整；原本静音的流不会在结束时被取消静音。服务最多记录 256 个实际静音流，按流身份恢复，避免把复用的节点 ID 当作原流。结束或取消录音时先停止监听，再恢复音频，防止恢复后又被后台线程静音。

### 随包录音提示音

Linux 安装包包含 Windows 固定提交中的开始、结束录音提示音，离线转换为 16 kHz 单声道 PCM，通过已有 PulseAudio、PipeWire 或 ALSA 播放工具输出，不增加运行时 MP3 解码依赖。是否播放只看输入法转发的总开关 `sound_enabled`。音频来源见 `data/voice/SOURCE.md`；安装路径随自定义前缀定位。单独复制服务脚本而未带音频资源时仍使用短音回退。

### 语音服务默认模型

私有语音配置的 `asr`、`polish` 及各 provider profile 中，省略 `model` 或填写空字符串时，沿用 Windows 固定基线的默认模型；非空模型保持原样。凭据仍须明确配置；省略端点时使用对应服务的默认地址。

| 服务 | ASR 默认模型 | 润色默认模型 |
| --- | --- | --- |
| OpenAI | `whisper-1` | `gpt-4o-mini` |
| Groq | `whisper-large-v3-turbo` | `llama-3.3-70b-versatile` |
| SiliconFlow | `FunAudioLLM/SenseVoiceSmall` | `Qwen/Qwen3-8B` |
| DeepSeek | — | `deepseek-v4-flash` |
| Doubao | 使用流式资源配置，无模型名 | — |

这是兼容 Windows 提交 `7fa6fb1a7862c5ca1541b9cb839d9bea3a06e2c6` 的配置回退规则。共享设置中明确指定的非空模型仍须与解析后的私有配置一致。

语音私有配置的 `endpoint` 省略或为空时，也沿用 Windows 同一固定基线的默认地址：OpenAI、Groq、SiliconFlow 分别使用各自音频转写或聊天完成接口，DeepSeek 使用聊天完成接口，Doubao 使用流式识别 WSS 接口。明确指定的非空端点保持不变，继续要求 HTTPS/WSS 且禁止重定向。默认服务和备用配置的 provider 名称按 ASCII 大小写归一化（例如 `OpenAI` 与 `openai` 等价）；大小写不同但实际重复的备用配置会被拒绝，模型名保持大小写敏感。

### 手写候选操作

手写面板默认点击候选复制到剪贴板，沿用 Windows 手写面板行为；可切换为直接输入到已记录的目标窗口，选择保存在本机面板偏好中。候选旁保留另一种操作的快捷按钮，方便临时复制或输入。宿主只提供一种能力时自动使用可用操作；操作进行中禁止重复提交与切换，失败时保留候选和笔画供重试。

手写候选字号随实际按钮宽度和 Unicode 字符数调整，沿用 Windows 的 13–34px 范围；扩展汉字按一个字符计算。多字候选可换行，窗口尺寸变化时重新计算，悬停显示完整候选与当前操作。字号调整只影响展示，复制与输入仍传递完整候选。

外部手写服务的候选响应在运行时按原顺序去重，避免重复候选占用面板位置；空文本、超过 4096 UTF-8 字节或包含控制字符的候选会使响应被拒绝。保留最多 12 条响应的边界，候选由 Engine 按共享手写策略处理：中文优先，同组内保留识别器顺序。

本地离线手写识别和外部 socket 识别共同使用 Engine 的候选策略：去重、中文候选优先、同组保持原顺序，最多十二项。本地识别也从八项扩展为十二项。中文范围对齐 Windows 手写面板固定基线的 CJK、扩展 A 与兼容汉字范围；排序不由平台界面维护。

构建只安装按 `resources/handwriting-model.lock.json` 校验过的模型，文件名固定为 `handwriting-zh_CN.model`，保证桌面面板和 `msime-linux-handwriting --local` 自动找到同一模型（发布页的 deb/rpm 不装模型，两者都改用设置应用下载的那份）；要试别的模型，在运行时用显式模型参数或 `MSIME_HANDWRITING_MODEL` 指过去。桌面配置或环境变量的模型路径为空时视为未配置并继续查找安装资源；非空但无效的显式路径仍会报错，不切换到其他模型。

### Wayland 剪贴板变更通知

启用剪贴板历史时，监视服务优先使用 `wl-paste --watch` 接收复制事件，减少快速连续复制被轮询漏掉的情况。每个事件通过标准输入读取最多 4096 字节并重新读取历史开关；空、清除或标记为敏感的选择不保存。关闭历史或停止服务会结束监听及其子进程。缺少工具或监听异常退出时回退到有界轮询，失败后至少间隔 30 秒再尝试监听；不支持 data-control 时改经 XWayland 监听，见上文。协议依据 [wl-clipboard 官方手册](https://github.com/bugaevc/wl-clipboard/blob/master/data/wl-clipboard.1)。

剪贴板监视读取完整 runtime options 和偏好文件时允许最多 1 MiB，容纳自定义语音提示词等设置，不再因为超过 16 KiB 而停用历史。读取只接受普通文件，使用非阻塞打开并核对读取前后的文件信息；遇到写入中的不完整配置会跳过本轮，后续重新读取。剪贴板文本自身仍保持 4096 字节上限。

语音服务在 socket 同目录保存仅用户可读的 `.audio-mute.json` 恢复记录：先原子保存再静音，启动、下一次录音及结束录音时尝试恢复。记录只含音频后端、流编号、系统启动标识及流/进程身份摘要，不含应用名称、输入、音频或凭据；通过进程启动时间避免 PID 复用误恢复。服务异常退出后重启可恢复仍匹配的播放流，工具失败时保留记录。无法安全保存恢复记录或读取进程身份时跳过该流静音，继续录音。

语音润色采用 3 秒总时限，覆盖 DNS、连接、TLS 和响应读取；超时终止并回收请求子进程，直接提交原始 ASR 文本。缓慢持续返回数据不会延长润色等待。请求通过匿名管道传递，凭据和识别文本不进入命令行、文件或日志。

批量 ASR 每次请求设置 60 秒总时限，覆盖 DNS、TLS、上传与响应读取。用户取消时终止并回收网络请求进程；润色同样响应取消。硅基流动仅在网络错误、总超时或 HTTP 5xx 时等待 400 毫秒后重试一次；其他服务商不自动重试，HTTP 4xx 和无效响应不重试。网络错误仅在进程间传递可重试分类，不传递服务器错误正文或敏感诊断。

豆包流式识别使用独立收发线程：音频发送等待期间仍接收实时转写，收到最终回复或接收失败时关闭传输以释放阻塞上传。结束或取消时停止并回收接收线程，保留 30 秒响应/最终结果等待上限与现有有界音频队列。

剪贴板面板与 Windows 已提交版保持紧凑预览：每条记录单行省略，换行与制表符只在列表预览中显示为空格；悬停提示保留多行并限制为 200 个 Unicode 字符，不拆分字符。复制、粘贴和删除始终使用完整原文，搜索无匹配时显示明确提示。

表情与符号面板在切页、切换子分类和修改搜索时回到列表顶部；若原列表项目仍持有键盘焦点，将焦点重置到新列表首项，搜索框和分类按钮的焦点保持不变。普通网格在顶部/底部继续按上/下键时定位到首项/末项，颜文字继续按实际排列进行垂直导航。后台剪贴板更新不会触发上述切页重置。

手写画布按 SVG 实际屏幕变换映射触控笔/鼠标坐标，适配边框、缩放和 viewBox 留白。连续采样采用 Windows 面板的半单位抖动阈值，避免微小移动过早耗尽采样容量；抬笔终点始终保留，继续支持 Linux 触控与笔输入的单点笔画。

屏幕全键盘按实际包含空格键的行应用 Windows 宽度比例，不依赖行号，因此加入功能键和数字小键盘后仍保留宽空格、Caps Lock、Enter 及左右 Shift 的尺寸。九宫格使用等宽按键，避免混入全键盘的 Backspace/Enter 宽度比例。

屏幕键盘每次发送按键前重新获取前台输入目标，打开键盘后切换编辑器也会跟随当前窗口；获取失败则停止该次发送，不回退到旧窗口。目标捕获和发送在后台执行，保留按键队列顺序。手写、语音等需要编辑内容的工具面板仍保留打开时的原输入目标。

X11 屏幕键盘及工具面板的 Ctrl+V 先通过 `windowactivate --sync` 激活目标，再通过 XTEST 发送按键，避免应用忽略 `--window` 的 XSendEvent 输入。激活和发送共用 3 秒期限，目标无法激活或进程失败时返回失败，不自动重放。依据 [xdotool 官方手册](https://raw.githubusercontent.com/jordansissel/xdotool/master/xdotool.pod) 的 SENDEVENT NOTES。

X11 工具面板的单行文本提交也使用激活目标后的 XTEST 输入，文本经匿名管道送入 `xdotool type --file -`，不放进命令行或临时文件。激活、管道写入和输入共用 3 秒期限；超时终止并回收工具，不自动重放可能部分提交的文本。换行/制表符文本保留剪贴板粘贴路径。依据 [xdotool type 官方实现](https://raw.githubusercontent.com/jordansissel/xdotool/master/cmd_type.c)。

Wayland 单行文本通过匿名管道传给 `wtype -` 或 `ydotool type --file -`，不进入进程参数。wtype 发送限时 3 秒；ydotool 为默认按键保持时间预留每个 ASCII 字符 30 毫秒并加 3 秒余量。超时终止并回收工具，不自动重发。Sway 先切回原目标；多行文本和 ydotool 非 ASCII 文本保留剪贴板粘贴。接口依据 [wtype](https://raw.githubusercontent.com/atx/wtype/master/README.md) 和 [ydotool](https://raw.githubusercontent.com/ReimuNotMoe/ydotool/master/manpage/ydotool.1.scd) 官方说明。

Sway 面板输入在发送前解析窗口切换命令的成功回复，并读取窗口树确认原目标已获得焦点；目标关闭、切换失败或焦点不匹配时停止发送。命令等待限时 2 秒，焦点读取限时 1 秒，回复均有大小限制且不写入日志。wtype 按键发送也设置 3 秒期限，失败不自动重放。协议依据 [Sway IPC 官方说明](https://raw.githubusercontent.com/swaywm/sway/master/sway/sway-ipc.7.scd)。

前台目标获取对每个外部命令设置 1 秒期限，并限制窗口树和工具输出大小。wtype 通过空标准输入探测虚拟键盘连接，不发送文字或按键，不使用上游不支持的 `--version`；文本发送省略延时参数，使用默认零延时，避免显式 `-d 0` 被拒绝。依据 [wtype 官方参数解析](https://raw.githubusercontent.com/atx/wtype/master/main.c)。

IBus 与 Fcitx5 菜单顶层提供「关于水杉输入法…」入口，与 macOS 输入法菜单和 Windows 托盘菜单对应，直接打开共享设置的关于页。桌面启动器也提供「关于水杉输入法」快捷操作；命令行可用 `msime-linux-settings --panel about`。宿主以 `--route=<路由>` 参数启动设置命令（关于页是 `--route=settings:about`），自定义的 `MSIME_CLIENT_SETTINGS_COMMAND` 同样收到这个参数。

IBus 桌面工具和桌面启动器提供「本地剪贴板」入口，`msime-linux-settings --panel clipboard` 可直接进入现有剪贴板历史页，使用同一份历史及搜索、复制、粘贴、删除功能。这个独立窗口的页头叫「剪贴板」，不显示表情面板的分类导航，Esc 直接关闭窗口；列表上方一行显示设置应用里剪贴板采集最近一轮的状态（与 macOS 原生剪贴板页的状态行相同，另有一种「读不到系统剪贴板」，说明缺少 wl-clipboard、xclip 或 xsel），状态变化时随剪贴板变化通知刷新。记录的 × 删除按钮只在悬停或聚焦这一行时出现（触屏上一直显示），右键（键盘上的菜单键或 Shift+F10）菜单里有「发到云剪贴板」；悬停或用键盘移到一条记录上片刻，会在行的上方（放不下时在下方）显示最多 12 行的长文本预览气泡，代替桌面主题的原生 tooltip。未开启历史时保留主动开启界面，不自动开启采集。该面板与其他可编辑面板一样，在通用 Wayland 粘贴前释放焦点。

IBus「剪贴板历史」菜单内可直接打开完整历史面板，即使历史关闭也能进入主动开启界面；关闭或当前输入不可用时，刷新、条目提交及清空仍禁用，菜单不展示缓存条目。菜单预览将换行和制表符压成空格，提交与删除仍使用完整原文。

IBus 的空格锁定仅在录音期间仍按住 RAlt、Ctrl+Win 或 RCtrl+RAlt 快捷键时生效；它固定为开，不再读 `hotkey_hold_space_lock`。菜单或 Ctrl+F9 启动的录音、松开快捷键后的锁定录音，以及识别和润色阶段，都不会把普通空格作为锁定操作吞掉；这些空格继续走编辑器/Engine 的常规输入路径。

IBus 已消费的语音快捷键和锁定空格会保留按键记录直到抬键，停止录音、取消或服务提前返回结果不会清除该记录；期间自动重复的按下事件继续被消费，避免重新启动录音或向编辑器泄漏半个按键周期。失焦、宿主重置和会话关闭时清除记录，避免在新的输入上下文沿用旧按键状态。

IBus 按住式语音快捷键与 Ctrl+F9 切换式快捷键保持不同语义：已有未锁定的录音时，按住 RAlt、Ctrl+Win 或 RCtrl+RAlt 会继续录音，松开相应组合键才结束；空格锁定后再次按快捷键则立即结束录音。通过菜单或 Ctrl+F9 启动的录音也可由按住式快捷键接管，组合键先松开 Ctrl 同样结束录音。Ctrl+F9 始终使用开始/结束切换。IBus 按物理键判断按住式快捷键：RAlt、Ctrl+Win、RCtrl+RAlt 在按下时不要求修饰位已经置上（X11/GDK/mutter 报告的是按键之前的状态），Super 的 `MOD4` 与 `SUPER` 位视为同一个键。

IBus 为 Ctrl+F9 和按住式语音组合键分别保留已消费按键的抬键记录。两者重叠使用时不会覆盖彼此；同一时刻只激活一个按住式组合键。组合键先松开 Ctrl 会结束该按住状态，但仍消费随后 Win/RAlt 的抬键；空格锁定后的录音可继续，重新按下组合键可结束录音。

IBus 空格锁定录音后立即在语音属性和辅助文本中显示“录音已锁定”，提示可松开快捷键，以及再次按快捷键、点击语音菜单结束或 Esc 取消。语音活动期间还显示独立的“取消语音输入”菜单项，录音、识别和润色阶段均可用鼠标取消且不提交结果；活动结束后隐藏该项。此入口对应 Windows 语音浮层的取消操作，适配为 Linux IBus 原生菜单。

空格锁定使用启动录音时已记录、且尚未松开的按住式组合键状态，不重新匹配空格事件的全部修饰键。录音期间额外按下 Shift、Alt 或其他修饰键不会阻止锁定；松开该录音组合键的必要按键后即结束按住状态，菜单和 Ctrl+F9 录音仍不会误用空格锁定。

IBus 语音服务失败或返回无效结果时，在辅助文本中提示检查语音服务、麦克风和提供商配置；成功返回空文字时提示重新录音，结果处理失败也有独立提示。提示仅使用固定文案，不显示 provider 原始错误、网络响应或凭据。取消、失焦与过期代次沿用原有丢弃规则，不弹出失败提示；下一次常规输入或录音刷新辅助文本。

IBus 语音启动阶段的配置或 Host API 异常会取消可能已开始的语音代次，并提示检查语音设置；发送结束录音失败时会取消本次语音并明确提示检查服务后重试。取消同时清理识别阶段与麦克风音量展示状态，已消费按键仍按原规则保留到抬键。所有失败提示均为固定文案。

关闭 `stream_inline_preedit` 后，IBus 仍在辅助区域显示服务返回的实时转写，对应 Windows 语音浮层中的文本展示；编辑器内不显示中间预编辑，仍只提交最终结果。辅助区域预览最近 160 个 Unicode 字符，换行和制表符仅在预览中转为空格；完整结果沿用原有界限与提交路径。转写在取消、完成、失焦或关闭会话后清除，过期代次不能重新显示。

语音开始/结束提示音按 PulseAudio（paplay）、PipeWire（pw-cat）、ALSA（aplay）依次尝试可用播放器；某个客户端已安装但对应服务不可用或播放超时时，自动回退到下一后端。任一后端成功即停止尝试；所有后端失败也不阻断录音和识别。提示音开关与原有 PCM 资源保持一致。

录音期间静音播放流时，单个流退出、拒绝操作或返回无效信息不会中断本轮其他流的处理。每轮最多尝试 32 个未处理流，保留总量上限与取消检查；操作失败前已写入的恢复意图仍保留，以覆盖命令失败但音频服务可能已执行静音的情况。

PipeWire 静音恢复逐流处理音量读取失败：可读取且身份匹配的流继续恢复，读取失败的流保留恢复记录供后续重试，不将其当成已消失而删除。整个图枚举失败仍保留该后端全部记录；常规内存恢复也隔离单个流的身份或操作异常。

PipeWire 恢复目标节点仍存在但暂时缺少 info、props、media.class 或 object.serial 时，按不可读取处理并保留恢复记录；不会把缺失属性当成节点已经消失。身份可读取后再匹配并恢复，其他节点仍独立处理。

单个播放进程在枚举后退出或身份无法读取时，只跳过该流，不禁用其他流的恢复日志；真正的日志写入失败仍禁止追加静音操作。恢复后删除记录的写盘失败时，内存重新保留这些记录供后续恢复重试；没有可清理记录时不重复写盘。

屏幕键盘有按键正在发送或排队时，语音入口提示等待发送完成，避免打开语音面板改变输入目标。语音窗口打开期间暂停键盘按键及重复打开/关闭操作，打开失败显示可重试提示；不会自动重放按键。

手写识别板关闭失败时保留内容并显示可重试提示；关闭请求期间不重复发起关闭、开始新笔画或提交候选。关闭成功会使待处理识别及提交回调过期，避免窗口关闭后旧结果继续更新面板。

手写识别板关闭请求期间，撤销、重做、重写、重新识别、候选模式和候选按钮均不可操作；对应处理函数也拒绝快捷键或其他途径触发，避免继续创建识别请求。仍在书写中的笔画需要先完成再关闭；关闭失败后恢复正常操作。

手写候选支持键盘导航：Tab 聚焦候选后，左右方向键移动到相邻候选，上下方向键跨四列移动，Home/End 到首末候选并自动滚动到可见位置。Enter/空格沿用按钮行为，执行当前复制或输入模式；组合快捷键、输入法组合输入和旁边的独立操作按钮不被导航拦截。

与 macOS 的表情面板一样，「输入到原应用」或「粘贴」成功后 Linux 的表情和本地剪贴板窗口随即关闭，只插入一次；复制不关闭，复制、输入等临时结果以浮在内容上方的胶囊提示显示 1.6 秒（「已复制  X」，过长截断），底部状态行只留操作说明和错误。表情与本地剪贴板面板的关闭按钮和首页 Esc 共用异步操作管理。复制、粘贴或修改历史期间不能同时关闭；关闭过程中也不能重复关闭或启动剪贴板操作。关闭失败保留面板并显示重试提示，旧客户端或卸载后的失败不会更新当前面板。

本地剪贴板页面提供“刷新列表”入口。历史记录或启用状态读取失败时显示明确错误，不再伪装为空历史或沿用旧启用状态；重试成功后恢复列表。刷新沿用请求代次保护，旧响应不会覆盖新结果，复制/粘贴或关闭操作期间不可手动刷新。

剪贴板变化通知不存在或订阅失败时，仅在剪贴板页启用 400ms 回退轮询；页面不可见、正在复制/粘贴/关闭或上一轮读取未结束时跳过轮询，恢复可见时立即尝试刷新。通知可用时继续优先使用通知，切页、换客户端和卸载时清理回退计时器与可见性监听。

连续剪贴板变化通知会合并处理：同一订阅下只保留一轮进行中的历史读取，以及一次待刷新标记。读取期间的新通知使旧快照过期，读取完成后补读最新列表；切页、换客户端或卸载会丢弃待刷新标记，避免后台继续补读。

剪贴板首次列表读取不等待通知订阅返回。订阅仍在等待时，剪贴板页先使用原有可见页轮询；订阅成功后停止轮询并补读一次，覆盖注册过程中的变化。订阅失败或缺失则继续轮询，迟到的订阅在页面离开后立即取消。

删除当前聚焦的本地剪贴板记录后，焦点移到删除位置的下一条记录（末尾则移到上一条）；列表为空时回到搜索框。删除未生效、用户切页/修改搜索或已主动移动焦点时不重新定位，避免抢焦点。

本地剪贴板记录聚焦时，Ctrl+C 复制完整原文，Delete 删除当前记录并沿用相邻记录焦点恢复。快捷键仅在宿主提供对应能力时生效，输入法组合事件和其他修饰键组合不拦截；自动重复按键不连续执行，搜索框编辑不受影响。

表情、符号、颜文字与本地剪贴板面板中，Ctrl+F 聚焦搜索框并选中当前搜索词，便于从列表直接改搜；搜索框按向下键继续进入结果列表。输入法组合输入、其他修饰键组合与已处理的按键事件不被拦截。

面板搜索分别匹配项目原文和不区分大小写的关键词，不拼接字段，避免跨字段误匹配。搜索保留首尾空白，首尾空格可用于匹配剪贴板原文；剪贴板仍不区分大小写，预览折叠空白不影响匹配。分类搜索提示与当前表情、颜文字、符号或剪贴板页面对应。

Linux 在线 provider 的云候选、AI 和翻译 HTTP 请求在独立子进程中执行，总时限覆盖进程启动、DNS、TLS、上传及响应读取。云候选沿用 Windows 的 2 秒总预算、AI 沿用 7 秒、单次翻译沿用最多 2.5 秒预算；超时终止并回收请求进程，provider 正常退出时也清理仍在运行的 HTTP 请求。HTTP 子进程同时设置不超过 7 秒的进程内定时终止；即使父服务异常退出，DNS 或缓慢响应也不能让子进程无限挂起。请求文本及凭据只经匿名管道传递，不进入命令行或错误日志；响应继续限制为 64 KiB，禁止重定向。

IBus 对同一 Engine 查询分别发出云候选和 AI 请求，各自完成后立即回填，云候选不等待较慢的 AI。传输请求只启用对应来源，云候选请求不携带 AI 配置与上下文；回填仍使用原始 Engine 查询身份，并仅接受该请求对应来源的结果。每个来源各自维护一个在途请求和重复查询记录；AI 仍在等待时，云候选可处理新一轮输入。失焦、配置变化及旧候选代次仍沿用原有失效机制，最终排序由 Engine 决定。

语音批量识别和润色的 HTTP 子进程分别设置 60 秒、3 秒的进程内时限，覆盖读取请求正文、DNS、TLS、上传和响应处理；父 provider 异常退出后也能自行终止。父进程仍执行原有取消与总时限控制，并将子进程的定时终止归类为可重试失败；音频、转写和凭据不进入命令行或日志。

桌面辅助面板定位保留宿主坐标类型：X11 窗口几何使用物理坐标，Sway 容器几何使用逻辑坐标。首次创建与复用面板均通过同一带类型的位置接口设置，新窗口定位后再显示，副屏负坐标不再强制归零。通用 Wayland 与 ydotool 输入目标没有可用窗口几何时，由桌面环境放置面板；窗口管理器仍可拒绝应用主动定位。

面板定位读取 `xdotool getwindowgeometry` 和 `swaymsg get_tree` 时使用 1 秒总时限，输出分别限制为 4 KiB、1 MiB。超时、命令失败或无效几何均跳过主动定位，面板继续打开；子进程按已有工具回收，不转发外部命令诊断。

X11 辅助面板首次打开和再次显示时，将目标位置限制在最近显示器的工作区内，面板逻辑尺寸按该显示器缩放比例转换后参与计算，保留负坐标副屏。面板大于工作区时对齐工作区起点，保持顶部操作区域可访问；无法取得显示器信息时沿用原位置，Sway 逻辑坐标不混用物理工作区。

Sway 面板从同一次有界 `get_tree` 响应读取编辑器所在工作区矩形，沿普通节点与浮动节点查找所属工作区，并在逻辑坐标中限制面板位置。负坐标工作区得到保留；面板大于工作区时对齐起点。缺少有效工作区矩形时保留编辑器附近的位置，最终定位仍由 compositor 决定。

桌面菜单、IBus 工具入口和 `msime-linux-settings --panel …` 启动辅助面板时，捕获前台编辑器后复用设置窗口内的定位流程，包括 X11 工作区限制和 Sway 逻辑工作区限制。读取位置失败时仍由桌面环境安排窗口，不影响面板启动。

Linux 桌面设置外壳在 session D-Bus 可用时使用单实例路由。重复执行 `msime-linux-settings --panel …` 不会创建第二个设置进程；已运行的外壳会显示对应设置页或辅助面板，并重新捕获输入目标。关闭主设置窗口时先隐藏外壳，保留最多十分钟供后续路由复用，超时后才退出；辅助面板仍按各自窗口生命周期关闭。启动器仍只传递受限路由参数，不会把它们写入输入内容。

X11 面板选择显示器及水平居中时以编辑器的物理中心为锚点，再按目标显示器缩放比例换算面板宽度，随后限制到工作区；避免高 DPI 下仅按逻辑宽度计算导致右偏，也避免用面板左边缘误选邻近显示器。

屏幕键盘快捷键 Ctrl+Shift+Super+K 和 Ctrl+Shift+Alt 的重启、退出、清缓存快捷键在一次按住期间只执行一次。宿主按物理键码持续吞掉重复按下及对应松开事件，即使先松开修饰键也不会泄漏该字母；无物理键码的合成事件使用统一大小写的键值。焦点离开、会话关闭或 IBus reset 时清除记录。启动失败时保留原有透传行为。

IBus 属性菜单与 Fcitx5 状态区顶层的「显示工具栏」是持久化的工具栏开关，对应 macOS 输入法菜单的「悬浮工具栏」和 Windows 托盘中的悬浮工具栏开关。关闭后收起悬浮工具栏、隐藏「工具栏」子菜单，开关本身仍在顶层，可以重新开启；悬浮工具栏的工具菜单里的「隐藏悬浮状态栏」走同一条保存路径。后台线程从共享 PreferencesStore 读取最新快照，只修改 floating_toolbar.enabled，并按 revision 比较保存；成功后更新当前菜单，其他会话和桌面设置沿用偏好热重载。保存期间禁止重复提交，版本冲突或存储失败不乐观切换显示状态。未配置绝对 preferences_directory 时开关禁用。悬浮工具栏拖过的位置与 Windows 一样只记在这台机器的状态目录里（`floating-toolbar-position.json`，见「输入细节与平台差异」），不进共享偏好；「工具栏」子菜单本身的位置由 IBus 面板和桌面环境管理，不伪造或持久化。

IBus 菜单的云联想和候选翻译开关在配置绝对 preferences_directory 时写入共享 PreferencesStore，重启和切换输入上下文后保留，并通过已有热重载同步到桌面设置。后台保存只修改对应字段，采用 revision 比较，成功前不改变当前开关；保存失败保留原状态。与工具栏开关共用单个待保存操作，避免重复提交。未配置偏好目录的直接预览仍保留会话内临时开关行为。

翻译目标语言菜单同样使用共享偏好存储，支持英语、法语、日语、西班牙语、俄语、德语和韩语。只响应单选项的选中事件，忽略旧选项取消选中的通知；保存期间禁止重复提交，成功后通过现有偏好更新路径清除旧语言翻译并调度新请求。版本冲突或存储失败不改变选择；无偏好目录的预览保留临时语言选择。

候选明暗（跟随颜色模式、浅色、深色）、候选排列方向和预编辑显示菜单接入共享偏好保存。配置绝对 preferences_directory 时，后台按版本比较写入单个字段，成功后通过已有偏好热重载更新显示，不为外观修改主动提交当前组合或重建输入会话。保存期间禁用选项，忽略单选取消通知；失败不改变当前设置。无偏好目录的预览保留原有会话级行为。

「主题」菜单使用统一的 GlobalTheme 单选动作，先列共享层 `msime_client_theme_catalog` 给出的全局主题（顺序与标题都取自共享层，宿主不保留副本），再逐个列出已加载的外部皮肤，每个皮肤一个单选条目（Fcitx5 的皮肤条目在运行配置的皮肤目录变化时重建并注册动作名，托盘与 kimpanel 同样能直接选中任意一个）；与设置页的皮肤卡片一样，选外部皮肤即选中自定义主题并把它写进清单 `base` 所属明暗的槽位、把 `base` 写成清单的 `base`：深色底的皮肤写 `candidate_skin_dark`（并清掉旧文档留在 `candidate_skin` 里的深色皮肤），浅色底的写 `candidate_skin`（深色槽位空着、原来的 `candidate_skin` 又不是浅色皮肤时先把它挪进深色槽位），跟随系统的两个槽位都写，规则与设置页 `applyCandidateSkin` 相同；选「自定义」与设置页的「自定义」卡片一样按原样选中自定义主题、不改动其中任何字段；若它正引用一个已列出的外部皮肤，菜单随后勾选的是该皮肤条目，因为画出来的就是它；两个槽位各引用一款时勾选候选窗当前明暗下的那款，这个明暗的槽位没有已列出的皮肤时勾另一个槽位的（不再引用皮肤要在设置页用「自定义主题不使用外部皮肤」）。已退役的旧内置皮肤 id 不被接受。点击时复查条目仍在菜单中。配置共享偏好目录时，选择通过后台 revision 比较保存，成功后热重载显示；失败保持原主题。保存期间禁用选择，取消选中通知不触发切换。无存储目录的预览继续使用原有会话级切换。

IBus 属性菜单与 Fcitx5 状态区按 macOS 输入法菜单（`platforms/macos/src/input/InputController.mm` 的 `menu`）的顺序和文案排列，两个前端共用 `src/core/MenuLabels.h` 里的文案：「中文输入」「英文候选模式（Ctrl+Shift+E）」；「繁体输出」「全角字符（Ctrl+Shift+Space）」「中文标点（Ctrl+.）」「显示译文」；「输入方案（<当前方案>）」「主题（<当前主题>）」；「显示工具栏」及紧跟的「工具栏」子菜单、「水杉表情面板…」「云剪贴板…」「云词库…」「水杉屏幕键盘…」「手写输入…」（不提供手写的版本不列）、「开始/结束语音输入」；「水杉输入法设置…」「关于水杉输入法…」。IBus 在各组之间画分隔线。IBusProperty 和 fcitx::Action 都没有快捷键字段，macOS 画在右侧的快捷键只能写进名字里，写进去的都是两个宿主固定处理、设置里不能改的组合键。macOS 菜单之后是 Linux 独有的入口：「词库…」、候选操作、九键拼音、剪贴板（Fcitx5 还有表情选择），再往后是三个子菜单：「输入选项」（双拼键位、辅助码、中英混输、符号、Emoji、颜文字、云候选、九键、数字行、以词定字和快捷模式等）、「标点与翻译」（智能标点、配对、标点锁定、整句翻译及目标语言）、「候选与词频」（候选布局、每页候选数、候选明暗、预编辑、双拼与五笔提示、学习与词频），最后是「桌面工具」（本地剪贴板…、语音面板…、使用帮助…、问题反馈…、重启输入法服务、启用语音输入）。没有删减任何入口，只改了位置和名字。「输入方案」子菜单的名字与 macOS 相同：全拼、双拼（<键位方案>）、五笔 86 或五笔 98、日语、韩语、粤拼、注音、越南语、藏文、笔画，粤拼、注音与笔画只在其词库已安装时出现。Fcitx5 按 Engine 的方案顺序列成一组，与 macOS 一致；IBus 仍是「中文」（回到上次的中文方案）加日语、韩语、越南语、藏文一组，分隔线后是中文方案一组，两组内部都按 Engine 顺序，单选因此互不牵连——macOS 没有「中文」这一行，去掉它要先决定这一行承担的「回到上次中文方案」怎么办。macOS 的「中文输入」「英文输入」是一对单选，两个前端都只放一个「中文输入」开关（勾选即中文、未勾选即英文）：它就是 Shift 切换的那个状态，IBus 的面板还从这个属性读 中/英 符号。直接挂在 Fcitx5 状态区的动作保留图标（kimpanel 把它们画成独立按钮），只出现在子菜单里的动作不带图标，与 macOS 的菜单一样。GNOME Shell 的 IBus 面板只注册输入模式与「水杉输入法设置…」，保持原有的精简菜单。Fcitx5 状态区本身不放分隔线：kimpanel 会把状态动作原样列出，分隔线在那里只会是空白项，所以分组只在子菜单内部用分隔线；kimpanel 的 IBus 后端同样把分隔线画成空白项，这是该面板的限制。子菜单中的每个条目和分隔线都向 Fcitx5 注册了动作名，托盘（SNI/dbusmenu）与 kimpanel 能看到并触发它们。Fcitx5 的托盘菜单（SNI/dbusmenu）列出、点击时作用的都是最近聚焦的输入上下文，而在 Hyprland 上的 noctalia、waybar 等桌面，点托盘本身就会让应用失焦，所以应用失焦时宿主不关它的会话，而是把这个上下文留给托盘：组字丢掉、录音和音乐停下，但会话、状态动作和勾选都还在，菜单里的开关点下去照常作用于它的会话并保存，中英文切换也记进按应用的记忆。语音、候选维护、剪贴板、云剪贴板和表情选择这几项只往有焦点的输入框里写，失焦时先从状态区拿掉。同一时间只留一个上下文，任何输入上下文获得焦点时它的会话才关闭，不是同一个的话连状态动作一起移除；回到同一个上下文时照旧关掉重开，重新读取运行配置。换到别的输入法时状态动作照旧移除（Fcitx5 也在每个输入法激活前自己清空这一组）。

IBus 候选表只发布可表达的行级颜色：候选边框、圆角与阴影只由 Fcitx5 classicui 主题绘制，固定排位的行画 macOS 的固定色 `#379AD3` 而不是皮肤的 accent，圆角、阴影与 hover 在 IBus 中无法呈现。固定候选仅在未高亮时画 macOS 和 Windows 的固定色 `#379AD3`（Adwaita 的选中底色是同一色系的蓝，压在上面读不出来），高亮后与其他候选一样优先使用选中行正文颜色。

候选数量（1–10，第十个候选用 0 键选、序号标 0）、候选学习开关、词频调节模式、触发次数和线性调整步长菜单在配置共享偏好目录时持久化保存，后台只修改对应字段并保留其他调频参数。成功后由共享运行时接收偏好更新，组合中的应用时机和候选排序仍归 Engine，不主动完成组合。单选取消通知及保存期间的重复点击被忽略，页大小动作仅接受 1–10，触发次数和线性步长仅接受 1–10；失败保留原设置。未配置存储目录的预览保留会话级覆盖，切换这些选项会先结束当前组合再重建 Engine 会话。私密输入和受限字段禁用候选学习开关。

智能标点、重复标点转中文、成对标点及标点锁定菜单在配置共享偏好目录时持久化保存。重复标点转中文只取决于智能标点和它自己的开关，与成对标点补全无关，与 Windows `_CanInterceptSmartPunctuationRevert` 一致；智能标点关闭时中文标点模式下每个标点键都输出中文标点，与 Windows 在智能标点关闭时直接取标点表一致。后台按 revision 比较只写入选定字段，成功后通过共享偏好更新路径应用，保存失败保留原设置。保存期间禁止重复提交，标点锁定只响应选中通知。未配置存储目录的预览保留原会话级行为。

拼音错位纠错和邻键纠错菜单在配置共享偏好目录时持久化到对应 quanpin 字段，保留全拼其他参数与兼容纠错总开关。后台保存成功后经共享运行时更新，组合与纠错算法继续由 Engine 处理，不因菜单保存主动完成组合。保存期间禁用重复操作，失败保留原状态；无偏好目录的预览保留会话级切换。

英文、符号、Emoji 和颜文字混合候选菜单开关在配置共享偏好目录时持久化到各自 mixed_input 字段，保留其他混合输入选项。后台保存成功后由共享运行时应用，候选生成、排序和组合更新时机仍归 Engine；不因保存主动完成组合。保存期间禁止重复操作，失败保留原设置。无偏好目录的预览保留原会话级行为。

辅助码开关和编码方案菜单在配置共享偏好目录时持久化保存。操作创建时固定当前全拼或双拼目标，只修改该方案的 enabled 或 schema 字段；异步保存期间切换输入方案不会把设置写到另一方案。保存成功后经共享运行时应用，冲突或失败保留原设置，单选取消通知不切换方案。无存储目录的预览保留会话级行为。设置页为这个方案选了辅助码表插件（`plugins.helpcode_pack_quanpin` / `helpcode_pack_shuangpin`）时，Engine 按插件的码表出辅助码，schema 只在插件载入失败时回退使用：IBus 的「辅助码方案」菜单这时勾选一项不可点的「插件：<id>」，内置方案都不勾选；Fcitx5 状态区显示「辅助码：插件 <id>」，展开的子菜单同样列出全部内置方案、一项都不勾选。Fcitx5 的「辅助码方案」与 IBus 一样是单选子菜单，不再每点一下轮换一个方案：轮换时菜单随点随关，用户看不到可选的方案，也没法直接选中想要的那个。在两个菜单里选内置方案与设置页同一约定，写 schema 的同一次保存里清掉这个方案的辅助码表插件，所以能从托盘切回内置方案；规则在 `src/core/HelpcodePack.h`，由 `tests/core/helpcode_pack.cpp` 钉住。在 Fcitx5 子菜单里选中偏好库里原有的那个方案时，存的 schema 与原来相同，所以只有 schema 一致且插件已清掉才算落盘：保存失败时选择照样保留到重试成功，不会因为 schema 本来就一样而被当成已保存、随即让位给仍在偏好库里的插件。状态区这条路径（新会话按偏好库取插件、设置页之后再选插件时让位、保存失败后保留并在重试时清掉插件）由 `fcitx5-native-test` 钉住。

双拼键位方案菜单在配置共享偏好目录时持久化小鹤、自然码、首道或微软选择，只修改 shuangpin_profile，不改变当前输入方案或辅助码设置。保存成功后经共享运行时更新，组合中的应用时机由 Engine 管理，不主动完成组合；保存失败保留原选择。忽略单选取消通知并阻止保存期间的重复提交，无存储目录的预览保留原会话行为。

五笔码表版本（共享偏好 `wubi_profile`，`wubi86` 或 `wubi98`，缺省 86）只在设置页切换；IBus 输入方案菜单与 Fcitx5 状态区的五笔一项按它显示「五笔 86」或「五笔 98」，与 macOS 菜单相同，Engine 按它读写 `wubi86` 或 `wubi98` 码表与个人词条。宿主不改写这个字段，设置页同步到 runtime options 的偏好原样交给 Engine。

输入方案菜单在配置共享偏好目录时保存全拼、双拼、五笔、粤拼、注音、笔画、日语、韩语、越南语或藏文选择。选择中文方案（全拼、双拼、五笔、粤拼、注音、笔画）时在同一次 revision 比较保存中同步 last_chinese_scheme，切换日语、韩语、越南语或藏文保留该记录，“中文”入口恢复最后使用的中文方案。成功后经共享运行时更新，组合期间的切换时机由运行时与 Engine 管理；失败不改当前方案。忽略单选取消通知并阻止保存期间重复提交，无存储目录的预览保留会话切换。

韩语方案（偏好 `scheme = "korean"`，Engine 方案 4）是两套式（Dubeolsik）谚文自动机，除了下面按键打开的汉字列表没有候选，也没有云候选、学习和中文标点。两个前端都把正在组合的音节（View 的 `reading`）画成带下划线的内嵌预编辑，光标在音节末尾，预编辑样式选「不显示」时也照样显示。字母键按 Shift 决定大小写发给 Engine（Shift+Q/W/E/R/T/O/P 是 ㅃ ㅉ ㄸ ㄲ ㅆ ㅒ ㅖ），CapsLock 不改变字母，Shift+字母也不进入本地模式。开始新音节时上一个音节随同一次转换上屏；空格、回车、方向键、Home/End、Delete、数字和 Tab 等键先上屏当前音节，再把按键交还应用；标点永远是半角 ASCII，组合中与音节一次上屏，空闲时交给应用自己输入，智能标点、配对标点、连按转中文和全角都不作用于韩语；Backspace 逐个删除字母，Esc 丢弃音节，Ctrl 等快捷键先上屏音节再交给应用。失焦时音节上屏到离开的那个客户端：IBus 以 `IBUS_ENGINE_PREEDIT_COMMIT` 模式显示预编辑，由 IBus 在失焦时把它交给客户端，宿主只让会话结束组合、不再重复上屏；Fcitx5 由框架（或声明 ClientUnfocusCommit 的客户端）上屏客户端预编辑，只有不支持预编辑、音节画在面板里的客户端由宿主自己上屏。Fcitx5 切换到别的输入法时同样先上屏音节。

组合音节时按 `Hangul_Hanja`（韩国键盘的汉字键）或单独的 F9（与 ibus-hangul、fcitx5-hangul 的默认键一致；Ctrl+F9 仍是语音开关），把正在组合的这一个音节转换成汉字（`MSIME_CONVERT_HANJA`）：两个前端的候选窗都列出该音节的汉字，有训音（훈음，如「나라 이름 한」）的把训音作为该行的释义显示在汉字后面：IBus 与 Fcitx5 classicui 的候选行都只有一行文字，画不出第二行，于是 IBus 把训音放在候选翻译的位置（`韓 · 나라 이름 한, 한나라 한`），皮肤设了翻译颜色时同样着色（分段颜色是否生效取决于桌面面板），Fcitx5 把它作为单独一段、经典界面以斜体显示；训音不受「候选翻译」「英文释义」开关影响，打开这些开关且该汉字查到翻译时，翻译跟在训音后面。训音只用于显示，选中候选只上屏汉字本身。键位字母不显示，候选操作菜单不提供置顶、固定和删除。列表打开时，空格和回车提交高亮的汉字；数字行按页选择，超出本页的数字被吞掉，关闭「数字行选词」时数字先上屏谚文再交给应用，不会插在仍挂着的音节和列表前面；Tab、PageUp/PageDown 与上下方向键按导航设置翻页和移动高亮，关掉的导航键与列表外一样先上屏谚文再交给应用；Home/End 跳到首项或末项；Esc 和 Backspace 只关闭列表，音节继续组合；再按一次触发键关闭列表；输入字母关闭列表并继续组合；标点（包括平时翻页和以词定字的 `-` `=` `[` `]` `,` `.`）关闭列表，谚文与标点一起上屏；左右方向键、Delete、快捷键、切换输入法与失焦都上屏谚文。只有显式选择才上屏汉字。音节没有汉字（例如单独的辅音 ㄱ）时触发键由输入法吞掉，不会先上屏音节再把 F9 交给应用；没有组合时触发键照常交给应用。只能转换正在组合的那一个音节：已经上屏的音节和 ibus-hangul、fcitx5-hangul 那种读取光标前文本的词级转换不在本次范围内，汉字选择也不学习。汉字表来自 libhangul，编进 Engine（许可证见上文的第三方声明）。列表关闭时韩语的按键行为与上一段完全相同。

粤拼（偏好 `scheme = "cantonese"`，Engine 方案 5）与注音（`scheme = "zhuyin"`，Engine 方案 6）是中文方案，数据来自安装在资源目录旁 `language-dictionaries/` 的 `msime-cantonese.db` 与 `msime-zhuyin.db`。宿主启动时 `msime_client_refresh_host` 把这个目录写进运行时选项的 `language_dictionaries`，两个前端只在对应词库存在时列出该方案；偏好里选了词库缺失的方案时，Host API 退回最后使用的中文方案（它也不可用时退回全拼），菜单勾选与面板符号都跟随实际生效的方案。两者都直接输出繁体，「繁体」开关对它们不可用；它们不学习进主词库，候选操作菜单不提供固定和删除。粤拼的候选窗与全拼相同，组合中 `'` 是音节分隔符而不是标点，智能标点与配对标点照常。注音只支持大千键盘：数字行与 `,` `.` `/` `;` `-` 是注音键（没有组合时声调键 3、4、6、7 与空格除外），组合中空格是一声，这些键由 Engine 的 `spelling_symbols` 列出，宿主先交给 Engine，不再当作选词数字、翻页键或中文标点。注音组合时没有候选窗，组合画成内嵌预编辑（预编辑样式选「不显示」时也照样显示），光标在末尾；按下方向键、`Hangul_Hanja` 或单独的 F9（`MSIME_OPEN_CANDIDATE_LIST`）打开候选列表，列表打开后 1–9 选词，`0` 仍是注音键，下方向键移动高亮，回车提交高亮项。注音标点走中文标点表，但智能标点、配对标点和连按转中文不作用于它；方向键交还应用、快捷键、切换输入法与失焦都先上屏当前组合。

笔画（偏好 `scheme = "stroke"`，Engine 方案 9）同样是中文方案，数据来自同一目录下的 `msime-stroke.db`，与粤拼、注音一样只在词库存在时列出，缺失时按同样的规则退回。按笔顺输入：`h` 横、`s` 竖、`p` 撇、`n` 点（捺）、`z` 折，组合中 `x` 是通配符，匹配任意一笔；没有组合时 `x` 和其它字母不开始组合，交还应用，组合中其它字母被吞掉。最长 64 笔。候选只有单字，精确匹配在前、前缀补全在后，按字频排序；它直接输出词库中的字形，「繁体」开关对它不可用，不学习进主词库，候选操作菜单不提供固定和删除。组合的 `editing_text` 仍是 ASCII 字母，预编辑画的是 Engine 在 `reading` 里给出的笔画字形 `一丨丿丶乛＊`，所以「原始」预编辑样式也显示字形。候选窗与全拼相同：数字 1–9 选词，空格提交高亮项，回车上屏键入的字母，Backspace 删最后一笔，Esc 清空；`'` 不是它的输入键。标点、智能标点与配对标点照常，失焦与切换方案不上屏组合。

越南文（`scheme = "vietnamese"`，Engine 方案 7）与日语、韩语一样是非中文输入语言，切换到它时保留 last_chinese_scheme，「中文」入口恢复最后使用的中文方案。它不需要额外数据：按偏好中的 Telex 或 VNI 把按键组成带声调的越南文单词，组合本身就是要上屏的文字，没有候选，也不学习。组合画成内嵌预编辑，光标在末尾；VNI 的声调数字在组合中由 Engine 的 `spelling_symbols` 列出并交给 Engine。标点永远是半角 ASCII，全角、智能标点与配对标点都不作用于它；空格、回车、方向键、快捷键、切换输入法与失焦都先上屏当前单词再把按键交还应用。CapsLock 打开时字母照样进入组合并保持大写，与 macOS 一致，不像中文方案那样直接交给应用。

藏文（`scheme = "tibetan"`，Engine 方案 8）同样是非中文输入语言，切换到它时保留 last_chinese_scheme，默认不启用，不需要额外数据。它在拉丁字母键盘上按 EWTS（扩展威利转写）输入：组合保存当前音节串的威利原文，内嵌预编辑显示 Engine 转换出的藏文，光标在末尾，没有候选，也不学习。威利转写区分大小写，`T` `D` `N` `Sh` `A` `I` `U` `M` `H` 等大写字母是拼写，按 Shift 给出的大小写发给 Engine；CapsLock 打开时大写字母同样交给 Engine，不交给应用；威利读不了的字母（`A` `D` `H` `I` `M` `N` `R` `S` `T` `U` `W` `X` `Y` 以外的大写字母，以及小写 `q` `x`）不进组合，Engine 先上屏已有的藏文再原样写出该字母，拉丁字母不会混进转换结果。`kSh`（ཀྵ）按 EWTS 读作叠字；以 ང 结尾的音节按 `/` 时在垂符前保留音节点（ང་།）。`'`（achung 开头的音节，空闲时也算）、`+`（叠写）、`.`（消歧）、`-` 和 `/` 由 Engine 的 `spelling_symbols` 列出，宿主先作为字符交给 Engine。组合中按空格上屏藏文加音节点「་」（U+0F0B），按 `/` 上屏藏文加垂符「།」（U+0F0D），回车只上屏藏文，这三个键都由输入法吞掉，不再把空格、斜杠或换行交给应用；没有组合时 `/` 单独输出垂符，空格和数字照常交给应用，数字不转成藏文数字。Backspace 逐个删除原文按键；第一次 Esc 把预编辑退回威利原文，第二次丢弃组合。其他标点与组合一次上屏，永远是半角 ASCII，全角、智能标点与配对标点都不作用于它；方向键、Delete、Tab、快捷键、切换输入法与失焦都按显示上屏藏文（不加音节点）再把按键交还应用。

全拼九键菜单在配置共享偏好目录时保存既有 touch_keyboard_layout：开启为 nine_key，关闭为 twenty_six_key，与共享设置使用相同字段。后台按版本比较保存，成功后通过 Host API 既有布局更新路径应用，组合期间由共享运行时管理切换时机。保存失败保留原状态，保存期间禁止重复操作；无偏好目录的预览保留原会话级九键切换。

快捷模式菜单的 Unicode、日期时间、快捷短语、Emoji、颜文字、超级简拼、临时英文、临时日文和拆字开关在配置共享偏好目录时持久化到各自 local_modes 字段。后台只保存所选开关，保留其他模式，通过共享运行时更新，组合处理仍归 Engine。保存失败保留原状态，保存期间禁止重复提交；无偏好目录的预览保留会话级行为。临时日文（R 模式）在全拼或双拼、空组合时由 Shift+R 触发，显示的 R 仅是前缀；罗马字候选上屏或 Enter 原始提交后恢复原中文方案，退格可取消空前缀，关闭该开关时 Shift+R 留给应用。拆字（C 模式）打开后在全拼、双拼或五笔、空组合时由 Shift+C 触发，之后输入字的各个部件的拼音（如 huohuohuo 得 焱），部件之间可以用 `'` 分隔：IBus 在本地模式里把 `'` 走标点路由，由运行时交给 Engine 当分隔符，Fcitx5 在组字中直接把它当字符；候选注释是这个字的读音，宿主下方标签显示「拆字」。

数字键选词与以词定字菜单在配置共享偏好目录时分别保存 number_row_selection 和 word_character.enabled。以词定字保留既有按键选择，并由共享偏好校验处理与翻页键的冲突；冲突或写入失败不改变当前开关。保存成功后通过既有宿主偏好热重载更新按键分派，九键模式下数字选词开关仍禁用。无存储目录的预览保留会话级行为。

简繁输出菜单及工具栏入口在配置共享偏好目录时保存 traditional_chinese_output，成功后更新候选和上屏转换偏好，并通过共享存储与设置页同步。后台按 revision 比较写入，失败保留原状态，保存期间拒绝重复操作；日语方案仍不提供简繁切换。无存储目录的预览保留原会话级行为。

中文标点菜单和工具栏入口在配置共享偏好目录时保存 chinese_punctuation，保留标点锁定及智能标点配置。后台按 revision 比较写入，成功后更新共享偏好，失败保留原状态；保存期间拒绝重复操作。标点锁定仍按既有规则决定有效输出，无存储目录的预览保留会话级切换。

菜单设置保存期间，“桌面工具”显示保存中状态；存储失败或版本冲突后显示“设置未保存，点击重试”。重试重新读取最新偏好，只重放上次选择的字段并再次比较 revision，不覆盖其他并发修改。提示不展示原始错误、路径或输入内容；更换运行配置后旧重试入口失效，新的菜单修改替代旧重试记录。

菜单保存状态按共享代次同步到当前焦点上下文，即使保存由另一个编辑器发起、保存失败未改变偏好文件，或偏好读取仍在进行，也会在既有刷新周期更新保存中、可重试及菜单可用状态。运行配置变更同样使旧重试提示刷新失效，不额外启动轮询线程。

候选操作菜单现在根据焦点、输入启用状态和当前会话动态标记可用性；失焦、密码输入或会话尚未建立时，固定、删除、定位和取消固定动作会整体禁用，避免向已失效的 Engine 身份发送操作。候选来源和代次校验仍由宿主与 Engine 共同执行。

全角/半角输出菜单和工具栏入口在配置共享偏好目录时保存 character_width（fullwidth 或 halfwidth），成功后应用共享偏好并把宽度同步到正在运行的会话，宿主的空闲 ASCII 转换与会话上屏的组合文本保持同一宽度。失败保留原模式，保存期间禁用重复操作；无存储目录的预览保留会话级切换。

候选皮肤目录或运行配置热更新后，宿主在发布 IBus 菜单前重新验证会话级主题覆盖；覆盖在任一槽位引用的外部皮肤已移除时自动清除覆盖并回退到共享配置，避免菜单显示或渲染引用失效资源。

切换全拼、双拼、五笔或日语方案时，宿主会清除旧方案的会话级辅助码、纠错、九键和双拼方案覆盖，再创建新 Engine 会话读取共享偏好；共享文件本身不被改写。这样从双拼切到日语或五笔不会遗留旧方案菜单状态。

桌面工具菜单增加“启用语音输入”开关，持久化 voice_input.enabled；关闭时禁用语音快捷键与录音入口，活动录音由既有取消流程结束。保存成功后共享偏好热重载同步，失败保留原状态。

候选翻译目标语言的每个 IBus 单选项现在与父菜单共享同一 provider、会话、焦点和保存中状态；provider 消失或保存进行中时，子项也同步禁用，避免用户点击看似可用但不会执行的语言选择。

标点锁定的跟随、固定中文、固定英文子菜单现在与父菜单共享焦点、输入启用、活动会话和保存中状态；失焦、密码输入、会话尚未建立或保存进行中时，子项同步禁用，避免无效切换。

候选布局、每页候选数和词频调节的子菜单项现在与父菜单共享焦点、会话、输入启用和保存中状态；失焦或保存进行中时统一禁用，避免无效选择。

Linux key-router 现在使用与 Windows/Host API 一致的 dispatch outcome：只有 `DEFINITELY_NOT_SENT` 允许宿主执行本地 fallback；`DELIVERY_AMBIGUOUS` 必须等待 lease 恢复，不能重复注入按键。IBus focus-in 在 Engine 会话建立后安装 lease，focus-out 只撤销精确匹配的当前 lease。

IBus 与 Fcitx5 也保留 Backspace 长按的物理按键所有权：若首次按下发生在活动组字中，重复键可以继续删除预编辑；最后一个组字字符消失后，同一次按住的后续重复仍由输入法吞掉，不会开始删除编辑器里已经上屏的正文。按键松开、失焦、reset 或按下其他键都会解除所有权，之后一次新的 Backspace 仍正常交给编辑器。实现复用纯策略，但输入事件仍分别走 IBus 与 Fcitx5 的原生入口。

字典导入沿用 Windows 固定提交 `6e03f577` 的全拼规则：普通字典和个人字典的 Pinyin 词条在 Host API 中调用固定 Engine 的全拼切分，并按词条汉字数解决无分隔拼音歧义，例如两字词 `西安` 的 `xian` 规范化为 `xi'an`。无效音节、非法 apostrophe 或无法匹配词长的行会被跳过并报告；Wubi、快捷短语和英文导入不经过该规范化。规范化只发生在带 Engine 选项的实际导入请求中，Linux IBus 不复制输入算法。

X11 的传统装饰图启用 `candidate_overlay_*` 后，复用原生候选窗口按当前页和真实高亮绘制图内词，保留简繁转换并独立控制云／AI 来源标注。图片解码成功才接管，长词最小字号后仍完整单行，窗口放不进工作区时保留桌面面板。关闭声明、失效、菜单和隐藏会清理文字；无配色的 system 包沿用当前明暗的 Linux 原生颜色。其他 Wayland、IBus 和第三方 UI 的静态回退范围不变。专项入口为 `fcitx5-decoration-overlay-{horizontal,vertical}-scale{1,2}`，需要已验证词库、Xvfb、D-Bus 和真实 GTK 编辑器，与原生 canvas 用例共用隔离环境。
