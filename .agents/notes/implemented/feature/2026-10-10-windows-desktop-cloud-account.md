# Agent Note: Windows 设置应用用账号会话服务云词库、快照、社区词包与设置同步，并与输入法共用会话文件

Status: implemented

## Problem

Windows 的设置应用（Tauri）已经能登录水杉账号，但账号之外的云功能几乎都接不上。云词库面板的每个请求在 `apps/desktop/src-tauri/src/lib.rs` 的 `cloud_dictionary_request` 里走 `#[cfg(not(unix))]` 分支，直接返回 `unavailable`，而词库页和账号页照常显示「云词库」入口，用户点进去只看到失败。「完整云词库备份」和「应用到本机」由 `cloudDictionaryCapabilities` 只对移动端和 macOS 打开。社区词包与回复模板的命令只编进 iOS、Android。设置同步卡片在桌面端没有客户端。账号会话放在设置应用自己的 `%LOCALAPPDATA%\<tauri identifier>`，输入法 Server 只认识自己在 `%LOCALAPPDATA%\<版本用户目录>\account` 注册的匿名账号，读不到用户登录的账号，后面要接「水杉账号」候选翻译时无从下手。

## Decision

### 云词库

账号操作（列表、目录、增删改、导入导出、候选调频、固定排位）的分发从 `mobile_account_helpers.rs` 移到 `apps/desktop/src-tauri/src/platform/cloud_dictionary.rs` 的 `account_request`，对会话的后端和存储泛型；iOS、Android 经原来的 `cloud_dictionary_account_request` 包一层调用它，行为不变。Windows 的 `cloud_dictionary_request` 在 `not(unix)` 分支把请求交给 `platform/desktop/desktop_cloud_dictionary.rs`，用 `desktop_account::AccountState` 的会话。Linux 与 macOS 的 provider socket、原生会话路径不动。

快照在 `desktop_cloud_dictionary.rs` 里做，`cloudDictionaryCapabilities("windows")` 是 `snapshot: true, snapshotNative: false`，与移动端同一条非原生路径：

- 备份：`snapshot_export` 下载完整快照、用 host-api 的 `inspect_snapshot_json` 校验后把文本交给页面，页面用 WebView2 的下载链接保存；`snapshot_restore_preview` / `snapshot_restore` 校验页面选的文件，比对预览给出的内容校验和与云端 revision 后上传。中转文件放在本用户私有的 `%LOCALAPPDATA%\<tauri identifier>\dictionary-snapshots`，用完即删，启动时清掉上次遗留的预览。
- 应用到本机：预览时先向 host-api 的持久队列（`snapshot_queue_json`，与鸿蒙经 `msime_client_snapshot_queue` 用的同一个队列）要本机词库版本，下载并校验快照，按账号代次登记预览。入列前确认账号没换、云端在预览之后没有新改动，然后把快照交给 Server 状态目录（HostOptions `user_data` 的上一级）下的 `dictionary-snapshots` 队列，随即处理：用 `QuiescedHosts` 经辅助管道请 Server 放开输入会话，直到词库能拿到独占锁，再让队列在本进程里准备并激活新的一代（暂存在同级的 `dictionary-snapshot-staging`，与词库同卷，激活靠改名），`QuiescedHosts` 离开作用域时交还会话。会话放不开、或激活没完成时，请求留在队列里（`queued` / `preparing`），面板轮询 `snapshot_status` 时再处理，两次之间至少隔 60 秒，避免一份总是激活不了的快照让输入每两秒断一次。
- `snapshot_choose_restore`、`snapshot_restore_native`、`snapshot_restore_cancel` 是 macOS 由宿主选文件的那一套，Windows 返回 `snapshot_unavailable`。

### 社区词包与回复模板

浏览、详情、发布、导入云端词库、收藏、评分、删除自己的发布这七个命令从 `mobile_community.rs` 移到 `platform/community_resources.rs`，存储类型按目标选（移动端是 `MobileStorage`，Windows 是 `desktop_account::Storage`），状态 `CommunityResourceState` 由各平台建账号状态时登记。「添加到高情商回复键盘」的两个命令只有移动端有，留在 `mobile_community`：`CommunityResourceClient` 的 `storeReply` / `removeReply` 变成可选，宿主不给时详情页不显示这两个按钮，列表上的「添加」只收藏。Windows 的社区页没有皮肤画廊（候选窗口皮肤社区在主题页），`CommunityHomePage` 的 `skins` 变成可选，只有「词库」「回复模板」两个分类；入口是账号页「我的内容」里的词库、回复模板几行，`communityHasSkins={false}` 让「我发布的皮肤」不经社区页打开。macOS 的 Tauri 应用不接这些命令：macOS 由输入法自己的账号窗口提供词包与回复模板。

### 设置同步

`settings_sync.rs` 把 Android 映射里各平台共有的 `input.*` 部分抽成 `insert_input_settings` / `apply_input_settings`，新增 `export_desktop_settings` / `apply_desktop_settings` 只用这一部分。`platform/windows/windows_settings_sync.rs` 注册与移动端同名的四个命令；应用时按读到时的修订号经 `save_preferences_impl` 写入，期间本机设置变过就拒绝，并同步进运行时选项。卡片的说明文字由 `SettingsSyncClient.description` 给出，Windows 写明只同步输入设置；应用成功后的提示由 `appliedMessage` 给出，Windows 只说「已应用云端设置。」，不沿用移动端「请重新打开键盘」那半句，因为 Server 从共享偏好里当场读到新设置。

### 与输入法共用账号会话

Windows 设置应用的会话改放在 `%LOCALAPPDATA%\<版本用户目录>\account\account-session.json`（full 是 `%LOCALAPPDATA%\MSIME\account`），即 Server 注册匿名账号的目录，与 macOS 设置应用和输入法共用 Application Support 里那份会话是同一个做法：同一个文件，同一把 `account-refresh.lock`。启动时新目录没有会话而旧目录有，就把旧文件改名搬过去（`desktop_account::adopt_legacy_session`）。输入法进程用 host C ABI 取令牌：

```c
char *msime_client_account_access_token(const uint8_t *request, size_t length);
// request: {"directory": "<绝对路径>", "rejected_token": "<可选>"}
// value:   {"access_token": "...", "user_id": "...", "anonymous": bool}
```

实现是 client-core 的 `account::shared_access_token`：已登录的账号优先，刷新时持同一把锁；没有登录、或登录的会话被服务端吊销时退回 `anonymous-session.json`；两者都没有时报 `account_unauthorized`。匿名会话在这条路径上也按多进程共用的存储处理：每次重新读文件，刷新时持同一把 `account-refresh.lock`，Server 的几个线程同时取令牌时只有一个去刷新，其余读到轮换后的会话，不会拿用过的刷新令牌再刷新一次让服务端吊销整个匿名会话（`ensure_anonymous_account` 和其他宿主读写匿名会话的方式不变）。每次调用都重新读文件，设置应用里的登录、退出和换号下一次调用即生效，不需要进程间通知。

## Alternatives considered

- **Windows 云词库也走 provider socket 或命名管道，由 Server 服务** — 与 Linux 的结构对称，输入法侧也能复用。但 Server 里没有任何账号客户端，要把令牌、HTTP 和快照校验都搬进 C++；设置应用进程里这些都现成，iOS、Android 已经这样服务同一个面板。
- **「应用到本机」只入列，等 Server 在空闲时处理** — 这是 Android 和鸿蒙的做法，激活发生在持有会话的输入法进程里，最不打扰输入。但 Windows Server 里没有队列处理和快照激活的代码，要新写一条辅助管道请求和 C++ 侧的空闲判断；设置应用已经能经辅助管道让 Server 放开会话（词库导入就是这样做的），在本进程里激活只多一次短暂的输入暂停。
- **社区资源命令为 Windows 另写一份** — 改动只落在桌面模块，不碰移动端。但七个命令除了存储类型逐字相同，两份会各自漂移；按目标选存储类型的一份共享模块，移动端只多登记一个状态。
- **桌面设置同步也映射 `platform.macos.*` 键** — 候选窗口、快捷键这些桌面设置就能跨机器同步。但那些键是 macOS 原生偏好的映射，取值和 Windows 共享偏好并不一一对应，Windows 写进去会改掉用户 macOS 上的设置；只同步各平台共有的 `input.*` 不会。
- **Server 读设置应用目录下的会话文件** — 不用搬文件。但设置应用的目录名随 Tauri identifier 走，Server 要知道每个版本的 identifier；匿名账号已经在 `%LOCALAPPDATA%\<版本用户目录>\account`，两份会话放在一处，C ABI 只需要一个目录。

## Consequences

- **收益**：Windows 用户能用云词库面板的全部功能，包括完整备份、恢复和把云端快照替换本机词库；能浏览、收藏、发布词包与回复模板并把词包导入云端或本机词库；能上传和应用输入设置。输入法 Server 有了取账号令牌的接口，「水杉账号」候选翻译可以直接用用户登录的账号。
- **代价与已知上限**：
  - 「应用到本机」激活时输入会暂停，长短取决于准备整份词库的时间；Server 在最后一次 `DictionaryQuiesce` 之后 30 秒自己交还会话，所以处理队列期间每 10 秒续一次放开，准备再久也不会中途把会话交回去；激活没完成时请求留在 `preparing`，每 60 秒在面板轮询时重试一次。面板关着时没有人处理队列，下次打开云词库面板才继续。共享面板因此按 `snapshotAppliesInPanel` 换掉移动端「输入法将在下一次空闲边界应用」的说法：确认框说替换马上进行、输入会暂停、要保持面板打开；没能当场应用时提示保持面板打开等重试，面板轮询时重试成功或作废就把这句换成结局（结束的结果在下一次轮询时被取走，不能指望处理结果一节一直在），待应用期间在处理结果里写明关掉面板就停在待应用、之后输入改了本机词库会因冲突作废。真实 Windows 上大词库的准备耗时、以及改名换入时有没有别的进程握着词库文件，都还没有在真机上量过。
  - 回复模板在 Windows 上只能收藏、评分、举报、发布，不能用于输入：桌面没有「高情商回复」键盘。
  - 设置同步只覆盖 `input.*`，候选窗口、快捷键、皮肤等桌面设置不跨机器同步。
  - 设置应用的账号页还不显示本机匿名账号（macOS 原生账号窗口的「本机账号」标记和登录后弃用匿名账号）。Server 的翻译线程用 `msime_client_account_access_token` 取令牌做「水杉账号」候选释义，见 [Windows 的释义列快捷键与「水杉账号」候选释义](2026-10-10-windows-gloss-columns-and-account-glosses.md)。
  - Linux 与 macOS 的 Tauri 应用没有社区词包与设置同步；要在 Linux 打开时，把 `community_resources` 和设置同步命令的目标条件加上 Linux 并在 `main.tsx` 同步打开。

## Verification

- `apps/desktop/src-tauri/src/platform/desktop/desktop_cloud_dictionary.rs` 的测试用合成后端驱动账号分发、备份导出（不带 `fileSha256` / `engineRecords`）、恢复的校验和比对、macOS 专有操作的拒绝、状态目录推导和轮询节流；`desktop_account.rs` 的测试锁住旧会话文件的搬迁规则。
- `crates/client-core/src/account/anonymous.rs` 的 `the_input_method_token_prefers_the_signed_in_account_over_the_anonymous_one` 锁住令牌选择顺序和退出后的回退，`concurrent_input_method_calls_refresh_the_anonymous_session_only_once` 锁住并发取令牌时匿名会话只刷新一次。
- `apps/desktop/tests/core/desktop-host-services.test.ts`、`tests/input/mobile-host-capabilities.test.ts`、`tests/community/community-resources.test.tsx`、`tests/account/account-page.test.tsx` 锁住 Windows 客户端调用的命令、快照能力位、无皮肤画廊的社区首页、无回复键盘时的详情页和账号页入口。
