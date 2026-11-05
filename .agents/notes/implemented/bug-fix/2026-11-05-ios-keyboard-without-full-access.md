# Agent Note: iOS 键盘没有完全访问时只读地准备运行时

Status: implemented

## Problem

#10945：iPhone XR（iOS 18.7.11）上关掉「允许完全访问」，键盘只显示「输入运行时准备失败：Operation not permitted (os error 1)」，打不出组字。iOS 给没有完全访问的键盘扩展的 App Group 是只读的：读得到 App 写下的东西，任何写都被沙盒以 `EPERM` 拒绝。

同一台 XR 当天的系统日志里，键盘扩展被拒的第一个文件操作是：

```
Sandbox: MSIMEKeyboardExtension(53425) deny(1) file-write-data /private/var/mobile/Containers/Shared/AppGroup/<G>/MSIME/preferences.lock
```

之后每 3 秒一条同样的拒绝。第一条来自 `msime_client_prepare_host`：`prepare_shipped_host_configuration` 先调 `edition_preferences`，`PreferencesStore::write_first_document` 要读写打开偏好锁文件取独占锁，准备在这里就失败。之后每 3 秒的那条是键盘空闲维护里的偏好重读（`msime_client_load_preferences` → `PreferencesStore::load_reusing`），它同样要读写打开锁文件，所以没有完全访问时 App 里改的设置也到不了键盘。测试包是 develop 加四个只改 iOS Swift 的 PR，失败的这段 Rust 在 develop 上一样。

用权限位把共享目录改成只读、在 macOS 上跑同一条准备，偏好锁之后还会依次撞上：已就绪代次的日志回放（`open_ready_generation` → `replay_into` 写代次里的工作词库）、建会话时的词库租约（`DictionaryAccess::try_session` 读写打开、没有就建 `.msime-dictionary-access.lock`，报 `dictionary access unavailable`）。模拟器的扩展不受这条限制：模拟器上关掉完全访问，develop 照样能打字（`testTypingPinyinWithoutFullAccessCommitsIntoTheHostEditor` 在改前也通过），这个问题只有真机看得到。

## Decision

键盘按 `hasFullAccess` 选择准备方式；没有完全访问时只读地准备，共享目录一个字节也不写，打字照常，只是不学习。

- **Swift**：`KeyboardViewController.session` 改成第一次用到时才建（`viewDidLoad` 一开始），有完全访问时 `sharedStateWritable` 为真；没有时再在共享容器里独占地建一个临时文件再删掉（`MetasequoiaInputSessionBridge.sharedStateIsWritable`），建得出来仍为真。`hasFullAccess` 要连上键盘宿主才是真值，单元测试的宿主直接创建控制器、从不连上，它恒为 false，而那里的共享目录写得进；只看它时 CI 上约 270 个用真实控制器的单元测试全走了只读准备。真机没有完全访问时试写被沙盒拒绝，结果相同。`sharedStateWritable` 为 false 时 `MetasequoiaInputSessionBridge` 在 bootstrap 里加 `read_only_state: true` 和 `private_root`（扩展自己容器里的 `Application Support/MSIME-read-only`）。iOS 在用户切换完全访问时结束扩展进程，进程活着时这个值不变，所以不处理中途切换。
- **`msime_client_prepare_host`**：`read_only_state` 时走 `host-api` 的 `read_only_state::prepare_read_only_host`，必须有绝对路径、不在 `state_root` 里的 `private_root`，不能同时给 `runtime_options`。
  - 偏好经 `PreferencesStore::load_for_runtime_without_writing` 从共享目录读：不取锁、不建目录和锁文件。写偏好的一方一律整份写进临时文件再改名（`atomic_write`），不持锁读到的也一定是某一次完整的写入。没有偏好文件时是本版本的默认值。
  - 资源校验标记（`verified-resources.json`）和缓存写进 `private_root`。
  - App 选中并装好的下载代次：只读租约（`dictionary_updates::lease_read_only`）取得到、校验通过才用，否则用扩展随包的资源。
  - 词库布局二选一。共享目录里有本版本的就绪代次（`msime_engine::host::open_prepared_options_for`：`.ready`、主库、英文库、已并入的五笔表、资源带着的 n-gram 表都在，格式不比引擎新），两把租约锁文件也只读取得到共享锁，就用它：`user_data` 是共享的 `MSIME/user`，用户词库和学到的东西都在。否则用只读布局（`read_only_layout_options_for`）：代次目录就是随包资源，`user_data` 和 `cache` 在 `private_root`，没有用户词库，但能打字；退回时经诊断出口报一行 `read-only runtime on the bundled dictionaries`。
  - 文档带 `read_only_state: true`（`HostOptions` 新的可选键，只在内存里交给宿主，不写进任何发布的 runtime-options.json）。
- **会话**（`create_session`）：带 `read_only_state` 时 Engine 选项的 `learning` 和回车学英文词的 `learn_english_on_enter` 都为假，偏好变了重建 Engine（`apply_pending`）时也保持为假。后者是单独的开关，学的词写进代次里的 `msime-english.db`，只读布局下那就是随包资源：起初漏了它，iOS 模拟器上 App 包可写，回车学一次词随包资源就再也通不过校验（resource length or digest mismatch），单元测试整批失败。不记选词统计（当作 `statistics_recorded_elsewhere`），不写 `dictionary-updates/loaded.json`。词库租约用 `DictionaryAccess::try_session_read_only`：只读打开已有的锁文件取共享 flock，不建。只读布局下读的是随包资源，没有谁会改它，不取租约（`_dictionary_access` 改为 `Option`）。下载代次的租约同理只读打开 `<代次>.lock`。
- **可写的准备现在就建好锁文件**：`prepare_shipped_host_configuration` 准备完代次后取一次再放 `DictionaryAccess::try_session`，`prepare_host_with_updates` 用下载代次时同样取一次再放 `dictionary_updates::lease`。App 的首页每次出现都准备一次运行时（`prepareOnboardingResources`），于是 App 准备过的代次总有锁文件，没有写权限的键盘才取得到租约。
- **偏好的只读入口不再因为打不开锁失败**：`PreferencesStore::load`、`load_reusing`、`load_for_runtime` 打开锁文件遇到 `PermissionDenied`（`EPERM`、`EACCES`）或 `ReadOnlyFilesystem` 时不取锁照读，理由同上。保存仍然要锁，照旧失败。

### 没有锁时怎么不读到正在被换掉的代次

只读的键盘不靠「不加锁」，而是取同一把锁：flock 不要求描述符可写，只读打开锁文件取到的共享锁与 App、有完全访问的键盘读写打开取到的是同一把。所以 App 的维护（编辑个人词库、重置学习挪走工作词库、删旧的用户词库代次和下载代次）取独占锁时照样被挡住，与有完全访问时完全相同；锁的持有期也一样只在键盘可见期间（`viewWillAppear` 建会话、`viewWillDisappear` 销毁，#10634 的 0xdead10cc 规则不变）。

代次目录本身按内容命名，只在暂存好、写完 `.ready` 后改名就位，就位后不再改名；只读的准备只认带 `.ready` 的普通文件。任何一步拿不到这样的保证（代次没就绪、缺东西、锁文件不在、flock 被拒）都不去读共享代次，退回只读布局：随包资源在扩展自己的 bundle 里，没有谁会改它。

## Alternatives considered

- **在扩展自己的容器里准备一整份私有代次**——最强的理由是引擎和会话一行都不用改：状态目录换成可写的私有目录，现有的准备、回放、租约、学习原样工作，用户词库也能从 App Group 复制一份日志进来回放。不用：一份代次约 245 MB（主库并入 142 MB 的五笔码表、英文库、两张 12 MB 的 n-gram 表），第一次出现要在扩展里复制、合并五笔表、回放日志、建反查索引，扩展有内存和启动时限，最慢的设备上这要几秒到几十秒；盘上多占一份；复制进来的日志是那一刻的快照，私有的学习在下次复制时被覆盖，开完全访问后也不会合回去。
- **不取锁直接读共享代次**——最简单，只读打开数据库本身也不需要锁。不用：重置学习会把工作词库改名挪走再换成新的，删旧代次会整目录删掉，不持租约的读者可能读到一半被换掉的代次；锁文件只读打开就能取到同一把锁，没有理由放弃它。
- **没有完全访问时一律用随包词库**——同样一个字节不写、也不需要任何锁，实现最小。不用作默认：没开完全访问的用户会丢掉「我的词条」和学到的东西，这正是用户打字时最在意的；只在拿不到共享代次时作为退路。
- **Rust 自己试写一个文件来判断有没有写权限**——宿主不用传任何东西。不用：模拟器上的扩展写得进 App Group，这条只读路径在模拟器上永远走不到，系统界面用例测不到它；试写本身就是在共享目录里写；`hasFullAccess` 是系统给出的明确信号。Swift 只在 `hasFullAccess` 为 false 时试写一次，补的是它在没连上宿主时恒为 false 这一点，见 Decision 的 Swift 一条。

## Consequences

- **收益**：没有完全访问的键盘能离线打字，用 App 准备好的代次时「我的词条」和学过的词照常出现，App 里改的设置也能经偏好重读到达键盘（以前重读同样卡在锁上）。
- **代价**：没有完全访问时不学习，选词不调频、不造词，打字统计不记；App 为当前版本准备代次之前（安装或升级后还没打开过 App 的首页），键盘只用随包词库。只读宿主的诊断行写不进 App Group 里的诊断日志（`DiagnosticLog` 的目录在那里），真机上看不到退回只读布局的那一行。
- **代价**：`HostOptions` 多了一个只在内存里用的键；旧版宿主读不了带它的文档，所以它绝不能写进发布的 runtime-options.json。
- **已知上限**：只读的读者遇到 App 写到一半崩溃留下的回滚日志（`-journal`）时 SQLite 回滚不了，那次查询会失败，直到 App 再次打开这个库。iOS 上日志和词库都不用 WAL（见 `prefer_write_ahead_log`），所以不存在只读读者打不开 `-shm` 的问题。
- **仍会留在系统日志里的拒绝**：没有完全访问时，键盘扩展还会被拒几类写，都不挡打字。`MSIME/diagnostic.log`（`DiagnosticLog` 的目录在 App Group，写不进就丢）；`Library/Preferences/group.app.msime.ios.plist`（App Group `UserDefaults` 镜像，改前就写不进）；`MSIME/preferences.lock`（使用统计同意的轮询经 `usage_reporting_consent_reusing` 取独占锁，取不到就当没有同意，不上报）；代次里的 `msime-pinyin.db` 和 `user/msime_user.db` 的 `file-write-data`（会话按读写方式打开词库和日志，SQLite 被拒后自己改为只读打开，学习关着，不会真写）。
- **重访信号**：真机系统日志里 `MSIMEKeyboardExtension` 出现上面以外针对 `AppGroup/.../MSIME` 的 `deny(1) file-write-*`，或者打字出错，说明还有写路径没有收住；锁文件上的 flock 被拒说明只读 flock 在某个系统版本上不再允许。只读键盘的私有目录里出现 `MSIME-read-only/user`，说明它退回了只读布局。
- **没有改变的**：只读的会话同样持着 App Group 里的锁，键盘被挂起时还持着它们照样会被 RunningBoard 以 `0xdead10cc` 结束；持锁期仍然只在键盘可见期间（[键盘只在可见期间持有引擎会话](2026-11-02-ios-keyboard-session-only-while-shown.md)），这次没有动它。2026-11-05 的真机验收里，两次在有完全访问时、测试宿主刚拉起键盘几秒就按 Home 再结束宿主的启动里出现了这种结束（RunningBoard 记的是 `user/.msime-dictionary-access.lock` 和代次里的那一把），属于 #10910 正在处理的路径。

## Verification

- Rust：`msime-engine` 的 `access::tests::a_read_only_session_shares_the_lock_without_creating_it`、`a_read_only_session_works_in_directories_it_cannot_write`；`msime-client-core` 的 `preferences_are_readable_from_a_directory_that_cannot_be_written`、`reading_preferences_without_writing_leaves_no_lock_file`；`msime-host-api` 的 `tests::read_only_state`（共享代次上打字并挡住维护、整棵共享目录前后不变；没有就绪代次或共享目录根本不在时退回随包词库；锁文件不在时不读共享代次；学习开着也不学；bootstrap 校验；`MSIME_EVAL_RESOURCES` 指向真实词库时打 nihao 选「你好」上屏）。共享目录用权限位模拟只读，root 跑时这几条直接返回。
- iOS 单元测试：`MSIMEKeyboardTests/ReadOnlySharedStateTests`，权限位只读的共享目录上，只读的桥用 App 准备的代次打出「你好」并上屏，会话持锁、收起后放锁，共享目录不变；共享目录什么也没有时用随包词库照样出「你好」。
- 真机：2026-11-05 在 iPhone XR（iOS 18.7.11）上装本分支的包，系统设置里关掉完全访问，在 `MSIMEKeyboardTestHost` 的普通输入框里打 nihao，候选栏出「你好」，点首选上屏；没有「输入运行时」报错。扩展自己的 `Application Support/MSIME-read-only` 里只有 `cache` 和 `verified-resources.json`，没有 `user`，说明用的是 App 准备的共享代次，不是随包词库的退路。开回完全访问后同样打出并上屏「你好」。
- 系统键盘：`MSIMEClientUITests/KeyboardExtensionEditorUITests/testTypingPinyinWithoutFullAccessCommitsIntoTheHostEditor` 先打开一次 App，在设置里关掉完全访问，在独立测试应用的普通输入框里打 nihao、点首选上屏，结束时开回完全访问。模拟器不拦 App Group 的写，这条用例在改前也通过，它守的是只读路径在系统扩展里走得通。2026-11-05 在 iPhone 17 Pro 模拟器（iOS 27.0）上另外把模拟器的 App Group `MSIME` 整棵改成只读再跑同一条用例，照样通过，用例前后 `MSIME` 里每个文件的大小和修改时间都没变。这条用例不在 CI 里（`run_ui_tests.sh` 不跑 `MSIMEClientUITests`），要在本机跑。
