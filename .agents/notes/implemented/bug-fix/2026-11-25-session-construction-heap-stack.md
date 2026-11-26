# Agent Note: 建会话别再把 16 KiB 的 `InputSession` 摊在输入线程栈上

Status: implemented

## Problem

Windows 上水杉是当前输入法、中文状态、在某个编辑器里打开或新建文档时，进程直接崩掉，事件查看器里是 `0xc00000fd`（`EXCEPTION_STACK_OVERFLOW`）。报告者（#11893）抓到的 minidump 栈共 43 帧，用 `msime-windows-full-0.6.1` 的 DLL 和符号包按 RVA 还原后，最底下的调用链是：

```
1  _alloca_probe+0x37                      ← 探页时撞到保护页
2  msime_engine::session::input::InputSession::new+0x16
3  msime_engine::session::Session::new+0xd7
4  msime_engine::host::session::Session::new+0x113
5  msime_host_api::ffi::session::create_session+0xc79
6  closure+0xad → 7 msime_client_create+0x27
8  msime::tsf::EngineSessionAdapter::create+0x19a
9  msime::tsf::create_host_session+0x32
10 CCompositionProcessorEngine::InitializeHostSession+0xda
11 CCompositionProcessorEngine::SetupLanguageProfile+0x32c
12 CMetasequoiaIME::_AddTextProcessorEngine+0x168
13 CMetasequoiaIME::ActivateEx+0xb31
14–43 MSCTF → IMM32 → USER32 → Unreal 建窗 → 线程入口
```

这不是递归，是同一条调用链上四层构造函数各自留了大栈帧，它们同时保持，把宿主那条线程的栈吃完：

| 函数 | 入口 `sub rsp` |
| --- | --- |
| `create_session` | 49,224 B |
| `host::session::Session::new` | 9,720 B |
| `session::Session::new` | 39,976 B |
| `InputSession::new` | 42,280 B |

加起来约 141 KiB。帧 1 是 `InputSession::new` 刚进来、`_alloca_probe` 还没把这 42 KiB 探完就撞到保护页，说明进这一帧之前剩下的栈不足 42 KiB。这些大帧不是结构体自身的大小（`InputSession` 16,184 B、`host::Session` 4,664 B），而是构造过程中留在栈上的临时值：`Box::new(InputSession::new(&options)?)` 里的 `Result<InputSession>` 临时值就有 16 KiB，外面再套一层 `Result`、`SessionOptions`、`HostSession` 的返回临时值。

问题只在宿主线程栈比我们默认的假设小的时候暴露。仓库里本来就有这条预算：`crates/host-api/tests/host_options_small_stack.rs` 按记事本 512 KiB 主线程栈的一半、即 256 KiB 建会话并销毁。记事本的调用链比 Unreal 建窗时的激活链短，也没那么深，所以这条验收一直是绿的，而 0.6.1 在 UE 里溢出了。

## Decision

`crates/engine/src/session/input.rs` 的 `InputSession::new` 改成直接返回 `Box<Self>`：

```rust
let mut session = Box::new(Self { /* … */ });
```

`Box::new` 把结构体字面量写进刚分配的那块堆，栈上不再摆一份 16 KiB 的 `Result<InputSession>` 临时值再 `memcpy` 过去。`session::Session::new` 里原来那行 `Box::new(InputSession::new(&options)?)` 随之变成 `InputSession::new(&options)?`，`Result<Box<Self>>` 只有指针那么大。改的是唯一调用点，`InputSession::new` 不是 `pub`，外面没人用。

`host_options_small_stack` 的预算从 256 KiB 收到 192 KiB（测试名改成 `prepared_options_fit_a_192_kib_thread_stack`）。改完之后在本机（macOS arm64、debug）二分出这条链实际需要 147,456–163,840 B 之间，192 KiB 留了余量；改之前同样这个测试在本机 256 KiB 下刚够，等价于把「建会话最多占多少栈」钉低了一档。这条验收是 debug 构建，release 的帧更小，192 KiB 的 debug 预算对 Windows 上那条 256 KiB 左右的宿主线程来说是够的。

## Alternatives considered

- **把 `SESSIONS` 表的值改成 `Box<HostSession>`，顺手把 `create_session` 那 49 KiB 帧也收掉。** 这是这条链上最大的帧，但 `HostSession` 本身只有 15,744 B，剩下的来自 `Runtime<Session>`（约 14 KiB）和 `EngineOptions` 的构造临时值，要一路改到 `Runtime::new` 的返回值。改动面比这次的病根大得多，而崩溃点是帧 2 请求的 42 KiB；先把这一帧拿掉，看够了再谈。
- **用 `stacker` 在栈不够时切到新分配的栈上继续跑。** 能一次性挡住所有未来的大帧，但会话构造链上的结构体带 `Rc`、`PhantomData<Rc<()>>` 之类的线程限定标记，换栈等于把它们搬到另一条栈段上跑，且要把整个 `create_session` 包起来才有效；新依赖也要先问维护者。
- **把建会话挪到一条大栈的辅助线程上，再交回输入线程。** 不行：`host::session::Session` 带 `_thread_confined: PhantomData<Rc<()>>`，本来就不再跨线程；搬过去构造再搬回来，等于绕过这个约束。
- **在 TSF 侧延后到第一次按键才建会话。** 崩溃点变成用户第一次打字，还是同一条链；只是把崩溃推到更难复现的位置。
- **只收紧验收、不改构造。** 验收会红，问题还在。

## Consequences

- 建会话在栈上的占用降了一档，`host_options_small_stack` 把新预算钉住：以后谁再往构造链上放大结构体临时值，本机门禁直接红，而不是等宿主在小栈线程上崩。
- 改了 `InputSession::new` 的返回类型（`Result<Self>` → `Result<Box<Self>>`）。它是 crate 内部的 `pub` 方法、只有 `session::Session::new` 一个调用点，外部 crate 拿不到。
- 剩下的 49 KiB / 40 KiB 帧仍在，比这次的 141 KiB 总账小得多。但 Unreal 那条线程的整条栈只有约 86 KiB，这条链（110 KiB）在那边仍旧放不下，建会话后来改成在剩余栈不够时切到自己的栈上跑，见 [建会话在宿主的小栈线程上切到自己的栈](2026-11-27-session-creation-grows-stack.md)。

## Testing

- `cargo test -p msime-host-api --test host_options_small_stack`：改后 192 KiB 通过；临时把栈大小做成环境变量二分，得出本机 debug 下 147,456 B 溢出、163,840 B 通过，据此定 192 KiB。
- 同一测试在不带小栈的默认线程上跑一遍真实建会话、销毁，确认 `Box` 化之后的构造与原来行为一致（测试里原本就有的 `msime_client_create` / `msime_client_destroy` 断言）。
- `#11893` 的复现依赖 Unreal 编辑器建窗时的激活链，本机没有该环境；等含本改动的 Windows nightly 由报告者复验。
