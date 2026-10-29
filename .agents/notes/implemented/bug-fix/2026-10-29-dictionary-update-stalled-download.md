# Agent Note: 词库在线更新的停滞超时、断点续传与用户镜像

Status: implemented

## Problem

#10134：Linux 0.14.0 用户点「检查词库更新」后界面一直停在「正在下载 0 / 258 MiB」，国内直连、VPN、TUN 都一样，只能关掉自动更新。共享 Rust 层 `crates/client-providers/src/dictionary_updates.rs` 的下载与资源包下载器有四处不一致，六个平台都受影响：

- 停滞要等半小时。客户端设了 `.timeout(30 * 60)`，每个词库文件的请求又按请求设了 `30 * 60` 秒。reqwest 阻塞客户端里客户端级 `timeout` 限制的是每次读取（`blocking/response.rs` 的 `wait::timeout(self.body_mut().read(buf), timeout)`），连接没有数据时要等 30 分钟才换下一个源，CDN 和原地址两个源最长约一小时。按请求设的 `timeout` 则会进入异步请求的 `TotalTimeout` 扩展，在 `async_impl/body.rs` 里变成整次传输（含正文）的截止时间：慢而稳定的连接下完 70 MB 的拼音库超过 30 分钟就被掐断，从头再来。
- 不能续传。每个文件写进匿名暂存文件（`tempfile::tempfile_in`），失败或换源时 `set_len(0)` 从零下载，下次检查也从零开始。
- 不用用户填的镜像。只用 `PROJECT_MIRRORS`（`https://dl.msime.app/gh/`），而资源包下载会把偏好 `voice_input.asr_model_mirror` 排在前面。
- 进度到不了总量。`total_bytes` 计入全部文件，从本机复用的文件不计进 `downloaded_bytes`，已存在的代次（`ResourceStore::install` 提前返回）也不计。下载途中关掉开关也停不下来，只是下完后不发布。

## Decision

词库文件的下载改走资源包下载器的同一套机制，manifest 和版本 API 保持原样：

- `Transport` 分成两种来源。`open(url, timeout)` 只给版本列表和 manifest 这类小文档用，按请求 30 秒总时限，不变。新增 `files()`，交出一个 `local_models::Fetcher`；生产环境是 `local_models::HttpFetcher`，与资源包同一个客户端配置：连接 30 秒，客户端级 `timeout(60s)` 只限制每次读取，不按请求设总时限。连接停滞时一分钟内换下一个源，慢而稳定的下载不受时长限制。文档客户端的客户端级超时也从 30 分钟改为 60 秒，它只在每次读取时生效，按请求的 30 秒照旧是总时限。
- 断点续传复用 `local_models::download_from_sources` / `download_resumable`。新增 `local_models::download_partial`：文件下进 `<state>/dictionary-updates/.partial-downloads/<sha256>-<name>`，已有的部分用 HTTP Range 接着下，摘要不符就删掉重下，规则与资源包完全相同。交出的 `PartialFile` 读完丢掉时删除自己（Windows 上先关句柄再删），所以完整下完的文件在被 `ResourceStore` 复制进代次后立即释放，同一时刻多占的磁盘与原来匿名暂存文件一样，只有一个文件。`retain_partials` 在开始下载一个选择前删掉不属于这次选择的旧部分（新版本发布后旧摘要续不上），检查结果为 `current` 或 `ready` 时清空整个目录。失败或取消时保留，下次检查接着下。
- `UpdateContext` 增加 `mirrors`。host-api 只在 `automatic` / `check` 真正起任务时从实时偏好文件读 `voice_input.asr_model_mirror`（`PreferencesStore::load`），读不了时退回宿主文档里的偏好副本，用户改了镜像不用等宿主重新准备。providers 用 `valid_model_mirror` 过滤后交给 `resource_packs::download_prefixes`，顺序为用户镜像、项目 CDN、GitHub 原地址，只作用于 `DOWNLOAD` 前缀的 Release 附件（含 manifest）。版本 API 始终直连，选择记录继续保留 GitHub 原地址。
- 进度：从本机复用的文件按整份大小计入 `downloaded_bytes`；每个代次安装成功后 `downloaded_bytes` 设为该代次开始时的值加上它全部文件的长度，所以成功时进度正好停在 `total_bytes`。
- 停下：检查开始时自动更新开着（`enabled_at_start`），下载途中被关掉，就视为用户要求停下。每个文件开始下载前查一次开关，有数据进来时每写一次状态（约每 1 MiB 或文件结束）查一次，通过 `download_partial` 的 `cancel` 中止。停滞的连接收不到数据，进度回调不会被调用，所以另外两处兜底：`SwitchAware` 包住文件来源，每向一个源发请求前查一次开关，读取超时后 `download_from_sources` 换下一个源时就以取消结束，不再试其余的源；最后一个源读取超时报的网络错误，如果这时开关已关，也按取消报。结果是 `UpdateError::Cancelled`，在 `check_with` 里折成 `phase = "cancelled"`、不报错、不发布，已收到的字节留给下次续传。自动检查开始时开关必然开着，所以自动检查的行为与以前「关开关后不发布」一致，只是不再先下完整套；开关关着时发起的手动检查不受开关影响，与原来一样。
- `install` 记下下载函数自己的错误，`ResourceStore` 失败时报这个原因，不再一律报 `dictionary_update_integrity`：网络失败报 `dictionary_update_network`，取消能被识别出来。

用户字面上说的「下载链接太卡」，即 CDN 和 GitHub 在用户网络里的速度，代码改不了。两个都不通时只能靠用户镜像，或 #9529 已经提供的手动下载清单（见 [Linux 词库下载链接](../feature/2026-10-24-linux-dictionary-download-links.md)）。

## Alternatives considered

只把两处 `30 * 60` 改成 60 秒，改动最少。但按请求设的 `timeout` 在 reqwest 阻塞客户端里是整次传输的截止时间，改成 60 秒等于要求 70 MB 的拼音库在一分钟内下完，国内慢网络上每个文件都会失败。所以大文件不能按请求设时限，只能靠客户端级的每次读取时限，这正是 `HttpFetcher` 的配置。

在 `dictionary_updates` 里自己实现 Range 续传，只用一个 reqwest 客户端，依赖面最小。但 `local_models` 已经有经过测试的续传逻辑：206 的 `Content-Range` 校验、416 后整份重下、源不支持 Range 时从头写、续出来的摘要不符时删掉重下，以及 Unix 上按目录描述符操作、不跟随符号链接的 `PartialDirectory`。另写一份迟早和资源包分叉，所以复用并只暴露 `download_partial` / `retain_partials` 两个入口。

完整下完的部分文件一直留到整次更新发布后才删，实现最简单，失败后重试也能少下一些。代价是同一时刻要额外占一整套词库（约 258 MiB）的磁盘，移动端吃不消；改为复制进代次后立即删，峰值与原来的匿名暂存文件相同。代价是只有正在下的那一个文件能续传：同一套里排在前面、已经复制进暂存代次的文件，在后面的文件失败时随 `ResourceStore` 丢掉的暂存目录一起没了，下次检查要从零重下。桌面这一套有 74 MB、67 MB 和两个 12 MB 等几个大文件，在 #10134 那种反复断线的网络上，前面的大文件下完、后面的文件每个源都停滞，下次检查又要重下前面的 74 MB，这不是少见路径；连接一直这样掉，整次更新可能始终下不完。这时用户能走的仍是用户镜像或 #9529 的手动下载清单。如果之后还有人报告下不完，再考虑只在桌面上把下完的文件留到整套装好。

取消也可以加一个独立的 `cancel` 命令和界面按钮，停得更及时。triage 的 partial_scope 允许先只做「关开关即停」，不新增按钮和跨平台的界面改动；Windows 原生设置、共享 React 面板都不用改，它们已经认识 `cancelled`。一个连接停滞时最多要等这一次连接（30 秒）或读取（60 秒）超时才能停下，之后不再换源，可以接受。只在进度回调里查开关是不够的：最早的实现就是这样，审查发现停在 0 字节时回调一次也不会调，读取超时后照样去试 CDN 和原地址，最后报「更新失败」，正是 #10134 的场景。

镜像也可以从宿主文档里的偏好副本读，不用再碰偏好文件。但那份副本是准备宿主那一刻的，用户在设置页改了镜像后它不会立刻更新；只在真正起任务时读一次实时偏好，状态轮询（每两秒）不读。

## Consequences

收益：连接停滞一分钟内就换源或报失败，不会在 0 MiB 停半小时以上；中断的下载下次检查接着下；用户填的镜像对词库更新也生效；成功时进度停在总量；关掉开关能停下下载。改动都在共享 Rust 层，六个平台一起受益，界面和原生宿主不用改。

代价：`dictionary-updates/` 下多一个 `.partial-downloads/` 目录，失败或取消后会留着没下完的部分（最多一个选择的文件，约 258 MiB），直到下次检查续传、版本变化被清理，或检查结果为最新时清空。关掉自动更新（`configure(false)`）和回退（`rollback`）都不清它：关开关正是停下载的办法，这时下载线程可能还拿着这些文件，删掉既有竞态，也丢了续传；所以照 #10134 的绕过办法关掉自动更新、之后再也不手动检查的用户，这些字节会一直留着。iOS 上 `dictionary-updates/` 没有像资源包、模型目录那样调用 `IOSSharedPaths.excludeFromBackup`，这部分字节也会进 iCloud 备份。HarmonyOS 键盘只镜像 `generations/` 和 `installed.json`，不受影响。文件下载的 User-Agent 从 `msime-dictionary-updater` 变成与资源包相同的 `msime/<版本>`，版本 API 和 manifest 的请求不变。

验证：`cargo test -p msime-client-providers --lib dictionary_updates` 覆盖用户镜像先于 CDN、不合法镜像被跳过、用户镜像不可用时落到 CDN；CDN 停滞后换到原地址并从已收到的字节用 Range 续传；失败后下次检查续传，新版本发布后旧部分被删；从本机复用文件时进度到总量；下载途中关开关停下、不发布、保留已收到的字节，之后开关关着的手动检查不重新下载。停滞的模拟是读完前几个字节后报 `TimedOut`，与 reqwest 每次读取超时的错误同类；60 秒这个数值本身和真实网络下的表现没有在设备上验证。

## Related notes

[全平台词库独立更新](../feature/2026-10-18-dictionary-auto-update.md) 持有更新流程、CDN 优先和逐文件换源的原决定，其中「匿名暂存文件」一段对词库文件已被本篇取代（manifest 仍用匿名暂存文件）。资源包的续传与镜像顺序来自 `resource_packs::install` 和 `local_models::install_files`。
