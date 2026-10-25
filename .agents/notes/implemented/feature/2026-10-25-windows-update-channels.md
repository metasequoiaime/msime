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
- 「版本更新」标签画出来、还没有结果时，在工作线程上调 `msime_client_update_pending`（目录是窗口的数据目录，即 Server 的状态根），显示 Server 后台检查已经找到、仍比本版本新、按当前通道找到的构建。点 Server 的通知打开这一页时用户马上看到是哪个版本，不必再点一次检查。
- Windows 7 专用设置（`settings/win7/`）不变：不加通道，请求不带 `channel`，共享层对 `windows7` 也一律按稳定版。

### Server 的后台检查

- `src/system/UpdateNotifier.cpp` 只在受管理的 Server 里创建（预览实例不联网、不弹通知）。工作线程启动五分钟后第一次调用 `msime_client_update_background_check`，之后每小时一次；偏好里的通道（随偏好发布更新到一个加锁的字符串）变了时，一分钟内不等到点再调用一次，共享层见到和记录不同的通道会立刻重查。到没到时间、成功一天失败六小时的节流、只提醒一次、开发构建和 `MSIME_DISABLE_UPDATE_CHECK` 全在共享层，Server 不重复；共享层回答 `disabled` 时线程结束。
- 请求的平台是 `windows`，Win7 构建（`MSIME_WINDOWS7_CLIENT`）是 `windows7` 且不带通道；版本 id 和不带架构的写法与设置窗口相同；目录是 `config.state_root`，与设置窗口读缓存的目录相同，每个版本（edition）各有自己的状态根。请求、回答解析和通知文字在不碰 Windows API 的 `src/system/UpdateNotification.h`。
- 联网只在这个工作线程上。有要提醒的构建时，线程把它放进共享状态、`PostMessage` 给消息循环线程上的一个不显示的窗口；窗口用 `Shell_NotifyIconW` 加一个临时图标（先加图标、设 `NOTIFYICON_VERSION_4`，再用 `NIM_MODIFY` 弹出 `NIF_INFO` 气泡，保证回调从第一条起就是新格式），Windows 10/11 显示成通知，Windows 7 显示成气泡。标题「水杉输入法有新版本 vX」，正文写明通道和构建，写法与设置窗口共用 `UpdateChannelText.h`。
- `NIN_BALLOONUSERCLICK`、`NIN_SELECT`、`NIN_KEYSELECT` 打开设置窗口的「关于」页，走托盘「关于水杉输入法…」同一条 `settings:about` 路由（`ShellSurfaces.h` 的 `OpenAbout`）；设置窗口起不来时用浏览器打开回答里的发布页（只收本仓库 `releases/tag/` 下、字符合形状的地址）。`NIN_BALLOONTIMEOUT`、`NIN_BALLOONHIDE`、右键图标、或者六分钟内没有任何回音（例如气泡被勿扰压下）时删掉图标。窗口从不显示，带 `WS_EX_TOOLWINDOW`，不抢焦点。
- 退出时主循环之后第一步停它：设停止标志、唤醒线程，最多等两秒；线程正在联网时 `detach`，它只持有 `shared_ptr` 的共享状态，回来后看见停止标志就退出，不再碰已经销毁的窗口；诊断日志本来就有意不析构，线程晚回来写日志也安全。进程退出时操作系统收走线程和共享层的文件锁，记录文件由共享层原子替换，不会半写。

### 预发布的下载指引

`platforms/windows/scripts/edition_windows.py` 给带后缀的预发布写的那句改成：稳定版通道不提示它，把更新通道换成测试版或每日构建后才会提示 develop 发出的测试版，`-alpha` 一律不提示。

### 既有笔记审计

- [更新通道 core 切片](../architecture/2026-10-25-update-channels-core.md)：部分重叠，本篇是它的 Windows 宿主部分，契约以那篇为准。
- [Windows 设置「关于」页标签](2026-10-20-windows-settings-about-tabs.md)：部分重叠，「版本与更新」组多一行、结果措辞与「前往下载」的去向按通道变；已在那篇加回链。
- [Windows 7 专用发布](2026-10-24-windows7-update-channel.md)：部分重叠，Win7 设置不加通道，Win7 Server 的后台检查请求 `windows7`。
- [Windows 发布页的下载指引](../process/2026-10-20-windows-release-page-symbols.md)：其中「检查更新只提供正式版」一句的事实已就地改成按通道的说法。

## Alternatives considered

- **在 Server 里常驻一个真正的通知区域图标，提醒挂在它的菜单上**：有了图标，提醒错过了也能回头点。但 Windows 上水杉的入口是 TSF 语言栏，再加一个常驻图标就是两套入口，用户要多管一个托盘图标；提醒是一年几次的事，用临时图标弹完即删，错过的提醒在「关于」页仍能看到（`msime_client_update_pending`）。
- **用 WinRT 的 `ToastNotificationManager` 发原生通知**：Windows 10/11 上外观最好，能放按钮。但未打包的 Win32 进程要先注册 AppUserModelID 和开始菜单快捷方式，安装器要跟着改，Windows 7 又完全没有；`Shell_NotifyIconW` 气泡在两边都能用，Windows 10/11 自动显示成通知，代价是只能有标题和一段正文。
- **由设置窗口做后台检查**：设置窗口是 WinUI 程序，界面和检查放一起最简单。但它只在用户打开时运行，Linux 那种「打开设置才检查」在 Windows 上等于不提醒；常驻的只有 Server。
- **工作线程直接 `sleep` 到下一个整点**：代码最少。但换通道要等最多一小时才生效，而且停止时要么等它醒、要么强行打断；用条件变量按分钟醒来看停止标志和通道，退出和换通道都能及时响应。
- **退出时一直 `join` 工作线程**：最干净，不留任何线程。但它在联网时最多要十几秒才回来，Server 退出（包括升级安装时被结束、看门狗重启）会被拖住；等两秒、过时放手，线程只碰自己的共享状态，安全且有界。
- **点通知时直接打开发布页或官网**：不依赖设置窗口。但设置窗口能显示安装包的校验值、未签名警告和国内镜像，比 GitHub 页面对用户更有用；只在设置窗口起不来时退回发布页。
- **未知的通道在设置窗口里原样显示成第四个选项**：与「每页候选项数量」那样保留越界值的做法一致。但通道的取值集合由 client-core 封闭定义，文档里出现未知值时 Rust 已经按稳定版读、保存时写回缺省，界面跟着显示稳定版才和实际生效的一致。

## Consequences

- **收益**：Windows 用户不打开设置窗口也能收到新版本提醒，选了测试版或每日构建的用户能拿到对应通道的构建；节流和只提醒一次与 Linux、macOS 是同一份 Rust 规则；点提醒直接落在显示校验值和镜像的「关于」页。
- **网络与隐私**：受管理的 Server 每天最多成功联网一次（失败后六小时重试），请求是不带凭据的 GitHub 发布列表，每日构建通道另外 GET `releases/tags/nightly`；PRIVACY.md 的更新段落要随之补上（不在 Windows 切片的文件范围内，见 core 切片的未决事项）。
- **代价与已知上限**：
  - 气泡超时后图标被删，Windows 10/11 操作中心里的那条通知也随之消失；错过的提醒只能在「关于」页看到。
  - 气泡被勿扰或专注助手压下时共享层已经把这个构建记成提醒过，不会再弹；同样只能在「关于」页看到。
  - Windows 7 构建只走稳定版，不能选通道。
  - 状态根不可写时共享层返回存储错误，Server 每小时照常再试，只在第一次失败时记一条诊断。
  - 共享层哪天改了回答形状（`notify` 或 `update` 里的字段），`UpdateNotification.h` 和 `main.cpp` 的解析要跟着改；两边都容忍多出来的字段。

## Verification

- `windows-update-notification`（`tests/system/update_notification.cpp`，主机构建也编）：偏好里的通道归一、普通构建和 Win7 构建的请求形状、`disabled`/`not_due`/`unavailable`/`checked` 与失败的回答、每日构建和测试版的提醒、旧共享层没有 `channel` 和 `build_run`、版本号或通道不合形状时不提醒、外仓或空 tag 的发布页不用、负的运行 ID、通知标题和正文、节奏常量。
- `windows-about-page-model`（`tests/ui/about_page_model.cpp`）：通道选项、两种宽度字符的归一、三个通道的构建称呼、结果句、下载按钮的去向和文字、nightly 发布页的镜像地址。
- `tests/tools/settings_ui.ps1` 核对「版本更新」标签里有「更新通道」。
- `main.cpp`（WinUI）只能在 Windows CI 上编；`UpdateNotifier.cpp` 和 `server_main.cpp` 用本机的 MinGW 做过 `-fsyntax-only -Wall -Wextra -Werror` 检查（普通与 Win7 两种宏），没有链接、没有在 Windows 上运行，气泡、点击打开「关于」页和退出时的线程收尾都还没有实机验证。
