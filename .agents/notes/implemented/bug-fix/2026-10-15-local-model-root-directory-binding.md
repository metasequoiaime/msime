# Agent Note: 本地模型写操作绑定已打开的根目录

Status: implemented

## Problem

本地模型安装、文件包安装、归档成员安装、收编和删除虽然已经把 staging、partial、发布和清理的大部分步骤改成目录句柄操作，但锁取得之后仍有若干入口按 `root` 路径重新打开根目录。根目录被替换成另一个真实目录时，路径操作可能与取得锁的目录不是同一个对象；发布还可能把结果路径指向替换后的目录。

## Decision

Unix 上成功取得模型锁时同时打开并保留模型根目录句柄，锁文件也相对于该句柄打开。所有模型写入口都用该句柄创建 staging、清理残留、创建 partial 和发布；发布前后核对路径当前仍指向原目录，目录身份变化就返回 `InvalidRoot`。Windows 保留路径实现。现有公开接口、模型目录格式和锁文件名称不变。

发布先打开 staging，再移动旧模型；旧模型是否存在直接依据相对根目录句柄的 `renameat` 结果，不通过完整路径另做检查。这样 staging 已丢失时旧模型仍留在原位，后续发布改名失败也能走原有恢复分支。

旧模型保留到发布后的身份检查成功为止；若检查失败，先把新模型移回同一个 staging，再恢复旧模型，之后交给安装入口原有的 partial 或来源恢复逻辑。这个顺序保证「返回失败」不会同时删除旧模型、又把新模型文件抽回 partial。

发布和删除使用 `renameat`、`openat` 及相对于已打开目录的清理，利用目录描述符在路径被重命名后仍指向原目录的语义；依据为 [POSIX openat](https://www.man7.org/linux/man-pages/man3/openat.3p.html)、[POSIX renameat](https://www.man7.org/linux/man-pages/man3/renameat.3p.html) 和仓库 `resources::ResourceStore` 的根目录身份检查模式。

## Historical notes

- [本地模型 staging 创建绑定父目录句柄](2026-10-09-local-model-staging-directory-handle.md)：本篇把同一绑定扩展到锁、发布和所有安装入口。
- [本地模型发布绑定根目录句柄](2026-10-09-local-model-publish-directory-handle.md)：本篇保留其发布时使用 `renameat` 的做法，并补上锁取得后的根目录身份契约。
- [本地模型锁释放](2026-10-15-local-model-lock-release.md)：负责操作结束时释放锁；本篇负责持锁期间所有写入对象属于同一个根目录。

## Alternatives considered

- **每个路径操作前再次检查根目录**：能复用现有路径 API，改动最少；但检查与下一次打开或改名之间仍然存在竞态，不能保证锁、staging 和发布属于同一目录。
- **只给发布步骤增加目录句柄**：能集中修改一个入口并保护最后一次改名；但锁文件、残留清理和 partial 仍可能作用于替换后的目录，操作边界不一致。
- **把根目录替换视为正常切换并继续返回路径**：能让目录迁移时的进行中操作继续完成；但会让调用者拿到指向新目录的路径，而实际写入对象在旧目录，安装状态不可解释，因此改为拒绝身份变化。

## Consequences

模型写操作在 Unix 上相对于锁取得时的根目录执行；根路径被重命名或替换时不会把模型数据、partial 或清理动作重定向到新目录。目录身份变化会使安装失败并由 staging 守卫清理，调用方不会收到一个指向错误目录的成功路径。Windows 不具备同样的目录句柄接口，继续使用原有路径和符号链接拒绝策略。

## Verification

新增安装入口回归在 `verify` 回调里把根路径替换为另一个真实目录，并放入合成的既有模型。对基线实现运行时，该模型被挪走，读取哨兵文件以 `NotFound` 失败；修复后新目录保持不变，安装返回 `InvalidRoot`。另一个回归确认根目录身份不符时直接发布也返回 `InvalidRoot`。staging 缺失回归先复现旧模型路径消失，再验证修复后哨兵文件完整保留。

独立审查指出发布后的身份检查必须先于旧模型删除。将这一生产阶段提取为 `finish_publish` 后，确定性回归先复现旧模型被新模型覆盖；修复后要求旧模型恢复、新模型回到 staging、替换目录保持原样。

最终 `cargo test -p msime-client-core --locked` 为 1450 passed、2 ignored，另有集成测试通过。Android API 28 和 API 35 的完整 client-core 设备单测各为 1396 passed、13 ignored、7 filtered out（设备脚本跳过 SELinux 无法准备的硬链接用例）。

`cargo test -p msime-host-api --locked` 为 633 passed、2 ignored，集成测试通过；严格 `cargo clippy -p msime-client-core --lib --locked -- -D warnings`、`cargo fmt --all --check`、`git diff --check` 和 `pnpm run verify-notes` 通过。

本地 `scripts/verify-local.sh --quick` 通过，包含 Android Java/JNI、Linux 设置组件编译、Linux 原生宿主 87 项测试、macOS 原生宿主及共享 Apple bridge 构建。本机缺少 MinGW 与完整 HarmonyOS 构建前置，相关阶段明确跳过，不视为这些平台的设备验收。

最终原生 APK 在 API 28 完整执行 `smoke.sh emulator-5566 --core`，安装、词库准备、中文上屏、输入限制与剪贴板保存生命周期均通过。首轮在中文上屏断言失败；保留同一安装包，单独运行 `DeviceSmoke` 和随后完整重跑均通过，首轮失败日志一并归档，不将它归因于本批修复。
