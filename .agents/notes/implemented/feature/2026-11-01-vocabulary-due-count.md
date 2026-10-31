# Agent Note: 背单词的待复习计数与「不在打字时复习」边界的收窄

Status: implemented

## Problem

背单词的间隔复习六端都有，但「今日待复习」只出现在设置里的背单词页上。不打开设置的用户看不到今天有卡在等，排程排得再准，卡也只会越积越多。要在设置以外提醒，有两处挡着：

- `crates/client-services/src/vocabulary.rs` 的模块注释把边界写成「复习是用户打开的东西，不在用户打字时发生」，只留了「宿主显示卡片、报告作答、读回进度行的计数」这一个面。在输入法自己的菜单里显示一个数，按字面就越过了这条线，后来的人没法判断它是有意的还是越界。
- 唯一的接口是 `msime_client_vocabulary_review` 的 `load`。它读全部内置书（八本约 3 MB）、扫插件目录、列导入书目，再把整条队列的词条拷进应答；而且一进来就取进度锁，没有状态目录时会建目录和锁文件。给页面用没问题，给每次点开菜单都要问一次的地方用太重，还会让从没开过背单词的用户因为点开输入法菜单就多出文件。

打扰也是现实的顾虑：#7599 等反馈都在说功能默认开着、不请自来很烦。

## Decision

**边界收窄为：复习本身仍然只在用户打开的背单词页里发生；宿主可以在候选窗以外被动地显示今日待复习的张数，但组字期间从不显示，也不进候选窗。** 「被动」指用户自己点开才看得到的地方：菜单栏状态项的菜单、托盘菜单、IBus/Fcitx 的输入法菜单这一类。不弹窗、不发系统通知、不出声，也不在打字的任何时刻冒出来；点这一项只是打开现有的背单词页。模块注释已经照这句改写。

**计数接口：**

- 共享层 `vocabulary::session::due_count(directory, resources, plugins, day) -> Result<usize, ReviewSessionError>`。返回值与 `ReviewStatus.due`（页面上的「今日待复习」）是同一个数，包括今天会话要出的新卡，含义见 [背单词进度行的计数](../bug-fix/2026-10-20-vocabulary-progress-row-counts.md)。排程仍交给 `build_queue`，只是不组装书目和卡片。
- 只读进度文档和选中的那一本书。内置书经新增的 `builtin::load_one` 只打开那一个文件；导入书和插件书沿用 `WordbookLibrary::load`、`wordbook_pack::load_book`。
- `VocabularyProgressStore::load_existing`：进度文件不存在时返回 `None`，不建目录、不建锁文件；存在时照 `load` 在锁内读，与写入方互斥。
- 没有进度文件、没选书、选中的书已经不在时是 0。进度文档损坏、选中的书读不出来时报错，不当成 0。
- C ABI `int32_t msime_client_vocabulary_due_count(request, length)`，请求 `{directory,resources,day,plugins?}`，字段含义与 `msime_client_vocabulary_review` 相同、`deny_unknown_fields`。手里只有 HostOptions 的宿主可以不给 `resources`，改给 `host_options`（HostOptions 文档），资源根按 `bundled_resource_root` 求出，两者恰好给一个。返回张数，请求不合法或读不出来时返回 -1，宿主据此隐藏这一项。声明在 `crates/host-api/include/msime_client.h`。

`selected_book` 的内置书分支顺带改用 `load_one`：作答时不再为了一本书读八本。`apply` 的结果不变，因为作答之后组装状态时 `status_with` 仍读全部内置书，某本内置书损坏照样报出来。

## 宿主上的入口

三个桌面宿主在各自的输入法菜单里加一行「背单词（今日待复习 N）…」，文案、显示条件和要打开的面板只写在 `platforms/common/VocabularyDueMenu.h`：张数不大于 0（包括 -1）或正在组字时不显示；点它按 `vocabulary` 路由（client-core 的 `SurfaceRoute::Vocabulary`）打开现有的背单词面板；日期由宿主按本机时区给出。移动端键盘这一轮不加。

- **macOS**：输入菜单（IMK 的 `menu`，即菜单栏里输入法图标下的菜单），排在「开始/结束语音输入」之后。菜单在用户点开时现建，张数这时现读；组字时（`editing_text` 或候选不为空）连读都不读。资源根是设置应用的 `Contents/Resources`（经 LaunchServices 找 `MSIMEClientApplicationIdentifier`），找不到时退回输入法自己的资源目录。菜单栏状态项（`InputModeStatusItem`）只是一个模式图标、默认不显示，也没有菜单，不在它上面加数字：那会变成一直挂在菜单栏上的角标，不再是被动的。
- **Windows**：托盘卡片（`TrayMenuLayout.h`），排在「云词库…」之后，只在共享应用在场（`TrayMenuCapabilities::vocabulary_panel`）时列出。Server 在卡片每次打开时现读，候选窗开着（正在组字）时不读不列；悬浮工具栏的方案菜单不读。目录与设置窗口相同：状态目录、Server 所在目录、状态目录下的 `plugins`。
- **Linux**：IBus 属性菜单与 Fcitx5 状态区，排在语音之后、设置之前。张数只在获得焦点时（Fcitx5 还有失焦留给托盘时）读一次并记在会话状态上，之后的属性更新、状态区重排都用这份，不在按键路径上读盘。资源根经 `host_options` 按 HostOptions 求，词库在线更新后仍和设置应用找的是同一处。IBus 始终注册这个属性（包括 GNOME Shell 的精简菜单），没有待复习、正在组字、失焦或受限时设成不可见，属性树的形状不随计数变化；Fcitx5 的动作没有可见性，不显示时从状态区拿掉。

## 相关笔记

- [背单词进度行的计数](../bug-fix/2026-10-20-vocabulary-progress-row-counts.md)：部分重叠。那篇定义了 `due` 包括新卡，本篇的计数沿用它，不另起定义。
- [背单词的文件读取和确认绑定客户端代次](../bug-fix/2026-10-15-vocabulary-action-owner.md)、[Windows 7 独立背单词面板](2026-10-24-windows7-vocabulary-panel.md)：无关，讲的是页面和面板内部的状态归属。

## Alternatives considered

- **宿主直接调 `load`，取应答里的 `due`** — 最强的理由是零新接口，而且数字和页面天然一致。不用：每次点开菜单都要读约 3 MB 的内置书、扫插件目录、拷出整条队列，还会给没用过背单词的用户建目录和锁文件。一致性靠共用 `build_queue` 一样能保证，测试逐个场景断言两者相等。
- **只数积压的到期卡，不算新卡** — 最强的理由是 Anki 也把新卡和到期卡分开，而且选过一次词书就不再背的用户不会天天看到「20」。不用：页面上的「今日待复习」包括新卡，菜单写 0、点进去页面写 20 是读不通的；这个分歧正是进度行那篇笔记修掉的问题。对不再背的用户，宿主只在菜单里显示，不主动打扰。
- **不读书，用「词书总数 − 已学张数」估新卡** — 最强的理由是连一本书都不用解析（最大的 GRE 不到 1 MB）。不用：内置书的总数本身就要读书才知道；插件升级删掉了词时，已学张数里有书里已经没有的词，估出来的数会和页面不一致。
- **在菜单栏状态项上画数字角标** — 最强的理由是不用点开菜单就能看到，提醒效果最好。不用：那是一直挂在屏幕上的提醒，违背「被动、用户自己点开才看得到」这条边界；状态项本身默认不显示，大多数用户也看不到。
- **加一个「在菜单里显示待复习」开关，默认关** — 最强的理由是新功能默认关的惯例，#7599 也在说默认开着的功能打扰人。不用：只有选过词书的用户才会看到这一行，选词书本身就是开启背单词的动作；一行只在点开菜单时出现的文字，代价比六端各加一个设置项、再接偏好覆盖检查小得多。有用户反馈它打扰时再加。
- **在作答时把计数写进一个小文件，宿主读它** — 最强的理由是读起来最便宜。不用：计数过了午夜就会变，而那时没有任何写入，文件里的数第二天一早就是错的。
- **沿用 `{ok,value}` 的 JSON 应答** — 最强的理由是和大多数入口一致，错误带原因。不用：三类原生宿主只要一个数，错误时一律隐藏，原因对它们没有用；`msime_client_typing_statistics_enabled` 已经是同样形状的整数返回。

## Consequences

- **收益**：宿主不用自己算待复习，菜单上的数和页面上的数由同一段排程算出；没用过背单词的用户什么文件也不会多。
- **代价与已知上限**：每次调用仍要取进度锁并解析一本书，宿主只应在打开菜单时或工作线程上调，不能每个按键都调。宿主要自己给出本地日期，跨过午夜后下一次打开菜单才会变。选过词书却不再背的用户，菜单里会一直有一个数；如果有用户反馈这一项本身是打扰，要重新考虑是否需要一个单独的开关。

## Verification

- `cargo test -p msime-client-services vocabulary::`：`the_due_count_*` 覆盖没有进度（不建目录、不留锁文件）、没选书、跨午夜计数上升且不写进度、导入书和插件书、选中的书不在、损坏的进度文档（报错且原样保留）和非法日期，并逐个断言计数等于 `status().due`。
- `cargo test -p msime-host-api vocabulary_due_count`：C ABI 的 0、与 `review` 应答一致、-1 的各种情形，以及经 `host_options` 找到随包的内置书；`the_c_header_and_the_rust_exports_agree` 确认头文件声明了新入口。
- `common-vocabulary-due-menu`（macOS、Linux 的 CTest）：文案、0 和 -1 隐藏、组字时隐藏、本地日期格式。
- macOS `shortcut` CTest 的 `TestVocabularyDueMenuRow`：没开过背单词时没有这一行、也不建进度文件；导入两个词后菜单写「背单词（今日待复习 2）…」，组字时不列出。
- Windows `windows-tray-menu-layout`、`windows-shell-surfaces`、`windows-toolbar-mode-command`：这一行的位置、文案、各种隐藏情形和 `vocabulary` 路由。
- Linux `engine_smoke`：IBus 注册顺序里有这一行且起初不可见；导入两个词、重新获得焦点后可见并写出张数，点它启动 `vocabulary` 路由，删掉词书后又不可见。
