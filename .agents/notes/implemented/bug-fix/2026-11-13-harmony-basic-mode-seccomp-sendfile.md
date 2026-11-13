# Agent Note: 鸿蒙基础访问模式下键盘被 seccomp 杀掉：拷贝文件不走 sendfile

Status: implemented

## Problem

鸿蒙用户在系统设置里开启水杉、设为默认输入法后，点输入框键盘不弹出。系统设置里显示已启用，QQ 群里的回复是「打开完整体验模式就好了」，`platforms/harmony/README.md` 也写着「输入法要在 `FULL_EXPERIENCE_MODE` 下才会被框架驱动」。开启输入法时系统默认给的是基础访问模式，所以大多数新用户第一次都会碰到。

那条结论来自 2026-09-21 的一次模拟器观察：基础访问模式下框架打 `panel not create`、`entry is nullptr`，扩展一行 ArkTS 日志都没有。之后键盘里又写了不少「基础访问模式下只读共享目录、只收不发」的逻辑，前提是键盘在这个模式下照常运行，两边一直是矛盾的。

2026-11-13 在真机 JUY-AL50（HarmonyOS，API 26）上复现：`ime -e app.msime.hmos -b` 后在设置的搜索框获得焦点，键盘不弹出；hilog 里 `app.msime.hmos:inputMethod` 在 1.2 秒内以 4 个不同的 pid 起来，每次打完 `module loaded` 就收到 signal 31。Faultlogger 的 cppcrash：

- `Reason:Signal:SIGSYS(SYS_SECCOMP) syscall number is 71`（aarch64 的 `sendfile`）
- 栈：`onCreate`（`KeyboardExtensionAbility`）→ `StagedResources.stage` → `fs.copyFileSync` → `libfs.z.so CopyFile::Sync` → libc++ `filesystem::__copy_file` → `sendfile`

基础访问模式下系统给输入法进程加了 seccomp 白名单，白名单外的调用不返回错误，内核直接杀进程。这个模式下键盘还用独立的扩展沙箱（`/mnt/sandbox/100/+extension-entry-KeyboardExtensionAbility+app.msime.hmos`），完整模式下暂存过的资源在这里没有，所以每次启动都要重新拷，每次都死在同一处。

## Decision

- 键盘进程里的文件拷贝一律走新的 `keyboard/FileCopy.ets`：`openSync` 后按 1 MiB 的块 `readSync`、`writeSync`，处理部分写入。替换了 `StagedResources.stage`、`SharedStateMirror` 的单文件镜像和目录树拷贝、`SharedStateSync` 合并键盘生词本前的暂存拷贝，共 4 处。`StagedResources` 设置应用也用，一并走这里；设置应用自己的 `fs.copyFile`（`pages/Settings.ets`、`HarmonyAccountCloudBridge`）跑在主进程，不受白名单限制，不动。
- `tests/run.sh` 加一条约束：`ets/keyboard` 和 `ets/inputmethodextability` 下（`FileCopy.ets` 除外）出现 `copyFileSync`、`fs.copyFile`、`copyDirSync`、`fs.copyDir`、`moveFileSync`、`fs.moveFile` 就失败。
- Rust 引擎建词库代次的两处文件对文件拷贝（`user_dictionary/generation.rs` 的 `copy_private_file` 和代次暂存）不再用 `io::copy`，改用同文件里的 `copy_by_read_write` 显式读写。标准库在 Linux（OpenHarmony 的 `target_os` 也是 `linux`）上对文件对文件会先用 `copy_file_range`、`sendfile`，失败才退回读写；seccomp 直接杀进程时退回来不及。引擎在键盘进程里建代次，第一次输入会话就会走到。
- README 里「要在完整体验模式下才会被框架驱动」改成真实原因和排查办法（`hidumper -s 1201 -a '-p Faultlogger'` 看 cppcrash 的系统调用号）。

## Alternatives considered

- **引导用户打开完整体验模式（PR #11408）。** 最强的理由是不用碰原生路径，文案一改就能让卡住的用户自救。不采用：基础访问模式是系统默认，键盘在这个模式下本该能用，引导等于把一个崩溃说成系统限制；完整体验模式还会向用户要更多权限。#11408 关闭。
- **只换掉 `StagedResources` 那一处。** 改动最小，崩溃栈里也只有它。不采用：同一类调用在共享镜像里还有三处，基础访问模式下键盘照样导入设置应用的发布，修完第一处就会死在下一处；Rust 的 `io::copy` 是同一个机制。
- **在 Rust 里给整个进程关掉标准库的内核拷贝。** 标准库没有这样的开关，只能在调用处换写法。

## Consequences

- 基础访问模式下键盘能正常弹出、打字；共享沙箱、账号会话等按 `getSecurityMode()` 区分两种模式的既有逻辑不变。
- 大文件拷贝从内核态拷贝变成用户态读写，首次暂存一百多 MB 资源会慢一些、多占 1 MiB 缓冲；只在资源代次变化时发生。
- 白名单里还缺什么系统调用无从查表，只能真机试。别的白名单外调用以后一样会表现为「键盘不弹、没有报错」，排查办法写在 README。
- 插件导入、皮肤文件夹导入、本地语音模型里的 `io::copy` 在设置应用主进程里运行，这次不改；以后如果这些路径搬进键盘进程，要一并换掉。

## Testing

- `bash platforms/harmony/tests/run.sh`：0 failure(s)，新约束对 develop 上原来的 `StagedResources.ets` 能命中。
- `cargo test -p msime-engine --lib user_dictionary`（编译机）：213 项通过。
- 真机 JUY-AL50：修复前 `ime -e app.msime.hmos -b` 后在设置搜索框获得焦点，键盘不弹出，键盘进程 1.2 秒内被拉起 4 次、每次 SIGSYS。装上本分支的调试签名包（覆盖安装）后，同样在基础访问模式下获得焦点：键盘弹出，hilog 有 `attached to editor`，键盘进程始终是同一个 pid；用 `uitest` 点 `n i h a o`，候选栏出「你好、妳好、你好像…」；装包后 Faultlogger 没有新的 cppcrash。切回完整体验模式重复一遍，结果相同。
