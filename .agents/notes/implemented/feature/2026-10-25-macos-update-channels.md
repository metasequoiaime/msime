# Agent Note: macOS 的更新通道：分通道 appcast 与按提交时间的构建号

Status: implemented

## Problem

桌面三个平台加了更新通道（稳定版、测试版、每日构建），偏好、共享检查和发布元数据见 [core 切片](../architecture/2026-10-25-update-channels-core.md)。macOS 的应用内更新不走共享检查，走 Sparkle：输入法本体每天在后台检查、静默下载、切换输入源时安装（[输入法本体经 Sparkle 自己更新](2026-10-15-macos-sparkle-input-method-updates.md)），完整设置应用在用户点「检查更新」时更新整包（[完整设置应用在应用内更新](2026-10-18-macos-settings-in-app-updates.md)）。两者都只读 `macos-appcast` 上的一份 appcast，而这份 appcast 只由 main 上的正式版更新；develop 的测试版和每日构建虽然也带 feed，装上之后只跟随正式版，选了测试版或每日构建的用户收不到它们。

还有一个藏着的问题：`CFBundleVersion` 的末尾九位是构建号，发布工作流缺省用 `GITHUB_RUN_ID % 999999999`。运行 ID 现在约 3.88e10，取模结果大约两个月回绕一次；同一版本号里回绕之后发出的构建比之前的小，Sparkle 的安装进程会把它当成降级拒装。只发正式版时同一版本号基本只有一个构建，问题不显；有了测试版和每日构建，同一个 X.Y.Z 会连续发出很多构建，这条规则就必须成立。

## Decision

### 每个通道一组 appcast

- 稳定版的文件名不变：输入法 `appcast-<版本 id>.xml`，设置应用 `appcast-settings-<版本 id>.xml`，更新包 `<dmg_prefix>-VERSION-input-method.zip` / `-settings.zip`。测试版和每日构建在版本 id 前插入通道名：`appcast-beta-full.xml`、`appcast-settings-nightly-wubi.xml`；更新包名里带通道和构建号，`<dmg_prefix>-VERSION-beta-<构建号>-input-method.zip`，同一版本号的几次测试版、每日构建和正式版互不覆盖。打包与发布一侧的规则在 `platforms/macos/scripts/sparkle_feed.py`（`feed_name`、`archive_name_for`、`parse_feed_name`、`archive_channel`，命令行 `names`、`feeds`、`wider`、`archive-channel`）。
- 包里的 `SUFeedURL` 不随通道变，始终是稳定版的 feed；`edition_bundle.py` 与 `settings_updater.py` 改写 feed 名的逻辑因此不用动。客户端按用户选的通道从它推出要读的地址（`platforms/macos/src/core/UpdateChannelFeed.h` 的 `MSIMEUpdateFeedURLForChannel`，输入法和设置应用共用这一个头文件）。地址不是 https、带查询或片段、文件名不是 `appcast-[settings-]<版本 id>.xml` 时推不出来，返回 nil，Sparkle 退回 `SUFeedURL`，也就是稳定版，不去请求拼错的地址。
- 通道取自偏好 `update_channel`：缺省、不认识的值、类型不对的值都按稳定版（`MSIMEUpdateChannelFromPreference`，与 client-core 读文档的宽松规则一致）；宿主请求里的通道严格解析（`MSIMEUpdateChannelParse`），写错的通道名不检查。

### 输入法

- `MetasequoiaSparkleUpdateDriver` 实现 `SPUUpdaterDelegate` 的 `feedURLStringForUpdater:`。Sparkle 每次检查前取 `SPUUpdater.feedURL` 都会问它，所以通道改了，下一次检查就读新通道的 appcast。
- 通道是驱动里缓存的值，不在 `feedURLStringForUpdater:` 里读文件：Sparkle 在主线程问 feed 地址，而输入法的主线程处理按键，读偏好要拿偏好的写锁，`InputController` 自己的偏好读取也特意放在后台队列。缓存从两处更新：
  - 启动时 `input_method_main.mm` 先在后台队列读出偏好里的通道，再回到主线程用 `+[MSIMEUpdateController sharedControllerWithUpdateChannelPreference:]` 创建控制器，驱动在 Sparkle 启动之前就拿到通道，Sparkle 安排的第一次检查不会先按稳定版取 feed。
  - 之后 `InputController` 每次读到新修订号的偏好（`completePreferenceLoad`）都把 `update_channel` 交给控制器（`applyUpdateChannelPreference:`）。控制器只在值变化时转给驱动；驱动看到通道变了就调用 `resetUpdateCycleAfterShortDelay`。SPUUpdater.h 写明 feed 或允许的通道这类设置变化后应调用它，它会重排检查计时、在 feed 变了时于后台马上检查一次；稍后才重排，用户来回切换时只算最后一次。
- 换通道时处理已经下好的更新：驱动在 `updater:willInstallUpdateOnQuit:immediateInstallationBlock:` 里记下安装闭包的同时，按更新包文件名记下它属于哪个通道（`MSIMEUpdateChannelOfArchiveName`，与 `sparkle_feed.py` 的 `archive_channel` 同一条规则；复制到更宽通道的 appcast 指向原通道的更新包，所以按文件名而不是按当时读的 feed 判断）。`setUpdateChannel:` 发现新通道不接受这个更新（`MSIMEUpdateChannelAccepts`：每日构建接受三者，测试版接受测试版和正式版，正式版只接受正式版）时丢掉闭包，切换输入源时不再安装它；换到更宽的通道时它仍被接受，照常安装。用户从每日构建换回稳定版就是不想要手里那个每日构建，装上之后稳定版也不会把它降级回去。
- 没有 feed 的构建不创建控制器，官网下载页和不可用的驱动没有通道的概念，不实现 `setUpdateChannel:`。

### 设置应用

`crates/host-macos/native/updater.mm` 给 `SPUStandardUpdaterController` 传一个只实现 `feedURLStringForUpdater:` 的委托（Sparkle 只弱引用委托，由静态变量持有），新增 `msime_macos_settings_update_check_channel(const char *channel)`，Rust 侧是 `msime_host_macos::check_settings_update_on_channel(&str)`。每次点「检查更新」先把委托的通道换成这次的再调 `checkForUpdates:`。原来的 `msime_macos_settings_update_check()` / `check_settings_update()` 保留，等同于按稳定版检查；Tauri 命令 `application_update_check` 和共享关于页把所选通道传下来是 apps/desktop 那一侧的改动。

### 发布

- `package-release.sh` 读 `MSIME_UPDATE_CHANNEL`（`stable` 缺省、`beta`、`nightly`），只决定 appcast 和更新包的文件名；核对时 bundle 的 `SUFeedURL` 仍要等于稳定版的地址。
- `release-macos.yml` 在「Resolve independent macOS version」里定通道和要不要上传（`channel`、`feed_publish`）：main 上 `publish` 且不是 `prerelease` 的正式版是 `stable`，与以前的条件相同；develop 上 `publish` 的测试版发布是 `beta`；`nightly.yml` 给可复用 workflow 传新输入 `appcast_channel: nightly`，只有运行在 develop 上（定时触发总是如此）时才上传。alpha、从其他分支手动触发的每日构建和不发布的构建只留在 workflow artifact 里。「Verify release assets」按通道的文件名核对。每日构建这一步设了 `continue-on-error`：它失败会让 macOS 的 nightly 作业失败，`nightly.yml` 于是跳过这个平台的每日构建下载，比 appcast 停在上一次更糟。
- `publish-sparkle-feed.sh <dir> <version> [channel]`：
  - `<dir>` 里的 appcast 和更新包必须都是这个通道的文件名，测试版的构建写不进稳定版的 feed。
  - 同一通道已发布的 appcast 构建更高时停下（原来的守卫，现在对三个通道都成立）；全部 appcast 都已经是这个构建、指向同一个地址、更新包也在时不传更新包和本通道的 appcast。构建号来自提交时间，每日构建在没有新提交的日子编出的是同一个构建，重传同名更新包会在 appcast 换上新签名之前让正在下载的用户验签失败。这时仍往下做复制到更宽通道和清理：上一次运行可能传完自己的 appcast 之后在复制那一步失败，重跑要能补上。复制的是已经在线的那份 appcast 而不是本地重新生成的，重跑打出的更新包不一定逐字节相同，本地 appcast 里的长度和签名对不上在线的更新包。
  - 自己的 appcast 上线之后，复制到更宽的通道：正式版也进测试版和每日构建，测试版也进每日构建，只在那边的构建更旧或还没有那份 appcast 时写。appcast 里只有更新包地址，不含自己的文件名，同一份签过名的文件换个名字照样有效。这样测试版的 feed 始终提供正式版和测试版里最新的一个，每日构建的 feed 提供三者里最新的一个，与共享检查「更宽的通道也接受更窄通道的发布」一致。
  - 只删自己通道、不再被任何 appcast 指向的更新包。另一个通道的发布可能正在进行，它先传更新包后传 appcast，中间这段时间它的更新包还没有被引用，按旧规则会被这边删掉。
  - 更新包的摘要文件按通道分开上传：正式版仍叫 `sparkle-SHA256SUMS`，测试版和每日构建是 `sparkle-SHA256SUMS-beta`、`sparkle-SHA256SUMS-nightly`，各自只列本通道的更新包。三个通道共用一个文件名时，每晚的每日构建都会用 `--clobber` 把它换成只列 nightly 更新包的版本，稳定版的更新包在已发布的摘要里就查不到了。更新包名里带着通道，某个更新包的摘要在哪个文件里由它自己的名字决定；本地目录里的文件名不变，还是 `package-release.sh` 写出的 `sparkle-SHA256SUMS`。
  - `MSIME_SPARKLE_SKIP_WIDER=1` 关掉复制，给只重签已有稳定版 appcast 的 `relocate-release-extras.sh` 用；它也跳过测试版和每日构建的 appcast，只要求它们不指向要删的文件。

### 构建号

`release-macos.yml` 的缺省构建号改为所构建提交的提交时间（`git log -1 --format=%ct HEAD`）距 2026-01-01T00:00:00Z 的分钟数，九位数够用到 3900 年之后，超出 1 到 999999999 时报错要求显式传 `build_number`。用提交时间而不是运行时间：同一版本号的正式版、测试版和每日构建按代码的先后排序；同一个提交重跑得到同一个构建号，测试版之后 main 以同一个提交发正式版时两者 `CFBundleVersion` 相同，Sparkle 不会把同样的代码当成更新再装一次。`CMakeLists.txt` 的编码规则（版本号 × 10^9 + 构建号）不变，只改了注释。

## Alternatives considered

- **一份 appcast 里放几项，用 Sparkle 2 的 `<sparkle:channel>` 和 `allowedChannelsForUpdater:` 选** — 最强的理由是这是 Sparkle 推荐的做法，一个 feed、不用推地址，也天然处理「测试版用户也要拿到更新的正式版」。没有采用：现有的生成、核对、发布全按「每份 appcast 恰好一项、由 `generate_appcast` 从一个目录生成并签名」来写（`sparkle_feed.py` 的 `single_item`、守卫和清理都依赖它），改成多项就要在三个各自触发、可能并发的 workflow 之间合并同一个签名文件，任何一次合并出错影响的是全部通道的用户。分文件时每个通道各写各的，最坏只影响一个通道。
- **测试版用户的 feed 不存在（404）时在客户端回落到稳定版** — 最强的理由是不依赖服务端一定有那份文件。没有采用：Sparkle 一次检查只认一个 feed 地址，`feedURLStringForUpdater:` 在请求之前就要给出地址，404 和断网在委托里分不开，要回落只能在失败后改地址重试，把一次检查拆成两次。改为服务端保证：发布正式版时测试版和每日构建的 appcast 不存在就建起来，发布测试版时同样建每日构建的；此外任何一次发布（每晚的每日构建最频繁）都用在线的稳定版 appcast 补齐所有版本里还不存在的测试版、每日构建 feed，只补缺的，已有的不动。
- **三个通道合并成一份 `sparkle-SHA256SUMS`** — 最强的理由是沿用一个文件名，README 和 release 说明里的「摘要见 sparkle-SHA256SUMS」照旧成立。没有采用：要先下载在线的那份、去掉本通道和已删除的更新包再合并上传，三个可能并发的发布会互相覆盖对方刚合并进去的行，正是分文件 appcast 要避开的那种并发写；按通道分文件时各写各的。
- **换通道时一律丢掉已下好的更新** — 最强的理由是规则简单。没有采用：委托接管安装之后 Sparkle 不再运行新的检查周期，从稳定版换到每日构建时若丢掉手里的稳定版更新，这次会话里既不装它也不会检查新通道，只等进程退出时 Sparkle 自己装；它本来就是新通道接受的构建，照常安装更好。
- **构建号用运行时间（每次运行的当前时间）** — 最强的理由是不依赖提交时间是否单调，任务描述的初版设计也是这样。没有采用：重跑旧运行会得到更大的构建号，把旧代码当成新构建推出去，守卫拦不住；同一个提交的测试版、每日构建和正式版也会互相当成更新。提交时间的代价是依赖 develop 和 main 的提交时间大体单调：GitHub 合并 PR 时写的是合并时刻，手工推送时本机时钟偏差会让新提交的构建号偏小，同一通道的守卫会因此停下，每日构建只是这一步失败。
- **保留运行 ID 取模、在回绕时人工递增版本号** — 最强的理由是不改发布流程。没有采用：回绕约两个月一次，而且发生在哪个版本号里事先看不出来，靠人记不住。

## Consequences

- **收益**：选了测试版或每日构建的 macOS 用户，输入法在后台、设置应用在点「检查更新」时就能装上 develop 的测试版和每晚的构建；正式版用户的 appcast、更新包名和发布条件都不变。同一版本号里的构建号按代码先后单调，不再回绕。
- **对已发布构建号的影响**：运行 ID 取模的构建号在 1 千万到近 10 亿之间（0.54.0 是 267138363），提交时间算出的现在约 43 万，比它们小。版本号部分更高时不受影响，所以下一个版本号起一切照常；受影响的只有与已发出的构建同一个版本号的新构建：已经装上 0.54.2-beta 等运行 ID 构建号的用户，Sparkle 不会把同为 0.54.2 的新构建当成更新，设置应用启动时也不会用同版本号、更小构建号的内嵌输入法覆盖它。过渡办法：合并前 develop 已把 `platforms/macos/version.txt` 升到 0.55.0（#9690，`build-number.txt` 为 1），合并后的每日构建和测试版版本号部分已经高于发出过的 0.54.2，不受影响；确实要在当前版本号里再发一次时，用 `build_number` 输入显式传一个比已发出的更大的值，但之后同版本号内的提交时间构建号又会比它小，所以只适合最后一次。
- **代价与已知上限**：
  - 合并之后、第一次以新流程发布之前，`macos-appcast` 上没有测试版和每日构建的 appcast。实际影响很小：带通道选择的输入法只能从合并之后的构建装上，最早是当晚的每日构建，而那次运行会写入每日构建的 appcast，并用稳定版补上测试版的（`publish-sparkle-feed.sh` 的「补齐还不存在的更宽通道 feed」）。补上的测试版 feed 指向正式版，直到第一次测试版发布。
  - 输入法的通道缓存只在它读偏好时更新：偏好变化要等输入法有活动客户端时才读到，设置页里换了通道而之后一直没用水杉打字，这段时间的后台检查仍按旧通道。用户回到水杉打字的那一刻读到新偏好，Sparkle 随即重排检查。
  - `resetUpdateCycleAfterShortDelay` 是否立即检查由 Sparkle 决定（SPUUpdater.h 写的是「may trigger」）；不检查时下一次按计划的检查（最长一天）读新通道，手动「检查更新」任何时候都读当前通道。
  - 每日构建的 feed 只由 develop 上的运行更新。`nightly.yml` 可以从任意分支手动触发，从功能分支跑一次往往只是想看包能不能编出来；Sparkle 会把它静默装到每日构建用户的机器上，而功能分支的提交通常比 develop 新，「不退回更旧构建」的守卫拦不住，它的 `version.txt` 若更高，之后 develop 的每日构建还会一直被守卫判成退回。代价是从别的分支手动跑的每日构建只有 workflow artifact，要给每日构建用户试一个分支，先合进 develop。
  - 从每日构建或测试版换回更窄的通道时，切换输入源不再安装手里那个不被新通道接受的更新，但 Sparkle 2.9.6 没有公开接口撤回已经交出的退出时安装（SPUUpdaterDelegate.h：无论委托返回什么，Sparkle 都会在应用退出时尝试安装），输入法进程正常退出（注销、关机）时它仍会被装上；委托接管安装后 Sparkle 也不再运行新的检查周期，这次会话里不会按新通道检查，要到下次启动。
  - 从每日构建换回稳定版不会降级：已装的构建号更高，要等正式版追上；共享检查的语义相同。
  - `macos-appcast` 上多了最多四组 appcast 和测试版、每日构建各自的更新包，按引用清理，旧的随下一次同通道发布删掉。
  - `release-macos` 并发组只在同一个 workflow 的直接运行之间排队；测试版发布和每日构建同时改 `macos-appcast` 时，各自只写自己的文件、只删自己通道的更新包，复制到更宽通道之前各自读一次那边的构建，极端情况下一个旧一点的构建可能后写，下一次同通道发布时纠正。

## Verification

- `ctest -R update-controller`（`tests/core/UpdateControllerTest.mm`）：偏好原值经控制器规范化后只在变化时转给驱动，缺省、未知值、类型不对和大小写不同都按稳定版；没有通道概念的驱动不受影响；启动时带偏好取共享控制器得到同一个控制器并换上通道；按通道推 feed 覆盖输入法和设置应用两种文件名，以及通道写错、没有 feed、不是 https、带查询或片段、文件名不合规、已经是通道 feed 时返回 nil。
- `python3 scripts/test-macos-release-extras.py`：新增的 `FeedNameTests` 覆盖稳定版沿用旧文件名、通道文件名、全部 macOS 版本的往返解析、更宽通道表和拒绝的写法；发布流程覆盖测试版只写自己的 appcast 并建起每日构建的、不碰稳定版；正式版只在更宽通道的构建更旧或没有时复制；一个通道拒绝另一个通道的文件；每日构建拒绝倒退、同一个构建不再上传；每个通道只清理自己的更新包，另一个发布刚传上去的更新包不被删；原有的迁移流程（`MSIME_SPARKLE_SKIP_WIDER`）结果不变。
- 审查修复补充的检查：`ctest -R update-controller` 覆盖按更新包文件名判断通道（稳定版、测试版、每日构建，构建号为 0 和多出后缀的不算）与各通道接受哪些构建；`python3 scripts/test-macos-release-extras.py` 新增上一次运行在复制到更宽通道时失败、以同一构建号和不同字节的更新包重跑时只补更宽通道的 appcast、复制的是在线的 appcast 并能用在线的更新包验签，以及测试版的摘要传成 `sparkle-SHA256SUMS-beta`、不覆盖稳定版的 `sparkle-SHA256SUMS`。两项新测试在修复前的脚本上失败。`SPUUpdater` 换通道时丢掉闭包的分支依赖真实 Sparkle 会话，没有单元测试。
- `python3 scripts/test-macos-settings-updater.py`：设置应用和输入法的 `SUFeedURL` 文件名就是 `sparkle_feed.py` 写出的稳定版名字。
- `actionlint .github/workflows/release-macos.yml .github/workflows/nightly.yml` 通过。
- 在编译机上以 `target/macos-isolated` 配置并编出 `MSIMEClientInputMethod`、`shortcut-test`、`update-controller-test`，`ctest -R update` 通过；`cargo clippy -p msime-host-macos --all-targets -- -D warnings` 与 `cargo test -p msime-host-macos --lib` 通过（`updater.mm` 随 build.rs 编译）。
- 没有在真实发布上运行：Sparkle 按通道换 feed、`resetUpdateCycleAfterShortDelay` 的实际表现、三种发布写 `macos-appcast` 的结果都要等合并后的第一次测试版和每日构建验证。
