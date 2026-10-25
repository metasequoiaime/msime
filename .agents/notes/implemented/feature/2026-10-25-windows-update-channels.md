# Agent Note: Windows 的更新通道与常驻进程后台提醒

Status: implemented

## Problem

桌面三个平台加了更新通道（稳定版、测试版、每日构建）和客户端主动提醒，规则与后台检查已经下沉到共享 Rust（[core 切片](../architecture/2026-10-25-update-channels-core.md)）。Windows 这边原来只有原生设置窗口「关于」页的手动「检查更新」，只认正式版；常驻进程（Server）从不检查，用户不打开设置窗口就永远不知道有新版本。Windows 的「托盘菜单」其实是 TSF 语言栏菜单，Server 没有自己的通知区域图标，也就没有现成的地方弹提醒。

## Decision

### 设置窗口

- 「关于」页「版本更新」标签的「版本与更新」组在「当前版本」和结果块下面加「更新通道」下拉框（稳定版、测试版、每日构建），照其他偏好行经 `PreferencesDocument` 写 `update_channel`。读的时候按 `common/UpdateChannelText.h` 归一：只认三种写法，其余按稳定版，与 client-core 读文档的宽松规则一致，所以请求里永远是合法的通道。
- 「检查更新」的请求带 `channel`；结果里构建的称呼照共享页的 `describeUpdateVersion`（`AboutPageModel.h` 的 `update_build_title`）：正式版与以前一样只写「v0.3.1」，测试版和每日构建写明通道（「发现新版本 测试版 v0.3.1」「水杉 IME 每日构建 v0.3.1（构建 123）已发布。」，括号里是回答的 `build_run`）。回答里没有 `channel`、`build_run` 时按正式版、没有运行 ID（旧共享层）；`channel` 不是三种之一时按检查失败。说明文字也与共享页相同。
- 下载按钮对正式版仍是开官网下载页的「前往下载」，对测试版和每日构建是开回答里发布页的「前往发布页」：官网只列正式版。国内镜像照原规则从发布页和安装包名拼，nightly 的 tag 和带 `-nightly.<run>.<attempt>` 的安装包名都合形状。
- 换通道时上一次的结果作废；检查和换通道各加一次 `update_generation_`，在路上的检查回来时代次不对就丢掉，旧通道的结果不会盖住新通道。
- 「版本更新」标签画出来、还没有结果时，在工作线程上调 `msime_client_update_pending`（目录是本用户的更新记录目录，与 Server 写的是同一个，见下），显示 Server 后台检查已经找到、仍比本版本新、按当前通道找到的构建。点 Server 的通知打开这一页时用户马上看到是哪个版本，不必再点一次检查。
- Windows 7 专用设置（`settings/win7/`）不变：不加通道，请求不带 `channel`，共享层对 `windows7` 也一律按稳定版。

### Server 的后台检查

- `src/system/UpdateNotifier.cpp` 只在受管理的 Server 里创建（预览实例不联网、不弹通知）。工作线程启动五分钟后第一次调用 `msime_client_update_background_check`，之后每小时一次。偏好里的通道（随偏好发布更新到一个加锁的字符串）每分钟看一次，变了以后把下一次调用推到一小时（`update_channel_settle`）之后，已经更晚的不提前：换通道几乎总在设置窗口「关于」页上做，用户多半当场点了「检查更新」，页上已经写着那个构建，马上再弹一条系统通知说同一个版本是重复的；一小时后还没装，再提醒才有用。到时共享层见到和记录不同的通道会立刻重查，不受一天一查的节流限制。到没到时间、成功一天失败六小时的节流、只提醒一次、开发构建和 `MSIME_DISABLE_UPDATE_CHECK` 全在共享层，Server 不重复；共享层回答 `disabled` 时线程结束。
- 请求的平台是 `windows`，带通道、不带架构，与设置窗口相同（同一个安装程序同时支持 x64 和 Arm64）。Win7 构建（`MSIME_WINDOWS7_CLIENT`）是 `windows7`，不带通道，但带本程序编译成的架构（`update_build_architecture()`，`x86_64` 或 `x86`）：共享层的 `windows7` 分支对空或未知架构一律跳过（[Windows 7 专用发布](2026-10-24-windows7-update-channel.md)），不带它就永远找不到发布；Win7 专用设置的手动检查一直带它。
- 记录目录是本用户的 `%LOCALAPPDATA%\<本版本的用户目录>\update`（`common/UpdateRecordDirectory.h`，Server 和设置窗口共用这一个函数），与匿名账号目录同一个父目录，每个版本各一份。不放状态根：安装后的状态根是安装器的 HKLM `DataDir`，整台机器一个目录，几个 Windows 用户共用一份 `notified_version`/`notified_run` 时，先登录的用户被提醒过的构建，后登录的用户永远收不到通知。拿不到这个目录时不起后台检查，只记一条诊断。请求、回答解析和通知文字在不碰 Windows API 的 `src/system/UpdateNotification.h`。
- 联网只在这个工作线程上。有要提醒的构建时，线程把它放进共享状态、`PostMessage` 给消息循环线程上的一个不显示的窗口；窗口用 `Shell_NotifyIconW` 加一个临时图标（先加图标、设 `NOTIFYICON_VERSION_4`，再用 `NIM_MODIFY` 弹出 `NIF_INFO` 气泡，保证回调从第一条起就是新格式），Windows 10/11 显示成通知，Windows 7 显示成气泡。标题「<产品名>有新版本 vX」，产品名是 `MSIME_EDITION_DISPLAY_NAME`（几个版本同时安装时分得清是哪个），正文写明通道和构建，写法与设置窗口共用 `UpdateChannelText.h`。
- 共享层返回 `notify` 时已经把这个构建记成提醒过，宿主弹不出、用户没看见就再也不会提醒，所以宿主自己保证这一条不丢：弹出前查 `SHQueryUserNotificationState`，不是 `QUNS_ACCEPTS_NOTIFICATIONS`（全屏游戏或演示、锁屏、新用户登录后的安静时间、全屏商店应用）时先留在内存里，每分钟再看一次，接受了再弹；最多等两小时，到时照样弹出，因为 `QUNS_BUSY` 有误报（`FullscreenForeground.h`），无限等会让某些机器永远不提醒。等待中又来一个新构建时换成新的，计时从最早那个算起。
- `NIN_BALLOONUSERCLICK`、`NIN_SELECT`、`NIN_KEYSELECT` 打开设置窗口的「关于」页，走托盘「关于水杉输入法…」同一条 `settings:about` 路由（`ShellSurfaces.h` 的 `OpenAbout`）；设置窗口起不来时用浏览器打开回答里的发布页（只收本仓库 `releases/tag/` 下、字符合形状的地址）。点过以后删掉图标。`NIN_BALLOONTIMEOUT`、`NIN_BALLOONHIDE` 不删图标：Windows 10/11 删掉图标会连操作中心里的那条通知一起删掉，横幅五秒没点、被勿扰或专注助手直接压进操作中心的通知都靠图标留着才还在；Windows 7 的气泡收起后，用户也还能点图标。右键图标只删掉图标（提示里写着「点击查看，右键移除此提醒」）；都不理会时一天后删掉，那时共享层早已不会再为它提醒，「关于」页仍显示它。窗口从不显示，带 `WS_EX_TOOLWINDOW`，不抢焦点。
- 退出时主循环之后第一步停它：设停止标志、唤醒线程，最多等两秒；线程正在联网时 `detach`，它只持有 `shared_ptr` 的共享状态，回来后看见停止标志就退出，不再碰已经销毁的窗口；诊断日志本来就有意不析构，线程晚回来写日志也安全。进程退出时操作系统收走线程和共享层的文件锁，记录文件由共享层原子替换，不会半写。

### 预发布的下载指引

`platforms/windows/scripts/edition_windows.py` 给带后缀的预发布写的那句改成：稳定版通道不提示它，把更新通道换成测试版或每日构建后才会提示 develop 发出的测试版，`-alpha` 一律不提示。

### 既有笔记审计

- [更新通道 core 切片](../architecture/2026-10-25-update-channels-core.md)：部分重叠，本篇是它的 Windows 宿主部分，契约以那篇为准。
- [Windows 设置「关于」页标签](2026-10-20-windows-settings-about-tabs.md)：部分重叠，「版本与更新」组多一行、结果措辞与「前往下载」的去向按通道变；已在那篇加回链。
- [Windows 7 专用发布](2026-10-24-windows7-update-channel.md)：部分重叠，Win7 设置不加通道，Win7 Server 的后台检查请求 `windows7`。
- [Windows 发布页的下载指引](../process/2026-10-20-windows-release-page-symbols.md)：其中「检查更新只提供正式版」一句的事实已就地改成按通道的说法。

## Alternatives considered

- **在 Server 里常驻一个真正的通知区域图标，提醒挂在它的菜单上**：有了图标，提醒错过了也能回头点。但 Windows 上水杉的入口是 TSF 语言栏，再加一个常驻图标就是两套入口，用户要多管一个托盘图标；提醒是一年几次的事，用临时图标：只在有提醒时出现，点过、右键或一天后删掉，删掉后的提醒在「关于」页仍能看到（`msime_client_update_pending`）。
- **用 WinRT 的 `ToastNotificationManager` 发原生通知**：Windows 10/11 上外观最好，能放按钮。但未打包的 Win32 进程要先注册 AppUserModelID 和开始菜单快捷方式，安装器要跟着改，Windows 7 又完全没有；`Shell_NotifyIconW` 气泡在两边都能用，Windows 10/11 自动显示成通知，代价是只能有标题和一段正文。
- **由设置窗口做后台检查**：设置窗口是 WinUI 程序，界面和检查放一起最简单。但它只在用户打开时运行，Linux 那种「打开设置才检查」在 Windows 上等于不提醒；常驻的只有 Server。
- **工作线程直接 `sleep` 到下一个整点**：代码最少。但换通道要等最多一小时才生效，而且停止时要么等它醒、要么强行打断；用条件变量按分钟醒来看停止标志和通道，退出和换通道都能及时响应。
- **记录放 Server 的状态根**：设置窗口本来就知道这个目录，不用另找。但安装后的状态根是整台机器共用的 HKLM `DataDir`，「只提醒一次」变成整台机器只提醒一个用户；偏好 `update_channel` 同样在状态根里、按机器生效，那是偏好存放位置的既有约定，记录不必跟着它。
- **换通道后一分钟内马上检查**：通道生效最快。但用户正在「关于」页上，多半刚手动检查过，紧接着弹出的通知说的是页上已经显示的那个构建。也考虑过让设置窗口的手动检查把结果记成提醒过，那要共享层新增一个写记录的入口，而且手动检查和后台检查的记录语义会混在一起；推后一小时只改宿主的节奏。
- **气泡超时或被关掉就删图标**：通知区域里不留任何东西。但共享层已经把构建记成提醒过，Windows 10/11 上删图标连操作中心的那条也删，用户五秒内没点、或者开着勿扰时，这次提醒就彻底丢了。
- **一直等到系统接受通知再弹**：最尊重用户的勿扰状态。但 `QUNS_BUSY` 在部分机器上误报，会让提醒永远弹不出来；两小时的上限加上图标保留，最坏也落在操作中心里。
- **退出时一直 `join` 工作线程**：最干净，不留任何线程。但它在联网时最多要十几秒才回来，Server 退出（包括升级安装时被结束、看门狗重启）会被拖住；等两秒、过时放手，线程只碰自己的共享状态，安全且有界。
- **点通知时直接打开发布页或官网**：不依赖设置窗口。但设置窗口能显示安装包的校验值、未签名警告和国内镜像，比 GitHub 页面对用户更有用；只在设置窗口起不来时退回发布页。
- **未知的通道在设置窗口里原样显示成第四个选项**：与「每页候选项数量」那样保留越界值的做法一致。但通道的取值集合由 client-core 封闭定义，文档里出现未知值时 Rust 已经按稳定版读、保存时写回缺省，界面跟着显示稳定版才和实际生效的一致。

## Consequences

- **收益**：Windows 用户不打开设置窗口也能收到新版本提醒，选了测试版或每日构建的用户能拿到对应通道的构建；节流和只提醒一次与 Linux、macOS 是同一份 Rust 规则；点提醒直接落在显示校验值和镜像的「关于」页。
- **网络与隐私**：受管理的 Server 每天最多成功联网一次（失败后六小时重试），请求是不带凭据的 GitHub 发布列表，每日构建通道另外 GET `releases/tags/nightly`；PRIVACY.md 的「资源与更新下载」已随 core 切片补上这些请求。
- **代价与已知上限**：
  - 用户不点也不右键时，图标会在通知区域（Windows 11 默认在溢出区）留到一天；Server 退出时随之消失。
  - `SHQueryUserNotificationState` 报不出专注助手的状态，被它压下的通知不会推迟，而是直接进操作中心，靠图标保留住。
  - 推迟中的提醒只在内存里，Server 在弹出前退出（注销、升级、看门狗重启）就丢了，共享层不会再提醒这个构建；「关于」页仍显示它。
  - 换通道后最早一小时才有后台提醒；想马上知道要在「关于」页手动检查。
  - Windows 7 构建只走稳定版，不能选通道。
  - 记录目录跟着 `%LOCALAPPDATA%`，不跟 `DataDir` 或数据目录环境变量；后台检查还没发布过，没有放在状态根里的旧记录要迁移。目录不可写时共享层返回存储错误，Server 每小时照常再试，只在第一次失败时记一条诊断。
  - 共享层哪天改了回答形状（`notify` 或 `update` 里的字段），`UpdateNotification.h` 和 `main.cpp` 的解析要跟着改；两边都容忍多出来的字段。

## Verification

- `windows-update-notification`（`tests/system/update_notification.cpp`，主机构建也编）：偏好里的通道归一、普通构建（带通道不带架构）和 Win7 构建（不带通道、带 x86_64 或 x86、认不出的架构不带）的请求形状、`disabled`/`not_due`/`unavailable`/`checked` 与失败的回答、每日构建和测试版的提醒、旧共享层没有 `channel` 和 `build_run`、版本号或通道不合形状时不提醒、外仓或空 tag 的发布页不用、负的运行 ID、通知标题按产品名、正文、图标提示和定长字段的长度、节奏常量（换通道后至少等一个检查间隔）、推迟弹出的判断（系统接受时立刻、不接受时等到上限）。
- `windows-about-page-model`（`tests/ui/about_page_model.cpp`）：通道选项、两种宽度字符的归一、三个通道的构建称呼、结果句、下载按钮的去向和文字、nightly 发布页的镜像地址。
- `tests/tools/settings_ui.ps1` 核对「版本更新」标签里有「更新通道」。
- `main.cpp`（WinUI）只能在 Windows CI 上编；`UpdateNotifier.cpp` 和 `server_main.cpp` 用本机的 MinGW 做过 `-fsyntax-only -Wall -Wextra -Werror` 检查（普通与 Win7 两种宏；`UpdateNotifier.cpp` 另用 i686 编过），没有链接、没有在 Windows 上运行，气泡、推迟弹出、超时后图标和操作中心条目是否保留、点击打开「关于」页和退出时的线程收尾都还没有实机验证。
