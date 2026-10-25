# Agent Note: 更新通道与后台检查下沉到共享 Rust

Status: implemented

## Problem

Windows、Linux、macOS 三个桌面平台要加「更新通道」：稳定版（默认）、测试版、每日构建，有新版本时由客户端定期在后台检查并主动提醒。原来的共享检查（`crates/client-services/src/update_check.rs`）只认正式发布，`select_platform_release` 跳过所有预发布；后台检查、记录和「只提醒一次」只写在 Linux 的 Tauri 设置应用里，Windows 常驻进程要复用就得在 C++ 里再写一份。

通道还带来一个旧规则答不了的问题：包内版本号始终是 X.Y.Z，develop 的 beta、之后 main 的正式版、重跑同一版本、每天的 nightly 都可能是同一个版本号。只比版本号时，beta 用户等不到同版本的正式版，nightly 用户每天的新构建也永远「已是最新」。

## Decision

这一篇是 core 切片：偏好、构建身份、选择规则、后台检查和发布元数据。各平台的界面与提醒（Windows 托盘气泡、Linux 通知、macOS 分通道 appcast，见 [macOS 的更新通道](../feature/2026-10-25-macos-update-channels.md)）各有自己的笔记。

### 偏好

`client-core` 的 `Preferences::update_channel: UpdateChannel`，取值 `stable`、`beta`、`nightly`，缺省 `stable`，等于缺省时不写进文档。反序列化是宽松的：不认识的字符串按 `stable` 读，不让以后的版本加的通道把整份偏好读坏；不是字符串仍报错。它进 `settings_document::LOCAL_SECTIONS`（换电脑导入设置文件不该顺带把新电脑换到预发布），也不进账号同步：服务端字段表本来没有这个键，客户端另在 `cloud_sync::policy::LOCAL_ONLY_FRAGMENTS` 里把它列为本机设置，以后服务端字段表收录它也不会上传或应用（一台机器上试每日构建，不该让同账号的其他设备跟着换通道）；恢复默认设置回到 `stable`。

### 构建身份与发布元数据

- `release-windows.yml`、`release-linux.yml`、`release-macos.yml` 在 workflow 级导出 `MSIME_BUILD_RUN: ${{ github.run_id }}`；`nightly.yml` 复用它们时 `github.run_id` 就是 nightly 那次运行。Linux 的包在 docker 里编，`platforms/linux/package-container.sh` 设置了它时原样传进容器。
- `update_check::BUILD_RUN = option_env!("MSIME_BUILD_RUN")` 解析成 `Option<u64>`：一到十九位数字、不为零，否则为 `None`。本机构建、CI 测试构建和发行版从源码编的包都是 `None`。
- 正式版、测试版和 alpha 的发布正文末尾由 `scripts/release-build-marker.py` 追加一行 `<!-- msime-build {"run":<u64>,"source":"<40 位 sha>","channel":"stable|beta|alpha"} -->`，通道取自发布名的后缀。客户端（`build_marker_run`）只认正文去掉末尾空白后的最后一行：标记前面的更新说明是 `generate-release-notes.py` 拿 PR 标题、正文和提交说明让模型写的，外部贡献者能控制其中的文字，混在中间的同名标记不能算数。
- `scripts/publish-nightly.py` 的状态块每个平台条目新增可选的 `version`（包内 X.Y.Z）和 `run`（int），`state.version` 仍为 1，旧状态照常解析，字段存在但形状不对时整轮拒绝改动。`version` 取自复用 workflow 上传的 artifact 名末尾（`msime-windows-full-0.3.1`、`msime-linux-deb-x86_64-0.11.0`、`msime-macos-0.52.0`），一批文件的 artifact 版本不一致或取不到时不记，客户端也就不提供这一项。

### 选择规则

`UpdateCheckRequest` 加可选的 `channel`，缺省 `stable`，别的值让请求不合法（`Invalid`）。宿主从偏好读来的值已经由 client-core 归一，所以请求里出现未知通道只可能是宿主写错了，直接报错比悄悄当稳定版更容易被测试发现。`windows7` 只有正式发布，任何通道都按 `stable`（`UpdateCheckRequest::effective_channel`）。

`select_channel_release` 在 `select_platform_release`（现在等于它的稳定版）之外：

- `beta` 另外接受 `prerelease` 且 tag 恰为 `<platform>-vX.Y.Z-beta` 的发布，`-alpha`、`-beta.1`、`-beta2` 都不算；
- `nightly` 再接受 tag 为 `nightly` 的发布，页面必须是 `https://github.com/metasequoiaime/msime/releases/tag/nightly`，从正文状态块里取本平台条目（没有 `version` 的不提供），附件取状态里的 `files`：必须也在发布附件里，GitHub 给了摘要时要和状态里的 sha256 一致，摘要用状态里的。附件名在扩展名前多出的 `-nightly.<run>.<attempt>` 先去掉，再交给正式版的同一套 Windows 安装程序、Linux `.deb`/`.tar.gz` 匹配规则，选中后换回真实附件名；Linux nightly 只有 `.deb` 和 `.rpm`，按架构选 `.deb`。
- 候选按（版本号，运行 ID，没有记 0）取最大，完全相同时取列表里靠前的。

nightly 发布是很久以前建的、之后原地更新，在按建立时间排序的 `releases?per_page=100` 里会被挤出第一页，所以 `nightly` 通道另外 GET `https://api.github.com/repos/metasequoiaime/msime/releases/tags/nightly`（404 当作还没有 nightly，其他失败当作检查失败）。

是否更新（`is_newer_build`）：版本号更高必为新；版本号相同时，只有发布和当前构建的运行 ID 都知道、且发布的更大才为新。

`ReleaseUpdate` 新增 `channel`（`stable`/`beta`/`nightly`）和 `build_run: Option<u64>`，都带 serde 缺省值，Linux 以前写下的记录照常读出。

### 后台检查

`update_check::background` 接管原来 Linux Tauri 里的逻辑：记录 `update-check.json`（字段 `checked_at`、`attempted_at`、`channel`、`notified_version`、`notified_run`、`available`），成功后一天、失败后六小时内不再联网；写入时持有同目录的 `update-check.lock`，拿不到锁（另一个进程正在检查）时这次跳过而不是等。新加的规则：

- 记录存通道；请求通道和记录不同时立刻重查，旧通道的检查时间和找到的版本作废，只保留「提醒过哪个构建」；关于页的 `pending_update` 也只认同通道的记录。
- 提醒过的键是（`notified_version`，`notified_run`），只有 `notified_version` 的旧记录按运行 ID 为空比较，升级后不会对同一版本再提醒一次。
- 开发构建（`cfg!(debug_assertions)`）和构建时、运行时的 `MSIME_DISABLE_UPDATE_CHECK` 关掉后台检查，返回 `disabled`；发行版用 release 配置自编的包照旧检查。

原生宿主经由 host-api 调用：`msime_client_update_background_check` 和 `msime_client_update_pending`，请求都是检查请求加上宿主状态目录 `directory`（与通知、遥测等接口的目录约定相同），见 `crates/host-api/include/msime_client.h`。

## Alternatives considered

- **当前构建没有运行 ID 时按 0 比较**（任务给出的初版设计）：规则最整齐，所有构建都按（版本号，运行 ID）比。但发行版仓库（COPR、OBS、PPA、AUR、Nix）从源码编的包没有 `MSIME_BUILD_RUN`，而同版本号的 GitHub 发布带了标记，于是每个源码包都会一直把自己的版本当成更新来提醒。改成「当前不知道自己的运行 ID 时只比版本号」，代价是本机开发构建看不到同版本号的新构建，这本来就不是它们要的。
- **未知通道按稳定版处理**：与偏好的宽松读法一致，请求永远不会因为通道失败。但请求只来自我们自己的宿主，宿主读偏好时 client-core 已经把未知值归成 `stable`；请求里再出现未知值只能是宿主拼错了，按稳定版吞掉会让「选了测试版却收不到测试版」变成没人发现的静默错误。
- **nightly 的版本号从 reusable workflow 的 outputs 取**：显式，不依赖 artifact 命名。但三个 release workflow 的 `workflow_call` 都要新增 outputs，`release-macos.yml` 那部分属于 macOS 切片；artifact 名本来就带版本号，并且是 nightly 下载 artifact 时已经依赖的约定，取它不新增跨文件契约。
- **只从发布列表第一页找 nightly**：少一个请求。但 nightly 发布的建立时间是固定的，六个平台频繁发版后它迟早掉出第一页，每日构建通道就会静默退化成测试版。
- **取正文里第一个（或最后一个）`msime-build` 标记**：不要求标记独占最后一行，维护者在网页上给发布正文后面补几句话也不丢运行 ID。但取第一个时，更新说明里出现的标记（本功能 PR 描述里的示例就是一个）会盖过末尾的真标记：写坏的让这个发布没有运行 ID，写成合法 JSON 且 `run` 很大的会让装了这个版本的每个用户一直被提醒同版本号的「新版本」。取最后一个能挡住这一类，但「末尾那行由脚本追加」才是能核对的契约；只认最后一行把它写死，代价是有人在标记后面手动加字时这个发布退回到只按版本号比较，不会误报。
- **后台检查持锁等待**：两个进程同时到期时第二个等第一个结束、再看到「未到时间」。但网络请求最长十秒，等待期间调用方线程被挂住；拿不到锁就跳过，结果相同，也不会阻塞。

## Consequences

- **收益**：选哪个发布、哪个构建算新、什么时候再查、哪个版本已提醒过，三个桌面平台只有 Rust 一份；Windows 常驻进程拿到后台检查不必重写；beta 用户能拿到同版本的正式版，nightly 用户能拿到同版本号的新构建。
- **对现有安装的影响**：没有 `MSIME_BUILD_RUN` 的旧构建只比版本号，行为与以前相同。稳定版请求的选择结果与以前相同；结果 JSON 多了 `channel` 和 `build_run` 两个字段，宿主读结果时要容忍。同一版本的发布被重跑（新运行 ID）时，带运行 ID 的旧构建会把它当成更新提醒一次，这是有意的：重跑通常就是为了修包。
- **隐私**：每日构建通道多一个对 `api.github.com/.../releases/tags/nightly` 的 GET，与发布列表请求一样不带标识；PRIVACY.md 的更新段落需要随平台切片一起补上。
- **代价与上限**：
  - nightly 的版本号依赖 artifact 名末尾的 `-X.Y.Z`；哪天 release workflow 改了 artifact 命名，状态里就不再有 `version`，每日构建通道会退回到只看正式版和测试版（不会选错包）。`scripts/test-nightly-partial-publishing.py` 用带版本号的 artifact 目录覆盖这条路径。
  - 运行 ID 只在同一个仓库的 Actions 里单调；fork 自己发版不影响官方仓库的比较。
  - `MSIME_BUILD_RUN` 每次发版都不同，sccache 对 `msime-client-services` 及依赖它的 crate 每次都要重编。
  - 测试版只认 develop 发出的 `-beta`；手工在 main 上勾选 prerelease 的发布既不进稳定版也不进测试版。
- 相关笔记：[检查更新移到 Rust](2026-10-09-update-check-in-rust.md)（部分重叠，本篇在它的选择规则上加通道）；[Windows 7 专用发布](../feature/2026-10-24-windows7-update-channel.md)（部分重叠，Win7 忽略通道）；[nightly 按平台更新](../process/2026-10-16-nightly-partial-platform-updates.md)（部分重叠，状态块加 `version`、`run`）；[Linux 面板与设置对齐 macOS](../feature/2026-10-20-linux-panels-and-setup-mac-parity.md)（部分重叠，后台检查的规则下沉到这里，Linux 设置应用改调共享实现由 Linux 切片完成）。

## Verification

- `rbuild cargo test -p msime-client-services --lib update_check`：40 项通过，新增覆盖稳定版不受 beta/alpha/nightly 影响、测试版只收恰为 `-beta` 的 tag、nightly 状态解析与 Windows 安装程序和 Linux 按架构的 `.deb` 匹配（`.rpm`、legacy、别的版本不选）、附件缺失或摘要不符、外仓或未来版本的状态、（版本号，运行 ID）比较与「不知道自己运行 ID 时只比版本号」、标记与 `MSIME_BUILD_RUN` 的解析（标记只认正文最后一行，混在更新说明里的同名标记不算，CRLF 正文照常读出）、windows7 忽略通道、未知通道报错、旧形状的 `ReleaseUpdate` 和旧记录照常读出、通道切换使记录失效、同一构建只提醒一次、另一个进程持锁时跳过。
- `rbuild cargo test -p msime-client-core`：偏好的缺省值、不写入、未知值按稳定版、恢复默认，以及设置文件不导出也不导入通道。
- `rbuild cargo test -p msime-host-api --lib`：两个新 ABI 在开发构建里返回 `disabled` 且不建目录，`pending` 读同通道记录、换通道后为 null，坏请求全部拒绝。
- `rbuild cargo clippy -p msime-client-core -p msime-client-services -p msime-host-api --all-targets -- -D warnings` 通过。
- `scripts/test-release-build-marker.py`、`scripts/test-windows-release-page.py`（发布步骤原样运行，正文末尾带标记）、`scripts/test-nightly-partial-publishing.py`（`version`、`run` 的写入、保留与非法值拒绝）通过。
- 没有在真实发布或 nightly 上运行：标记与状态字段要等下一次发版才会出现在线上，之前发布的版本只按版本号比较。Windows MSVC 与 Tauri 桌面 crate 没有在本机编译。
