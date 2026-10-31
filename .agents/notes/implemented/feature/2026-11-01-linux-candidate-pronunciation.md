# Agent Note: Linux 候选释义显示英文音标

Status: implemented

## Problem

共享偏好 `candidate_pronunciation`（「显示读音」，默认关闭）和离线读音查询 `msime_client_pronunciation_request` 早已存在，macOS、Windows 和 Android 都在画音标，Linux 的 IBus 和 Fcitx5 宿主却只把这个偏好列进了「改了就作废翻译请求」的清单，从不查读音、也不画；设置页（`apps/desktop/src/main.tsx` 的 `candidatePronunciation`）只对 macOS 和 Windows 打开这一行。另外 Linux 的安装包不带音标表 `pronunciations/en-phonetic.db`：[英文释义三张表随 macOS 与 Windows 发布包下载并打包](../process/2026-10-11-ship-english-gloss-tables.md) 把 Linux 留在了范围外，桌面设置的资源包入口（`desktop_resource_packs.rs` 的 `offered_by`）又把音标表当作 Android 专属，所以只接上宿主，Linux 用户打开开关也什么都看不到。

## Decision

**宿主。** 两个宿主共用 `platforms/linux/src/candidates/CandidatePronunciation.h`，规则照 macOS 的 `CandidatePronunciation.h` 和 Windows 的 `CandidateGlossReadings.h`，只取英文这一半：释义按 `\n` 分行与目标语言对应（Linux 只画主目标语言那一行），英文目标那一行的第一个词是纯英文时整行交给读音表，英文候选在英文目标那一行读它自己。日文不标罗马音：macOS 用系统分词器、Windows 用微软日语输入法，Linux 没有对应的系统组件，引入分词库属于新依赖，不在这次范围。

读音不另起一条异步管线，跟着已有的翻译工作线程走：IBus 的 `TranslationTask`、Fcitx5 的 `startTranslation` 在打开开关时，于同一个工作线程上为这一段的释义回答调一次 `msime_client_pronunciation_request`，结果 `{文本: 读音}` 随回答回到主线程，存进宿主的 `CandidatePronunciations`（问过没有音标的记空串，满 4096 条整个清掉）。缓存随 IBus 的 `invalidate_providers`、Fcitx5 的 `invalidateTranslationRequests` 一起清空，也就是换焦点、换偏好时重来，用户刚下载音标表后切换一次输入框就会重新问。整句翻译和 `/fy` 的回答不是释义，不查。

**画法。** Fcitx5 的 classicui、原生画布的 macOS 结构（`NativeCanvasChrome.h`）和 canvas 包（`NativeCanvas.h`）都把读音放在它那一行释义下面单独一行，与 [macOS 的读音单独一行](../bug-fix/2026-10-16-macos-candidate-pronunciation-own-line.md) 一致；原生画布按释义色的 0.72 倍透明度画读音（`kCandidatePronunciationOpacity`），选中行不减淡，释义里共享层标出的词性和分隔符区段按行换算到新位置。canvas 包的竖排有读音时释义总放在正文下面。IBus 的查找表只有一行纯文字，读音隔两个空格接在释义后面。

**数据。** 沿用 `resources/pronunciations.lock.json` 固定的 ECDICT 表（MIT，5,718,016 字节），不是新数据。它不随 Linux 安装包，而是像 Android 一样作为资源包 `pronunciations` 按需下载：`offered_by` 在 Linux 上、资源目录旁边没有这张表时列出它（`Installation::pronunciations`），macOS 和 Windows 的发布包带着它，照旧不列出；启动时的补齐（`needed_by_preferences`）不含它。设置页的这一行在 Linux 上叫「显示英文音标」（与 Android 同名，macOS 和 Windows 仍叫「显示读音」），说明只讲英文音标和下载，不描述读音画在哪里（Fcitx5 画在释义下面，IBus 接在释义后面），开关打开后在它下面给出「离线英文音标词典」的下载行，用户点「下载」才联网，装进偏好目录的 `resource-packs/pronunciations/`；宿主把偏好目录下的 `resource-packs` 作为 `supplementary_resources` 交给共享层，共享层先找资源目录旁边，再找这里，都没有时回答空列表。这一行的组件参数 `japaneseRomaji`（默认 true）和 `downloadable` 与 iOS、鸿蒙那一侧的改动共用一套：不标罗马音的宿主换标题和说明，要下载的宿主再换成讲下载的说明。资源包列表只在 Linux 上读，其他平台这一页不调用宿主的资源包接口。`PRIVACY.md` 的资源包清单补上这一项、它的下载地址、换源顺序（用户镜像、本项目镜像 `https://dl.msime.app/gh/`、直连 GitHub）和锁文件，`docs/third-party.md` 写明 Linux 不随包而是下载。

出厂默认不变，仍是关闭。

## Alternatives considered

- **随 Linux 安装包带上音标表**，和 macOS、Windows 一样打开开关就能用，不用多一步下载。但 Linux 的包有好几条构建路径：`package-container.sh` 出的 deb/rpm、Arch 的 PKGBUILD、Debian、RPM、Gentoo 的发行版定义、源码包的 vendor 目录和 Nix，每一条都要接上 `fetch_gloss_tables.py` 和 CMake 的安装段，漏一条那个渠道的用户就静默没有音标，而这些渠道大多没法在本机验证。资源包只走设置应用这一条共享路径，所有渠道一样；宿主先找资源目录旁边，将来哪个渠道决定随包带上，不用改宿主。
- **读音单独起一条工作线程和请求队列**，像 macOS 的 `_pronunciationQueue` 那样按页收集缺的文本再问，能覆盖不经过翻译任务就出现的释义（例如「仅当前高亮」时从答案缓存直接交给 Rust 的那一份）。但 Linux 两个宿主的每一份释义本来就是某次翻译任务的回答，答案缓存里的那些在第一次回答时已经查过读音，缓存清空时两边一起清；另起一条管线要再做一套代次和会话围栏，代价比收益大。
- **接在释义行尾**，格子不多一行高。这是 macOS 和 Windows 以前的画法，macOS 因为读音把横排格子撑宽一倍、贴着下一格的释义而改成单独一行，Windows 随后也改成单独一行（#10461），Linux 不再从这个旧版式起步。只有 IBus 查找表只能画一行，才退回行尾接法。

## Consequences

- **收益**：Linux 用户打开「显示英文音标」、下载一次约 5.7 MB 的音标表后，IBus 和 Fcitx5 的候选释义标出英文音标；安装包体积不变，开关关着时宿主不查表、不改版式。
- **代价**：第一次使用要在设置页多点一次「下载」，下载完要切换一次输入框；在那之前问过的词记着空串。读音跟着翻译回答一起到，不会早于释义出现。日文释义不标罗马音，Linux 的说明文字因此只讲英文。有读音的 Fcitx5 横排页多一行高，和 macOS 一样不为读音预留。
- **验证缺口**：Wayland 和 X11 下的实机效果（Fcitx5 classicui、原生画布、canvas 包和 IBus 面板）还要在真机上看一次。

## Verification

- `platforms/linux/tests/candidate/candidate_pronunciation.cpp`：第一个词、纯英文判定、按行对应、法文目标不读、英文候选读自己、请求和回答的文档形状、补充目录、工作线程查询只在有文本时发、缓存上限、读音单独一行时区段位置的换算和 IBus 的行尾接法。
- `candidate_native_canvas_chrome.cpp`：读音排在释义之后、插件注释之前，格子多一行高。`candidate_native_canvas.cpp`：canvas 包的读音按 0.72 倍透明度画、选中行不减淡，词性区段颜色照旧。
- `fcitx5/tests/native.cpp`：classicui 把读音画在释义下面、原生画布拿到分开的释义和读音、读音不进上屏文字；宿主按偏好和目标语言给出读音，翻译请求作废时清掉记下的读音。
- `desktop_resource_packs.rs`：音标表只在 Linux 安装不带它时列出，启动补齐不含它；`apps/desktop/tests/settings/resource-packs.test.tsx`：Linux 的说明只讲英文、开关打开后才出现下载行、不自动下载、点了才安装、装好后下载行消失。

本机跑过的：`candidate_pronunciation.cpp` 用本机 clang 编译运行通过；两个画布测试在本机 clang 下语法检查通过，其中新增和改动的 `pronunciation_appearance`、`translation_appearance`、`rows_and_glosses` 换成桩的皮肤解码函数链接后运行通过；`resource-packs.test.tsx` 29 项和 `settings.test.tsx` 里的读音用例通过，`tsc --noEmit`、`pnpm run lint`、`format:check` 通过。整套 Linux 编译和 ctest、`msime-desktop` 的 Rust 单测在本机和编译机没跑完（编译机工作盘写满；第一次运行发现并修好了 Fcitx5 的聚合初始化编译错误），由 PR 的 CI 跑完：Linux native host 的 ctest 113 项全过，含 `linux-candidate-pronunciation` 和两个画布测试；Linux 设置应用的 `only_a_linux_install_without_the_table_offers_the_pronunciations` 通过。
