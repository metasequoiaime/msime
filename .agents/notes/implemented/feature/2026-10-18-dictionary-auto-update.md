# Agent Note: 全平台词库独立更新

Status: implemented

## Problem

词库版本编译在资源锁里，补词后即使已有 Release，用户仍必须等待客户端发版。六个平台需要相同的兼容判断、校验、节流和回退规则。

## Decision

构建锁继续固定随包基线。设置宿主经现有词库请求 updates 启动共享 Rust 后台任务，默认自动更新，每天检查一次；输入进程只读已安装代次，不联网。使用官方仓库 HTTPS Release API 的资产摘要校验 manifest，再交叉核对产物摘要、格式版本、日文 magic 和许可状态。仅更新安装版本有的组件；保留原来固定的模型。

不可变 Release 附件（含 manifest）复用 `resource_packs::PROJECT_MIRRORS`，优先从现有 `https://dl.msime.app/gh/` CDN 获取，网络、HTTP、长度或摘要失败时逐文件回退 GitHub 原地址；版本 API 不经过永久缓存 CDN。复用 `local_models::download_from_mirrors` 的换源与校验规则，摘要始终来自官方 HTTPS API，CDN 不提供信任依据。每个文件先流式写入更新目录中的匿名暂存文件，核对后再交给 ResourceStore；换源截断暂存字节并重算当前文件进度，选择记录与资源代次继续保留 GitHub 原地址，不因下载渠道变化而换代。

完整下载到独立不可变目录后原子发布选择记录，下一次宿主准备或启动刷新时采用；工作词库通过现有 prepare_options_for 从持久日志回放个人词条和学习。下载失败保留当前选择，格式不兼容时寻找更新的兼容版本，已安装资源损坏时使用随包基线。提供自动开关、状态、手动检查与回退，回退关闭自动检查防止马上升回去。

鸿蒙只随 `installed.json` 的变化镜像已完成的不可变代次，先发布文件再发布选择，不复制 `incoming-*` 暂存目录；设置桥继续观察任务完成，切换设置页不会丢掉发布。键盘在焦点边界结束组字、重建会话后采用自己的镜像与用户日志。iOS 和鸿蒙查询更新状态不准备工作词库，也不获取输入会话的维护锁。

资源路径先消除系统允许的路径别名（例如 macOS 的 `/var` 与 `/private/var`），旧会话的每个下载代次保留自己的资源清单供快照校验。可选语言资源各用独立校验标记，避免主词库与语言词库互相挤掉缓存；音效、插件、独立模型与离线释义继续从安装包或原有资源包解析。

## Alternatives considered

继续随客户端发版能复现全部输入，但补词等待过长。直接覆盖现用 SQLite 文件最省空间，但 Windows 映射和各平台活跃会话使其无法安全替换；因此用分代安装。只信下载文件携带的摘要无法确定来源，因此入口固定为官方 GitHub 仓库，摘要来自 HTTPS API 并与 manifest 交叉检查。

附件直接走 GitHub 的代码最少，但绕过项目已有的国内 CDN，国内用户更新缓慢或失败；因此下载采用现有 CDN 优先与原站回退。只在建连失败时换源能直接流入 ResourceStore，却无法在 CDN 回了错误字节时重新取原站，改为逐文件校验后交给安装器。直接把文件装入内存也能重试，但大词库会明显提高移动端内存峰值，所以使用匿名暂存文件。

## Verification

共享词库测试通过（providers 14 项、host-api 142 项），包括真实 SQLite 新代次加载、个人词条、内置词权重与删除、英文学习回放、回退及损坏后使用基线；clippy 无警告。桌面与鸿蒙共享界面类型检查、构建和 6 项界面测试通过。Android API 28 与 35 模拟器均通过 client-base、client-core、client-providers、client-services 整套单测；正式 APK 构建与签名校验通过。

CDN 补验：providers 全套 222 项通过（2 项显式联网测试默认跳过），clippy 无警告；词库更新 17 项覆盖 CDN 优先、网络失败、长度与摘要错误后的逐文件回退、双源损坏保持旧代次和不重复累加下载进度。Android API 28、35 都运行并通过这 222 项 providers 测试，覆盖匿名暂存文件的设备文件系统行为。显式运行 `live_project_cdn_delivers_verified_dictionary_update --ignored --nocapture`，在 macOS 和 Android API 35 上从真实 GitHub API 检查版本，再仅从 `dl.msime.app` 取得 `dict-v2.0.30` 的 manifest 和 1,314,816 字节的 `msime-others.db`，摘要和安装目录校验通过；附件未请求 GitHub 原地址。线上测试只用临时安装目录，不改用户当前词库。

CDN 补验的 Android API 28 四个共享 crate 整套通过；API 35 的 base、core、providers 通过，services 在最后一项统计存储测试期间因模拟器掉线而中断，不能算整套通过。宿主门禁中 Windows staged runtime 初跑因本机磁盘不足失败，清理旧产物后单独重跑 10 项通过。

鸿蒙逻辑和 C++ 套件、ArkTS 规则检查、iOS 修改文件的 Swift 语法解析通过。原生 WinUI、完整 iOS App/键盘与鸿蒙 HAP 仍需要对应构建及设备验收；Linux 原生宿主和设置外壳容器检查受本机没有 Docker 限制。这些缺口不能由共享层测试或语法检查替代。

`scripts/verify-local.sh --quick` 通过，包含 macOS 原生宿主与共享 Apple 桥编译；Windows 交叉构建因共享依赖锁被其他任务占用而跳过，宿主可运行的 168 项 Windows 原生测试通过。

## Consequences

新代次需要额外磁盘空间。更新只在设置 App 有运行机会时检查，移动系统暂停进程期间不承诺后台调度；文件已准备好后重开键盘生效。保留旧代次以供回退，不在活跃输入会话里清理。

CDN 暂存最多额外占一个附件的磁盘空间，文件句柄关闭即清理；ResourceStore 复制时再校验一次。CDN 不可用时保留原站路径，不要求修改 CDN Worker 或部署；GitHub 版本 API 的可用性仍是检查更新的前提。

## Related notes

现有日本模型进程缓存决定与这里的不可变安装部分重叠，保留 [.agents 日本缓存](../../implemented/architecture/2026-10-10-japanese-model-process-cache.md)。[鸿蒙沙箱](../../implemented/architecture/2026-10-09-harmony-ime-shared-sandbox.md)约束键盘与设置 App 的数据交换，继续适用；更新资源不得只存在设置进程不可读的私有目录。[Rust 更新检查](../../implemented/architecture/2026-10-09-update-check-in-rust.md)只处理 App 安装包，与数据更新职责独立。
