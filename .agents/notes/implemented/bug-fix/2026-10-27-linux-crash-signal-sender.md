# Agent Note: Linux 崩溃记录写明别人发来的信号是谁发的

Status: implemented

## Problem

崩溃组 `a71ba81276ea76b9`（Linux，`SIGSEGV: segmentation fault`）近 7 天 13 次、13 台设备，首次出现在 2026-10-05，样本全是 aarch64 上的 IBus 宿主：ibus 的信号回调调 `msime_client_destroy`，在 `libmsime_host_api.so` 里走十几层后停在 libc 的 `__open64+0xdc`（libpthread）或 `__xstat64+0x14`。`__open64` 在 libpthread 里说明 glibc 早于 2.34，大多是麒麟、统信这类国产 ARM 发行版。

逐项排查后，这不是宿主的内存错误：

- 用 Debian 10 arm64 的 glibc 2.28 反汇编，两个崩溃位置都恰好是 `svc #0` 之后那条 `cmn x0, #0x1, lsl #12`，比较指令自己不会出错。信号是在 open、stat 系统调用返回时投递的。
- 0.14.0 的处理函数在 `si_code > 0` 时会在摘要后写 `(code N)`（`SEGV_MAPERR` 是 1，内核 `force_sigsegv` 是 `SI_KERNEL`，128）。13 份报告都没有，所以 `si_code <= 0`：SIGSEGV 是某个进程用 `kill`、`tgkill` 或 `sigqueue` 发来的。
- 仓库里没有任何产品代码向宿主发 SIGSEGV：启动器只转发 SIGTERM，设置应用和打包脚本都不发这个信号。
- 栈不深，崩溃处理函数在 `sigaltstack` 上运行，排除了栈溢出。

发信号的是系统里另一个进程，崩溃记录却只写了 `SIGSEGV: segmentation fault`，看不出是谁。发布的 `libmsime_host_api.so` 也去掉了符号表，五个样本还来自四个布局不同的构建，没法符号化。

## Decision

POSIX 崩溃处理函数（`platforms/common/Telemetry.cpp`，Linux 的 IBus 宿主和 Fcitx5 插件共用）在信号来自 `SI_USER`、`SI_QUEUE` 或 `SI_TKILL` 时，在摘要后写明来源：发送者是本进程时写 `(raised by this process)`，否则写 `(sent by <进程名>)`，进程名读 `/proc/<si_pid>/comm`，只接受可打印 ASCII，读不到时写 `another process`。只用 `open`、`read`、`close`、`getpid`，仍然可以在信号处理函数里调用。`si_code > 0` 的路径不变。PRIVACY.md 同步说明。

不写 pid：服务端分组会掩掉数字，pid 也查不到是谁。进程名是系统里另一个程序的名字，不含用户数据。

## Alternatives considered

- **改销毁路径，不在 `msime_client_destroy` 里做文件 I/O**：如果是 I/O 本身有问题，这样能直接避开。否定：证据表明信号来自外部，I/O 只是信号到达时恰好在做的事；把落盘挪走会丢掉销毁时该写的学习记录，也不会让外部的 SIGSEGV 消失。
- **把别人发来的信号归为「被系统结束」（摘要以 `Killed` 开头），不计入崩溃率**：和 iOS 对 SIGKILL 的处理一致。否定：发 SIGSEGV 的很可能是卡死看门狗一类，它要的就是一次崩溃转储，背后可能是宿主卡住。先查清是谁、为什么，再决定怎么归类。
- **同时记录 pid 或发送者的命令行**：信息更多。否定：pid 事后没有意义；命令行可能带路径和参数，涉及用户数据。

## Consequences

装了这个版本的 Linux 宿主再遇到这类 SIGSEGV 时，崩溃组标题会带上发送者，例如 `SIGSEGV: segmentation fault (sent by xxx)`，按发送者自成一组，可以据此去查是哪个系统组件、在什么条件下发的。代价是标题变了，同样的事件会进新的崩溃组。`raise()` 产生的崩溃（例如测试，或别的库用 `raise(SIGSEGV)`）也会写明是本进程发的。

## Verification

- 在 aarch64 Linux 容器（Debian bookworm）里，把真实的 `Telemetry.cpp` 链接到 Host API 遥测调用的桩上，用三种方式触发：`raise(SIGSEGV)` 记为 `(raised by this process)`；真实的空指针写入仍是 `(code 1)`；另一个进程 `kill` 发来的记为 `(sent by driver)`，子进程照样被 SIGSEGV 结束。
- `platforms/common/tests/telemetry.cpp`：原有用例改为断言 `raise` 的摘要，新增父进程 `kill` 子进程的用例（单独的目录，直接读崩溃记录）；`platforms/linux/tests/core/ibus_startup_telemetry.py` 外部发 SIGSEGV 的断言改为写明测试进程的名字。两者都只能在 Linux 上跑，由 CI 的 Linux 原生宿主任务运行；本机只在 macOS 上做了语法检查。
- `scripts/run-checks.sh` 通过。

## Related notes

检索了活跃笔记里的崩溃上报、信号处理与 Linux IBus 宿主的停止信号：[MetricKit 崩溃诊断按进程号对回键盘会话](2026-10-25-ios-crash-diagnostic-session-attribution.md) 部分重叠，本篇沿用它「先把系统强制结束和真崩溃分开」的思路，但在查清发送者之前不改归类；没有被本篇吸收或需要否决的旧笔记。
