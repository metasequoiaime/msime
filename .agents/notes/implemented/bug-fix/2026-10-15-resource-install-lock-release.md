# Agent Note: 资源安装结束时显式释放安装锁

Status: implemented

## Problem

`ResourceStore::install` 持有裸 `File`，返回时仅关闭当前描述符。Unix 的 `flock` 跟随打开文件描述，复制或 `fork` 继承的描述符仍能保留锁，让后续资源安装继续等待。

## Decision

Unix 在成功取得锁之后构造私有 `ResourceInstallLock`，析构时复用 `file_lock::unlock`。守卫声明早于 staging，保证错误返回和栈展开时先清理 staging，再释放锁。Windows 保持现有关闭句柄的释放方式。

## Historical notes

- [资源安装根目录绑定](../../implemented/bug-fix/2026-10-09-resource-install-root-binding.md)与 staging、清理、代次目录句柄笔记部分重叠，继续约束锁和文件操作的目录身份；本篇只规定安装结束时的锁释放边界，不替代其决定。
- [本地模型锁释放](../../implemented/bug-fix/2026-10-15-local-model-lock-release.md)、[个人词库锁释放](../../implemented/bug-fix/2026-10-15-personal-dictionary-lock-release.md)、[词库访问锁释放](../../implemented/bug-fix/2026-10-15-dictionary-access-lock-release.md)部分重叠，复用同一 Unix 解锁语义，各自保留不同的操作边界。
- 检索活跃笔记未发现资源安装锁释放的已有归属，没有完全吸收或需要废弃的提案。

## Alternatives considered

- 继续依赖描述符关闭：实现最少，正常无副本时已能工作；但无法让释放时间跟随安装操作结束，继承副本可以延长阻塞。
- 在返回路径显式解锁：正常结束可传播解锁错误；但缓存命中、多个 `?` 和栈展开分别处理容易遗漏，且需要额外保证 staging 清理先于解锁。

## Verification

描述符副本回归在旧行为下因安装结束后仍占锁而确定性失败，修复后资源模块 31 项通过。实际安装回归验证下载期间仍互斥，成功、下载失败、校验失败和缓存命中后可重新取得锁，并确认未留下 staging；副本回归验证旧副本关闭不释放后来竞争者的锁。完整 client-core 单测 1452 项通过、2 项忽略，另有 6 项集成测试通过；host-api 单测 633 项通过、2 项忽略，另有 15 项集成测试通过。两 crate 严格 Clippy 通过。Android API 28/35 核心单测各 1398 项通过、13 项忽略，7 项硬链接用例由既有设备脚本过滤；新增两项回归均在设备上执行通过。本地 quick 门禁通过，包含 macOS 原生宿主与共享 Apple bridge 构建、Linux 桌面编译、Linux 原生 87 项测试、Android 宿主与 wasm 编译；HarmonyOS 真编译缺前置环境，Windows 原生和 pipe-only 缺 MinGW，按脚本跳过。最终完整开发 APK 的 API 28 `smoke.sh emulator-5566 --core` 与 API 35 默认 `smoke.sh emulator-5580` 均通过；没有跳过默认套件，未启用额外设置、统计和手写套件。推送钩子的 quick 复查也通过。

## Device setup

首轮 Gradle 构建使用本机默认 JDK 25 时以 `25.0.2` 失败，指定已安装的 JDK 21 后成功。首轮瘦包缺少注音词库，API 35 默认验收及隔离复跑均在注音九键布局断言失败。默认完整验收需要这些语言数据：本批按 `resources/language-dictionaries.lock.json` 下载并校验粤拼、注音、笔画词库和许可文本，用 `MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1` 构建完整开发包后两级验收均通过，原先失败的注音九键布局也通过。首轮日志保留，不计为通过。

## Consequences

其他共享存储的作用域释放由[共享文件锁守卫](2026-10-15-scoped-file-lock-release.md)统一处理；与本篇部分重叠，各自保留目录身份或租约边界的决定。

安装结束后描述符副本不再延长互斥，后续安装可以继续。析构无法返回解锁错误，失败仍由描述符关闭兜底。回归复现描述符共享语义，不声称捕获实际产品的 `fork` 现场，也不改变安装的目录身份检查和资源格式。
