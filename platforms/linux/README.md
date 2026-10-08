# Linux 原生宿主（IBus 与 Fcitx5）

## 目录结构

Linux 平台代码按职责分层：`src/` 下按 `core/`、`candidates/`、`clipboard/`、`providers/`、`voice/`、`overlay/`、`system/` 和 `entrypoints/` 分层放置 C++ 实现及头文件，`tests/` 放置本地与容器测试，`scripts/` 放置运行时 Python/启动脚本，`data/` 放置 systemd、桌面入口和协议模板，`fcitx5/` 保留 Fcitx5 适配器，`cmake/` 保留安装辅助模块。平台根目录只保留构建入口和说明文档。

IBus 与 Fcitx5 是两条并列的系统入口，链接同一个 `msime-host-api` ABI，功能面一致。编译门禁、容器隔离验收、CMake 安装与 CPack 打包各有对应入口，分别写在下面的「构建与运行」「隔离验证」「生成 Linux 安装包」三节。

## Fcitx5

Linux 另有一个原生 Fcitx5 插件。它与 IBus 宿主并列，不是挂在后者下面的东西：在 Fcitx5 上，输入法**本身就是**被 `fcitx5` 进程加载的 addon，正如 IBus 的 engine 本身就是一个说 D-Bus 的独立进程，两者链接的是同一个 `msime-host-api` ABI。插件按每个 Fcitx 输入上下文保持一个 Host API 会话，不运行 IBus daemon，也不使用 Fcitx 的 IBus 兼容前端。普通的 Linux 构建只要机器上装了 Fcitx5 开发包就会配置它；传 `-DMSIME_ENABLE_FCITX5=OFF` 可以退出，没装开发包时配置阶段会把取得它的办法打出来。在进程内这一点有个后果值得预先考虑：Engine 和随包词库活在 `fcitx5` 进程里，Engine 一崩，所有应用同时失去输入法，而 IBus 宿主崩掉的只是它自己的进程，daemon 还在。把构建出的插件和 `msime.conf`/`inputmethod/msime.conf` 安装到 Fcitx5 前缀后，用 `fcitx5-configtool` 选择「MSIME」。用户配置不在通常的 XDG 位置时，用 `MSIME_FCITX5_OPTIONS` 指定绝对的 runtime-options JSON 路径。密码、数字和敏感上下文一律不处理，预编辑、Engine 拥有的候选页、翻页和候选身份则通过 Fcitx5 原生输入面板转发。

Fcitx5 的状态栏现在按共享 `floating_toolbar` 的八个组件开关提供一个「工具栏」子菜单。Windows 画的是一个悬浮窗口，IBus 把同一组开关映射到属性子菜单（脱离输入上下文的窗口不是 IBus engine 该拥有的东西），Fcitx5 这里的对应面就是状态栏；在此之前这些开关在这个宿主上一个都不起作用，而设置页按能力位把它们全都显示着。子菜单里放的是已有的那些 action 本身而不是副本——一个行为和旁边状态栏项目略有不同的入口，等于同一个开关有了两份实现。中英文模式项恒定存在（与 Windows、IBus 一致），其余各自跟随自己的开关，默认值也与那两个宿主相同，所以屏幕键盘是唯一默认隐藏的一项。偏好热重载时立即重建，不等下一次焦点变化。

Fcitx5 现在也消费共享 `keybindings` 的五个模式快捷键与 `default_ime_mode`。此前这个宿主只在状态栏上提供中英文开关：设置页按 `mode_switch_shortcuts` 能力位把四个开关全都显示出来，而它们在这里一个都不生效，选了「英文」启动的用户照样得到中文。裸 Shift 与裸 Ctrl 按 Windows 的手势判定——按下只是布防，松开才切换，且期间不能打其他键、不能带别的修饰键、按住不超过 500ms；Ctrl+Space（默认开启）与（开启时）Ctrl+Alt+Space 切换中英文，Ctrl+Shift+F 切换简繁；这些组合键和 Ctrl+Shift+Space 一样每按一次只切换一次，按住的自动重复与松开都被吞掉，Ctrl+Shift+F 切换时保留正在输入的拼音，只改写候选的简繁。中英文切换位于英文透传门禁之前，因此用快捷键切到英文后仍能用同一个快捷键切回中文；屏幕键盘与语音的宿主快捷键也不随拼音输入一起被停用。`default_ime_mode` 按输入上下文只套用一次，重新聚焦或重建 Engine 会话保留用户已经选择的状态，与 IBus 一致。契约由 `tests/core/fcitx5_contract.py` 静态钉住。

`msime-linux-settings` 的「快捷键 → 输入模式切换」下拉框提供 Shift、单击 Ctrl、Ctrl+Space、Ctrl+Alt+Space 和「不使用」。Ctrl+Space 对应 `keybindings.switch_language_ctrl_space`，旧配置缺少字段时仍默认开启；多项同时启用时按选项顺序显示第一项，打开页面不改写配置。用户选择时一次更新四个布尔值，仅保留选中的切换键；「不使用」全部关闭。关闭后两个宿主都不拦截新的 Ctrl+Space 按下、重复和松开，也不改变正在输入的拼音；保存后热重载。下拉框不改 IBus/Fcitx5 的全局快捷键，框架若占用同名组合键，需在框架设置中另行关闭。专项验收复用现有原生测试入口：`ibus-engine-smoke <已验证词库目录> --ctrl-space` 与 `fcitx5-native-test <已验证词库目录> --ctrl-space`。

Fcitx5 状态栏切换输入方案、双拼方案和辅助码方案时，除了写共享偏好库，还给当前输入上下文留一个 override：会话按 `runtime-options.json` 建立，而这份文件只有设置页会同步。override 只在与偏好库一致时保留；设置页或别的窗口改过偏好库后，偏好热重载和新建会话都会丢掉它、改用偏好库的值，所以与 Windows 一样，设置页改方案所有窗口都跟着变。新建会话时这几项一律取偏好库的值，不取 `runtime-options.json` 里可能已过时的值；状态栏切输入方案时与 IBus 一样清掉辅助码方案 override，全拼下选的辅助码不会带进双拼。保存失败的选择不回滚：与 IBus 的 `failed_menu_save` 一样，待重试的保存跨焦点保留，只在保存成功或偏好目录变更时清掉，「未落盘」标记记在各个 override 自己身上，偏好库真正存下这个值之前菜单不会跳回去。

Fcitx5 的 `ime_mode_scope` 也真正决定中英文状态的记忆范围：`app` 按输入上下文报告的程序名保存有界的应用级状态，`global` 在当前 Fcitx5 进程内共享一个状态；失焦时先记录（会话留给托盘菜单，到有输入上下文获得焦点时才关闭，见「状态区」的说明；托盘上的中英文切换会更新这份记录），重新聚焦并重建会话时恢复，而不是重新套用 `default_ime_mode`。匿名或缺失程序名使用共享的匿名槽位，不把状态写入偏好文件。

Fcitx5 候选操作通过原生候选 Action（新版本）和输入上下文 status action（旧版本兼容入口）提供置顶、固定到第 N 位、取消固定和删除动作；动作携带候选身份，适配 Linux 面板而不依赖 IBus 兼容前端或 Windows 原生窗口。

候选页显示时，Home/End 将高亮移动到整份候选列表的首项或末项，页码随之切换（End 会先展开按需加载的候选）；没有候选页时仍交给编辑器处理。

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

## 安装后首次使用

安装包的 Debian `postinst` 会为当前已登录且可联系到的用户自动注册本机匿名水杉账号；网络暂时不可用时不影响安装，在线 provider 会在之后重试。安装本身仍不准备词库和运行配置，也不会替你选中输入法。其余首次配置由随装的 `msime-linux-setup` 补齐：

```sh
msime-linux-setup              # 用已备齐的词库准备状态目录
msime-linux-setup --download   # 允许按随装的词库锁取回缺失词库（首次约 170 MB）
msime-linux-setup --update --download   # 升级之后只取回过期的那几项词库并切换过去
```

它按 `desktop-dictionary.lock.json` 逐个核对词库的名称、大小和 SHA-256，再调用 `msime-linux-prepare` 在 `$XDG_CONFIG_HOME/msime-client`（默认 `~/.config/msime-client`）建立状态并发布 `runtime-options.json`，最后把输入法加入正在运行的 Fcitx5 或 IBus 的输入法列表（见下文）。词库目录按「随包安装的 `${CMAKE_INSTALL_DATADIR}/msime-client/resources` → `$XDG_DATA_HOME/msime-client/resources`」的顺序查找，`--resources` 显式指定优先；状态目录必须尚不存在，已存在时报错而不是覆盖，只有下文的 `--update` 例外。所有需要取回的词库先在同一文件系统的临时目录里下载并逐个校验，全部通过后才改名放进词库目录；校验不过或下载中断就中止，词库目录一个文件都不动，不会留下半份或新旧混杂的词库。锁之外的文件同样报出来，因为宿主会因此拒绝整份词库，而下载补不掉它们，需要用户自己移走。例外是 Engine 的 `helpcodes` 子目录（`crates/client-core/src/resources.rs` 的 `ResourceStore::verify`）。

`--download` 之外不发起任何网络请求。词库锁里每一项都带 HTTPS 下载地址；需要取回的某一项没有地址时，在发出第一个请求之前就中止并提示用 `--resources` 指向已备齐的目录，不会下出半份词库。词库锁无条件随装：此前它只在配置阶段传了 `MSIME_ENGINE_RESOURCES` 时才安装，也就是最需要它的那种安装里反而没有，只拿到安装包的用户没有任何办法把词库凑齐。

不想开终端也可以直接打开应用列表里的「水杉输入法」：还没有 `runtime-options.json` 时，`msime-linux-settings` 不再报错退出，而是打开首次配置页。页面运行的就是同一个随装的 `msime-linux-setup`（优先取桌面二进制旁边的那份，其次 `PATH`），状态目录固定为设置窗口读取的那个 `runtime-options.json` 所在目录，输出逐行显示在页面上；下载只在勾选「词库不完整时从固定地址下载」时才加 `--download`。配置完成后直接进入设置，窗口每次读取都会重读这份文件，所以不需要重启。目录已存在但缺少 `runtime-options.json` 时，只有其中仅有安装流程创建的匿名账号文件这一种情况仍可继续；含其他文件时页面只说明原因、不提供按钮，与脚本拒绝覆盖的规则一致；已安装系统级配置（`MSIME_SETTINGS_SYSTEM_CONFIG`）时仍以它为准，不出现首次配置页。

升级安装包之后不需要重新准备状态。Windows 安装程序在升级时回放用户词库，Linux 没有这样一个按用户执行的安装步骤，所以改由输入法宿主在启动时完成：IBus 宿主和 Fcitx5 插件在建立任何会话之前调用 `msime_client_refresh_host`，它用库里编译进去的词库锁算出当前代次，与 `runtime-options.json` 中 `dictionaries` 指向的代次目录比较；不一致时校验新词库、由 Engine 复制出新代次的工作词库并把用户词库日志回放进去，再原子替换这份文件，只改 `resources` 和 `dictionaries` 两项，provider socket、模型路径等其他键原样保留。同一次刷新还让 `language_dictionaries` 跟上资源目录旁的 `language-dictionaries` 目录（粤语与注音词库，安装包带上时装在 `share/msime-client/language-dictionaries`，见下文构建一节）。两项都不需要改时只读一次文件。旧代次目录不会被改动，还在用它的另一个宿主重启前照常工作；准备失败时文件保持原样，输入法继续用旧词库，下次启动再试。符号链接、系统级 `/etc/msime-client/runtime-options.json` 以及路径不符合 `msime-linux-prepare` 布局的文件都不会被改写。

升级后两个宿主都要换到新程序才会用上新版本。Windows 安装程序先停掉输入法进程、装完再启动新的；dpkg 则把新文件改名覆盖旧文件，正在运行的 IBus 宿主和 Fcitx5 进程继续执行已被替换的旧程序。`.deb` 升级时 postinst 只重启正在运行的用户服务（见下文「生成 Linux 安装包」），两个宿主各自换到新程序。IBus 宿主在输入焦点变化时检查 `/proc/self/exe`，发现自身程序已被新文件替换、且当前没有组字、候选或语音录音时，在焦点事件处理完后以专用状态 78 退出，launcher 立即带 `--recovered` 启动新版本，不等待，也不计入崩溃退避（见下文「IBus 宿主崩溃后自动恢复」），所以通常切换一次窗口就换到了新版本，组字中的内容不会丢，有焦点的编辑器也不必重新选择输入法；正在组字时推迟到之后的焦点变化。程序被删除而不是替换（`apt remove`）时宿主不退出，因为没有可以启动的新程序，退出只会让输入法提前消失；它继续运行到 `ibus restart` 或注销，期间若崩溃，launcher 发现程序已不在就不再重启。Fcitx5 插件与 Fcitx5 同进程，自己重启会带走其他输入法，设置页的「重启输入法服务」对 Fcitx5 只在进程内重置水杉插件的会话和配置（见下文 Fcitx5 的 `Ctrl+Shift+Alt+R` 一段），不加载新程序，所以插件在激活时检查 `/proc/self/maps` 里自身代码所在的共享库：已被替换时在面板上提示「水杉输入法已升级：执行 fcitx5 -r 或注销后重新登录即可使用新版本」，已被删除时提示「水杉输入法已卸载：执行 fcitx5 -r 或注销后重新登录即可完成卸载」（Fcitx5 作为 systemd 用户服务运行时，例如 Omarchy 的 `omarchy-fcitx5.service`，提示里的 `fcitx5 -r` 换成 `systemctl --user restart <该服务>`，见下文「Omarchy」），状态每变化一次只提示一次（卸载后没有重启 Fcitx5 就重新安装时，改为提示已升级），优先于配置相关的提示，下一次组字时被输入面板的内容替换。从源码树重新编译后仍在运行的旧宿主也会这样处理，换到的就是新编译的程序。设置应用也一样：关闭设置窗口只是把它隐藏、进程再留十分钟，升级后再打开设置时，单实例会把启动参数转给这个仍在执行旧程序的进程。它因此在收到转来的启动时检查 `/proc/self/exe`：程序已被替换（而不是删除）、且没有正在下载或删除的模型和资源包时，先隐藏所有窗口让页面把待保存的编辑写完，一秒后放开单实例的 D-Bus 名、以转来的参数启动新程序并退出，用户这次打开看到的就是新版本；新程序起不来时留在旧程序上照常打开界面。检测由 `linux_program_handover` 的单元测试（含真实内核的改名覆盖）钉住。检测由 `tests/core/replaced_program.cpp`（真实内核：改名覆盖和删除正在运行的程序与已映射的文件）钉住，launcher 的处理由 `tests/core/ibus_launcher_supervisor.py` 钉住。

随包提供词库的安装由包管理器在升级时换掉词库，宿主下次启动就切到新代次。用 `msime-linux-setup --download` 自己下载的词库没有人替换：升级抬高了词库版本，而记录的词库目录仍是旧的那份时，`msime_client_refresh_host` 的校验失败以 `dictionary_outdated:` 开头报告，与其他准备失败区分开。这时宿主照旧继续用旧代次，IBus 宿主在 stderr 写明原因，Fcitx5 写诊断 `reason=dictionary_outdated`，两者都以不阻塞的方式调用随装的 `msime-linux-first-run-guide --reason dictionary-outdated`。有图形会话时它发一条桌面通知「水杉输入法词库需要更新」，指向 `msime-linux-setup --update --download`，不打开窗口；限流与首次配置引导相同，但标记是独立的 `dictionary-outdated.stamp`，两种引导互不抑制。`msime-linux-setup --update`（状态目录已有 `runtime-options.json` 时给 `--download` 即视为 `--update`）对应 Windows 安装程序升级时的用户词库回放：它读出这份配置记录的 `resources`，按随装的词库锁核对；不符时加 `--download` 在它旁边另建一份按锁内容命名的 `resources-<摘要>` 目录，与锁一致的文件直接硬链接过去，只取回缺失或不符的几项。`msime-linux-setup` 不往正在使用的那份目录写任何文件：旧代次的会话每次查询都按路径读其中的 `msime-others.db` 等文件，原地替换会让它读到新旧混杂的一份，切换失败时也无法退回；旧目录保留，确认后由用户自行删除，锁之外的多余文件也随之留在旧目录里。随后它像设置窗口做词库维护那样写入 `.msime-dictionary-quiesce` 租约（在切换期间续期），并独占会话锁证明两个宿主都已关掉输入会话，再运行 `msime-linux-prepare --refresh`，走与宿主启动时同一条刷新路径切到新代次并回放用户词库。新目录先写进同目录下的一份配置副本交给刷新，刷新成功后这份副本才原子替换 `runtime-options.json`。切换期间不开会话是为了回放完整：Engine 只在准备新代次时回放一次用户词库日志，之后一个仍开着的会话学到的词只会写进旧代次和日志，新代次再也拿不到。会话在 2.5 秒内未释放时不切换、配置不变，已下好的新目录留给下次重试。记录的词库目录是本次安装随包提供的那份（前缀下的 `share/msime-client/resources` 与 `$XDG_DATA_HOME/msime-client/resources` 重合时除外，例如装在 `~/.local` 下，那里的词库是首次配置时用户自己下载的），或不归当前用户所有、当前用户不可写时拒绝更新，说明它由包管理器负责；`--update` 不接受 `--resources`，没有 `runtime-options.json` 时报错并提示先做首次配置。`msime-linux-prepare --refresh` 输出 `refreshed` 或 `current`，词库仍与本版本不符时以 3 退出，其他失败以 1 退出，文件都保持原样。租约撤下后宿主重新打开的会话即使用新代次：Fcitx5 在建立会话时读取这份文件；IBus 宿主监视它并在 100 ms 内重读，监视不可用时每 5 秒重读一次，所以 IBus 在运行时租约会在切换后再保留 6 秒。

**状态准备好后，setup 把输入法加进正在运行的宿主的输入法列表**，对应 Windows 安装程序注册 TSF profile 后输入法直接出现在列表里。两个宿主都在跑时以 Fcitx5 为准。Fcitx5 经 D-Bus（`org.fcitx.Fcitx5` 的 `/controller`）先看 `AvailableInputMethods` 是否已列出 `msime`，没有才调用 `Restart`（Fcitx5 只在启动时加载插件，写入组里的未加载输入法会被它丢掉），然后读出当前输入法组，把 `msime`（显示为「水杉输入法」，英文界面为「MSIME」）追加到组末尾后写回，再读回核对。IBus 下先看 `ibus list-engine` 是否已列出 `msime-linux`，没有才执行 `ibus restart`（ibus-daemon 只在启动时读组件文件），然后把 `('ibus', 'msime-linux')` 追加到 GNOME 的 `org.gnome.desktop.input-sources sources`（`XDG_CURRENT_DESKTOP` 含 GNOME 且装有该 schema 时；GNOME Shell 不读 IBus 自己的列表），其他桌面追加到 `org.freedesktop.ibus.general preload-engines`，输入源里显示为「Metasequoia 水杉输入法」。已在列表里时只读不写，重复运行不会重复添加；IBus 列表为空说明桌面在用一份没写进该项的默认输入源，这时不写，免得把它替换掉；Fcitx5 当前组为空时同样不写，因为组里第一项是非激活状态下使用的输入法，应当是键盘布局，只放本输入法会让它无处可切回。任何一步失败（宿主没在跑、D-Bus 调用失败、宿主重启后仍未加载本输入法、列表或当前组为空）都只在 stderr 说明原因并打印手动步骤：Fcitx5 用 `fcitx5-configtool` 把「水杉输入法」加入当前输入法组，IBus 执行 `ibus restart` 后在输入源里添加「Metasequoia 水杉输入法」；已经准备好的状态不受影响，退出码仍为 0。`--no-register` 跳过这一步，只打印手动步骤。卸载时由 `msime-linux-setup --unregister` 把本输入法从这些列表里移除（见下文「卸载 CMake 安装」）。`msime-linux-setup --register` 是反过来的那一步：默认位置的状态目录已有 `runtime-options.json` 时，重新启用首次配置启用的在线、语音和剪贴板服务、链回 Omarchy 的钩子和插件，并按上面的规则把输入法加回正在运行的宿主的列表，不检查词库、不改状态；没配置过时什么也不做。发行版仓库的包替换另一个同样内容的包之后自动替每个用户运行它（见下文「包管理器」）。

**先选了输入法、还没做首次配置时，宿主会把用户引到首次配置页，而不是静默失效。** Windows 的安装程序在安装时就把数据目录准备好，输入法一能选中就能用；Linux 安装包不产生用户状态，此前在 IBus 里选中 MSIME 时启动器只往 stderr 写一行就退出，用户看到的是一个没反应的输入法，Fcitx5 则只显示笼统的「请检查运行配置」。现在只有「没有显式覆盖、用户的 `runtime-options.json` 与系统级配置都不存在」这一种状态被判为「尚未完成首次配置」；显式指定的 `MSIME_IBUS_OPTIONS`/`MSIME_FCITX5_OPTIONS` 无效、文件不可读或是悬空符号链接，仍按配置损坏处理，不做引导。判为尚未配置时，IBus 启动器照旧写 stderr 并以非零状态退出，Fcitx5 在面板上显示「水杉输入法尚未完成首次配置：请打开「水杉输入法」设置，或在终端运行 msime-linux-setup」，按键继续交给应用；两者随后都调用随装的 `msime-linux-first-run-guide`（Fcitx5 只在激活输入法时调用，按键只显示提示，免得打字途中弹出的窗口抢走键盘焦点）。这个脚本在有图形会话（设置了 `DISPLAY` 或 `WAYLAND_DISPLAY`）时以脱离调用方的方式打开 `msime-linux-settings`（窗口自己会进入首次配置页），并在装有 `notify-send` 时发一条桌面通知；没装设置窗口的最小安装只发通知，改为指向 `msime-linux-setup`。通知里的后续步骤按宿主区分（调用方传 `--host ibus|fcitx5`）：Fcitx5 下次按键或聚焦就会重读配置，写的是「完成后即可直接输入」；IBus 组件已经退出，写的是先切换到其他输入法再切回，仍不行就运行 `ibus restart`。ibus-daemon 每次选中都会重新拉起启动器、Fcitx5 每次聚焦都会激活输入法，所以引导有限流：标记放在 `$XDG_RUNTIME_DIR/msime-client/first-run-guide.stamp`，每个登录会话两个宿主合计只弹一次，标记不按时间过期，重新登录后才会再弹；会话没有 `XDG_RUNTIME_DIR` 时退到跨会话保留的 `$XDG_CACHE_HOME/msime-client/`，只能按 5 分钟冷却期限流。没有图形会话时既不弹窗也不写标记。脚本不创建状态目录（`msime-linux-setup` 拒绝准备已存在的目录），也不发起任何网络请求，下载仍只在用户勾选或传 `--download` 时发生。契约由 `tests/core/first_run_guide.py`（桩掉设置窗口与 `notify-send`，验证只调用一次、同一会话内不再调用、并发调用只引导一次、缓存目录下的冷却期、按宿主区分的通知文案、无图形会话时不调用，以及 `--reason dictionary-outdated` 只发通知不开窗口、与首次配置各用一个标记互不抑制）、`tests/core/first_run_guidance.cpp`（Fcitx5 的配置定位与「尚未配置」判定）和 `fcitx5/tests/native.cpp` 里的首次配置用例（真实插件：面板提示、按键不被拦截、按键不拉起引导、激活只拉起一次、不写失败诊断；词库过期时写 `reason=dictionary_outdated`、以 `--reason dictionary-outdated` 拉起引导且不改写配置）钉住。

**Fcitx5 的状态区入口需要桌面提供托盘宿主。** 中英文、简繁、工具栏这些动作挂在输入法状态区，由托盘（StatusNotifierItem）承载；部分发行版以 `fcitx5 --disable notificationitem` 启动，那些入口就不会出现。输入本身不受影响，设置也仍可从应用列表里的「水杉输入法」或 `msime-linux-settings` 打开；要让它们显示，需要在桌面侧恢复托盘。这属于发行版与桌面的配置，输入法不代劳，也不因此改为创建脱离输入上下文的悬浮窗口。

判定规则由 `tests/core/setup_resolution.py` 钉住，不需要词库也不联网：哪份词库算可用、锁里没有 URL 的那项归哪个归档、去哪里找锁和已有词库。加入输入法列表由 `tests/core/setup_registration.py` 钉住：用桩代替 `gdbus`、`gsettings`、`ibus` 与 `pgrep`，验证每种宿主写的是哪一项、`ibus restart` 只在引擎未被列出时执行、重复运行不重复写入、D-Bus 失败与 Fcitx5 丢弃写入时退回手动步骤、`--no-register` 不发起任何调用。升级更新由 `tests/core/setup_update.py` 钉住：词库从本机的 HTTP 桩取，验证只请求过期的那一项且下载进新目录、旧目录原样不动、一致的文件是硬链接，`msime-linux-prepare --refresh` 运行时租约有效且会话锁被独占、拿到的是新目录，配置随后指向新代次且不留副本与租约；会话不释放、刷新失败、下载中途失败时配置都不变，旧目录不动；装在与数据目录重合的前缀下时照常更新，随包词库目录以及不带 `--update` 的重复首次配置都被拒绝；门禁里它还直接运行构建出的 `msime-linux-prepare --refresh`，确认相对路径以 2 退出、与编译进去的锁不符的词库以 3 退出且不改写配置。

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
- **状态栏插件代替托盘。** `--disable notificationitem` 让 Fcitx5 不注册托盘图标，水杉的中/英标签和托盘菜单在 Omarchy 上就都看不到了。`msime-linux-setup` 在 Omarchy 上把随包安装的插件目录 `<前缀>/share/msime-client/omarchy/plugin` 链接为 `~/.config/omarchy/plugins/metasequoia.msime`，并提示运行 `omarchy plugin enable metasequoia.msime` 把它放上状态栏（setup 不改用户的状态栏布局）。插件显示当前的 中/英/日/⇪，悬停显示当前方案；左键打开菜单（设置、词库、手写、屏幕键盘、表情、语音输入、云剪贴板、重载水杉输入法，`omarchy-fcitx5.service` 在运行时还有重启 Fcitx5），中键直接打开设置。数据来自 Fcitx5 插件写的 `$XDG_RUNTIME_DIR/msime-client/input-status.json`：`{"active":bool,"label":"中","scheme":"quanpin"}`，焦点上下文由水杉处理时 `active` 为真，切到别的输入法或失去焦点后为假，插件随之隐藏；内容不变时不重写。`--unregister` 只移除指向水杉插件的链接。由 `tests/core/input_status.cpp` 和 `tests/core/setup_omarchy.py` 钉住。

## 生成 Linux 安装包

发行版由 `.github/workflows/release-linux.yml` 手动触发，标签为 `linux-v<版本>`（版本默认取 `platforms/linux/version.txt`），附件按 x86_64 与 aarch64 两个架构各有一套（两者分别在 GitHub 的 x64 与 arm64 runner 上原生构建）：一个 Debian 包 `msime-linux_<版本>_<架构>.deb`（架构是 dpkg 的 `amd64`/`arm64`）、一个与它同一套文件、按 `/usr` 布局的归档 `msime-linux-<版本>-linux-<架构>.tar.gz`（`x86_64`/`aarch64`）、一个 Fedora 包 `msime-linux-<版本>-1.<架构>.rpm`（`x86_64`/`aarch64`），另有发行版源码构建用的三个 tarball `msime-<版本>.tar.xz`、`msime-<版本>-vendor.tar.xz` 与 `msime-<版本>-frontend.tar.xz`（见下文「包管理器」），以及覆盖全部附件的 `SHA256SUMS`。设置页的检查更新按 `linux-v` 标签前缀挑选发行版并打开发行页；附件只用来显示校验值，按扩展名 `.deb`（没有时取 `.tar.gz`）识别，并按宿主报告的架构（`HostCapabilities.arch`）只看本机架构的那一份；宿主没报告架构时，一个发行版里有两份就不显示校验值。同一个标签下其他版本（见下文「多版本」）各有同样的一组附件，包名换成 `msime-linux-<id>`，设置页按本版本的包名挑选自己的那一份。

安装前先核对校验值：`sha256sum -c SHA256SUMS --ignore-missing`。Debian/Ubuntu 用 `sudo apt install ./msime-linux_<版本>_<架构>.deb`，依赖由 apt 一并装好，卸载用 `sudo apt remove msime-client`。对应 Windows 安装程序在卸载和升级时停止输入法进程：`apt remove` 删除文件前，包的 prerm 在每个已登录（或启用了 linger）用户的 systemd 用户实例里逐个 `disable --now` 与 CMake 卸载相同的那组在线、语音和剪贴板单元，免得它们指着已删除的程序反复重启，再在同一实例里以临时单元运行 `msime-linux-setup --unregister`，把本输入法从该用户的输入法列表里移除（见「卸载 CMake 安装」）；升级后 postinst 让这些实例重读单元文件，并重启其中正在运行的服务，使其换到新程序，socket 单元保持监听，输入法宿主自己换到新程序（见上文「安装后首次使用」里的升级一段）。联系不上的用户实例只打印该用户需要执行的命令；未登录的用户没有运行中的服务，但启用链接仍留在各自的 `~/.config/systemd/user`，需要时自行执行 `systemctl --user disable …`。没有 systemd 的环境（例如容器）两步都跳过，也都不会让 apt 失败。包里唯一不在 `/usr` 下的文件是剪贴板服务的 XDG 自启动项 `/etc/xdg/autostart/msime-linux-clipboard.desktop`（见「独立剪贴板采集」），它是 conffile：管理员修改或删除它之后，升级不会把它改回来；`apt remove` 留下它、`apt purge` 才删除，留下的自启动项在服务已被 prerm 停用后什么也不做。归档给不经 apt 安装的 Debian 系系统用，不是跨发行版的通用包：库目录是 Debian 的多架构布局 `usr/lib/<三元组>/`（例如 `usr/lib/x86_64-linux-gnu/`），Fcitx5 插件因此在 `usr/lib/<三元组>/fcitx5/`，Arch（`/usr/lib/fcitx5`）和 Fedora（`/usr/lib64/fcitx5`）上的 Fcitx5 不会去那里加载它。用法：`sudo tar -xzf msime-linux-<版本>-linux-<架构>.tar.gz --strip-components=1 -C /`，再执行 `sudo gtk-update-icon-cache -f -t /usr/share/icons/hicolor`；归档没有依赖声明，需由发行版提供 IBus 1.5.20+ 或 Fcitx5 5.0.20+、Python 3.9+，以及二进制链接的共享库（WebKitGTK 4.1、GTK 3、libsoup 3、ICU、libcurl、SQLite、D-Bus、Wayland、X11、xkbcommon 等，完整列表以同版本 `.deb` 的 Depends 为准）；它也没有卸载入口，删除时按归档内的文件列表（`tar -tzf`）逐个移除。两种方式装完都按上面的「安装后首次使用」执行 `msime-linux-setup`。

`.deb` 与归档在 Debian 12（bookworm）容器里构建，链接的是 bookworm 的库版本：`.deb` 的 Depends 因此含 `libicu72`、`libfcitx5core7 (>= 5.0.21)` 等，只能装在提供这些包的发行版上（Debian 12 可以；Ubuntu 22.04 与 24.04 提供的分别是 `libicu70` 与 `libicu74`，都不满足），归档在 ICU 版本不同的系统上同样无法启动。

`.rpm` 在 Fedora 44 容器里单独构建（`tests/tools/Dockerfile.package-rpm`），不是由 `.deb` 转换来的：rpmbuild 按二进制实际链接的库生成 Requires，在 Debian 上链接的二进制会带上 Debian 独有的 soname 与符号版本（例如 libcurl 的 `CURL_OPENSSL_4`），Fedora 上没有包提供它们，已归档的 MSIME-Linux 0.9.1 的 rpm 就是因此装不上（#2095）。它面向 Fedora 44 及提供同样库版本的 DNF 系统，用 `sudo dnf install ./msime-linux-<版本>-1.<架构>.rpm` 安装，`sudo dnf remove msime-linux` 卸载；维护脚本就是 `.deb` 的 prerm 与 postinst，前面加一段把 RPM 的实例计数换算成对应的 dpkg 参数，所以卸载与升级的行为与上面 `.deb` 的描述相同，自启动项是 `%config(noreplace)`。包自带的 Host API 库与 sherpa-onnx 运行库既不作为依赖要求、也不对系统声明提供。发行流程在构建后把它装进一个干净的 Fedora 容器再卸掉（`tests/tools/check-rpm-install.sh`），依赖解析不了就在那一步失败。支持更多发行版仍需按目标发行版分别构建。

本地生成同一套附件：先 `pnpm install --frozen-lockfile && pnpm --filter @msime/desktop build`（桌面二进制在编译期嵌入前端产物），再 `bash platforms/linux/package-container.sh [版本]`。脚本在构建门禁镜像之上叠加 Tauri 与 Debian 打包依赖（`tests/tools/Dockerfile.package`），依次构建 Release 版 Host API 库、带 `tauri/custom-protocol` 的桌面二进制（报告的版本与包版本一致），用 `collect-notices.py` 收集这两个 Rust 产物静态链接的 crate 与前端打包进去的 npm 包的许可证文件，以 `-DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/usr -DMSIME_ENABLE_PACKAGING=ON -DMSIME_ENABLE_FCITX5=ON -DMSIME_BUNDLE_HANDWRITING_MODEL=OFF` 配置并构建原生宿主（发布页的包不带离线手写模型，由设置应用按需下载）、跑与门禁相同的 ctest，最后用 CPack 生成 DEB 与 TGZ，产物在 `target/linux-package/dist`。`MSIME_PACKAGE_FORMAT=rpm` 换成 Fedora 镜像走同样的步骤，CPack 生成 RPM，产物在 `target/linux-package-rpm/dist`，之后可用 `bash platforms/linux/tests/tools/check-rpm-install.sh <rpm>` 做安装检查。`MSIME_PACKAGE_DESKTOP=0` 可在没有前端产物时只打原生宿主，但那样的包没有设置窗口，不能作为发行版。`build-container.sh` 仍是 Debug 构建的合并前门禁，不产出安装包。

在 Linux 上手工打包时，同样显式传入 `-DMSIME_ENABLE_PACKAGING=ON -DCMAKE_INSTALL_PREFIX=/usr`，并提供 Release 版 Host API 库、桌面二进制和已固定来源的资源；`-DMSIME_PACKAGE_VERSION=<版本>` 指定包版本，不传时取 `platforms/linux/version.txt`（发布工作流读的同一个文件）；IBus 宿主在启动与崩溃上报里报告的也是这个版本。打包构建必定包含 Fcitx5 原生插件（不受开发机上是否装有 Fcitx5 开发包影响）；若只需 IBus 开发构建，可显式传入 `-DMSIME_ENABLE_FCITX5=OFF`。打包构建不得设置 `MSIME_RUNTIME_OPTIONS_FILE`，也不得启用安装开发测试程序的 `MSIME_LINUX_VOICE`。打包构建还必须传入 `-DMSIME_RUST_NOTICES=<文件>`（`python3 platforms/linux/collect-notices.py cargo <文件> msime-host-api msime-desktop:tauri/custom-protocol`，不打桌面二进制时去掉后一项），打包桌面二进制时再传 `-DMSIME_FRONTEND_NOTICES=<文件>`（`python3 platforms/linux/collect-notices.py npm <文件> apps/desktop`，需先装好 `node_modules`），缺哪一个配置就失败。构建完成后运行 `cpack --config <build-dir>/CPackConfig.cmake -G "TGZ;DEB"`；DEB 需要 `dpkg-shlibdeps`（`dpkg-dev`）与 `file`。Debian 包在手写的 IBus、Python、Fcitx5 依赖之外，由 `dpkg-shlibdeps` 从 ELF 文件生成共享库依赖。归档不是可任意搬移的便携包。

许可证与第三方声明装在 `${CMAKE_INSTALL_DATADIR}/doc/msime-client/`，普通 `cmake --install` 与安装包相同：`copyright`（本项目 GPL-3.0）、`THIRD_PARTY_NOTICES.txt`（本平台随附组件总览，源文件 `data/THIRD_PARTY_NOTICES.txt`），词库来源声明 `msime-engine-dictionary-NOTICE.md`（从 msime-engine 原样带过来，固定副本在 `resources/licenses/`）、辅助码声明 `msime-engine-helpcode-NOTICE.md`（`resources/helpcodes/ENGINE-NOTICE.md`）与 `msime-helpcode-jiajia-NOTICE.md`（`resources/helpcodes/NOTICE.md`）、离线手写识别所移植的 zinnia 的许可证 `Zinnia-LICENSE.txt`（`resources/licenses/`）、`@` 模式内置地名所取自的 modood/Administrative-divisions-of-China 的许可证 `Administrative-divisions-of-China-WTFPL.txt`（`resources/licenses/`）、韩语汉字转换内置汉字表所取自的 libhangul `data/hanja/hanja.txt` 的许可证 `libhangul-hanja-BSD-3-Clause.txt`（`resources/licenses/`）、引擎粤语与注音方案的数据所取自的 rime-cantonese 与 libchewing-data 的许可证 `rime-cantonese-CC-BY-4.0.txt` 与 `libchewing-data-LGPL-2.1.txt`、笔画方案的笔顺数据所取自的 rime-stroke 的许可证 `rime-stroke-LGPL-3.0.txt`（`resources/licenses/`；装了这些词库时，词库旁还有随数据发布的 `msime-rime_cantonese_LICENSE.txt`、`msime-libchewing_data_LICENSE.txt` 与 `msime-rime_stroke_LICENSE.txt`）、藏文方案所用 ewts crate 的许可证 `ewts-MIT.txt`（`resources/licenses/`；这个 crate 没有附许可证文件，按包元数据收集的 `rust-crates-NOTICES.txt` 里只有它声明的许可证表达式）、装了手写模型时随模型的 `HandwritingModel-LICENSE.txt`、OpenCC 词典的许可证，编进原生宿主与 Fcitx5 插件的 nlohmann/json（`nlohmann_json-MIT.txt`，固定副本在 `data/licenses/`）与 Wayland 协议代码（配置时从实际编译的 `wlr-layer-shell-unstable-v1.xml`、`xdg-shell.xml` 取出 `<copyright>` 段，只在找到 `xdg-shell.xml`、协议代码确实编入时安装），以及打包时收集的 `rust-crates-NOTICES.txt` 与 `frontend-npm-NOTICES.txt`。输入引擎是本项目自己的 Rust 代码（`crates/engine`），由同为 GPL-3.0 的 msime-engine 移植而来，由 `copyright` 覆盖，不再单独附 Engine 的许可证；录音采集走 cpal，其许可证在 Rust 汇总里。本项目许可证在这里按 Debian 的要求叫 `copyright`（macOS 是 `GPL-3.0.txt`）。打包配置时任何一份声明的来源缺失（通常是没有传入 Rust/npm 汇总，或要随包却没有手写模型）都会直接失败；普通开发配置不要求 Rust/npm 汇总，其余缺失只给出警告并安装剩下的部分。

安装包不包含用户状态，不自动启用 provider 服务或切换输入法。首次使用由随装的 `msime-linux-setup` 备齐词库并准备运行配置（见上面的「安装后首次使用」）；语音录音、剪贴板、Wayland/X11 输入工具及可选模型按对应功能章节配置。包的内容取决于配置阶段传入了什么：没有传入桌面二进制或资源的构建只打包实际配置的部分，完整包需要同时提供二者。

### Nix 与 NixOS

仓库根目录的 `flake.nix` 提供 NixOS 模块和各个包，构建方式、包的组成与开发命令见 [nix/README.md](nix/README.md)。

在 NixOS 上用模块：

```nix
# flake.nix 的 inputs
msime.url = "github:metasequoiaime/msime";

# NixOS 配置（imports 里加 inputs.msime.nixosModules.default）
programs.msime.enable = true;
```

`programs.msime.enable` 接入 Fcitx5 插件，放上 `msime-linux-setup`、`msime-linux-settings` 等命令和设置窗口的桌面入口，并注册 provider 的用户单元。包按本系统的 nixpkgs 构建，与系统上的 Fcitx5 出自同一份，不需要另加 overlay。IBus engine 也在包里，但模块只接入 Fcitx5：`i18n.inputMethod.type` 不是 `fcitx5` 时会给出警告。

| 选项 | 默认 | 作用 |
|---|---|---|
| `programs.msime.services.online.enable` | 开 | 按 socket 激活的在线候选与翻译服务 |
| `programs.msime.services.voice.enable` | 开 | 按 socket 激活的语音输入服务，本地与云端识别都经过它 |
| `programs.msime.services.clipboard.enable` | 开 | 剪贴板历史监视器，只在偏好里开启剪贴板历史时才采集 |
| `programs.msime.package` | `msime-fcitx5` | 换成 `override` 过的包，见下 |

三个服务的默认与 `msime-linux-setup` 首次配置时为用户启用的一致。包默认带着设置窗口、离线手写模型和本地语音识别的运行库，与各发行版的包相同；不要哪一样就在 `package` 里去掉它（经 overlay 取包，仍按本系统的 nixpkgs 构建）：

```nix
programs.msime.package = (pkgs.extend inputs.msime.overlays.default).msime-fcitx5.override {
  settingsWindow = null;     # 不带设置窗口；插件菜单里打开设置和各个面板的项随之失效
  handwritingModel = null;   # 不带离线手写模型
  voiceRuntime = null;       # 不带本地语音识别，云端识别不受影响
};
```

设置窗口的前端用 nixpkgs 的 `pnpm_11` 和 `nodejs_24` 构建。系统的 nixpkgs 较旧、还没有它们时，包不带设置窗口，求值时给出一条警告，其余部分照常可用。

录音、提示音和静音用的音频工具不随包，用系统的音频栈（例如 `services.pipewire`）。

**首次使用。** 切换配置并重新登录后，运行 `msime-linux-setup --download`（见「安装后首次使用」），也可以打开设置窗口在首次配置页里完成；它会把水杉输入法加进当前的 Fcitx5 输入法组。包不带词库，与 `.deb` 一致：词库下载到 `$XDG_DATA_HOME/msime-client/resources`，配置里记录的也是这个用户目录。本地语音识别的模型在设置窗口的语音页下载。

**每次切换配置之后**，要让 Fcitx5 从新的会话环境启动：注销后重新登录，或在新开的终端里执行 `fcitx5 -rd`。从 Fcitx5 内部重启（托盘菜单的「重新启动」、`fcitx5-configtool`）沿用旧进程的环境，加载的仍是上一次构建的插件；新旧版本的词库不一致时，表现是能切到水杉输入法但打字没有候选。可以用 `grep msime-fcitx5 /proc/$(pgrep -x fcitx5)/maps` 核对正在运行的插件是否来自当前系统（`readlink -f /run/current-system/sw/bin/fcitx5` 所在的那份 `fcitx5-with-addons`）。

**从按路径启用的单元迁移。** 以前用 `systemctl --user enable /nix/store/…/msime-linux-online.socket` 之类启用过这些单元的话，`~/.config/systemd/user` 里会留着指向旧 store 路径的链接，它们优先于模块注册的单元，旧路径被垃圾回收后单元就加载不了。换到模块后执行一次 `systemctl --user disable msime-linux-online.socket msime-linux-voice.socket msime-linux-clipboard.service`（会提示这些单元仍在全局范围启用，即由模块拉起）和 `systemctl --user daemon-reload`，再用 `systemctl --user show -p FragmentPath <单元>` 确认它们来自 `/etc/systemd/user`。

**不用模块**、经 `overlays.default` 自己写配置时，把 `pkgs.msime-fcitx5` 加进 `i18n.inputMethod.fcitx5.addons` 和 `environment.systemPackages`，并加上 `systemd.packages = [ pkgs.msime-fcitx5 ];`，否则 provider 单元不会注册到 systemd。

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

**每次发布自动产出、不自动发布。** `release-linux.yml` 的 `distro-sources` job 用 `packaging/make-source-tarballs.sh` 生成上面三个 tarball，随发布上传；发布之后 `package-definitions` job 在同一提交上运行 `packaging/render-definitions.sh <版本> <目录>`，在各发行版的官方容器里渲染出 `rpm/`（`msime.spec`、`msime-rpmlintrc`、`msime-<版本>-1.src.rpm`）、`debian/`（`.dsc` 与 `.debian.tar.xz`）、`arch/`（两个包各自的 `PKGBUILD`、`.SRCINFO`、`msime.install`）和 `gentoo/`（带 Manifest 的完整 overlay），作为构建产物 `msime-package-definitions-linux-<版本>` 上传。版本号只来自 `platforms/linux/version.txt`（或手动触发时填的版本），`.rpm` 与三个 tarball 的校验值只来自发布页的 `SHA256SUMS`；AUR 的 `msime` 与 Gentoo 的版本 ebuild 还要 GitHub 为 `linux-v<版本>` 标签生成的源码归档，它不是发布资产、不在 `SHA256SUMS` 里，校验值由 `arch/render.py` 与 `ebuild manifest` 下载后现算，GitHub 改变归档的生成方式时这两个包的校验会失败。本地也可以对任何一个已发布的版本跑同一个脚本（需要 docker），`MSIME_DEFINITIONS=arch,gentoo` 只渲染其中几部分。

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

**openSUSE OBS** 是官方的发行版仓库：项目 `home:msime`（<https://build.opensuse.org/project/show/home:msime>），同时构建 RPM 与 Debian 包，目前覆盖 Fedora 43/44（x86_64、aarch64）、openSUSE Tumbleweed、Ubuntu 24.04/26.04、Debian testing/unstable 与 Arch Linux（含 Omarchy 等衍生版）（x86_64：OBS 的 Arch 基础仓库 `Arch:Extra` 只有 x86_64，Arch Linux ARM 不在 OBS 上）。Arch 仓库用 `arch/msime-bin` 的 PKGBUILD 重新打包发布页的 `.rpm`（OBS 构建不联网，`publish.sh` 把那个 `.rpm` 原名放进包里，makepkg 找到同名文件就不下载），用户把 `[home_msime_Arch]` 加进 `/etc/pacman.conf` 后 `pacman -S msime-bin`。aarch64 的 Arch（Arch Linux ARM、Asahi）没有这个仓库，从 AUR 装 `msime-bin`（它按架构取发布页的 x86_64 或 aarch64 `.rpm`）或从源码构建的 `msime`；OBS 自己解析 PKGBUILD 求依赖，不认数组里的注释，所以两个 PKGBUILD 的数组里不写注释。用户经 `curl -fsSL https://msime.app/install.sh | sh` 安装（脚本在 msime-web 仓库的 `public/install.sh`），它按发行版添加 `https://download.opensuse.org/repositories/home:/msime/<仓库>/` 和签名公钥，再用 dnf、zypper 或 apt 安装 `msime`；之后的升级随系统更新到来。Release Linux 在发布页生成后自动调用 `.github/workflows/publish-linux-obs.yml`，把这个版本的打包定义和源码包提交到 OBS（需要仓库 secret `OBS_USER`/`OBS_PASSWORD`，缺少时跳过并告警）；补发或重发某个版本用 `gh workflow run publish-linux-obs.yml -f version=V`。项目配置在 `packaging/obs/`：`repositories.txt` 列出构建哪些仓库（Ubuntu 要显式列出 `universe-update`、`update`、`universe`、`standard` 四个源，rustc 1.91 只在 `universe-update` 里），`prjconf` 固定 Ubuntu 上 cargo/rustc 的候选包与 Debian 上 libselinux 的提供者，`_constraints` 要求 8 GB 内存与 40 GB 磁盘，`publish.sh VERSION DEFS RELEASE` 据此写入项目 meta 并提交，本地运行时用 `osc` 当前登录的账号。Debian 12/13、Ubuntu 22.04 和 openSUSE Leap 不在其中：前两者的 rustc 低于 1.90，Ubuntu 22.04 的 Fcitx5 低于 5.0.20，Leap 不在 OBS 的发行版列表里（见 `repositories.txt` 的注释）。

**Launchpad PPA**：`ppa:msime/ppa`（<https://launchpad.net/~msime/+archive/ubuntu/ppa>），目前上传 Ubuntu 24.04（noble）与 26.04（resolute）。Ubuntu 用户执行 `sudo add-apt-repository ppa:msime/ppa && sudo apt install msime`。Launchpad 只收签名的源码上传，每个代号要单独的 `debian/changelog`；Release Linux 在发布页生成后自动调用 `.github/workflows/publish-linux-ppa.yml`，按 `debian/README.source` 的「上传到 PPA」为每个代号生成 `1~ppa1~ubuntu<版本>` 的源码包并签名上传（第一个代号带上两个 orig tarball，之后的复用），给 noble 的那份把 Build-Depends 的 cargo、rustc 直接换成 cargo-1.91、rustc-1.91，因为 Launchpad 只取第一个候选。签名密钥是仓库 secret `LAUNCHPAD_GPG_KEY`（`MSIME Release Signing <admin@msime.app>`，指纹 `B9C96EE4B38BF864233BFA1CF25F85719A49653E`，2028-10-04 到期，公钥在 keyserver.ubuntu.com 并登记在 Launchpad 账号 msime 上）；缺少时跳过并告警。Launchpad 不接受同一版本号重传，重新上传已被接受的版本用 `gh workflow run publish-linux-ppa.yml -f version=V -f ppa_revision=2`。构建在 Launchpad 上进行，结果看 PPA 的 +packages 页。Ubuntu 22.04 的 Fcitx5 过旧、Debian 13（rustc 1.85）不能作为目标。

**Gentoo**（建议单独的 overlay 仓库，例如 `metasequoiaime/gentoo-overlay`）：

```sh
git clone git@github.com:metasequoiaime/gentoo-overlay.git
cp -r defs/gentoo/. gentoo-overlay/
cd gentoo-overlay && git add -A && git commit -m "app-i18n/msime: add V" && git push
```

`defs/gentoo/` 已经是带 Manifest、通过 `pkgcheck scan --exit error` 的完整 overlay；版本 ebuild 只有 `~amd64 ~arm64` 关键字。

## 多版本（水杉拼音、水杉五笔、水杉日语、水杉越南语、水杉藏文）

除了现有的水杉输入法（full），版本表 `shared/contracts/editions.json` 还定义了水杉拼音（`pinyin`，全拼与双拼，带临时日文）、水杉五笔（`wubi`，只有五笔，混拼默认打开，不带临时日文），以及只有一个方案的水杉日语（`japanese`）、水杉越南语（`vietnamese`）和水杉藏文（`tibetan`）。三个语言版本只带英文词库及其 SCOWL 许可声明、`msime-others.db` 和清单（日文版另带日文词典和两份 Mozc 许可文本），不带中文主词库、n-gram 和落定重排模型；它们也没有手写和非英文离线释义（版本表 `features.handwriting`、`features.offline_glosses` 为 false），安装包里没有 `handwriting-zh_CN.model` 和 `offline-glosses/`，IBus 与 Fcitx5 菜单里没有手写识别板。几个版本是各自独立的包，可以同时安装、同时启用，互不覆盖：

- full 仍是今天的 `msime-linux`，装在 `/usr` 下，包结构、文件、IBus 引擎 `msime-linux`、Fcitx5 条目 `msime`、状态目录 `~/.config/msime-client` 和用户服务都与引入版本之前相同；
- 其他版本是 `msime-linux-<id>`，程序、词库、宿主库和文档整个装在 `/opt/msime-linux-<id>` 下；系统按名字查找的那几样装到系统目录，名字都带版本：IBus 组件 `/usr/share/ibus/component/msime-linux-<id>.xml`（引擎 `msime-linux-<id>`）、Fcitx5 插件 `libmsime-<id>-fcitx5.so` 与条目 `msime-<id>`、systemd 用户单元 `msime-linux-<id>-{online,voice}.socket` 等、桌面入口与自启动项、图标 `msime-linux-<id>`（与 full 同一张图），以及 `/usr/bin/msime-linux-<id>-setup` 和 `/usr/bin/msime-linux-<id>-settings`；
- 每个版本登记在默认方案所属的语言下：中文的版本是 IBus `zh`、Fcitx5 `zh_CN`，日文、越南文、藏文版是 `ja`、`vi`、`bo`（IBus 组件与引擎的 `<language>`、Fcitx5 输入法条目的 `LangCode`），GNOME 的输入源对话框和 Fcitx5 的配置工具把它们列在日语、越南语、藏语下；
- 每个用户的状态目录、运行时目录（`panel-input.sock`、`candidate-panel.json`、各 provider socket）、自行下载的词库和缓存都在 `msime-client-<id>` 下，宿主生成的 Fcitx5 主题、Omarchy 钩子与插件、使用统计目录和设置窗口的 Tauri identifier（`app.msime.linux.<id>`）也按版本分开；随装的词库锁是本版本的锁（`resources/editions/<id>.lock.json`，装成 `<前缀>/share/msime-client-<id>/desktop-dictionary.lock.json`），首次配置只取回本版本需要的词库。

装好之后运行该版本自己的首次配置命令，例如 `msime-linux-wubi-setup --download`；任何一个版本的 `msime-linux-setup` 也可以加 `--edition wubi`，它会转交给 `/usr/bin` 下那个版本的命令，那个版本没装时报错。卸载一个版本只停用、注销它自己的服务和输入法条目，其他版本不受影响。Fcitx5 下两个版本的插件被同一个 fcitx5 进程加载：插件以 `RTLD_LOCAL` 加载，非 full 的插件隐藏全部符号；宿主库在非 full 的版本里改名为 `libmsime_host_api_<id>.so`（full 仍是 `libmsime_host_api.so`），因为动态链接器按插件 DT_NEEDED 里的文件名复用已经加载的库，同名时两个插件会共用先加载的那个版本的宿主库，改名后各用各的那一份；每个插件注册的输入上下文属性也随插件名（full 仍是 `msimeState`），否则后加载的插件拿不到属性槽位；`crates/host-api` 的测试 `two_editions_with_their_own_state_roots_share_one_process` 另外确认，即使两份宿主库被合并成一份，两个状态目录的会话也互不影响。

构建某个版本时给 CMake 传 `-DMSIME_EDITION=<id>`（缺省 `full`），打包时前缀必须是该版本的前缀（`-DCMAKE_INSTALL_PREFIX=/opt/msime-linux-<id>`，`packaging.cmake` 检查）；换成别的前缀的开发安装把所有文件都留在前缀里，不碰系统目录。脚本和数据文件由 `scripts/edition_linux.py` 在配置阶段按版本改写，C++ 侧的名字来自它生成的 `src/core/LinuxEdition.h`；改了版本表之后运行 `python3 platforms/linux/scripts/edition_linux.py gen`。`scripts/test-linux-editions.py` 检查生成文件没有漂移、每个版本都能改写全部文件且不留下 full 的名字、各版本的 Fcitx5 动作名两两不撞、每个版本登记在正确的语言下。`package-container.sh` 缺省按版本表给每个版本各打一个包（`MSIME_PACKAGE_EDITIONS=full,wubi` 可以只打其中几个），单测只在 full 的构建上跑；发布流程逐个版本核对包名与版本，再用 `tests/tools/check-deb-coexistence.sh` 和 `tests/tools/check-rpm-install.sh` 把全部版本一起装进干净容器、逐个卸载，确认卸载一个不带走另一个的文件。

## 卸载 CMake 安装

保留执行安装的构建目录，可用 `cmake --build <build-dir> --target uninstall` 删除该构建的 `install_manifest.txt` 中记录的程序、资源和桌面入口。卸载前先切换到其他输入法并关闭 MSIME 面板。执行卸载所需权限与原安装相同。对应 Windows 卸载时停止输入法进程并删除登录任务：以普通用户身份卸载时，删除文件前会在当前用户的 systemd 用户实例里逐个 `disable --now` `msime-linux-setup` 启用过的在线、语音和剪贴板服务，免得它们指着已删除的程序反复重启；以 root 或带 `DESTDIR` 卸载时够不到各用户的实例，只打印每个用户需要执行的那条 `systemctl --user disable --now …` 命令。卸载不停止输入法宿主：正在运行的 IBus 宿主继续运行到 `ibus restart` 或注销，不会因程序被删而自行重启；Fcitx5 在下次激活 MSIME 时提示执行 `fcitx5 -r` 或注销后重新登录。

暂存安装使用相同的 `DESTDIR`，例如 `DESTDIR=/absolute/staging cmake --build <build-dir> --target uninstall`。若安装时用 `cmake --install` 的 `--prefix` 覆盖了配置前缀，使用 `cmake -DMSIME_UNINSTALL_PREFIX=/actual/prefix -P <build-dir>/uninstall.cmake`。程序文件必须位于该前缀内，唯一的例外是前缀为 `/usr` 时装到 `/etc/xdg/autostart/` 的剪贴板自启动项；前缀外的其他自定义绝对安装目录会使卸载在删除前中止，需要按原安装布局单独处理。

卸载时（`.deb` 的 prerm 对每个已登录用户、CMake 卸载对执行卸载的用户）在删除文件之前运行 `msime-linux-setup --unregister`，把水杉从 GNOME 输入源、IBus 预载引擎和当前 Fcitx5 输入法组中移除，对应 Windows 卸载程序注销 TSF profile；它两份 IBus 列表都检查，不看当前桌面，Fcitx5 没在该用户的会话总线上时不去唤起它；若移除会让列表变空则保持原样，已不在列表里时什么都不写，缺少 `gdbus`、`gsettings` 或会话总线时只说明原因，不会让卸载失败。环境里没有会话总线地址时（用户 systemd 实例未导入桌面环境）按 `$XDG_RUNTIME_DIR/bus` 补上。移除之后它还原宿主改过的桌面候选面板设置，对应 Windows 卸载程序删除水杉留下的全部状态：宿主第一次改写 Fcitx5 classicui 的 `Theme`、`DarkTheme`、`Font`、`WheelForPaging` 或 IBus 面板的 `custom-font`、`use-custom-font` 之前，先把原值和写入值记进 `$XDG_STATE_HOME/msime-client/panel-restore.json`（默认 `~/.local/state/`），之后每次写入更新写入值；若用户在两次写入之间自己改过，记下的原值换成用户的那个。卸载时仍等于水杉写入值的项恢复原值，用户之后改过的项保持不变。Fcitx5 在会话总线上时经 `org.fcitx.Fcitx.Controller1.SetConfig`（`fcitx://config/addon/classicui`）写回，由它自己生效并保存，调用失败或 Fcitx5 没在运行时直接编辑 `~/.config/fcitx5/conf/classicui.conf`；IBus 用 `gsettings set`，原本从未设置过的键用 `gsettings reset` 回到默认值。水杉只接管 Fcitx5 自带的主题，所以记录的原值从不是水杉自己的主题：宿主接管时 `Theme`、`DarkTheme` 已经是 `msime`（早先的版本写入时没有记录）就记成 `default`、`default-dark`；卸载时仍指向 `msime` 的这两项，无论有没有记录，也一并改回 `default`、`default-dark`。随后删除 `~/.local/share/fcitx5/themes/msime` 和记录文件；记录在宿主共用的 `panel-restore.json.lock` 锁下删除，且只在它仍是读出的那一份时删除，恢复期间仍在运行的宿主又记下的改动留给下一次 `--unregister`，锁文件本身保留，保证所有写入者锁的是同一个文件。某项没能恢复（例如缺少 `gsettings`）时记录保留，稍后可再运行一次 `--unregister`。这一步由 `tests/core/setup_panel_restore.py` 钉住。无法连到某个用户的会话时（以 root 或带 `DESTDIR` 的 CMake 卸载、联系不上的用户实例），打印该用户需要手动移除的项：桌面输入源里的「Metasequoia 水杉输入法」（IBus），或 `fcitx5-configtool` 当前输入法组里的「MSIME」（Fcitx5）——程序随即被删除，事后已无法再执行 `--unregister`。

卸载保留 `msime-client/runtime-options.json`、用户配置及学习数据，不递归删除目录，也不停止用户服务以外的进程。这一点与 Windows 不同：Windows 卸载程序删除它自己管理的数据目录，而 CMake 卸载无法得知哪些用户用这份安装准备过状态，所以不碰任何人的数据；需要清除时手动删除各自的 `$XDG_CONFIG_HOME/msime-client`。通过发行版包管理器安装的文件应由原包管理器卸载。

本目录只处理 IBus 系统入口，使用同一个 `msime-host-api` 动态库；不直接创建 C++ Engine、不复制候选分页、数字选词或配置持久化逻辑，不依赖 Tauri 常驻。按键与焦点在 GLib 主线程调用线程绑定会话，系统候选点击取当前共享视图中的代次和全局索引。预编辑采用 Engine 的 ASCII editing_text，避免把字节光标用于中文显示串。失焦、禁用和 reset 清除组合；修饰键与 key-up 透传，快捷键取消组合后透传。密码、PIN、数字与电话字段不处理输入，private/no-spellcheck 文本会话关闭学习。

共享运行时只返回当前候选页；IBus lookup table 显示该页，auxiliary text 标示共享页码。宿主读取共享 navigation 设置，支持减号/等号、逗号/句号、方括号、Tab/Shift+Tab、PageUp/Down 翻页及上下候选移动，也支持小键盘导航键。启用“鼠标滚轮”后，IBus 面板上的滚轮（面板发出的 `cursor_up/down` 信号和候选点击的按钮 4/5）分别翻到上一页/下一页；关闭时滚轮不做任何事，与 Windows 一致。键盘上下键走按键路径，不经过这两个信号。Fcitx5 classic UI 自己把滚轮换成翻页请求，与翻页按钮走同一个接口，宿主无法区分，所以这个开关写进 classicui 自己的 `WheelForPaging` 配置；它属于整个桌面，与候选字体同一条规则：设置仍是默认值时不写，之后每次修改写一次，卸载时恢复接管前的值（见「卸载 CMake 安装」）。设置通过验证后立即更新按键分派，不等待 Engine 组合结束；按键路径不读文件。按当前键盘布局的字符映射，Shift 符号不当作未按 Shift 的物理键，保留 Unicode `U+`。关闭的标点绑定交回 Engine，关闭的 Tab/Page/上下键先完成组合再交还编辑器；空闲时透传。IBus“候选操作”菜单在候选页存在时提供上一页/下一页，动作仍调用共享分页命令并受已渲染 session/generation 栅栏保护；不把当前页伪装成完整候选集自行分页。

智能标点使用 IBus 提供的 surrounding text：逗号、句号和冒号前若是 ASCII 字母或数字，则保留 ASCII；否则交由 Engine 的中文标点表转换。正在组合时优先使用当前高亮候选的末字符，候选提交和标点在同一运行时转换中完成。重复输入的 ASCII 标点在短时间内可按设置替换为中文标点；失焦、删除或其他编辑动作会使该状态失效。无法取得有效 surrounding text 时按中文标点处理，不读取或记录完整编辑器内容。

`smart_punctuation_space_convert` 现在也在 IBus 生效：Engine 刚单独上屏 `。，！？；：、`、单双引号的一侧、方括号、书名号或圆括号，且当前没有组合时，紧接着的一次无修饰空格把该标点改回半角 ASCII 并消费空格；全角输出模式不改变这次手势的替换结果。自动补全的成对标点不会武装改写，避免只替换左半边后留下混合对子。判据是文档指纹而不是计时器——同一个标点在文档里通常不止一处，窗口内移动光标又不是焦点变化，所以上屏时记下标点前面的那个字符，改写前回读两个字符核对：标点必须仍是刚上屏的那个，且仍跟在同一个字符后面。上屏时取不到指纹（标点开在文档首，或宿主没有可读的 surrounding text）按匹配处理，与来源一致；其间任何其他按键都会解除该状态。完整映射和指纹判定在 `src/core/SmartPunctuationSpace.h` 由两个宿主共用并带纯 C++ 单测（`linux-smart-punctuation-space`）。

设置页的「数字后直出」「字母后直出」两个智能标点开关现在在 IBus 上也生效：IBus 自己判断标点前面的字符，此前一律按字母和数字都保留 ASCII 处理，关掉任何一个都没有效果；Fcitx5 走共享路由，本来就遵守这两个开关。

Fcitx5 现在也走同一条路径：它此前只在状态栏带着智能标点和重复标点两个开关，却没有任何改写实现，`smart_punctuation_space_convert` 在这个宿主上同样什么都不做。现改为共用 `SmartPunctuationSpace.h` 的指纹判定、直接智能按键集合与完整空格改写映射，通过 `InputContext::deleteSurroundingText` 原位替换并消费成功手势的空格，沿用同样的解除点（任何其他按键、会话关闭）。ASCII→中文的三键重复转换与中文→ASCII 的完整空格转换明确分开，避免两个不同产品规则再次漂移。

Fcitx5 的「重复标点回切中文」同样从只有开关变为真的生效：智能标点把某个标点保留为 ASCII 后，两秒内再按同一个键会通过 `deleteSurroundingText` 换成中文标点，该按键被消费、不再交给 Engine。与 Windows/IBus 一致，退格删掉那个 ASCII 标点后重新按同一个键会走中文路径——Fcitx5 的这个判断在共享 Rust 路由里做，因此实现方式是不再把前一个字符交给路由，让它按「前面不是 ASCII 字母数字」处理。上屏识别用共享的 `ascii_mark_from_text`，按 Engine 实际输出的宽度匹配，宿主不另留一份按键清单；没有组合时路由把保留的 ASCII 标点交还编辑器，没有上屏可供识别，宿主在调用路由前按同样的条件（跟随锁定、中文与智能标点开启、前一个字符是 ASCII 字母或数字、无组合）判定并记下这个键。

Fcitx5 下 `Ctrl+Shift+Alt+R`、状态菜单「重载输入法服务」和设置页「重启」按钮都会重置水杉插件：结束所有输入上下文的组合、关闭 Engine 会话并重新读取 `runtime-options.json`，当前焦点立即获得新会话；不重启 Fcitx5 进程，其他输入法不受影响。快捷键和菜单只在输入上下文仍有焦点、不是受限或隐私字段时生效，重置在插件内直接完成，不启动任何外部程序；快捷键的重复按下和对应抬键在一次手势内由插件吞掉，不会把 `R` 泄漏给编辑器。设置页按钮经 `gdbus` 调用 Fcitx5 controller 的 `ReloadAddonConfig`（参数 `msime`）触发同一重置；`fcitx5-remote -r` 发送的 `ReloadConfig` 只重载 Fcitx5 的全局配置、不会传到插件，所以不用它。重置不会换到升级后的新程序，那仍需 `fcitx5 -r` 或重新登录。

候选快照同时携带 Engine 的来源编号，并与候选顺序绑定；GTK/Qt 或其他 Linux panel 可以据此区分词库、英文、Emoji、颜文字及在线候选，不需要从显示文本反推来源。来源只用于展示和交互提示，不改变候选身份、分页或提交文本。

候选注释也由 Engine 快照按候选顺序提供：启用帮助码时使用当前方案和帮助码表生成，无法生成时保留纠错提示。Linux 宿主只显示该注释，不重复实现帮助码计算。

可选的 `online_provider_socket` 顶层启动配置指定用户管理的绝对 Unix socket。宿主复制在线查询后在 GLib worker 中请求该服务，再通过 Host API 的代次校验回填候选；未配置时不发起在线请求。请求带有 `kind:"online"`，启用 AI 联想时还携带已校验的 provider、model、候选数量和提示词配置，但不携带 token；socket 服务负责凭据、网络和 provider 策略。独立入口 `msime-linux-online /absolute/provider.sock` 从标准输入读取同一 OnlineQuery JSON 并输出受界限的 JSON 响应，供 GTK/Qt 面板或其他 Linux 宿主复用。也可用 `translation_provider_socket` 或 `MSIME_TRANSLATION_PROVIDER_SOCKET` 指定独立的候选翻译服务；未指定时翻译继续复用在线 socket。`MSIME_ONLINE_PROVIDER_SOCKET` 可作为在线 socket 的环境变量回退。

候选翻译服务选择「水杉账号」时，Linux provider 只在这个选项被保存且没有其他可用翻译服务时发送候选词到 `/v1/translate`。安装包的 Debian `postinst` 会为每个可联系到的登录用户调用 provider 的 `--ensure-anonymous-account`，`msime-linux-setup` 也会在首次配置时执行同一步；它先生成本机匿名身份，再通过 `https://api.msime.app/v1/auth/challenges` 与 `/v1/auth/login` 换取翻译令牌。网络失败只会提示并留待下一次重试。身份和轮换后的令牌分别保存在 `$XDG_CONFIG_HOME/msime-client/anonymous-account.json` 与 `anonymous-session.json`，文件由当前用户独占读写（0600），不传给设置页或输入法进程。新装与恢复默认设置时默认选中「水杉账号」；已有配置里没有 `translation_account` 字段的按未选择处理。未选择「水杉账号」时不会发送候选翻译，但账号注册仍按安装流程完成。

当 JSON 和环境变量都没有指定 provider socket 时，宿主仅在 socket 已存在的前提下尝试 `$XDG_RUNTIME_DIR/msime-client/online.sock` 与 `voice.sock`；显式 JSON 路径和环境变量始终优先，不会自动启动服务或连接不存在的路径。

运行中的 IBus 会话会在配置文件热重载时同步读取新的在线、翻译和语音 provider socket；在线请求立即失效，正在使用旧语音服务的录音会被取消。未聚焦或尚未创建会话时，新焦点直接使用最新配置。

`preferences.cloud_candidates` 会随 OnlineQuery 传给 provider；关闭后宿主不发起仅云候选请求，并拒绝返回的云来源候选，但仍保留符合条件的 AI 联想。

IBus 属性菜单中的“云联想”在配置绝对共享偏好目录时按 revision 持久化，保存成功后使正在进行的 provider 请求失效；未配置目录的直接预览只覆盖当前会话。

IBus 属性菜单中的“显示译文”（候选翻译开关）在配置绝对共享偏好目录时按 revision 持久化，保存成功后使正在进行的请求失效；未配置目录时只覆盖当前会话，关闭后不会发起翻译 provider 请求。

“翻译目标语言”菜单可选择英语、法语、日语、西班牙语、俄语、德语或韩语；配置绝对共享偏好目录时按 revision 持久化，保存成功后使旧语言请求失效并按当前候选重新请求，未配置目录时只覆盖当前会话。

语音输入通过可选的 `voice_provider_socket` 顶层绝对 Unix socket 接入，也可用 `MSIME_VOICE_PROVIDER_SOCKET` 作为环境回退。IBus 属性中的“语音输入”首次点击启动录音，再次点击结束录音并等待识别结果上屏，与 Windows 托盘语音操作一致；识别和润色期间该属性不可重复操作，Esc 仍可取消当前语音代次。IBus 的语音输入与 Windows 一样不随中英文模式停用：英文模式下语音快捷键（包括本来会交给应用的单独 RAlt）、“语音输入”属性和 Esc 取消照常工作，录音途中无论用快捷键还是属性菜单切换中英文都不会取消录音。英文模式下通常没有 Engine 会话，这时录音代次由宿主自己编号（最高位置 1，与 Engine 代次不会相撞），识别结果不经 Engine 确认代次，直接按 provider 原文上屏，与 Windows 英文模式下语音不经过组字一致；繁体输出和全角设置仍照常套用，与中文模式上屏的文字相同。用户管理的 socket 服务收到 `{"version":1,"kind":"voice","query":{"language":"zh-cn","generation":1,"stream":true,"options":{"sound_enabled":true,"start_sound":true,"end_sound":true,"mute_system_audio":false,"polish_enabled":false,"stream_inline_preedit":true,"doubao_boosting_table_id":""}}}` 后负责 PipeWire/ALSA 录音、提示音、静音、ASR 凭据、网络和结果润色，并按行返回 `{"text":"中间结果","type":"partial","generation":1}` 以及最终的 `{"text":"识别结果","type":"final","generation":1}`；每个事件都必须带 `type` 和与请求相同的 `generation`，缺少任一项或 `generation` 不匹配的事件会被丢弃。取消时输入法另发 `{"version":1,"kind":"voice_cancel","query":{"generation":1}}`，provider 应停止对应录音并忽略后续结果；按住 RAlt、Ctrl+Win 或 RCtrl+RAlt 松开时则发送 `{"version":1,"kind":"voice_stop","query":{"generation":1}}`，provider 应停止录音并让原连接返回最终结果，Ctrl+F9 和再次点击 IBus“语音输入”属性也使用该完成路径。豆包流式识别且 `preferences.voice_input.stream_inline_preedit` 开启时，中间文本更新 IBus 预编辑；关闭该选项或使用其他识别服务时只在辅助区域显示实时转写并提交最终文本。Linux 上语音结果只经 IBus（或 Fcitx5）提交这一条路径，设置页不提供「结果提交策略」，偏好里存着的 `commit_mode` 在这里不起作用；设置应用里的独立语音面板同样把文本交给当前的输入法宿主，不按它改用粘贴。`options` 只包含非敏感行为配置（包括有长度上限的润色提示词和 Doubao boosting table ID），输入法不会转发 token、app key 或其他凭据；provider 可以忽略不支持的字段。结果回到 GLib 主线程后再次校验会话和代次；最终文本为空但已有有效中间转写时，输入法保留该转写提交，只有没有任何可提交文本的空结果、过期结果和取消结果才不会上屏。若 Engine 应用结果失败，宿主也会把同一有界文本直接提交到 IBus。每条响应文本最多 4096 字节，服务调用最长等待 730 秒（包含录音及识别），每 100ms 检查取消，整行响应最多 16 KiB。`preferences.voice_input.enabled` 和 `preferences.voice_input.language` 控制属性是否可用及识别语言。独立入口 `msime-linux-voice /absolute/provider.sock` 从标准输入读取同一查询 JSON 并输出受界限的 JSON 响应；加上 `--stream` 参数时按行输出 `partial`/`final` 事件，供 GTK/Qt 面板或其他 Linux 宿主复用，不在输入法进程内保存凭据或原始音频。

语音波形展示通过 `WaveOverlaySurface` 注入宿主。Wayland 且系统提供 `wayland-client`、`wayland-scanner` 和 `xdg-shell` 协议文件时，宿主优先使用 `wlr-layer-shell` 底部居中的 overlay layer；不支持该协议的 compositor 会回退到 IBus 辅助栏。X11 且同时提供 `x11`、`xfixes`、`xrandr` 开发模块时使用不抢焦点的原生浮层，按前台窗口所在显示器（取不到时用指针所在或主显示器，经 XRandR 枚举）的工作区底部居中定位，尺寸按 `Xft.dpi`/`GDK_SCALE` 缩放，每次显示时重新计算；没有 XRandR 时回退到 EWMH 工作区或 root 屏幕尺寸并保留多屏负坐标；录音阶段浮层两侧的取消、结束按钮仅接收按钮区域输入，其余区域保持输入透明，连接失败也会回退到 IBus。Wayland 后端继续不请求键盘焦点，仅在浮层两侧动作按钮区域接收鼠标输入；没有可用指针设备时仍可通过 IBus 菜单完成动作。可用 `MSIME_WAVE_OVERLAY_BACKEND=wayland`、`x11` 或 `ibus` 请求指定后端。Wayland 后端使用固定尺寸双缓冲共享内存，不请求键盘焦点，也不占用工作区 exclusive zone；检测到 `pangocairo` 时会在同一缓冲区绘制状态和实时转写文字，否则保留波形与转写长度指示。

Fcitx5 对同一组流式语音设置采用同样语义：豆包流式识别且 `stream_inline_preedit=true` 时把 partial 写入原生预编辑，与存着的 `commit_mode` 无关；关闭开关或使用其他识别服务时只在辅助区域显示。最终响应为空但已经收到有效 partial 时，保留最后一份有界中间转写作为最终提交；完成、取消、失焦和会话关闭都会清除预编辑与缓存，避免旧代次重新出现。

Fcitx5 宿主复用同一套 X11/Wayland 原生浮层和取消、结束按钮；没有可用的显示后端、浮层无法显示（例如 GNOME Wayland 没有 `wlr-layer-shell`）或 `MSIME_WAVE_OVERLAY_BACKEND=ibus`（也接受 `auxiliary`）时，实时状态回退到 Fcitx5 输入面板上方的辅助文字。浮层显示失败后本次录音都走辅助文字，下次录音再尝试浮层，与 IBus 的回退一致。Fcitx5 的浮层同样不请求键盘焦点，完成、取消、异常和焦点关闭都会清理浮层。

语音失败时两个宿主都会告诉用户，对应 Windows 语音服务弹出的提示框：浮层（没有浮层时是辅助栏）显示固定的一句话约 1.2 秒，分别说明未识别到文字、语音服务或提供商出错、结束录音被拒（本次语音随之取消）以及没有配置语音服务；语音服务报告缺少 websockets 或录音工具时改为说明该装什么，报告本地识别组件（`msime-voice-local` 或 sherpa-onnx 运行时）缺失时提示“本地语音识别组件无法加载，请重新安装输入法”，因为重新录音无法解决。语音服务没有给出结果（包括无法连接或返回失败）算作服务出错，不会提示未识别到文字。文字是固定的，不透传 provider 的错误信息，以免其中带出凭据等私人内容；流式 C ABI 只在 `detail` 是 `websockets`、`recorder` 或 `local_asr` 时以 `voice_dependency_missing:<detail>` 错误返回，其他失败仍返回空值。Fcitx5 此前在这些情况下只是收起浮层，看上去像按键没有反应。Fcitx5 浮层也和 IBus 一样区分「识别中」与「整理中」两种收尾状态，并在按住空格锁定录音时显示锁定标记。

两个宿主向 provider 转发的语音选项由共享的 `src/voice/VoiceProviderOptions.h` 生成，不再各维护一份：Fcitx5 以前那份漏掉了提示词，选了自定义润色方案的用户在这里得到的其实是默认的整理提示词。内置方案只以 `polish_prompt_id` 发送方案名，provider 使用该方案的内置提示词；自定义方案另以 `polish_prompt_custom_1`、`polish_prompt_custom_2` 或 `polish_prompt_custom_3` 发送所选槽位的内容，槽位为空时 provider 使用默认的整理提示词。超过 8 KiB 的提示词直接拒绝，不截断，以免被截断的指令改变润色的意思。

Fcitx5 每 5 秒通过 freedesktop Settings portal 读取 `org.freedesktop.appearance/color-scheme`，因此 `voice_theme=follow` 且全局主题为 `system` 时，已显示的语音浮层会跟随系统明暗变化；portal 不可用时保留上一次主题，不阻塞输入。

Fcitx5 切换中英文时，以及焦点移到另一个输入框时（共享偏好 `input_mode_hud`，默认开启），除面板自带的文字提示外还显示约 1 秒带产品 logo 的「中」/「英」徽章；从别的输入法切到水杉时由 Fcitx5 自己弹输入法名，不再叠加这个提示。IBus 下同一偏好在切换中英文和焦点移到新输入框时，于辅助区域显示约 1.2 秒「中」/「英」，IBus 协商客户端身份时重放的同一次焦点不会再显示一遍；密码框和私密输入中都不显示。Wayland 下经 `wlr-layer-shell` 固定在屏幕右下角；X11 下（没有 layer-shell 的 Wayland 会话如 GNOME 经 Xwayland 也走这里）是不抢焦点、点击穿透的原生窗口，每次显示时按语音浮层的同一套规则选显示器（前台窗口所在，取不到时用指针所在或主显示器，经 XRandR 枚举），放在该显示器工作区的右下角以避开面板，尺寸、图标和边距按 `Xft.dpi`/`GDK_SCALE` 缩放。徽章深浅与候选面板一致：`candidate_theme` 为浅色或深色时照用，为「跟随颜色模式」时取颜色模式，颜色模式为「跟随系统」时跟随上述 portal 报告的系统明暗。

同一个 socket 也承载候选翻译请求。候选视图更新后，宿主发送一行 JSON：

```json
{"version":1,"kind":"translation","query":{"generation":9,"target_language":"en","candidates":["你好","世界"],"provider":"tencent"}}
```

服务应在一行内返回 `{"translations":[{"text":"你好","translation":"hello"}]}`；未知候选可以省略。独立入口 `msime-linux-translation /absolute/provider.sock` 从标准输入读取同一 TranslationQuery JSON 并输出受界限的 JSON 响应，供 GTK/Qt 面板或其他 Linux 宿主复用。`provider` 是设置页「翻译服务」当前的选择（`none`、`account`、`tencent`、`niutrans`、`custom`），即使所选服务配置不全也照实填写，是必填字段，缺少它的请求按格式错误拒绝；服务只能询问这一家，选「关闭」或所选服务不可用时不发出任何请求，不得退回腾讯。腾讯密钥只存在于服务自己的 `tencent-provider.json`，协议里不传。启用自定义翻译时，请求还会携带已验证的 `custom_translation` endpoint 和 API Key，provider 可据此调用兼容 DeepLX 的服务。宿主只接受最多 9 个候选、每项最多 4096 字节、每次完整响应最多 8 秒、128 KiB，并把返回的 generation 原样交给 Host API 校验；过期视图不会被更新。服务必须由用户管理绝对 Unix socket，负责所有凭据、网络访问和日志策略，输入法不会记录原始输入或 API Key。关闭 `preferences.candidate_translations` 后不会发起该请求。候选翻译会按当前候选布局附加到 IBus 候选行。

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

桌面 Tauri 面板的系统剪贴板按 Linux 会话能力选择后端：优先使用 Wayland 的 `wl-paste` / `wl-copy`，不可用时回退到 X11 的 `xclip`；剪贴板历史仍只在用户开启设置后写入本地受限存储。桌面宿主运行期间以低频轮询捕获新的文本剪贴板内容，设置关闭后立即停止记录并清除本轮监视状态，读取失败不会伪造同步结果。

桌面 Tauri 面板在 Linux 上也接入了屏幕键盘、手写和语音提交。打开面板时宿主先保存当前输入目标：X11 使用 `xdotool getactivewindow`，Sway 使用 `swaymsg -t get_tree`；按键通过目标窗口的 `xdotool key`、Sway 的 `wtype` 或通用 Wayland 的 `ydotool` 发送。`ydotool` 仅在其 daemon 可用时启用，以 `/dev/uinput` 注入，不依赖面板重新夺取焦点；没有全局注入能力时回退到 `wtype`。手写候选和语音识别结果通过同一目标提交文本，语音面板消费 provider 的 partial/final 事件并实时显示转写，关闭面板时发送当前 generation 的取消消息。手写识别服务的绝对 Unix socket 由 `MSIME_HANDWRITING_PROVIDER_SOCKET` 提供，语音服务使用 HostOptions 的 `voice_provider_socket` 或 `MSIME_VOICE_PROVIDER_SOCKET`；服务仍负责录音、模型和凭据。缺少注入工具或服务时面板保留可见状态并返回宿主错误，不伪造提交。

IBus 和 Fcitx5 宿主还提供一条比上述工具更直接的提交路径，对应 Windows 面板经 TSF 回填当前编辑器的行为：输入法进程在 `$XDG_RUNTIME_DIR/msime-client/panel-input.sock`（目录 0700、socket 0600，只接受同一 uid 的对端）监听，面板的按键、手写候选、语音结果和 Ctrl+V 优先通过它由输入法自身提交或转发，不需要 `xdotool`、`wtype` 或 `ydotool`，Wayland 下也不依赖 compositor 支持虚拟键盘协议。请求为一行 JSON：`{"op":"generation"}` 取当前焦点代次，`{"op":"text","text":"…","after_generation":N}` 提交单行文本，`{"op":"key","key":"BackSpace","keycode":14,"shift":false,"control":false,"alt":false,"super":false}` 以 X keysym 名和 evdev 键码转发按键；应答为 `{"ok":true}` 或 `{"ok":false,"error":"no_focus|restricted|invalid"}`。面板获得焦点时先取代次、释放焦点，再要求输入法只在更新的焦点上投递，所以文本不会落回面板自己的 webview；700ms 内没有新的焦点则应答 `no_focus` 并丢弃请求，不会迟到上屏。密码等受限字段应答 `restricted`，面板不再改用注入工具绕过。含换行或制表符的文本仍走剪贴板或注入工具。两个宿主同时运行时先绑定的一方提供服务，不抢占仍存活的 socket。应答缺失或无法解析时桌面宿主报告错误而不回退，避免同一文本被提交两次。

`key` 请求与 Windows 屏幕键盘的 `SendInput` 一样先交给输入法：按下、松开各送一次，只有按下没被输入法消费时，才把这一对事件转发给编辑器，编辑器因此不会只收到半次按键；松开事件总会交给输入法，退格长按和已消费的快捷键都靠它收尾。这个顺序集中在 `PanelInputChannel.h` 的 `deliver_panel_key_stroke`，IBus 调引擎自己的按键处理，Fcitx5 构造 `fcitx::KeyEvent` 交给 `InputContext::keyEvent`，走该输入上下文当前的输入法。因此中文模式下屏幕键盘打出的字母起拼音组合，数字键、空格、退格作用于已有的组合串（包括物理键盘起的组合）；要打英文先切到英文模式。面板请求不带 CapsLock 状态，宿主沿用最近一次真实按键报告的锁定位，锁定时字母按物理键的规则变大写（同时按 Shift 则小写）并直接交给编辑器，不开始组字。`text` 请求（手写、Emoji、语音）仍原样上屏，不经过组字。

桌面手写和语音面板支持 `preferences.handwriting_theme` 与 `preferences.voice_theme`，取值为 `follow`、`dark` 或 `light`；`follow` 继承全局主题。设置保存后，已打开的面板通过偏好变更事件立即更新外观。

桌面 Emoji 面板（包括颜文字、符号和剪贴板页）支持 `preferences.emoji_theme`，同样取值为 `follow`、`dark` 或 `light`；设置保存后已打开的面板实时同步主题。

屏幕键盘和手写面板在 X11 上读取活动窗口矩形，在 Sway 上读取 focused container 的 `rect`，首次创建时定位到输入窗口下方并水平居中；窗口矩形不可用时回退到屏幕默认位置。通用 Wayland 的 `wtype` 注入不提供窗口几何查询，因此保留 compositor 默认位置，不伪造坐标。

屏幕键盘使用共享的 `touch_key_spacing_tenths` 和 `touch_row_spacing_tenths` 设置实时调整键位与行间距；启用 `touch_voice_shortcut` 时，键盘标题栏提供“语音”入口并复用已保存的输入目标打开语音面板。普通字符、编辑和候选提交键在按住 450 毫秒后以 75 毫秒间隔自动重复；粘滞修饰键与 Num Lock 只执行一次，松开、取消、失焦、关闭面板或宿主投递失败都会停止重复，不自动重放可能已部分发送的按键。设置变化只影响当前面板布局，不改变 IBus Engine 组合状态。

IBus 属性面板提供 `EnglishCandidates`、`EmojiCandidates` 和 `KaomojiCandidates` 三个混输开关。配置绝对共享偏好目录时，切换先按 revision 保存，再由共享运行时决定活动组合后的应用时机；未配置目录时会结束当前组合并重建本会话的 Engine，避免把新旧混输候选规则混在同一代视图中。

IBus 属性面板的「输入选项」子菜单另提供 `EnglishMode` 独立英文输入模式（紧挨「英文候选」；Fcitx5 放在「输入选项」的「混合英文」之后），它不是设计稿菜单里的「英文」，后者就是顶层「中文」开关的未勾选状态。Ctrl+Shift+E 或属性开关调用 Engine 的 dedicated English 模式，保留中文输入法会话和 IBus 输入源边界；它与 `EnglishCandidates` 混输候选开关相互独立。状态按当前 IBus 会话保留，切换时由 Engine 清理正在进行的组合。

Linux IBus 会话支持 `Ctrl+Shift+Super+K` 打开屏幕键盘面板。宿主只在当前输入上下文获得焦点且不是密码等受限字段时消费该组合，并通过现有桌面面板启动器打开键盘；客户端把 Super 报告为 `MOD4`、`SUPER` 或两者同时置位时都能识别；Super 组合是否能到达 IBus 仍由桌面环境的全局快捷键策略决定。

IBus 属性面板还提供 `TraditionalOutput`。开启后，中文方案的候选显示和提交文本经 `msime-host-api` 导出的 `msime_client_simplified_to_traditional` 转换为繁体，与 Windows 共用同一份 OpenCC `s2t` 词级表（「头发」→「頭髮」而不是逐字的「頭發」），不再依赖系统 ICU；Unicode 直接输入、日语方案和英文/Emoji 文本保持原样。配置绝对共享偏好目录时开关按 revision 保存 `traditional_chinese_output`，未配置目录时只覆盖当前会话。

`Ctrl+Shift+F` 使用同一简繁输出路径：配置了共享偏好目录时通过 revision 保存 `traditional_chinese_output`，保存成功后更新当前会话；没有可写偏好目录时保留会话级切换。持久化写入进行中，新的一次按下不切换也不吞掉该快捷键，避免重复操作覆盖较新的 revision；已经切换过的那次按下，按住时的自动重复和松开都归这个快捷键，不会再切换一次，与 Windows 一致。

IBus 与 Fcitx5 会话都支持 `Ctrl+Shift+Alt+1` 到 `Ctrl+Shift+Alt+8` 删除候选页对应的可编辑词条。宿主只传递候选快照中的会话、代次和全局索引，由 Host API 校验来源和执行词库删除；没有对应候选或不可编辑候选时按键交回应用。`Ctrl+Shift+Alt+C` 清除当前输入法会话的 Engine 候选缓存并刷新当前视图，不会结束正在进行的组合。`Ctrl+Shift+Alt+R` 重启或重载输入法：IBus 下执行用户会话的 `ibus restart` 重启服务，Fcitx5 下在进程内重置水杉插件、不影响其他输入法（见上文 Fcitx5 的 `Ctrl+Shift+Alt+R` 一段），设置页也提供同一动作的按钮。

`Ctrl+Shift+Alt+T` 立即退出当前 Linux IBus 宿主进程，快捷键由宿主消费，不会停止用户正在运行的其他 IBus 服务；宿主以专用退出状态结束，launcher 的崩溃守护据此不会重启它。Fcitx5 插件与 Fcitx5 同进程，退出就会带走其他输入法，因此 Fcitx5 不提供这个快捷键，设置页也按此注明；需要恢复时使用 `Ctrl+Shift+Alt+R` 重置水杉插件（见上文 Fcitx5 的 `Ctrl+Shift+Alt+R` 一段）。`Ctrl+.` 在两个宿主上都切换中英文标点，与 Windows 相同；标点锁定为「跟随」时，Fcitx5 把中文模式下的切换结果保存到共享偏好，和状态菜单里的同一开关一致；锁定为中文或英文时不保存（见下文英文模式一段）。英文模式下 `Ctrl+.` 同样生效（见下文英文模式一段），只改本次会话、不写偏好，到下一次中英切换为止；Fcitx5 状态栏的「中文标点」在英文模式下显示和切换的也是这一状态。菜单主题不在 Linux 设置页出现：IBus 属性菜单由桌面面板按自己的主题绘制，Fcitx5 classicui 的状态菜单跟随全局主题的候选调色板（见下文 Fcitx5 候选表一段），kimpanel 与 GNOME Shell 面板下同样由面板自己绘制。

Windows 配置中的 `candidate_arrow_navigation` 兼容名称也会映射到共享导航的 `arrows` 开关，保证迁移配置在 Linux 上保持一致。

Linux 的 `floating_toolbar` 偏好映射为 IBus 原生属性菜单中的“工具栏”入口，不创建脱离输入上下文的伪悬浮窗口。启用后，菜单按偏好显示中英文模式、独立英文输入模式、全角字符、中文标点、繁体输出、Emoji、屏幕键盘和设置动作；模式动作复用当前 IBus 会话，面板动作通过 `msime-linux-settings` 启动已有 Tauri 面板，并把当前输入目标交给面板保存。关闭工具栏或单独关闭组件后，入口会在配置热重载时同步隐藏。

## 构建与运行

IBus 提交也接入共享的聚合打字统计。统计在文本成功提交到 IBus 后异步写入 Host API，按当前方案、本地模式、英文模式或语音来源计数；只保留字符类别、来源和日期的聚合数据，不保存输入文本。输入法没有消费、交还给应用的可打印字符也计入统计（英文模式记为 `english` 来源），判据与 Windows `ShouldCountPassthroughChar` 相同，写在 `TypingStatistics.h` 的 `should_count_passthrough_character`：只算按下、有焦点、非密码与隐私输入、不带 Ctrl/Alt/Super 的键，不算控制字符和 DEL；Fcitx5 同样如此。未配置绝对的 `preferences_directory` 时跳过统计，统计写入失败不会影响输入。统计开关（默认关闭）由宿主缓存在 `TypingStatistics.h` 的 `TypingStatisticsSwitch` 里：启动时和每次偏好热重载时刷新，统计文件没变时只做一次 stat，变了才经 `msime_client_typing_statistics_enabled` 重读；关闭时上屏和透传按键在入口处直接返回，不取日期、不序列化请求、不起 GTask 或线程，也不碰统计文件和锁。在设置里打开统计后，要到下一次热重载（IBus 约 1 秒、Fcitx5 约 250 毫秒）才开始计入，这之间的上屏不记录，与 macOS 的做法一致。

英文模式与 Windows 一样仍处理中文标点和全角：标点锁定为「始终中文标点」，或锁定为「跟随」时在英文模式下按过 `Ctrl+.`，先把 ASCII 标点换成中文标点（引号交替、书名号嵌套，按 `shared/contracts/punctuation/policy.h` 的正向表），全角开着时再把可打印 ASCII 换成全角（空格为 U+3000），其余键交给应用；小键盘不转中文标点，带 Ctrl/Alt/Super/Hyper 的组合键照旧透传，`Ctrl+.` 和已启用的语音快捷键除外（见「语音输入」段）。中文和英文模式下的 `Ctrl+.` 都由宿主消费；标点锁定为「始终中文标点」或「始终英文标点」时，它和 IBus 属性菜单、Fcitx5 状态栏里的「中文标点」都不改变标点状态、也不写偏好，与 Windows 经 `ResolvePunctuationOpen` 按锁定值解析标点状态一致。IBus 与 Fcitx5 共用 `shared/input/EnglishModeOutput.h` 的 `english_mode_output`（macOS 宿主也用它）。每次中英切换后（包括焦点切换时按应用记忆恢复出另一种模式）按标点锁定重设会话标点：「跟随」时中文模式用中文标点、英文模式用英文标点（英文模式下 `Ctrl+.` 的选择随之作废），锁定为中文或英文时保持锁定值；这只改本次会话，不写偏好文件。裸 Shift/Ctrl 松开时切换模式，但这次松开仍交给应用，跟踪修饰键状态的程序不会以为它一直按着。

候选辅助文本在页码后展示 Engine 快照提供的本地模式标签（U+、日期时间、短语、Emoji、颜文字、简拼、EN、日文）。普通或未知模式不附加标签，取消组合或没有候选时隐藏辅助文本；不从预编辑前缀推断模式。

按 Windows 基线，全拼辅助码默认启用自然码且不在候选窗显示，双拼辅助码默认启用蓝天且显示；旧共享偏好或 Linux 运行选项缺少对应对象时使用相同的分方案默认值。当全拼或双拼启用辅助码候选显示时，Linux 也为 Engine 生成的整句候选按当前方案和词库映射计算辅助码；不再把整段原始预编辑拼音当作候选注释。辅助码仍是展示文本，不进入候选身份或提交内容。

全拼纠错按字母错位和邻键误触分别启用，两个字段缺失时均默认开启；已有明确保存的 `false` 仍保持关闭。纠错由 Engine 自动执行，不在设置页、IBus 属性或 Fcitx 状态栏显示开关。

候选行保留 Engine 的来源身份：本地词库和用户词库不额外标记，云候选显示 `云`，AI 候选显示 `AI`。来源标签只用于 IBus panel 展示，不进入提交文本、候选索引或异步结果校验。

候选颜色来自共享层解析的全局主题（ABI 3 的 `msime_client_resolve_theme`）：宿主以 `global_theme`、`custom_theme`、候选明暗和当前候选布局请求 ResolvedTheme，自定义主题引用已安装的外部皮肤时，再带上运行配置里该皮肤的目录条目；共享层拒绝该条目时宿主去掉它重新解析，仍失败才退回原生 token。解析出的 surface、正文、序号、accent 与选中行颜色映射为 IBus 候选文字、编号标签前景、固定候选的 accent 前景和行背景属性，以及 Fcitx5 classicui 主题；主题未给出的槽位（`system` 主题全部如此）使用 Linux 原生 token：浅色白底、深色 `#303030` 底，选中行 `#3584E4` 实色填充、白色正文。accent 颜色只在 IBus 生效，序号颜色在 IBus 和带 `CandidateLabelColor` 的 Fcitx5 版本上生效；边框、圆角与阴影只由 Fcitx5 classicui 主题绘制，IBus 的候选属性无法描边，两个宿主都没有 hover 状态，也不读取 `show_selected_bar`；设置页按 `candidate_border_color` 能力位在 Linux 显示边框颜色，而 hover 等选中外观随 `candidate_selection_appearance` 隐藏。候选字体族、回退字体和字号由宿主写进桌面 panel 自己读取的那一个字体描述：IBus 写 `org.freedesktop.ibus.panel` 的 `custom-font` 并打开 `use-custom-font`（与 ibus-setup 写的是同一对键），Fcitx5 通过 classicui 插件自己的配置写 `Font`，立即生效并保存在 `classicui.conf`。描述按 Pango 格式把主字体和回退字体依次列出、去重。字号在 IBus 上以像素写出。Fcitx5 5.1.18 起换算成磅（像素乘 3/4，例如 14px 写成 `10.5`）：这些版本的 classicui 画候选序号时把描述里的字号乘上主题的 `LabelTextSizeFactor` 再按磅设回去，写像素会让序号比候选大三分之一。5.1.22 起 classicui 在 X11 上按 DPI 整窗缩放、字体 DPI 固定为 96，Wayland 上除非设了 `ForceWaylandDPI` 也是 96，换算前后候选大小相同；5.1.18 到 5.1.21 的 X11 以 `Xft.dpi`（没有时取不低于 96 的屏幕 DPI）作字体 DPI，DPI 不是 96 时候选文字随之放大，与 5.1.22 起整窗缩放后的大小一致。更早的版本序号与候选共用同一个描述，本来就一样大，仍写像素。这个字体归整个桌面所有，所以设置从未改过、仍是默认值时宿主不写，之后用户每次修改都会写一次，回到默认值也算一次修改；例外是升级：偏好仍是默认值，而 `classicui.conf` 里的 `Font` 正是旧版本按像素写下的同一个字体时，宿主按磅重写一次。第一次写入之前宿主记下被替换的原值，卸载时只要这一项仍是水杉写入的值就恢复原值，用户之后自己改过的保持不变（见「卸载 CMake 安装」）。GNOME Shell 自带的候选弹窗跟随 Shell 主题、Plasma 的 kimpanel 跟随桌面字体，这两种面板上宿主不写任何东西：IBus 宿主在 GNOME Shell 会话（`XDG_CURRENT_DESKTOP` 含 `GNOME` 且会话总线上存在 `org.gnome.Shell`）中不写 `use-custom-font`。当前绘制候选的面板是否忽略这些设置由宿主写进会话运行目录的 `$XDG_RUNTIME_DIR/msime-client/candidate-panel.json`（`{"host":"ibus"|"fcitx5","limit":"gnome_shell"|"fcitx_theme"|"kimpanel"|null}`，仅在内容变化时原子替换），设置页的外观与皮肤页据此提示哪些设置不会生效；文件以最后写入的宿主为准。预编辑由应用自己绘制，因此 Linux 不提供单独的预编辑字号，设置页按 `candidate_preedit_font` 能力位隐藏它；英文字体同理不提供。

中英混输默认在预编辑达到 5 个字母后显示英文候选，Emoji 与颜文字混输默认关闭；旧宿主选项缺少这些字段时使用相同默认值。用户仍可在设置中选择 1–8 个字符并分别切换 Emoji/颜文字，显式配置优先于默认值。

新建共享偏好使用 Windows `develop` 的候选外观基线：跟随系统明暗、`system` 全局主题、每页 6 项、18px 候选文字、15px 候选预编辑，以及 `Noto Sans SC` / `Microsoft YaHei` 字体回退栈。Linux IBus 只应用 panel 协议可表达的主题色和页大小；字体继续由桌面 panel 管理，但 Tauri 设置与预览保留完整共享配置。已有偏好文件和显式宿主选项不被默认值覆盖。

Linux IBus 候选表画的是共享层解析出的全局主题（水杉、浅色、纸白、夜青、墨，或自定义主题及其外部皮肤）的 surface、正文、序号、accent 与选中行颜色。IBus 的候选属性只携带 RGB 前景/背景，不能表达原生窗口的 alpha、圆角、阴影、边框、hover、选中条或布局间距，因此半透明槽位先按设计合成到候选底色上再发布，其余装饰无法呈现；外部皮肤声明的顶部装饰图同样无法经 panel 协议显示，IBus 忽略它。主题没有选中填充时，以 accent 实色作为选中行，再没有 accent 时使用原生 `#3584E4`；选中行正文取主题的选中正文色，没有时取自定义文字色，再没有时按选中底色取对比度更高的黑或白。

Fcitx5 的候选表由 classicui 插件按主题绘制，宿主把同一份 ResolvedTheme（全局主题、自定义主题及其外部皮肤、`follow` 跟随颜色模式）写成用户数据目录下的主题 `$XDG_DATA_HOME/fcitx5/themes/msime/theme.conf`（默认 `~/.local/share/fcitx5/themes/msime/`），再通过 classicui 自己的配置把 `Theme` 和 `DarkTheme` 指向它；配置一写入插件就重新读取主题，改皮肤或系统明暗切换后无需重启。只有当前主题是 Fcitx5 自带的 `default`、`default-dark`、未设置或已经是 `msime` 时宿主才接管，`Theme` 与 `DarkTheme` 分别判断；用户在 fcitx5-configtool 里选过的第三方主题保持不变，此时 MSIME 的候选颜色不生效，第三方 `DarkTheme` 则在深色模式下生效。第一次接管之前宿主记下原来的 `Theme` 和 `DarkTheme`，卸载时仍指向 `msime` 的项恢复原值、主题目录随之删除，用户之后改过的保持不变（见「卸载 CMake 安装」）。接管不随切换输入法撤销：切到别的输入法后，classicui 为它绘制的候选窗仍使用 `msime` 主题和水杉写入的字体。主题文件只在内容变化时原子替换。classicui 主题只能把背景画成纯色矩形或九宫格图片，所以圆角与阴影由宿主画成图片：宿主按设计 token 生成 PNG（候选卡片 10px 圆角、1px 细边、下方阴影；选中行 6px 圆角实色；菜单 12px 圆角与 1px 描边、6px 圆角 hover、分隔线、勾选与子菜单箭头；候选翻页按钮 ‹ › 按序号颜色绘制，没有序号颜色时跟随正文），各带一份 `@2x`，按内容哈希命名为 `shape-<hash>.png` 与主题一起原子写入，颜色变化后旧图片随即删除；每一节同时保留纯色 `Color`，classicui 读不到图片时退回纯色矩形。PNG 由宿主自己编码（未压缩的 deflate 块），不新增依赖，cairo 在所有 Fcitx5 版本上都能直接读取。阴影所占的透明边写进 `ShadowMargin`，X11 上 classicui 定位时扣除它；Wayland 上 classicui 不读这个值，候选窗相对光标多偏移阴影边的宽度（左 12、上 8 像素），因此阴影比设计的 `0 6px 18px` 收紧为下移 4px、标准差 5px。没有合成器的 X11 会话里 classicui 没有 alpha 通道，透明的圆角外和阴影显示为黑色，这对所有带圆角图片的 Fcitx5 主题都一样。菜单没有阴影边可用，只画圆角不画阴影，宽度随内容而定，不是设计的 260px。选中行上下内边距为 6px 而不是设计的 5px（九宫格的角不能小于 6px 圆角），行距与设计相同。classicui 只在主题同时给出 `PrevPage` 与 `NextPage` 两张图片时才画翻页按钮，位置由它自己决定：内容区右侧，无页可翻的一侧变淡。主题写入 `PageButtonAlignment=Top`，2023 年 4 月之后的 Fcitx5 据此把按钮放到预编辑所在的顶行，对应设计中的标题行；更早的版本（如测试所用的 5.0.21）忽略这个键，按钮画在右下角。Fcitx5 宿主的候选窗不显示页码，按钮旁没有设计中的「1 / 3」。整张图片都是点击区域。classicui 主题没有 accent 的独立颜色，也没有与选中分开的 hover 状态，因此固定候选不单独着色；序号颜色写进 `CandidateLabelColor`、`HighlightCandidateLabelColor`，不认识这两个键的旧版本忽略它们，序号跟随正文颜色。菜单（托盘和状态区菜单）按 THEME_CONTRACT §3 从同一份 ResolvedTheme 的候选调色板派生：surface 作菜单底色，text 作条目文字，hover 作悬停条目，border 作分隔线和外描边，半透明的颜色合成在菜单底色上；主题留空的槽位（`system` 全部留空）取 Linux 菜单 token（浅色白底、深色 `#383838`，文字 82% 黑或纯白，hover 为 6% 黑或 8% 白，外描边 8% 黑），按解析出的明暗选取。border 全透明的主题菜单也不画外描边，分隔线仍用原生细线。契约里勾选条目的 selected 底色与 selected_text 在 classicui 菜单里没有对应项：勾选只由 CheckBox 图片表示，图片按条目文字颜色绘制。外部皮肤声明了顶部装饰（清单的 `decoration.top_inset_dip`、`decoration.width_dip` 与通过校验的 `decoration.image`，没有时取预览图）时，共享层发布给宿主的皮肤目录带上这两个尺寸和图片的绝对路径，宿主把图片按内容哈希命名（`decoration-<hash>.<扩展名>`）原子复制进主题目录，仅在内容变化时重写，再以 `Overlay=`、按清单 `decoration.align` 取的 `Gravity=Top Right`（默认）、`Top Left` 或 `Top Center` 和 `HideOverlayIfOversize=False` 写进主题；切换到别的皮肤或无装饰的皮肤后，旧图片随即删除。几何与各平台宿主一致：候选窗比卡片高出 `top_inset_dip`（这条带），带本身透明，不画底色、边框和阴影，卡片从带下方开始；装饰图片画在带里，底边压进卡片顶边以下一个卡片内边距（1px 细边加 6px，边框更宽时随之增大，即候选内容开始的位置），横向按对齐方式在卡片内收同样的内边距（居中时不留偏移），画在卡片之上。宿主生成的卡片九宫格图片顶部多出 `top_inset_dip` 行全透明像素，并计入九宫格上边距，这几行永不拉伸；带下面的阴影边与无装饰时相同。`ShadowMargin` 与内容上边距同样加上带高，所以 X11 上 classicui 把卡片（而不是带）对到光标下方，候选文字仍从卡片顶边加内边距处开始；Wayland 上 classicui 不读 `ShadowMargin`，整个窗口（带在内）从光标处开始，卡片比 X11 多下移带高。没有合成器的 X11 会话里这条带和阴影一样显示为黑色。classicui 在绘制背景时一并绘制 overlay，早于候选文字，所以压进卡片的部分在预编辑和选中行之下；它恰好止于内容开始处，不会盖住文字。classicui 按图片的原始像素尺寸绘制 overlay，不能像 Windows 那样按 `width_dip` 等比缩放，宿主也没有图片解码器，只能从 PNG 头读出高度：读得出高度时按底边定位，高于「带高加内边距」的部分在带顶被 `OverlayClipMargin` 裁掉；其他格式读不出高度，从带顶开始按原尺寸画，过高时会画到候选上。候选卡片的圆角按优先级取值：设置页「圆角大小」写入的 `candidate_corner_radius`（0–32，未设置时不写入）优先，其次是皮肤的 `candidate_window.corner_radius_dip`（0–32），两者都没有时保持设计的 10px；取到的值替换卡片圆角，九宫格的角随之变化，选中行的圆角取 6px 与卡片圆角中较小的一个，卡片为直角时选中行也是直角。Linux 只接这一项：设置页的「整体大小」和「不透明度」在 Linux 上不显示，缩放与字号设置重复，而没有合成器的 X11 会把半透明的卡片画成黑色。清单里的背景图（`candidate_window.background`）和悬浮工具栏配色在 Linux 上不绘制：背景图需要按卡片尺寸缩放并乘不透明度，classicui 的九宫格和唯一的 overlay 都做不到，宿主也没有图片解码器；两个前端都没有悬浮工具栏。皮肤的翻译色（`translation`，在解析结果里是与序号不同的 `secondary`）只有 IBus 使用，以区段前景属性画在释义上，面板是否支持区段属性取决于桌面；classicui 的一条候选只能是一种颜色。Plasma 的 kimpanel 和 GNOME Shell 面板不使用 classicui 主题。Fcitx5 宿主在当前界面是 kimpanel 时报告 `kimpanel`，在 classicui 的 `Theme` 或 `DarkTheme` 是用户所选的第三方主题时报告 `fcitx_theme`，设置页据此提示颜色与皮肤不会生效。

`tsf_preedit_style` 在 Linux IBus 中映射为：`raw` 显示 Engine 的 ASCII `editing_text`，`pinyin` 显示 Engine 的 `preedit`，`empty` 隐藏预编辑；设置热重载会更新当前会话的显示样式。IBus 预编辑（包括语音的流式预编辑）与 Fcitx5 一样整段加单下划线，是 Windows 组合串点状下划线（`TF_LS_DOT`）在 Linux 上的对应形式；`empty` 样式和清除预编辑时不带下划线。候选与上屏仍由 Engine 的共享状态决定。

`candidate_preedit_style` 在 Linux 中映射为候选面板辅助文本：`pinyin` 显示当前拼音，`empty` 隐藏拼音。`show_candidate_page_number` 独立控制当前页/总页数，默认 `true` 保持旧版行为；设置页「候选窗口 → 预编辑 → 显示页码」可以关闭它。两者都隐藏且没有模式标签或连击提示时，辅助行不显示。IBus 与 Fcitx5 都支持此设置，设置热重载立即更新现有会话，翻页快捷键和候选选择不受影响。

双拼方案提供 IBus 属性“双拼原始预编辑”，对应共享 `shuangpin_preedit_uses_raw`：开启时预编辑保留原始双拼编码，关闭时显示 Engine 展开的拼音。该选项仅在双拼方案下可用；有共享偏好目录时按 revision 持久化并由 Engine 在组合空闲后应用，没有偏好目录时切换会结束当前组合并重建当前会话。

共享设置页的“输入 → 双拼预编辑”在 Linux 上同样可见，对应 `HostCapabilities::shuangpin_preedit`。IBus 与 Fcitx5 都是自己把快照的 `preedit` 写进平台预编辑的，因此这个选择归宿主展示；此前该控件按平台名只给 macOS，Linux 用户只能在双拼方案生效时从原生状态菜单里找到它。

五笔方案提供 IBus 属性“五笔剩余编码”，对应共享 `wubi_code_hint`，默认开启；关闭后候选仍按 Engine 原文显示，但隐藏候选后的剩余五笔编码提示。该设置仅影响展示，不改变候选身份或提交文本；配置共享偏好目录时持久化，未配置时保留在当前会话。

Windows 的 `clipboard_history` 依赖独立剪贴板监听器和候选历史 UI；IBus Engine API 不提供剪贴板事件。Linux IBus 宿主只读取用户明确配置的历史文件，并通过属性菜单提供最近条目、删除和清空操作，不读取系统剪贴板，也不在输入线程监听剪贴板。Linux 桌面面板的剪贴板同步仍由独立 Tauri 服务承载。独立工具的 `get INDEX` 操作会将已存储条目写到标准输出，`remove-index INDEX` 按历史位置删除单个条目，供桌面服务或 compositor 显式接管粘贴和删除动作；它不会写入或读取系统剪贴板。

`candidate_theme` 是 Windows 候选窗口的整体深浅主题覆盖。IBus Engine 只提交 lookup table 内容与文本属性，候选 panel 的边框、间距和主题切换由桌面环境控制；Linux 保留共享设置，但不伪造 panel 主题覆盖。显式文字、编号和表面色按 IBus 属性传递。

个人词典维护使用共享 Host API 的独立 `msime-linux-dictionary` 原生入口，不由 IBus 输入线程执行。它从标准输入读取一个不超过 65536 字节的 JSON 请求并输出 JSON 响应；请求格式和 `list`/`edit` 操作见 `msime_client.h`。`list` 不带词库类型和编码前缀时只列出用户自己添加的词；指定类型并给出编码前缀（快捷短语不需要前缀）时，同时按前缀查到随输入法附带的内置词条，用户词排在前面，每条带 `source`（`user` 或 `bundled`）。内置词条只能调整权重或删除：`edit` 的替换项必须保持类型、编码和词不变，否则返回 `bundled dictionary entry is read-only`；改动与 Windows 一样记入用户词库日志，词库升级重放后仍然生效。拼音词库的 `export` 除用户词外还带出对内置词学到或设置过的权重，并与 Windows 一样略去单字；五笔、英文和快捷短语只导出用户词。调用方必须在编辑前停止使用相关词典的会话，API 负责共享访问锁、请求幂等和 Engine 原子写入；错误输出不包含词条内容。该入口不替代桌面设置页，便于 GTK/Qt 前端复用同一契约。

该入口也支持本地词库批量迁移：`import` 接受不超过 64 KiB、最多 1000 行的 UTF-8 文本，`standard` 格式为 `词条<TAB>编码<TAB>权重`，`windows` 格式为 `编码<TAB>词条<TAB>权重`，`rime` 格式兼容 `userdb.txt/dict.yaml` 的 `词条<TAB>编码[<TAB>权重]`、YAML 头和 `c=… d=…` 元数据；拼音词库的 `hans` 格式则每行接受纯汉字词条，由 Engine 从已验证主词典解析最高权重的规范拼音并以 10000 导入。省略权重时使用 10000。空行和 `#` 注释会跳过。调用方提供请求 ID 前缀，入口为每行生成稳定回执，重复提交同一请求安全。`export` 按页返回相同两种格式的文本和 `has_more`；设置窗口把拼好的文件交给 Tauri `save_export` 写进用户的「下载」文件夹（重名时另起 `name (2).txt`），并在页面上提示写入的完整路径。这一入口契约不变；桌面设置页导入不超过 32 MB 的文件时（更大的文件在读取前就被拒绝，提示拆分后分别导入），会在行边界把它拆成若干满足上述限制的请求（请求 ID 为 `<ID>-<序号>`，Rime YAML 头以空行代替），按整份文件的列顺序逐批导入，并把各批结果合并成一份报告，失败行号按整份文件计算；整份文件因控制字符、没有可用行等原因会被入口拒绝时，设置页在写入任何一批之前就拒绝。导入取得独占维护锁。活动 IBus / Fcitx5 会话持有共享锁时，设置窗口会在用户数据目录写入带过期时间的 `.msime-dictionary-quiesce` 租约（最长 30 秒）：两个宿主在偏好轮询时（Fcitx5 每 250 ms、IBus 每秒）上屏当前组合、关闭会话，租约存在期间不再打开新会话；设置窗口在约 2.5 秒内重试获取锁，分批导入时租约在各批之间保持并在每批前续期，整次操作结束后立即删除租约，仍取不到锁才返回 busy。导出使用共享锁，不会中断用户组合。

账号云词库使用独立的 `msime-linux-cloud-dictionary /absolute/provider.sock` 入口。它验证 `list`、`changes`、`add`、`update`、`delete`、`import` 和 `export` 请求后，经用户管理的 Unix socket 转发一行 `{"version":1,"kind":"cloud_dictionary","request":...}`；provider 负责登录态、凭据、网络和冲突同步，入口只输出有界 JSON 响应，不保存账号信息。Tauri 设置页通过 `cloud_dictionary_provider_socket` 或 `MSIME_CLOUD_DICTIONARY_PROVIDER_SOCKET` 接入同一 provider，提供词库选择、搜索分页、词条 CRUD，以及标准 TSV、Windows TSV 和拼音汉字自动注音导入。

### 账号

Linux 桌面外壳现在提供与 Windows、macOS 同样的九个账号命令（`account_status`、`account_providers`、`account_request_code`、`account_login`、`account_profile`、`account_rename`、`account_logout`、`account_delete`、`account_forget`），共享设置页的账号分类因此在 Linux 上可用。此前这些命令只为 Windows、macOS、Android、iOS 注册，`main.tsx` 也只在前两者注入 `account` 客户端，Linux 上整个账号界面没有宿主可调。

命令本体与 Windows 完全一致，差别只在会话文件所在的目录：Linux 没有一个所有目标桌面都保证在跑的密钥服务，因此会话保存在共享状态目录下的 `account-session.json`，以 0600 创建、按临时文件加原子重命名发布，读取前核对是普通文件（不跟随符号链接）、属主是当前用户、其他用户无任何权限、且大小不超过 64 KiB；不满足时报告存储错误而不是伪装成「未登录」。这与 Linux provider 私有配置文件一直声明的规则相同，也沿用 `msime-linux-prepare` 建立的 0700 状态目录。**它不加密静态数据**；macOS 与 Windows 桌面端现在用的是同一个文件存储（`crates/client-core/src/account/file_storage.rs`），不再用 Keychain 或凭据管理器。令牌只在桌面宿主进程内，交给 WebView 的仍是与其他平台相同的脱敏 DTO。

云剪贴板使用独立的 `msime-linux-cloud-clipboard /absolute/provider.sock` 入口。它验证列表、明确添加、删除和启停请求后，经同一类用户管理服务转发 `{"version":1,"kind":"cloud_clipboard","request":...}`；服务负责账号凭据、云端保留和冲突处理，不自动读取本地剪贴板。

桌面设置可在 HostOptions 中配置 `cloud_dictionary_provider_socket` 和 `cloud_clipboard_provider_socket` 两个绝对 Unix socket；未配置时分别回退到 `MSIME_CLOUD_DICTIONARY_PROVIDER_SOCKET` 和 `MSIME_CLOUD_CLIPBOARD_PROVIDER_SOCKET`。这两个字段会随 runtime-options 原样保留，但不会进入 Engine 选项或输入会话。

IBus 面板注册 `InputMode` 开关：选中时按当前输入方案转换，关闭时直接透传编辑器输入。面板符号为「文」或「A」，日语方案开启转换时为「日」，韩语方案为谚文「한」，粤拼、注音、笔画、越南文、藏文方案分别为「粤」「注」「笔」「越」「藏」（与其它宿主一致），CapsLock 打开时无论哪种模式都显示「⇪」（与 Windows 语言栏图标的优先级一致），不把日语、韩语等已配置方案误标为中文。Fcitx5 宿主通过输入法子模式标签在托盘和面板上显示同样的状态，用词与模式提示一致：「中」「英」「粤」「注」「笔」「日」「한」「越」「藏」「⇪」。CapsLock 状态取自按键事件（松开 CapsLock 时即更新）。切到直接输入前通过 Engine 完成高亮组合；再次聚焦保留当前实例的选择，关闭期间的候选点击和翻页无效。密码等受限字段及失焦时开关不可用，恢复正常字段后继续使用原选择。按 `ime_mode_scope` 分别记忆客户端或共享全局状态，不写回偏好文件或重启持久化，也不占用桌面已有的输入源切换快捷键。

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

准备配置必须在没有会话使用该状态目录时执行。运行入口动态注册独立的 `msime-linux`，不安装系统组件、不修改旧 Linux 产品或自动切换用户输入法；关闭进程即结束本次注册。安装后的 component 通过 `msime-linux-ibus-launcher` 启动，默认读取 `~/.config/msime-client/runtime-options.json`；也可用 `MSIME_IBUS_OPTIONS` 指向已准备好的绝对路径。launcher 按自身目录定位 Engine，支持自定义安装前缀。上面这条命令直接跑构建目录里的二进制，路径指向源码树，面向开发调试；面向发行的包见「生成 Linux 安装包」。宿主监听配置 JSON 的写入和原子替换事件；后续新焦点会话使用新配置，正在组合的会话保持原设置直到结束。

安装产物提供 `msime-linux-prepare`，首次准备状态无需 Cargo 或源码目录：

```sh
msime-linux-prepare /absolute/verified-resources /absolute/new-state
export MSIME_IBUS_OPTIONS=/absolute/new-state/runtime-options.json
msime-linux-ibus-launcher
```

两个参数必须是绝对路径，状态目录的父目录必须已存在；全新状态目录不能已有文件，但安装流程预先写入的 `anonymous-account.json` 或 `anonymous-session.json` 可以保留在其中。命令通过 Host API 按固定的 `desktop-dictionary.lock.json` 校验资源目录及其资源，再准备 Engine 用户数据、缓存和偏好配置；以 0700 创建状态目录，以 0600 原子发布 `runtime-options.json`，成功时输出配置路径。失败时保留已准备的数据，不覆盖已有目录或配置；重试需另选全新目录。若希望启动器自动发现配置，可将新状态目录选为 `$XDG_CONFIG_HOME/msime-client`（未设置时为 `$HOME/.config/msime-client`），并事先准备其父目录。自定义位置的 `MSIME_IBUS_OPTIONS` 需传入实际启动 IBus 的会话环境。

若安装时已在 CMake 配置阶段传入 `-DMSIME_ENGINE_RESOURCES=/absolute/verified-resources`，CMake 会按仓库内固定的 `resources/desktop-dictionary.lock.json` 校验每个词库文件的名称、大小和 SHA-256，并将这些文件安装到 `${CMAKE_INSTALL_DATADIR}/msime-client/resources`；仓库自带的六套辅助码表（`resources/helpcodes`，不在词库发布里）装进其中的 `helpcodes/`，Engine 从资源目录下的这个子目录读辅助码表，宿主校验放行它；锁文件作为同级的来源元数据安装，不会混入 Engine 运行目录。`msime-linux-setup --download` 下载的词库目录目前不带辅助码表。这样可以直接使用已安装资源准备状态：

```sh
msime-linux-prepare --installed /absolute/new-state
```

非英文翻译目标（法、日、西、俄、德、韩）的离线释义词典是可选的：用 `scripts/build_offline_glosses.py` 生成到 `target/offline-glosses` 后，配置时传 `-DMSIME_OFFLINE_GLOSSES=/absolute/target/offline-glosses`，CMake 把其中的 `zh-*.db` 连同必需的 `offline-glosses-NOTICE.txt` 装到资源目录的同级 `${CMAKE_INSTALL_DATADIR}/msime-client/offline-glosses`。使用显式资源目录时，把它们放在该目录同级的 `offline-glosses/` 下即可。IBus 与 Fcitx5 在主翻译目标装有词典时先显示词典释义，开启候选翻译且配置了自己的翻译服务时再逐个询问所有候选，服务的回答替换词典的，没回答的保留词典释义。没有这些文件时候选释义仍只有英文。

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

若要把 Tauri 设置窗口一并安装，可先用 `pnpm --filter @msime/desktop tauri build --no-bundle` 生成 Linux 二进制，再在 CMake 配置阶段传入 `-DMSIME_DESKTOP_BINARY=/absolute/path/to/msime-desktop`。安装会增加 `msime-linux-desktop`、`msime-linux-settings` 和桌面菜单项；设置启动器按 `MSIME_CLIENT_HOST_OPTIONS`、`MSIME_IBUS_OPTIONS`、用户配置路径、安装时配置的系统配置路径的顺序选择绝对 runtime-options，并把它传给 Tauri 宿主，不把开发机路径写入桌面文件。启动器默认设置 `WEBKIT_DISABLE_COMPOSITING_MODE=1` 与 `WEBKIT_DISABLE_DMABUF_RENDERER=1`，规避部分驱动上的 GBM 缓冲分配失败与 Wayland 显式同步协议错误（NVIDIA 驱动加 KWin 时 DMA-BUF 渲染器会让窗口一启动就报 Error 71 退出，只禁用合成不够），终端、桌面菜单及输入法菜单入口均生效；不强制切换 GTK 后端。排查时可显式把其中任一变量设为 `0` 恢复对应路径，需先退出已运行的设置进程，避免单实例复用旧环境。设置页的“语音输入”分类可打开独立语音面板，面板调用同一 provider 并把识别结果提交到打开前捕获的编辑器。Linux IBus 与 Fcitx5 菜单顶层的“词库…”“设置…”“关于水杉输入法”以及“桌面工具”中的“帮助”“反馈”分别路由到共享 Tauri 的对应设置页（`msime-linux-settings --panel dictionary|settings|about|help|feedback` 同样如此）；“快捷键”分类提供“重启输入法服务”按钮：先用 `fcitx5-remote --check` 探测当前会话，Fcitx5 正在运行时经 `gdbus` 调用它的 `ReloadAddonConfig`（参数 `msime`）重置水杉插件，否则调用当前用户的 `ibus restart`。探测不会通过 D-Bus 启动一个原本未运行的 Fcitx5，也不会为了刷新 MSIME 杀掉承载其他输入法的整个 Fcitx5 进程。普通配置保存仍通过 runtime-options 文件热重载，不需要为了设置变更重启服务。

## 隔离验证

候选操作也通过 IBus 属性菜单提供：对当前候选页的 1–9 槽位分别注册固定和删除动作，动作携带当前视图代次调用共享 Host API；菜单只对可写入用户词库的中文/英文候选显示这些动作，云端、AI、Emoji、颜文字、快捷短语和日文候选保持只读。桌面 panel 不支持 Windows 式右键候选窗时仍可使用该菜单路径。

候选操作菜单还提供将当前词库候选固定到第 1–5 位及取消固定。固定位置由 Engine 持久化并随候选快照返回；Linux 宿主只传递候选身份和目标槽位，不复制词典写入或排序逻辑，并在 IBus 候选行显示“固定 N”状态。

`bash build-container.sh` 在容器里编译整个 Linux 原生宿主（IBus engine、Fcitx5 插件、全部 provider 入口和单测）并运行 `ctest`，不需要词库、不启动任何 daemon，也不需要本机是 Linux；它由 `scripts/verify-local.sh` 作为编译门禁自动调用，Linux 主机上则直接用系统 ibus 开发包跑同一套配置。镜像定义在 `tests/tools/Dockerfile.build-gate`，与隔离验收镜像分开，以免给后者加上会改变其构建内容的 X11/XFixes/Fcitx5 开发包。

`scripts/verify-local.sh` 的「compile: linux desktop shell」阶段在非 Linux 主机上用 `tests/tools/Dockerfile.desktop-check` 构建的镜像跑 `cargo check -p msime-desktop --locked --all-targets`：与编译门禁同一个固定摘要的 `rust:1.97.1-bookworm`，预装 Tauri 外壳需要的 webkit2gtk、gtk3、libsoup、javascriptcoregtk 和 cpal 需要的 ALSA 开发包，apt 只在 Dockerfile 变化后的第一次运行时执行，`--quick` 和 pre-push 钩子不再每次重装。镜像构建日志留在 `target/linux-desktop-check/image.log`，apt 失败时阶段打印其末尾并 FAIL。

这两个镜像都按 checkout 路径打 tag（`msime-linux-build-gate:<哈希>`、`msime-linux-desktop-check:<哈希>`，哈希取仓库绝对路径的 SHA-1 前 12 位），每个跑过门禁的 worktree 各留一份，单个占 2.4–3.3 GB，worktree 删除后不会自动回收。清理只删这两类 tag，不要 `docker system prune`（会连带别的项目和并发会话在用的镜像）：先 `docker images 'msime-linux-*'` 看有哪些，再 `docker image rm <tag>` 删掉已不存在的 worktree 对应的那些，最后 `docker image prune` 回收失去 tag 的悬空层。当前 checkout 的哈希可用 `printf %s "$PWD" | shasum | cut -c1-12` 在仓库根目录算出；删错了也无妨，下次运行会重建。

隔离验收脚本只读挂载源码，输入引擎是仓库里的 Rust crate，不再需要预先准备或借用任何 Engine 树，因此在 worktree 里也能直接跑。它的测试镜像与编译门禁一样按 checkout 路径打 tag，并发的 worktree 不会互相覆盖镜像。随包在线/语音/剪贴板 provider、凭据、豆包鉴权、翻译缓存、录音设备这一整片 Python 测试都在容器内执行。

`bash tests/tools/check-container.sh /absolute/verified-resources` 创建专用 Linux 容器，源码与词库只读挂载，构建缓存仅写入本仓 target/linux。基础 Rust 镜像固定摘要，apt 开发依赖来自 Debian bookworm 仓库；不声称所有系统包字节级可复现。容器内创建独立 D-Bus 和 IBus daemon，不连接宿主桌面，不修改现有输入源，结束后移除容器并保留构建缓存。

`engine_smoke` 使用真实共享库与固定 Release 词库，通过 D-Bus 调用实际 IBusEngine：验证预编辑与候选信号、上屏、第二页全局索引点击、标点、修饰键/key-up、快捷键取消、失焦、密码隔离与私密文本恢复。另启动实际宿主可执行文件，由独立 Python IBus 输入上下文通过 daemon/factory 输入合成拼音并接收提交。共享核心/运行时/宿主 25 项 Rust 测试纳入本地脚本。

嵌套偏好对象整体可以省略（共享 `Preferences` 会给默认值），但它的成员一个都不能少：宿主若把单个键补进一个 runtime-options 文档里本来没有的 `mixed_input` / `local_modes`，写出的就是残缺对象，Host API 会判为 invalid options document。这些默认值因此由 `msime_client_default_preferences` 从共享层发布，宿主据此补全缺失成员，不在 C++ 里另写一份契约。

已验证的环境包括 Debian bookworm arm64、IBus 1.5.27 的容器链路，以及 Arch x86_64（Hyprland/Wayland、Fcitx5 5.1.22、IBus 1.5.34）上的真实安装：`cmake --install` 到 `/usr` 后，Fcitx5 加载 addon（日志中的 `Loaded addon msime`）、输入法出现在可用列表与当前输入法组、`msime-linux-settings` 拉起的设置窗口实际映射。CI 在固定容器里构建并跑 ctest；发行附件 `.deb`/`.tar.gz` 的包内文件、Depends 与动态链接在打包容器里检查过，尚未在真实系统上用 apt 安装验收；换发行版、换架构或改动桌面环境时，按本节的容器脚本和上面的安装步骤各跑一遍即可确认。

系统行为依据 [IBus Engine API](https://ibus.github.io/docs/ibus-1.5/IBusEngine.html) 和 [IBus InputContext API](https://ibus.github.io/docs/ibus-1.5/IBusInputContext.html)。

`candidate_follow_cursor` 是 Windows 候选窗口的定位选项。IBus Engine API 只提供候选表和输入上下文光标位置的通知，不提供由输入法宿主固定 panel 锚点的接口；候选 panel 的定位由桌面 panel 自己决定。因此 Linux 会读取并透传该共享配置，但不伪造 Windows 的固定候选窗口行为：在 Linux 上候选表始终交给 IBus panel 按当前输入上下文位置呈现。该限制属于 IBus/桌面环境边界，不影响候选内容、分页或选词。

Linux 关于页的“输入法宿主日志”对应共享偏好中的 `diagnostic_log.server`。开启后，IBus 与 Fcitx5 宿主在偏好目录写入同一个仅用户可读的 `diagnostic.log`，记录焦点会话、偏好应用、菜单保存、词库维护时释放会话和固定操作失败阶段；文件达到 1 MiB 时保留一个 `.1` 轮转副本。记录经过长度和 ASCII 控制字符限制，不包含按键、输入文本、候选文本、凭据、路径或 provider 响应；关闭开关后不再写入。开关变更随偏好热重载立即生效，IBus 与 Fcitx5 都不需要切换焦点。Windows 专用的 `diagnostic_log.tsf` 在 Linux 设置页隐藏，旧配置字段仍原样保存以保持跨平台同步。

Linux 关于页的“检查更新”读取水杉输入法仓库的 GitHub 发行版列表，只取 `linux-v` 标签下非草稿、非预发布的版本，按版本号取最新，不复用只发布 Windows 安装程序的 `msime.app/update.json`。发行页地址必须属于固定的 `metasequoiaime/msime` releases 路径才会显示；仓库尚无 Linux 发行版时显示正常的“暂无可用发行版”状态，网络错误或无效响应才报告检查失败。更新提示注明软件包未签名，并给出 GitHub 为 `.deb`（没有时为 `.tar.gz`）附件计算的 SHA256 和 `sha256sum <文件名>` 核对命令；GitHub 没有返回该附件的摘要、或同一发行版带有多个架构的包时不显示校验值，改为提示下载发行版里的 `SHA256SUMS`，用 `sha256sum -c SHA256SUMS --ignore-missing` 核对。

## 输入细节与平台差异

Linux 桌面设置页通过宿主能力显示共享的模糊音配置。总开关首次从关闭切换为开启时，偏好存储会一次性选中 11 条规则；用户之后删减规则、暂时关闭再恢复时保留删减结果，并用内部播种标记避免空规则集被再次填充。规则计算仍由 Engine 完成。

词库管理可从 IBus 与 Fcitx5 菜单顶层的“词库…”、桌面启动器的“词库”动作或执行 `msime-linux-settings --panel dictionary` 打开，与 Windows 桌面工具使用同一设置宿主和词典状态。

全角/半角输出与 Windows 模式面板对应：`CharacterWidth` 由 `input-runtime` 和 `msime-host-api` 按会话携带，IBus 属性菜单和 Fcitx5 状态栏都提供该开关，可打印 ASCII 在上屏时完成全角转换。配置了共享偏好目录时，模式按 `character_width` 持久化；没有该目录的直接预览配置保持会话级。Fcitx5 与 IBus 都在会话建立时按 `character_width` 设置全角，共享偏好热重载、属性/状态菜单和快捷键切换都会立即同步到正在运行的会话；焦点切换不会丢失全角状态。Fcitx5 新会话以偏好存储中的 `character_width` 为准，另一个窗口在状态栏切换的宽度也会带过来；状态栏切换后尚未写入存储的宽度（保存失败待重试，或隐私输入窗口中本不保存的切换）不会被热重载改回，下一个会话再以存储为准。IBus 冒烟夹具和 Fcitx5 原生上下文测试覆盖全角与半角 ASCII 上屏。

IBus 注册入口通过 launcher 启动，配置优先级为 `MSIME_IBUS_OPTIONS`、用户的 `$XDG_CONFIG_HOME/msime-client/runtime-options.json`（默认 `~/.config`）、安装时配置的系统 runtime-options。IBus 与桌面启动器仅在用户配置不存在时回退；显式覆盖、已存在但不可读的用户配置、悬空符号链接或相对用户配置目录会报错，不会悄悄改用系统配置。系统配置的写入权限沿用安装权限，启动器不会自动复制或改写配置。直接运行 launcher 时可用第一个参数指定系统配置回退路径。

**IBus 宿主崩溃后自动恢复。** ibus-daemon 不会重新拉起退出的 component，宿主一旦崩溃，用户只能切换输入法或 `ibus restart` 才能继续打字。launcher 因此不 `exec` 宿主，而是留在它前面做守护：`msime-linux-ibus` 以非 0 状态退出或被信号杀死（段错误、`SIGKILL` 等）时，按 Windows watchdog 的方式退避重启：第一次等 2 秒，之后每次翻倍、封顶 30 秒，宿主连续运行满 30 秒算健康，此后再崩溃又从 2 秒开始。重启的宿主带 `--recovered` 启动，重新注册 factory 后，若全局引擎为空或仍是 `msime-linux`，就把 MSIME 重新设为全局引擎，有焦点的编辑器不必重新选择输入法即可继续输入；全局引擎已是其他输入法时不干预。IBus 在动态注册的 component 消失时也会清掉来自其他 component XML 的全局引擎（例如 xkb 布局），所以用户切到这类输入法期间宿主恰好崩溃时，恢复后回到的是 MSIME 而不是原来的布局。四种退出不重启：`Ctrl+Shift+Alt+T` 维护退出（宿主以专用状态 77 退出，守护照此退出）；宿主以 0 退出，表示总线断开，即 ibus-daemon 自身在退出或重启：`ibus restart` 后由新的 daemon 在选中引擎时再拉起 launcher，`ibus exit` 后不再运行，这是预期行为；重启的宿主连不上总线时同样以 0 退出，说明 daemon 在退避期间已经消失（被 `SIGKILL` 或自身崩溃，没来得及停止守护），守护随之结束，不会成为孤儿反复重启，也不会在之后启动的新 daemon 上多注册一个 component；准备重启时配置文件已不可读；宿主程序已被删除或不再可执行（`apt remove` 后仍在运行的宿主又崩溃了），这种情况在退避之前就退出，不会每 30 秒对着缺失的文件重试。宿主以专用状态 78 退出表示程序已被升级替换（见「安装后首次使用」里的升级一段），launcher 立即带 `--recovered` 重启，不等待，退避状态保持原样。ibus-daemon 用 `SIGTERM` 停止 component，launcher 把它连同 `SIGINT`、`SIGHUP` 一律作为 `SIGTERM` 转给宿主，等宿主退出后自己也退出，退避等待中收到时立即退出。契约由 `tests/core/ibus_launcher_supervisor.py`（桩宿主：退避间隔、`--recovered` 参数、升级退出立即重启且不影响退避、各种不重启的退出与停止请求，含重启后以 0 退出）和 `tests/runtime/daemon_smoke.py` 的崩溃用例（真实 daemon：`SIGSEGV`/`SIGKILL` 后重新注册、有焦点的上下文不重新选择即可输入、维护退出不重启、`ibus exit` 结束守护）钉住。

**使用上报：IBus 宿主和 Fcitx5 插件都上报，默认开启，读共享偏好 `usage_reporting`，都不拖住输入。** 两个入口都经 `platforms/common/Telemetry.cpp` 调用 Host API 的 `msime_client_telemetry_*`，队列、本机随机安装 id、按 UTC 日去重的 `active`、会话和投递都在 Rust 里（见 `crates/client-core/src/telemetry.rs`）。一个宿主进程就是一次会话：`msime-linux-ibus` 每次启动（含崩溃守护带 `--recovered` 的重启）在连 ibus-daemon 之前开始会话，只做文件 I/O；主循环正常退出时排进一条 `session`。Fcitx5 插件以插件实例的生命周期为一次会话，文件放在单独的 `$XDG_STATE_HOME/msime/fcitx5`，不和同一用户的 IBus 宿主共用会话标记。崩溃时 `std::terminate` 回调和 SIGSEGV、SIGBUS、SIGILL、SIGFPE、SIGABRT 的 sigaction 处理函数只把这次会话的崩溃记录写到磁盘（信号或异常摘要加 `backtrace_symbols_fd` 的调用栈），然后照原样把信号交还：IBus 宿主走默认动作，Fcitx5 进程里交给 Fcitx5 自己先装的处理函数。下次启动时这条记录变成 `crash` 和 `session_crash`，模块路径只保留文件名；只留下会话标记而没有崩溃记录（被杀、注销、关机）不算崩溃。投递在注册 component 之后的后台线程里进行，之后每 30 分钟一轮；端点慢或不可达都不影响注册和输入，宿主退出时也不等它，没发出去的留在队列里下次再发。关掉 `usage_reporting` 后下一轮就清空队列、会话标记和崩溃记录，不再发送。字段、落盘位置和与其他平台的差异见 [PRIVACY.md](../../PRIVACY.md#使用统计与崩溃上报默认开启可关闭)。`tests/core/ibus_startup_telemetry.py` 用普通 dbus-daemon 加只应答 `RegisterComponent` 的桩代替 ibus-daemon，把 HTTPS 经 `HTTPS_PROXY` 指到一个只 accept 不回应的本地端点，钉住：注册先于投递且不等端点、端点挂起期间宿主主循环仍能应答 IBusFactory 的 `CreateEngine`、事件是带安装 id 的 `active`、`--recovered` 启动补报上一会话的 `session_crash` 和去掉目录的 `crash`、宿主收到 SIGSEGV 时只写崩溃记录不联网。`platforms/common/tests/telemetry.cpp` 在子进程里真实触发信号和 `std::terminate`，另外覆盖链式交还给原处理函数、偏好关闭时清空。

数字选词：IBus 属性菜单中的“数字选词”控制主键盘和小键盘 `1–0` 对当前候选页的选择，默认开启；状态按输入上下文保留，候选分页仍使用 Engine 提供的全局候选身份。候选表支持左键或中键选词，面板附带的修饰键状态（如 NumLock）不影响选词；右键不改动词典，只在辅助区域提示通过 IBus「候选操作」菜单固定、固定排位或删除。操作会校验会话、代次和全局索引。

两个宿主都按物理数字行选词（Windows 用虚拟键码，与布局无关），AZERTY 等未按 Shift 时打出 `&é"…` 的布局同样能选词；Unicode 模式下裸数字是十六进制输入，选词改用 Shift 加数字行，与 Windows 一致。Fcitx5 以前只认按键符号，这两种情况都选不了词，`0` 也不选第十项。AltGr 打出的布局字符（德语的 `@`、`[` 等）在两个宿主上都先完成组合再交给编辑器，不再被 Fcitx5 当作中文标点或翻页键。

九键输入：IBus 属性菜单中的“九键输入”只在全拼方案下可用。开启后数字键交给 Engine 组成九键拼音，候选视图中的数字选词自动让位；切换会先结束当前组合并重建会话，九键拼音候选和代次由 Engine 返回。关闭后恢复普通数字选词，设置只作用于当前 IBus 输入上下文。

九键歧义拼音：Engine 返回 `nine_key_spellings` 时，IBus 属性菜单显示当前代次的拼音选项（如 `ni`、`mi`）。选择菜单项通过 Host API 携带会话和 generation 调用 `choose_nine_key_spelling`；组合已变化或失焦后，旧菜单项会被忽略，不会改写新组合。

快捷模式：属性菜单中的“快捷模式”提供 Unicode、日期时间、快捷短语、Emoji、颜文字、超级简拼、临时英文/日文，以及计算与数字（V）、指令（/）和名单（@）模式的会话级开关。后三个默认关闭，偏好文件在它们关闭时不写这几个键，两个宿主的菜单因此把缺省读作关闭，其余模式缺省读作开启。切换会结束当前组合并重建 Engine 会话，开关只覆盖当前输入上下文；共享 Preferences 和设置页中的持久化开关仍作为新会话默认值。

小键盘标点：`KP_Decimal` 始终提交 ASCII `.`；`KP_Separator` 按逗号标点处理；`KP_Subtract`、`KP_Add`、`KP_Divide`、`KP_Multiply` 和 `KP_Equal` 映射为 `-`、`+`、`/`、`*`、`=`。候选或组合活动时，宿主先通过 Host API 提交高亮候选，再追加对应 ASCII 标点；空闲时算术键仍遵循 Engine 的标点策略，且不会触发减号/等号候选翻页绑定。

Fcitx5 采用同一规则：此前小键盘句点会按普通句号变成「。」，数字里的小数点因此被改掉。

Microsoft 双拼：当当前方案使用 Microsoft 键位且光标所在分音节已有奇数个按键时，未修饰的分号按键作为 `ing` 输入键交给 Engine，不会被中文标点路径提前消费；其他分号仍遵循普通标点处理。

Unicode 输入：进入 Unicode 本地模式后，`Shift++` 作为 Engine 的 `+` 输入继续组成 `U+` 前缀，不会被候选标点或减号/等号翻页路径拦截。

计算、指令与名单模式：空组合时 `Shift+V` 进入计算与数字模式，中文标点下空组合时 `/`、`@` 进入指令和名单模式，都由 Engine 决定，宿主不做模式判断。Engine 在视图的 `spelling_symbols` 里列出当前状态下它当输入收的非字母字符（计算模式是数字和 `+-*/.()%^`，Unicode 模式是数字，网址模式是数字和网址符号，空闲时是开启了的 `/`、`@`，全拼、双拼、五笔组字原文是 `www`、`http`、`https`、`ftp` 时是打开网址模式的 `.` 或 `:`），两个宿主按这份清单路由（共享的 `src/core/SpellingSymbols.h`）：本地模式正在拼写的字符在任何宿主绑定之前作为字符交给 Engine，所以计算模式里 `-`、`=`、`,`、`.` 不翻页，`(` 不补成对括号，`.` 不走智能标点，数字不选词；清单里有数字时选词改用 Shift 加数字行，但 Shift 打出的符号若也在清单里（美式布局上的 `%^*()`）仍是输入。空闲的 `/`、`@` 仍走普通标点路径，由共享 runtime 决定开不开模式（例如还有未完成的词组时不开）。组字中清单里的键同样先交给 Engine，所以 `www` 后按 `.` 打开网址模式，而不是翻页或上屏「。」；网址模式里键入网址用的字母、数字和符号，空格或回车上屏整串网址（全角模式下仍是半角），网址外的符号（如 `"`、`<`）先上屏网址再处理该键，网址不进入学习。这三个模式上屏的文本（算式结果、日期、名单里的名字）是 Engine 生成的，不计入打字统计。

按键音、上屏音与背景音乐：两个宿主在按键处理完之后调用 `msime_client_key_sound`（空格、回车、退格各有一类，其余按键一类），Engine 的转换有上屏时调用 `msime_client_commit_sound`，焦点进出、私密状态变化和开始录音时调用 `msime_client_music_set_active`；按键音和上屏音只在中文输入开启时响，Ctrl/Alt/Super 组合键、单独的修饰键和松开都不响；密码、PIN、数字类输入框以及 private/no-spellcheck 输入框一律不响也不放音乐，录音期间同样静音。音效开关、音色包和音量都来自共享偏好 `plugins`，由 Host API 在进程内的播放器里解码和混音，宿主只投递请求：什么都没开时不起线程也不打开音频设备，音频设备出错或播放器 panic 时本进程内音效关闭并在 stderr 留一行，不会影响打字。这一点对 Fcitx5 尤其重要，插件跑在 `fcitx5` 进程里，关着音效的用户不承担任何开销。背景音乐的「是否处于活动状态」对整个播放器只有一份，宿主因此只在它变化时通知，并在销毁会话前先告诉播放器停下。成就音由打字统计的 record 触发，不需要宿主调用。内置音色包安装在 `share/msime-client/sound-packs`，与资源目录并列；IBus 宿主按自身可执行文件的位置找到它，Fcitx5 插件用编译期的安装路径，两者都在运行时配置没写 `sound_packs` 时把它交给 Host API，因为用户下载了新词库后资源目录已不在安装目录里，库自己按资源目录旁边去找会落空。

成对标点：启用成对标点后，`(`、`[` 和 `<` 的中文开标点会由宿主追加对应闭标点，`{` 追加 ASCII `}`，并向编辑器转发一次左移以把光标留在标点中间；`<` 的嵌套层级仍由 Engine 决定，宿主自动补出 `》` 后通过专用接口通知 Engine 抵消本组深度，连续独立输入保持为 `《》《》`，显式嵌套仍为 `《〈〉》`。候选活动时先完成高亮候选再补成对标点；候选导航绑定优先于成对标点。宿主按焦点右侧的 surrounding text 校验闭标点，使用有界配对栈避免重复输入；带 Ctrl、Alt、Super、Meta、Hyper 或 Mod5 的组合不触发跳过，Shift 仍可作为 `)`、`}`、`>` 和引号的物理按键修饰键，且被拒绝的组合不会破坏配对栈。IBus 提供的表格客户端名为 `scalc`、`gnumeric` 或 `calligrasheets` 时跳过成对补全与闭标点移位，以适配表格单元格编辑；共享 `soffice.bin` 不列入排除，避免影响 Writer 等其他文档。

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

服务只接受同一用户连接，同时最多处理四个请求；云候选与 AI 并行请求，AI 失败时仍可返回云候选。HTTP 响应最多 64 KiB，拒绝 HTTP 重定向以保持凭据与端点绑定。AI 沿用 Windows 的 JSON 请求、上下文、candidate_limit 和 DeepSeek thinking 禁用设置；所选提示词槽位为空（含空白）时，system 消息使用与 Windows 默认 `[ai_assistant].prompt` 相同的内置联想提示词，与 client-core 的 `DEFAULT_CANDIDATE_PROMPT` 一致，因此默认配置和手动清空过提示词的配置都能得到可解析的候选。按配置最多保留 10 条有效且不重复的模型候选，再由 Engine 批量缓存和排序。成功的 AI 结果按 provider、endpoint、model 和拼音分段保存在有界进程内缓存中，可跨候选 generation 复用；缓存键不包含凭据、上下文、提示词、会话或原始输入，空响应和失败不会缓存。候选翻译同样按单项缓存，最多 4096 项；成功结果在当前 provider scope 下持续复用，直到配置 scope 变化或容量淘汰，失败项使用 8 分钟负缓存且在 TTL 内不会重复请求，独立候选不会互相抑制。服务不打印输入或网络错误正文，退出时仅删除自己创建的 socket。该入口同时实现候选翻译；语音由独立的随包 voice provider 提供；账号同步服务仍按各自契约接入。

设置页的「获取模型列表」和「AI 润色测试」也接进了同一个 provider。这两个按钮此前在 Linux 上不可用：持有 token 的宿主是自己发 HTTP 去取的，而这个平台按设计把 token 留在 provider 的所有者专用配置文件里，外壳手上没有可用于鉴权的东西。现在 provider 增加 `ai_models` 与 `ai_test` 两种请求：设置页只给出 provider 名、接口地址、模型、提示词和待润色文字，provider 先核对这些与私有配置中的 provider/接口地址（润色再加模型）一致，再用自己的 token 发请求；不一致就什么都不发。模型名最多 128 个、每个 256 字节，去重并保序；润色结果最多 16 KiB。新增能力位 `ai_provider_credentials` 表示「凭据归宿主的 provider」，设置页据此不显示 API Token 输入框，也不再因为没有 token 而禁用这两个按钮——此前模型列表那一段是按平台名藏掉的。

共享设置页可对当前腾讯翻译、NiuTrans、自定义翻译、AI 辅助、语音识别和语音润色选项执行“测试配置”。Linux Tauri 只把服务名和当前非敏感选项发送到已有 provider socket；AI、腾讯和语音 token 仍由 provider 从所有者专用配置文件读取，不返回 WebView。NiuTrans 与自定义翻译继续沿用本来就会进入候选翻译请求的设置字段。provider 使用固定的合成文本或静音 WAV 发出最小请求，并只返回有界的成功/失败文案，不转发服务端正文、URL、凭据或异常细节。测试不会保存草稿，也不采集真实输入；provider 不可用时设置页明确提示先启动服务。

### 随包候选翻译

IBus 宿主在后台通过共享 Host API 查询随包 `msime-english.db`。目标语言为英语且启用 `preferences.candidate_english_gloss` 时，先显示离线中英双向释义，再把未命中的候选发送给在线 provider；关闭 `preferences.candidate_translations` 也不会关闭这项独立的离线释义。未配置在线服务也能使用离线命中。其他目标语言直接使用在线 provider。日语方案和临时日语模式不请求候选翻译。离线与在线结果均校验会话、候选代次和配置代次，释义只用于显示，不进入选词提交文本。

英语目标的在线短释义还会通过 Engine 写入用户数据目录的 `translation-glosses.db`，后续离线查询先查用户释义，再查发布词库。 两个词库独立检查可用性：发布词库缺失或损坏时仍可读取用户释义；用户词库不可用时回退发布词库；两者均不可用时返回错误。保存沿用 Windows 的规则：仅中英候选、源文本不超过 40 字符、格式化后译文不超过 32 字符且不与原文 ASCII 大小写等价；其他目标语言不保存。读写在后台线程进行，不修改发布资源。重建 IBus 宿主且关闭在线 socket 后仍可读取已保存的释义；候选显示继续检查会话和代次。共享 C API `msime_client_translation_gloss_save` 接受 `{target_language,translations:[{text,translation}]}`，离线候选查询可传入 `user_data` 读取相同用户词库。IBus 在 Ctrl+Enter 遇到多个以分号分隔的译义时显示临时副候选页，空格、数字键、上下键、PageUp/PageDown 和鼠标点击只作用于译义，选择后直接上屏并恢复原 Engine 候选；单译义仍直接提交，译义页不会写入词库。

同一服务接受 `kind:"translation"`，沿用 Windows 默认腾讯 TMT、自定义 DeepLX 的选择顺序。默认翻译增加 `--tencent-config /absolute/private-tencent.json`，配置文件必须是当前用户所有、其他用户无权限的普通文件，包含 `secret_id`、`secret_key`，以及可选 `region`（默认 `ap-guangzhou`）。凭据只在 provider 中读取，TC3 签名请求固定发送到腾讯 TMT HTTPS 地址，不接受请求覆盖地址。未提供腾讯配置时仍可使用云候选、AI 和自定义翻译。腾讯私有配置在每次腾讯翻译请求开始时重新读取，无需重启服务；建议通过原子替换更新文件。读取使用非阻塞文件描述符，只接受不超过 16 KiB 的 UTF-8 普通文件，并核对读取前后的文件信息。文件删除、权限变宽或内容无效时跳过腾讯翻译，恢复有效配置后下次请求重新使用；自定义翻译不会读取腾讯配置。每批请求使用同一份凭据和区域快照，缓存按该快照摘要隔离。 腾讯凭据与 Windows 一致，先清除首尾 ASCII 空格、制表符和换行，再进行校验和签名；仅空白、占位符及含内部空白或控制字符的凭据仍会被拒绝。仅首尾空白变化不会使缓存失效。

启用共享设置中的 `custom_translation` 后，服务使用请求中的 endpoint 和可选 API Key 调用 DeepLX 兼容接口，支持本机 HTTP 服务或 HTTPS，禁止重定向。 与 Windows 一致，请求前清除 endpoint 和 API Key 的首尾 ASCII 空格、制表符及换行；归一化后的快照同时用于两个翻译方向和缓存标识。API Key 归一化后为空时不发送 Authorization。自定义服务失败不会回退到腾讯。英文候选译为中文；中文候选译为所选英语、法语、日语、西班牙语、俄语、德语或韩语。源文本超过 40 个字符或不符合中英文候选规则时跳过。中文筛选沿用 Windows 的汉字范围，包含兼容汉字和扩展区，允许带汉字的中英数字混排；包含 Emoji、符号图形、零宽连接符、Emoji 变体选择符或键帽组合标记时跳过。纯英文仍只接受字母、空格、连字符和撇号。IBus 在候选更新后等待 500 毫秒停顿再读取 Engine 当前查询并发出翻译请求；连续输入会重置定时器，失焦、禁用或切换配置会取消待发请求。腾讯按翻译方向批量请求，自定义服务逐条请求，每次网络操作超时 2.5 秒，批次在 6 秒预算用尽后停止发起新请求；宿主在后台最多等待 8 秒，过期代次仍由现有 Host API 拒绝。

翻译结果压平换行并过滤控制字符，内存缓存最多 2048 项，成功结果保留 480 秒、失败保留 30 秒。缓存按 provider、凭据摘要和语言方向隔离，不写入磁盘。

## 随包语音服务

`msime-linux-voice-provider` 接收现有 `voice`、`voice_stop` 和 `voice_cancel` 请求，实现 OpenAI、Groq、SiliconFlow 批量语音识别及可选润色。需要 Python 3.9+，录音使用 `pulseaudio-utils` 的 `parec`（也适用于 PipeWire 的 PulseAudio 兼容服务），或 `alsa-utils` 的 `arecord`。默认优先使用已安装的 `parec`，可用 `--capture alsa` 显式选择 ALSA；选定后设备打开失败会返回失败，不会偷偷改用另一麦克风。

```sh
msime-linux-voice-provider "$XDG_RUNTIME_DIR/msime-client/voice.sock" \
  --config /absolute/private-voice.json --capture pulse
```

先按在线服务章节创建当前用户专用的运行目录，再把 socket 绝对路径填入 `voice_provider_socket`。配置文件必须是当前用户所有、其他用户无权限的普通 JSON 文件，至少含 `asr` 与 `polish` 对象之一；文件不存在时服务照常启动，只有本地识别可用，云端识别请求在文件写好后的下一次录音生效。批量识别及润色对象包含 `provider`、`token` 两个非空字符串，以及可选的 `endpoint` 和 `model`。批量 ASR provider 支持 `openai`、`groq`、`siliconflow`；润色还支持 `deepseek`。这些批量接口须为 HTTPS 且不允许重定向，凭据不通过 socket 查询或命令行参数传递。设置中的 provider/model 须与服务配置一致；私有配置在每次录音开始时重新加载，更换凭据或端点无需重启服务。

服务通过 `parec`、`pw-cat` 或 `arecord` 捕获 16kHz 单声道 PCM（.deb 以 Recommends 声明 `pulseaudio-utils | pipewire-bin | alsa-utils`），在内存中封装 WAV 并发送到配置的 `/audio/transcriptions` 兼容接口。所选后端的录音工具不存在时服务照常启动并在日志中写明，录音请求返回 `detail` 为 `recorder` 的 `voice_dependency_missing`，IBus 与 Fcitx5 宿主都提示“未找到录音工具，请安装 pulseaudio-utils、pipewire-bin 或 alsa-utils”，装上工具后下一次录音即可使用。配置文件、socket 目录、录音设备名或录音上限无效，命令行参数有误，或服务已在运行、socket 无法恢复时，服务以状态 2 退出，systemd 不再重启。默认录音上限 300 秒，可用 `--max-recording-seconds` 设置为 1–600 秒；到时自动停止并识别。松开录音快捷键也走同一完成路径，取消则丢弃结果。小于 250ms 的录音不上传，上传音频不超过 20 MiB；SiliconFlow 按 Windows 行为补静音、省略 language 字段，并在网络或服务端错误后最多重试一次。每次 ASR 网络操作超时 60 秒，可选润色超时 3 秒，润色失败保留原转写。正在发送的 HTTP 请求不能撤回，但取消后其结果不会交给输入目标。

润色保留 Windows 的精炼整理、忠实校对、中翻英、口语整理及三个自定义提示词选择，并沿用 `<asr_text>` 包装和 provider thinking 设置。批量识别没有录音中的实时转写；启用润色时可先返回原始转写的 partial，再返回整理后的 final。Doubao 实时识别使用下述 WSS 配置。

提示音尊重 `sound_enabled`、`start_sound`、`end_sound`，通过 `paplay` 或 `aplay` 播放短提示音。`mute_system_audio` 在支持 JSON 输出的 `pactl` 上暂时静音最多 32 个现有播放流，结束或取消后恢复被本服务改变的流；已有静音、已消失或被替换的流不会强制改写。缺少这些可选工具时继续录音。单次仅占用一个麦克风，最多四条语音任务等待网络，控制请求另留容量；会话按连接进程及 generation 区分。客户端断开录音连接或服务收到终止信号时停止采集并恢复播放流。原始音频、转写和凭据均不写入磁盘或日志。

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

IBus 属性菜单中的“桌面工具”可直接打开手写识别板、屏幕键盘、表情与符号、语音面板、云词库和云剪贴板；“设置…”与“关于水杉输入法”在菜单顶层。该菜单独立于可配置工具栏，通过 `msime-linux-settings` 启动已有 Tauri 面板；需要安装桌面二进制，也支持 `MSIME_CLIENT_SETTINGS_COMMAND` 自定义启动器。密码等受限输入上下文禁用这些入口。

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

Wayland 使用 `wl-paste --type text --watch`；X11 构建环境提供 `x11` 和 `xfixes` pkg-config 模块时，安装 `msime-linux-clipboard-watch-x11`，通过 XFixes 监听 CLIPBOARD 所有权变化，并优先使用同一工具的 `--read` 原生读取路径，无需额外安装 `xclip`/`xsel`。读取支持 UTF8_STRING、STRING 编码回退和 INCR 分块传输，限制累计数据量并使用统一超时；原生读取不可用时仍可回退 `xclip` 或 `xsel`。再次复制相同文本也会触发捕获。监听模式只输出事件标记；读取模式将有界文本经标准输出管道交给监控器，不写日志。不支持事件监听的环境继续以 750ms 间隔轮询；每次文本读取限时 1 秒、最多 12000 个 UTF-8 字节，并保留最多 4000 个完整 UTF-16 单元。文本经标准输入交给 `msime-linux-clipboard-capture`，由 Host API 在偏好锁内重新检查开关并持有历史锁写入，避免关闭设置与写入竞态。监视器不打印剪贴板文本。

可按需执行 `systemctl --user enable --now msime-linux-clipboard.service`，使用默认 XDG runtime-options 路径。`make install` 本身不启用服务，由 `msime-linux-setup` 在首次配置时启用；语音和在线服务不依赖它。启用后服务随 `graphical-session.target` 启动，桌面会话需向用户服务管理器提供 `WAYLAND_DISPLAY` 或 `DISPLAY`。没有这个 target 的会话（Sway、i3、未用 uwsm 的 Hyprland 等）由随包安装的 XDG 自启动项 `msime-linux-clipboard.desktop` 在登录时启动，这对应 Windows 上剪贴板监视随输入法在每次登录时启动：它先把本次会话的 `WAYLAND_DISPLAY`、`DISPLAY`、`XAUTHORITY` 导入用户服务管理器，再重启服务，使 linger 下遗留的旧实例也换到本次会话的显示。从未启用服务的用户不受影响，自启动项什么都不做；`graphical-session.target` 已在运行时也交给它，不重复启动。安装包把它放在 `/etc/xdg/autostart/`；CMake 安装在前缀为 `/usr` 时同样放在那里，其他前缀放在 `<前缀>/etc/xdg/autostart/`，随 `cmake --install --prefix` 一起移动，但只有这个目录在会话的 `XDG_CONFIG_DIRS` 里时才会被读到。安装位置、卸载与启动条件由 `tests/core/clipboard_autostart.py` 钉住。

未显式指定 `clipboard_history_path` 时，IBus 使用 `preferences_directory/clipboard_history.json`，与共享设置存储及独立采集服务一致。显式历史路径仍优先；切换偏好目录时默认历史来源随之更新。监视器遇到非对象 JSON 或无效偏好结构时停止本轮采集并等待下次有效配置。

“候选操作”按当前页候选分组，一级菜单显示候选序号与完整 UTF-8 字符预览，子菜单包含置顶、删除、固定到第 N 位和取消固定。操作仍绑定会话与候选代次。九键拼音分支及「主题」子菜单在原生 IBus 菜单中可见并沿用既有选择回调。

外部皮肤目录只由 Linux 展示层消费，不传给严格校验的 HostOptions。目录由桌面设置写入运行配置的 `candidate_skin_catalog`：保存设置，或设置的外观页、皮肤页扫描皮肤目录时，读取状态目录下的 `skins/`，每个包保留 id、标题（清单 `name`）、清单的 `base` 与 `layouts`，以及清单声明的明暗主题下的调色板（未设任何颜色的模式是空对象）；宿主只列出 id 安全、标题非空、`base` 不是 `custom` 且布局只含 `horizontal`/`vertical` 的条目，调色板原样交给共享层解析，最多 32 个，当前选中的皮肤总在其中；IBus 与 Fcitx5 整读运行配置，上限 16 KiB，写入时整份文件超过 15 KiB 就从目录末尾删包，当前选中的皮肤保留。只把包拷进 `skins/` 而不打开设置时，宿主看不到它。启动、菜单主题选择及设置热更新均按最终选择重新解析；目录内容变化也会刷新当前展示。自定义主题的颜色选择器叠在外部皮肤之上，由共享层按设置页同一规则合成，外部皮肤的颜色不写入共享 preferences。

候选明暗 `follow` 跟随颜色模式（与 Windows 的 `theme_cand` 跟随 `theme_mode` 一致）：全局为浅色或深色时直接取用；全局为“跟随系统”（缺省值）时，才通过桌面门户的 `org.freedesktop.appearance/color-scheme` 获取系统明暗偏好，并监听后续变化；异步读取不阻塞 IBus，门户重启后重新接入。明确的 `light`/`dark` 设置优先，门户缺失或未表达偏好时使用浅色。全局主题、自定义主题与外部皮肤共用此解析；水杉、浅色、纸白、夜青、墨自带明暗，选中它们时候选按主题自身的明暗绘制。切换不会重建输入组合。接口依据 [XDG Desktop Portal Settings](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Settings.html)。

候选配色没有明确文字色时，会根据实际背景的相对亮度选择对比度更高的黑色或白色，避免系统主题与 IBus 面板主题不一致时出现深底深字或浅底浅字。有效的用户文字色和外部皮肤文字色仍优先。

Tauri 设置页按 HostCapabilities 分开暴露候选行颜色与原生卡片装饰：Linux IBus 显示候选强调色和选中行颜色，字体、字号、悬停色和边框色仍隐藏，因为 lookup table 没有对应能力。这样共享设置不会把 Linux 能消费的 RGB 颜色误判为不可用，也不会展示保存后无效的装饰选项。

运行配置通过父目录事件监听重载，连续写入合并为 100ms 后的一次读取，并每 5 秒进行低频回退读取，覆盖原子替换、删除重建、父目录替换及目录外符号链接目标更新。每次最多读取 16 KiB；相同内容不重复解析，无效中间内容保留上一次配置，后续有效保存会继续生效。

Linux 桌面未显式设置 `MSIME_CLIENT_STATE_DIR` 时，优先使用 runtime-options 的 `preferences_directory` 作为共享状态目录，保持 IBus、独立采集和桌面历史一致。面板列出历史时重新读取文件；桌面自动采集、手动同步和复制记录均通过共享偏好锁复查开关后写入，避免关闭历史后因旧检查结果继续记录。

### 按次录音选择设备

Linux 设置页的“语音输入 → 录音设备”可选择 PulseAudio、PipeWire、ALSA 或自动选择，并填写对应的设备名称。配置保存为 `preferences.voice_input.capture_backend` 和 `capture_device`；桌面语音面板与 IBus 在下一次请求中传递这两个字段，provider 为每次录音单独构造采集命令，不修改正在录制的会话。

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

Linux 安装包包含 Windows 固定提交中的开始、结束录音提示音，离线转换为 16 kHz 单声道 PCM，通过已有 PulseAudio、PipeWire 或 ALSA 播放工具输出，不增加运行时 MP3 解码依赖。`sound_enabled`、`start_sound`、`end_sound` 开关继续分别控制播放。音频来源见 `data/voice/SOURCE.md`；安装路径随自定义前缀定位。单独复制服务脚本而未带音频资源时仍使用短音回退。

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

启用剪贴板历史时，监视服务优先使用 `wl-paste --watch` 接收复制事件，减少快速连续复制被轮询漏掉的情况。每个事件通过标准输入读取最多 4096 字节并重新读取历史开关；空、清除或标记为敏感的选择不保存。关闭历史或停止服务会结束监听及其子进程。缺少工具、不支持 data-control 或监听退出时回退到原有有界轮询，失败后至少间隔 30 秒再尝试监听。协议依据 [wl-clipboard 官方手册](https://github.com/bugaevc/wl-clipboard/blob/master/data/wl-clipboard.1)。

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

IBus 与 Fcitx5 菜单顶层提供「关于水杉输入法」入口，与 Windows 托盘菜单对应，直接打开共享设置的关于页。桌面启动器也提供「关于水杉输入法」快捷操作；命令行可用 `msime-linux-settings --panel about`。宿主以 `--route=<路由>` 参数启动设置命令（关于页是 `--route=settings:about`），自定义的 `MSIME_CLIENT_SETTINGS_COMMAND` 同样收到这个参数。

IBus 桌面工具和桌面启动器提供「本地剪贴板」入口，`msime-linux-settings --panel clipboard` 可直接进入现有剪贴板历史页，使用同一份历史及搜索、复制、粘贴、删除功能。未开启历史时保留主动开启界面，不自动开启采集。该面板与其他可编辑面板一样，在通用 Wayland 粘贴前释放焦点。

IBus「剪贴板历史」菜单内可直接打开完整历史面板，即使历史关闭也能进入主动开启界面；关闭或当前输入不可用时，刷新、条目提交及清空仍禁用，菜单不展示缓存条目。菜单预览将换行和制表符压成空格，提交与删除仍使用完整原文。

IBus 的空格锁定仅在录音期间仍按住 RAlt、Ctrl+Win 或 RCtrl+RAlt 快捷键且启用 `hotkey_hold_space_lock` 时生效。菜单或 Ctrl+F9 启动的录音、松开快捷键后的锁定录音，以及识别和润色阶段，都不会把普通空格作为锁定操作吞掉；这些空格继续走编辑器/Engine 的常规输入路径。

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

表情与本地剪贴板面板的关闭按钮和首页 Esc 共用异步操作管理。复制、粘贴或修改历史期间不能同时关闭；关闭过程中也不能重复关闭或启动剪贴板操作。关闭失败保留面板并显示重试提示，旧客户端或卸载后的失败不会更新当前面板。

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

IBus“桌面工具”菜单提供持久化工具栏开关，对应 Windows 托盘中的悬浮工具栏开关。关闭后隐藏原生工具栏菜单，桌面工具入口仍可重新开启。后台线程从共享 PreferencesStore 读取最新快照，只修改 floating_toolbar.enabled，并按 revision 比较保存；成功后更新当前菜单，其他会话和桌面设置沿用偏好热重载。保存期间禁止重复提交，版本冲突或存储失败不乐观切换显示状态。未配置绝对 preferences_directory 时开关禁用。Linux 不创建脱离输入上下文的悬浮窗口，因此 Windows 工具栏拖动位置没有可迁移的坐标状态；菜单位置由 IBus 面板和桌面环境管理，不伪造或持久化位置。

IBus 菜单的云联想和候选翻译开关在配置绝对 preferences_directory 时写入共享 PreferencesStore，重启和切换输入上下文后保留，并通过已有热重载同步到桌面设置。后台保存只修改对应字段，采用 revision 比较，成功前不改变当前开关；保存失败保留原状态。与工具栏开关共用单个待保存操作，避免重复提交。未配置偏好目录的直接预览仍保留会话内临时开关行为。

翻译目标语言菜单同样使用共享偏好存储，支持英语、法语、日语、西班牙语、俄语、德语和韩语。只响应单选项的选中事件，忽略旧选项取消选中的通知；保存期间禁止重复提交，成功后通过现有偏好更新路径清除旧语言翻译并调度新请求。版本冲突或存储失败不改变选择；无偏好目录的预览保留临时语言选择。

候选明暗（跟随颜色模式、浅色、深色）、候选排列方向和预编辑显示菜单接入共享偏好保存。配置绝对 preferences_directory 时，后台按版本比较写入单个字段，成功后通过已有偏好热重载更新显示，不为外观修改主动提交当前组合或重建输入会话。保存期间禁用选项，忽略单选取消通知；失败不改变当前设置。无偏好目录的预览保留原有会话级行为。

「主题」菜单使用统一的 GlobalTheme 单选动作，先列共享层 `msime_client_theme_catalog` 给出的全局主题（顺序与标题都取自共享层，宿主不保留副本），再逐个列出已加载的外部皮肤，每个皮肤一个单选条目（Fcitx5 的皮肤条目在运行配置的皮肤目录变化时重建并注册动作名，托盘与 kimpanel 同样能直接选中任意一个）；与设置页的皮肤卡片一样，选外部皮肤即选中自定义主题并以该皮肤和其清单 `base` 绘制，选「自定义」与设置页的「自定义」卡片一样按原样选中自定义主题、不改动其中任何字段；若它正引用一个已列出的外部皮肤，菜单随后勾选的是该皮肤条目，因为画出来的就是它（不再引用皮肤要在设置页用「自定义主题不使用外部皮肤」）。已退役的旧内置皮肤 id 不被接受。点击时复查条目仍在菜单中。配置共享偏好目录时，选择通过后台 revision 比较保存，成功后热重载显示；失败保持原主题。保存期间禁用选择，取消选中通知不触发切换。无存储目录的预览继续使用原有会话级切换。

IBus 属性菜单与 Fcitx5 状态区按设计稿的顺序和文案排列：「中文」；「全角字符」「中文标点」「显示译文」；「输入方案」；「主题：<当前主题>」「词库…」「设置…」「关于水杉输入法」，各组之间以分隔线隔开。其后是语音、候选操作、九键拼音、剪贴板等随会话变化的入口，再往后把原先平铺的开关收进三个子菜单：「输入选项」（双拼键位、辅助码、繁体、中英混输、独立英文输入模式、Emoji、颜文字、云候选、九键、数字行、以词定字和快捷模式等）、「标点与翻译」（智能标点、配对、标点锁定、整句翻译及目标语言）、「候选与词频」（候选布局、每页候选数、候选明暗、预编辑、双拼与五笔提示、学习与词频）。没有删减任何入口，只改了位置。「输入方案」是全拼、双拼、五笔、粤拼、注音、日语、韩语、越南文、藏文单选项，粤拼与注音只在其词库已安装时出现；IBus 在日语、韩语、越南文、藏文与中文方案之间加分隔线，让两组单选互不牵连，Fcitx5 状态区点击「输入方案」改为展开菜单而不是循环切换。设计稿的「中文」「英文」两项都调用同一个 Shift 中英切换，所以两个前端都只放一个「中文」开关（勾选即中文、未勾选即英文），不拆成两个单选项；Engine 的独立英文输入模式（Ctrl+Shift+E）是另一项功能，留在「输入选项」里。GNOME Shell 的 IBus 面板只注册输入模式与设置，保持原有的精简菜单。Fcitx5 状态区本身不放分隔线：kimpanel 会把状态动作原样列出，分隔线在那里只会是空白项，所以分组只在子菜单内部用分隔线；kimpanel 的 IBus 后端同样把分隔线画成空白项，这是该面板的限制。子菜单中的每个条目和分隔线都向 Fcitx5 注册了动作名，托盘（SNI/dbusmenu）与 kimpanel 能看到并触发它们。Fcitx5 的托盘菜单（SNI/dbusmenu）列出、点击时作用的都是最近聚焦的输入上下文，而在 Hyprland 上的 noctalia、waybar 等桌面，点托盘本身就会让应用失焦，所以应用失焦时宿主不关它的会话，而是把这个上下文留给托盘：组字丢掉、录音和音乐停下，但会话、状态动作和勾选都还在，菜单里的开关点下去照常作用于它的会话并保存，中英文切换也记进按应用的记忆。语音、候选维护、剪贴板、云剪贴板和表情这几项只往有焦点的输入框里写，失焦时先从状态区拿掉。同一时间只留一个上下文，任何输入上下文获得焦点时它的会话才关闭，不是同一个的话连状态动作一起移除；回到同一个上下文时照旧关掉重开，重新读取运行配置。换到别的输入法时状态动作照旧移除（Fcitx5 也在每个输入法激活前自己清空这一组）。

IBus 候选表只发布可表达的行级颜色：候选边框、圆角与阴影只由 Fcitx5 classicui 主题绘制，accent 颜色只在 IBus 生效，圆角、阴影与 hover 在 IBus 中无法呈现。固定候选仅在未高亮时使用 accent，高亮后与其他候选一样优先使用选中行正文颜色。

候选数量（1–9）、候选学习开关、词频调节模式、触发次数和线性调整步长菜单在配置共享偏好目录时持久化保存，后台只修改对应字段并保留其他调频参数。成功后由共享运行时接收偏好更新，组合中的应用时机和候选排序仍归 Engine，不主动完成组合。单选取消通知及保存期间的重复点击被忽略，页大小动作仅接受一个 1–9 数字，触发次数和线性步长仅接受 1–10；失败保留原设置。未配置存储目录的预览保留会话级覆盖，切换这些选项会先结束当前组合再重建 Engine 会话。私密输入和受限字段禁用候选学习开关。

智能标点、重复标点转中文、成对标点及标点锁定菜单在配置共享偏好目录时持久化保存。重复标点转中文只取决于智能标点和它自己的开关，与成对标点补全无关，与 Windows `_CanInterceptSmartPunctuationRevert` 一致；智能标点关闭时中文标点模式下每个标点键都输出中文标点，与 Windows 在智能标点关闭时直接取标点表一致。后台按 revision 比较只写入选定字段，成功后通过共享偏好更新路径应用，保存失败保留原设置。保存期间禁止重复提交，标点锁定只响应选中通知。未配置存储目录的预览保留原会话级行为。

拼音错位纠错和邻键纠错菜单在配置共享偏好目录时持久化到对应 quanpin 字段，保留全拼其他参数与兼容纠错总开关。后台保存成功后经共享运行时更新，组合与纠错算法继续由 Engine 处理，不因菜单保存主动完成组合。保存期间禁用重复操作，失败保留原状态；无偏好目录的预览保留会话级切换。

英文、Emoji 和颜文字混合候选菜单开关在配置共享偏好目录时持久化到各自 mixed_input 字段，保留其他混合输入选项。后台保存成功后由共享运行时应用，候选生成、排序和组合更新时机仍归 Engine；不因保存主动完成组合。保存期间禁止重复操作，失败保留原设置。无偏好目录的预览保留原会话级行为。

辅助码开关和编码方案菜单在配置共享偏好目录时持久化保存。操作创建时固定当前全拼或双拼目标，只修改该方案的 enabled 或 schema 字段；异步保存期间切换输入方案不会把设置写到另一方案。保存成功后经共享运行时应用，冲突或失败保留原设置，单选取消通知不切换方案。无存储目录的预览保留会话级行为。

双拼键位方案菜单在配置共享偏好目录时持久化小鹤、自然码、首道或微软选择，只修改 shuangpin_profile，不改变当前输入方案或辅助码设置。保存成功后经共享运行时更新，组合中的应用时机由 Engine 管理，不主动完成组合；保存失败保留原选择。忽略单选取消通知并阻止保存期间的重复提交，无存储目录的预览保留原会话行为。

五笔码表版本（共享偏好 `wubi_profile`，`wubi86` 或 `wubi98`，缺省 86）只在设置页切换；IBus 输入方案菜单与 Fcitx5 状态区的五笔一项按它显示「86 五笔」或「98 五笔」，Engine 按它读写 `wubi86` 或 `wubi98` 码表与个人词条。宿主不改写这个字段，设置页同步到 runtime options 的偏好原样交给 Engine。

输入方案菜单在配置共享偏好目录时保存全拼、双拼、五笔、粤拼、注音、笔画、日语、韩语、越南文或藏文选择。选择中文方案（全拼、双拼、五笔、粤拼、注音、笔画）时在同一次 revision 比较保存中同步 last_chinese_scheme，切换日语、韩语、越南文或藏文保留该记录，“中文”入口恢复最后使用的中文方案。成功后经共享运行时更新，组合期间的切换时机由运行时与 Engine 管理；失败不改当前方案。忽略单选取消通知并阻止保存期间重复提交，无存储目录的预览保留会话切换。

韩语方案（偏好 `scheme = "korean"`，Engine 方案 4）是两套式（Dubeolsik）谚文自动机，除了下面按键打开的汉字列表没有候选，也没有云候选、学习和中文标点。两个前端都把正在组合的音节（View 的 `reading`）画成带下划线的内嵌预编辑，光标在音节末尾，预编辑样式选「不显示」时也照样显示。字母键按 Shift 决定大小写发给 Engine（Shift+Q/W/E/R/T/O/P 是 ㅃ ㅉ ㄸ ㄲ ㅆ ㅒ ㅖ），CapsLock 不改变字母，Shift+字母也不进入本地模式。开始新音节时上一个音节随同一次转换上屏；空格、回车、方向键、Home/End、Delete、数字和 Tab 等键先上屏当前音节，再把按键交还应用；标点永远是半角 ASCII，组合中与音节一次上屏，空闲时交给应用自己输入，智能标点、配对标点、连按转中文和全角都不作用于韩语；Backspace 逐个删除字母，Esc 丢弃音节，Ctrl 等快捷键先上屏音节再交给应用。失焦时音节上屏到离开的那个客户端：IBus 以 `IBUS_ENGINE_PREEDIT_COMMIT` 模式显示预编辑，由 IBus 在失焦时把它交给客户端，宿主只让会话结束组合、不再重复上屏；Fcitx5 由框架（或声明 ClientUnfocusCommit 的客户端）上屏客户端预编辑，只有不支持预编辑、音节画在面板里的客户端由宿主自己上屏。Fcitx5 切换到别的输入法时同样先上屏音节。

组合音节时按 `Hangul_Hanja`（韩国键盘的汉字键）或单独的 F9（与 ibus-hangul、fcitx5-hangul 的默认键一致；Ctrl+F9 仍是语音开关），把正在组合的这一个音节转换成汉字（`MSIME_CONVERT_HANJA`）：两个前端的候选窗都列出该音节的汉字，有训音（훈음，如「나라 이름 한」）的把训音作为该行的释义显示在汉字后面：IBus 与 Fcitx5 的候选行都只有一行文字，画不出第二行，于是 IBus 把训音放在候选翻译的位置（`韓 · 나라 이름 한, 한나라 한`），皮肤设了翻译颜色时同样着色（分段颜色是否生效取决于桌面面板），Fcitx5 把它作为单独一段、经典界面以斜体显示；训音不受「候选翻译」「英文释义」开关影响，打开这些开关且该汉字查到翻译时，翻译跟在训音后面。训音只用于显示，选中候选只上屏汉字本身。键位字母不显示，候选操作菜单不提供置顶、固定和删除。列表打开时，空格和回车提交高亮的汉字；数字行按页选择，超出本页的数字被吞掉，关闭「数字行选词」时数字先上屏谚文再交给应用，不会插在仍挂着的音节和列表前面；Tab、PageUp/PageDown 与上下方向键按导航设置翻页和移动高亮，关掉的导航键与列表外一样先上屏谚文再交给应用；Home/End 跳到首项或末项；Esc 和 Backspace 只关闭列表，音节继续组合；再按一次触发键关闭列表；输入字母关闭列表并继续组合；标点（包括平时翻页和以词定字的 `-` `=` `[` `]` `,` `.`）关闭列表，谚文与标点一起上屏；左右方向键、Delete、快捷键、切换输入法与失焦都上屏谚文。只有显式选择才上屏汉字。音节没有汉字（例如单独的辅音 ㄱ）时触发键由输入法吞掉，不会先上屏音节再把 F9 交给应用；没有组合时触发键照常交给应用。只能转换正在组合的那一个音节：已经上屏的音节和 ibus-hangul、fcitx5-hangul 那种读取光标前文本的词级转换不在本次范围内，汉字选择也不学习。汉字表来自 libhangul，编进 Engine（许可证见上文的第三方声明）。列表关闭时韩语的按键行为与上一段完全相同。

粤拼（偏好 `scheme = "cantonese"`，Engine 方案 5）与注音（`scheme = "zhuyin"`，Engine 方案 6）是中文方案，数据来自安装在资源目录旁 `language-dictionaries/` 的 `msime-cantonese.db` 与 `msime-zhuyin.db`。宿主启动时 `msime_client_refresh_host` 把这个目录写进运行时选项的 `language_dictionaries`，两个前端只在对应词库存在时列出该方案；偏好里选了词库缺失的方案时，Host API 退回最后使用的中文方案（它也不可用时退回全拼），菜单勾选与面板符号都跟随实际生效的方案。两者都直接输出繁体，「繁体」开关对它们不可用；它们不学习进主词库，候选操作菜单不提供固定和删除。粤拼的候选窗与全拼相同，组合中 `'` 是音节分隔符而不是标点，智能标点与配对标点照常。注音只支持大千键盘：数字行与 `,` `.` `/` `;` `-` 是注音键（没有组合时声调键 3、4、6、7 与空格除外），组合中空格是一声，这些键由 Engine 的 `spelling_symbols` 列出，宿主先交给 Engine，不再当作选词数字、翻页键或中文标点。注音组合时没有候选窗，组合画成内嵌预编辑（预编辑样式选「不显示」时也照样显示），光标在末尾；按下方向键、`Hangul_Hanja` 或单独的 F9（`MSIME_OPEN_CANDIDATE_LIST`）打开候选列表，列表打开后 1–9 选词，`0` 仍是注音键，下方向键移动高亮，回车提交高亮项。注音标点走中文标点表，但智能标点、配对标点和连按转中文不作用于它；方向键交还应用、快捷键、切换输入法与失焦都先上屏当前组合。

笔画（偏好 `scheme = "stroke"`，Engine 方案 9）同样是中文方案，数据来自同一目录下的 `msime-stroke.db`，与粤拼、注音一样只在词库存在时列出，缺失时按同样的规则退回。按笔顺输入：`h` 横、`s` 竖、`p` 撇、`n` 点（捺）、`z` 折，组合中 `x` 是通配符，匹配任意一笔；没有组合时 `x` 和其它字母不开始组合，交还应用，组合中其它字母被吞掉。最长 64 笔。候选只有单字，精确匹配在前、前缀补全在后，按字频排序；它直接输出词库中的字形，「繁体」开关对它不可用，不学习进主词库，候选操作菜单不提供固定和删除。组合的 `editing_text` 仍是 ASCII 字母，预编辑画的是 Engine 在 `reading` 里给出的笔画字形 `一丨丿丶乛＊`，所以「原始」预编辑样式也显示字形。候选窗与全拼相同：数字 1–9 选词，空格提交高亮项，回车上屏键入的字母，Backspace 删最后一笔，Esc 清空；`'` 不是它的输入键。标点、智能标点与配对标点照常，失焦与切换方案不上屏组合。

越南文（`scheme = "vietnamese"`，Engine 方案 7）与日语、韩语一样是非中文输入语言，切换到它时保留 last_chinese_scheme，「中文」入口恢复最后使用的中文方案。它不需要额外数据：按偏好中的 Telex 或 VNI 把按键组成带声调的越南文单词，组合本身就是要上屏的文字，没有候选，也不学习。组合画成内嵌预编辑，光标在末尾；VNI 的声调数字在组合中由 Engine 的 `spelling_symbols` 列出并交给 Engine。标点永远是半角 ASCII，全角、智能标点与配对标点都不作用于它；空格、回车、方向键、快捷键、切换输入法与失焦都先上屏当前单词再把按键交还应用。CapsLock 打开时字母照样进入组合并保持大写，与 macOS 一致，不像中文方案那样直接交给应用。

藏文（`scheme = "tibetan"`，Engine 方案 8）同样是非中文输入语言，切换到它时保留 last_chinese_scheme，默认不启用，不需要额外数据。它在拉丁字母键盘上按 EWTS（扩展威利转写）输入：组合保存当前音节串的威利原文，内嵌预编辑显示 Engine 转换出的藏文，光标在末尾，没有候选，也不学习。威利转写区分大小写，`T` `D` `N` `Sh` `A` `I` `U` `M` `H` 等大写字母是拼写，按 Shift 给出的大小写发给 Engine；CapsLock 打开时大写字母同样交给 Engine，不交给应用；威利读不了的字母（`A` `D` `H` `I` `M` `N` `R` `S` `T` `U` `W` `X` `Y` 以外的大写字母，以及小写 `q` `x`）不进组合，Engine 先上屏已有的藏文再原样写出该字母，拉丁字母不会混进转换结果。`kSh`（ཀྵ）按 EWTS 读作叠字；以 ང 结尾的音节按 `/` 时在垂符前保留音节点（ང་།）。`'`（achung 开头的音节，空闲时也算）、`+`（叠写）、`.`（消歧）、`-` 和 `/` 由 Engine 的 `spelling_symbols` 列出，宿主先作为字符交给 Engine。组合中按空格上屏藏文加音节点「་」（U+0F0B），按 `/` 上屏藏文加垂符「།」（U+0F0D），回车只上屏藏文，这三个键都由输入法吞掉，不再把空格、斜杠或换行交给应用；没有组合时 `/` 单独输出垂符，空格和数字照常交给应用，数字不转成藏文数字。Backspace 逐个删除原文按键；第一次 Esc 把预编辑退回威利原文，第二次丢弃组合。其他标点与组合一次上屏，永远是半角 ASCII，全角、智能标点与配对标点都不作用于它；方向键、Delete、Tab、快捷键、切换输入法与失焦都按显示上屏藏文（不加音节点）再把按键交还应用。

全拼九键菜单在配置共享偏好目录时保存既有 touch_keyboard_layout：开启为 nine_key，关闭为 twenty_six_key，与共享设置使用相同字段。后台按版本比较保存，成功后通过 Host API 既有布局更新路径应用，组合期间由共享运行时管理切换时机。保存失败保留原状态，保存期间禁止重复操作；无偏好目录的预览保留原会话级九键切换。

快捷模式菜单的 Unicode、日期时间、快捷短语、Emoji、颜文字、超级简拼、临时英文和临时日文开关在配置共享偏好目录时持久化到各自 local_modes 字段。后台只保存所选开关，保留其他模式，通过共享运行时更新，组合处理仍归 Engine。保存失败保留原状态，保存期间禁止重复提交；无偏好目录的预览保留会话级行为。临时日文（R 模式）在全拼或双拼、空组合时由 Shift+R 触发，显示的 R 仅是前缀；罗马字候选上屏或 Enter 原始提交后恢复原中文方案，退格可取消空前缀，关闭该开关时 Shift+R 留给应用。

数字键选词与以词定字菜单在配置共享偏好目录时分别保存 number_row_selection 和 word_character.enabled。以词定字保留既有按键选择，并由共享偏好校验处理与翻页键的冲突；冲突或写入失败不改变当前开关。保存成功后通过既有宿主偏好热重载更新按键分派，九键模式下数字选词开关仍禁用。无存储目录的预览保留会话级行为。

简繁输出菜单及工具栏入口在配置共享偏好目录时保存 traditional_chinese_output，成功后更新候选和上屏转换偏好，并通过共享存储与设置页同步。后台按 revision 比较写入，失败保留原状态，保存期间拒绝重复操作；日语方案仍不提供简繁切换。无存储目录的预览保留原会话级行为。

中文标点菜单和工具栏入口在配置共享偏好目录时保存 chinese_punctuation，保留标点锁定及智能标点配置。后台按 revision 比较写入，成功后更新共享偏好，失败保留原状态；保存期间拒绝重复操作。标点锁定仍按既有规则决定有效输出，无存储目录的预览保留会话级切换。

菜单设置保存期间，“桌面工具”显示保存中状态；存储失败或版本冲突后显示“设置未保存，点击重试”。重试重新读取最新偏好，只重放上次选择的字段并再次比较 revision，不覆盖其他并发修改。提示不展示原始错误、路径或输入内容；更换运行配置后旧重试入口失效，新的菜单修改替代旧重试记录。

菜单保存状态按共享代次同步到当前焦点上下文，即使保存由另一个编辑器发起、保存失败未改变偏好文件，或偏好读取仍在进行，也会在既有刷新周期更新保存中、可重试及菜单可用状态。运行配置变更同样使旧重试提示刷新失效，不额外启动轮询线程。

候选操作菜单现在根据焦点、输入启用状态和当前会话动态标记可用性；失焦、密码输入或会话尚未建立时，固定、删除、定位和取消固定动作会整体禁用，避免向已失效的 Engine 身份发送操作。候选来源和代次校验仍由宿主与 Engine 共同执行。

全角/半角输出菜单和工具栏入口在配置共享偏好目录时保存 character_width（fullwidth 或 halfwidth），成功后应用共享偏好并把宽度同步到正在运行的会话，宿主的空闲 ASCII 转换与会话上屏的组合文本保持同一宽度。失败保留原模式，保存期间禁用重复操作；无存储目录的预览保留会话级切换。

候选皮肤目录或运行配置热更新后，宿主在发布 IBus 菜单前重新验证会话级主题覆盖；覆盖引用的外部皮肤已移除时自动清除覆盖并回退到共享配置，避免菜单显示或渲染引用失效资源。

切换全拼、双拼、五笔或日语方案时，宿主会清除旧方案的会话级辅助码、纠错、九键和双拼方案覆盖，再创建新 Engine 会话读取共享偏好；共享文件本身不被改写。这样从双拼切到日语或五笔不会遗留旧方案菜单状态。

桌面工具菜单增加“启用语音输入”开关，持久化 voice_input.enabled；关闭时禁用语音快捷键与录音入口，活动录音由既有取消流程结束。保存成功后共享偏好热重载同步，失败保留原状态。

候选翻译目标语言的每个 IBus 单选项现在与父菜单共享同一 provider、会话、焦点和保存中状态；provider 消失或保存进行中时，子项也同步禁用，避免用户点击看似可用但不会执行的语言选择。

标点锁定的跟随、固定中文、固定英文子菜单现在与父菜单共享焦点、输入启用、活动会话和保存中状态；失焦、密码输入、会话尚未建立或保存进行中时，子项同步禁用，避免无效切换。

候选布局、每页候选数和词频调节的子菜单项现在与父菜单共享焦点、会话、输入启用和保存中状态；失焦或保存进行中时统一禁用，避免无效选择。

Linux key-router 现在使用与 Windows/Host API 一致的 dispatch outcome：只有 `DEFINITELY_NOT_SENT` 允许宿主执行本地 fallback；`DELIVERY_AMBIGUOUS` 必须等待 lease 恢复，不能重复注入按键。IBus focus-in 在 Engine 会话建立后安装 lease，focus-out 只撤销精确匹配的当前 lease。

IBus 与 Fcitx5 也保留 Backspace 长按的物理按键所有权：若首次按下发生在活动组字中，重复键可以继续删除预编辑；最后一个组字字符消失后，同一次按住的后续重复仍由输入法吞掉，不会开始删除编辑器里已经上屏的正文。按键松开、失焦、reset 或按下其他键都会解除所有权，之后一次新的 Backspace 仍正常交给编辑器。实现复用纯策略，但输入事件仍分别走 IBus 与 Fcitx5 的原生入口。

字典导入沿用 Windows 固定提交 `6e03f577` 的全拼规则：普通字典和个人字典的 Pinyin 词条在 Host API 中调用固定 Engine 的全拼切分，并按词条汉字数解决无分隔拼音歧义，例如两字词 `西安` 的 `xian` 规范化为 `xi'an`。无效音节、非法 apostrophe 或无法匹配词长的行会被跳过并报告；Wubi、快捷短语和英文导入不经过该规范化。规范化只发生在带 Engine 选项的实际导入请求中，Linux IBus 不复制输入算法。
