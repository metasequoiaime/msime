# Agent Note: 建会话在宿主的小栈线程上切到自己的栈

Status: implemented

## Problem

#11893：Windows 上水杉是当前输入法、中文状态，在 Unreal 编辑器里打开或新建文档时进程以 `0xc00000fd`（`EXCEPTION_STACK_OVERFLOW`）退出。0.6.1 已经把病根定位到建会话链上四层 constructor 各自的大栈帧，并把 `InputSession` 的直接构造搬到堆上（[建会话别再把 16 KiB 的 `InputSession` 摊在输入线程栈上](2026-11-25-session-construction-heap-stack.md)），小栈验收同时从 256 KiB 收到 192 KiB。

报告者用 `v0.6.1-nightly.45778076004.1`（`headSha` `3d5cd82838`，是 `3fddadb48b` 的子孙，确实带着那次改动）复验，仍然崩。用同一个 run 的符号包（`msime-symbols-windows-full-0.6.1`）按 RVA 还原两次崩溃的 dmp，栈顶 13 帧与栈帧大小（release，从 PE 的 `.pdata` unwind 信息里读出来）：

| 帧 | 函数 | 栈帧 |
| --- | --- | --- |
| 1 | `__chkstk+0x37`（探测页时撞到保护页） | — |
| 2 | `msime_engine::session::input::InputSession::new` | 39,576 B |
| 3 | `msime_engine::session::Session::new` | 11,464 B |
| 4 | `msime_engine::host::session::Session::new` | 9,768 B |
| 5 | `msime_host_api::ffi::session::create_session` | 49,368 B |
| 6 | `response<msime_client_create::{closure}>` | 488 B |
| 7 | `msime_client_create` | 72 B |
| 8–13 | `EngineSessionAdapter::create`、`create_host_session`、`InitializeHostSession`、`SetupLanguageProfile`、`_AddTextProcessorEngine`、`ActivateEx` | 约 1.5 KiB |

2–5 同时保持，合计 110,176 B。而崩溃线程的整条栈只有 86,536 B 和 85,704 B（minidump 的栈描述就是 RSP 到 StackBase，两个 dump 的栈顶都落在页边界上），宿主自己（MSCTF 9 帧、user32 12 帧、IMM32、UE WndProc）在这段里还压着约 16 KiB，建会话实际可用余量约 70 KiB。

所以堆化只削掉 2.7 KiB（`InputSession::new` 从 42,280 B 到 39,576 B），192 KiB 的验收预算比宿主真实栈大一倍多，从一开始就拦不住这条链；`create_session` 自己的 49 KiB 帧也还在。另外 43 帧是完整记录（`frames.total=43 unresolved=0`），各模块帧数（host_api 7、TSF 6、msctf 9、user32 12、ntdll 3、win32u 2、imm32 1、UnrealEditor_ApplicationCore 2、kernel32 1）与线性链一一对应，没有重复：不是 `OnSetFocus` / `ImmAssociateContext` 重入，也不需要加重入哨兵。

## Decision

`crates/host-api/src/ffi/session.rs` 的 `create_session` 拆成两层：

- 外层 `create_session` 只剩 `stacker::maybe_grow(256 KiB, 1 MiB, …)`。剩余栈不足 256 KiB 时，在**同一条线程**上切到一块 1 MiB 的栈，构造完再切回来。
- 实体 `build_session` 带 `#[inline(never)]`。它的栈帧就是那条一百多 KiB 的构造链，内联回外层就等于仍旧在宿主栈上展开。

两个常量按现场量到的数定的：`SESSION_STACK_RED_ZONE = 256 KiB` 大于 release 下 110 KiB 的构造链并留一倍余量，宿主用 512 KiB 主线程的记事本、Chromium 都不会切栈；`SESSION_STACK_SIZE = 1 MiB` 留了将近十倍的余量，debug 构建也放得下。

切的是栈不是线程：线程本地状态（`SESSIONS`、词库 journal 的线程缓存）、会话的 `!Send` 限定、返回后接着用会话的调用方都不受影响。`stacker` 在切换点用 `catch_unwind` 接住 panic，回到原栈后再 `resume_unwind`，panic 不会跨过手动切换的栈边界。

## Alternatives considered

- **继续削帧，把 `HostSession`、`Runtime`、`EngineOptions` 也搬到堆上。** 最强理由是不引依赖，且上次已经走通了一次堆化，改动只看 diff 就懂。但现场缺口是 110 KiB 对约 70 KiB，要再砍掉四成，改动会横跨 host-api 和 engine 的整条构造链，需要多轮实测；而且构造链每加一个大字段就可能重回悬崖，宿主给的栈没有下限，这条路没有终点。
- **把建会话挪到独立的大栈线程，建完再交回输入线程。** 最强理由是彻底不占宿主栈。不行：`host::session::Session` 是 `!Send`（`PhantomData<Rc<()>>`），会话表 `SESSIONS` 也是线程本地的，别的线程建完再交回等于绕过这两条约束。
- **在 TSF 侧延后到第一次按键才建会话。** 同一条栈，只把崩溃推到更难复现的位置。
- **只把 `create_session` 的 49 KiB 帧拆小。** 拆完仍留下约 60 KiB 的链和十几 KiB 的系统帧，没有余量，报告者那条线程照样崩。

## Consequences

- 新增依赖 `stacker 0.1.25`（连带 `psm` 与 `cc` 构建依赖，Windows 上用它自带的汇编写栈切换）。只加在 `crates/host-api`，不改变导出符号和 C ABI。
- 只有剩余栈不足 256 KiB 时才分配那 1 MiB；常规宿主照旧在原来的栈上跑，没有额外开销。
- 构造链以后再变大也不会崩，直到 1 MiB 也用完；96 KiB 的验收测试会在那种情况下先红。
- 每次在极小栈的宿主上建会话多一次 1 MiB 的分配与释放，相对建会话本身要读的词库、模型可以忽略。

## Testing

- `cargo test -p msime-host-api --test host_options_small_stack`：验收从 192 KiB 改成 `session_creation_fits_a_96_kib_thread_stack`，96 KiB 就是 Unreal 那条线程的整条栈。实测：临时把 `create_session` 改回直接调用 `build_session`（不经 `stacker`）时，这条测试以 `thread has overflowed its stack`（SIGABRT）失败；改回 `maybe_grow` 后通过，建会话、销毁和两条拒绝路径都走完。
- 同一条测试在 `ci-platforms.yml` 的 Windows 作业里以 `cargo test --locked -p msime-host-api --test host_options_small_stack` 真跑，所以 MSVC 下 `stacker` / `psm` 的汇编也会被这个作业编到并跑起来。
- 本机没有 Unreal 编辑器，也没有 Windows 的小栈线程可复现，所以真机验收仍靠报告者在含本改动的 Windows nightly 上复验。
