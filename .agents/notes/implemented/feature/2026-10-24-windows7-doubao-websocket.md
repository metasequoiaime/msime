# Agent Note: Windows 7 豆包流式语音传输

Status: implemented

## Problem

完整产品在 Win7 装载测试中发现 Server 静态导入 Win8 才提供的四个 WinHTTP WebSocket 接口。前面的手工闭包没有包含完整 Server，无法暴露这一问题。仅改为动态导入并报不支持会使原有豆包流式语音失效，不满足完整支持要求。

## Decision

Win7 构建使用同源 libcurl 的 WebSocket 传输；现代 Windows 继续使用原有 WinHTTP。共享 Rust 仍持有鉴权与豆包协议编解码，原生层只传输二进制帧。Win7 依赖需显式启用 curl 的 WebSocket 功能，产品构建运行能力探针，缺少时失败。专用 `compat/vcpkg.json` 固定官方基线 `04a9d8e5212d01ee1dd9478eadd9caade4f8b0d4`，与现代产品清单隔离；两个架构已用清单模式实际安装，不能只记录临时 classic 命令。

握手只允许 wss，验证证书和主机，拒绝重定向及凭据转发。发送、接收和关闭之间以互斥保证同一个 CURL 句柄不被并发操作；非阻塞收发在锁外等待。接收复制元数据后释放锁，重组分片并保持 1 MiB 上限，控制帧不会进入业务解码。取消唤醒接收，析构前先等待接收线程退出。握手、发送和接收都有有限时限。

接口语义依据 [curl_ws_send](https://curl.se/libcurl/c/curl_ws_send.html)、[curl_ws_recv](https://curl.se/libcurl/c/curl_ws_recv.html) 和 [线程安全约束](https://curl.se/libcurl/c/threadsafe.html)。

### 既有笔记审计

- [完整 Win7 支持](../../implemented/feature/2026-10-23-windows7-support.md)：部分重叠，本篇只补充 Server 的豆包网络传输，不替代语音面板、麦克风或真实服务验收。
- [共享端点描述](../../implemented/architecture/2026-10-21-shared-voice-stream-endpoint.md)：部分重叠，鉴权、配置和帧编解码继续复用共享实现，不复制业务。
- 按 `DoubaoAsrClient`、`WinHTTP` 检索活跃笔记，没有已有的 Win7 WebSocket 传输决定。

## Alternatives considered

- **动态探测 WinHTTP 并拒绝旧系统流式语音。** 代码最少，也能装载，但把完整支持变成隐蔽的功能缺失，因此不用。
- **在 C ABI 新增一套 Rust WebSocket 会话句柄。** 可以复用 tokio-tungstenite，但会新增跨 FFI 的连接注册、二进制缓冲和取消协议；项目已经链接 libcurl，因此先使用现有传输依赖。

## 已执行验证

Windows 11 与 Win7 SP1 x64 的交互桌面均通过 x64／x86 的真实合成 TLS 服务测试：拒绝不受信任证书和重定向，256 KiB 二进制分片与 ping、并发收发、取消以及文本／超限／关闭响应处理。测试证书回溯一天生效，避免测试机时钟偏差造成无关失败；证书校验保持开启。完整产品两架构依赖闭包和所有包内 DLL 的 Win7 装载检查通过，但先前预加载方式不覆盖 TSF 的冷启动；其独立缺陷与修复记录在[核心构建](2026-10-23-windows7-rust-core-build.md)。Windows 11 的现代 C++ Server 分支编译，以及 shell surfaces、DPI、豆包转录三项 CTest 通过；该回归复用兼容核心导入库，不代表普通工具链完整安装包已验收。真实豆包账号和麦克风尚未验收。

## Consequences

Win7 继续提供二进制流式传输，不削减现代路径；代价是专用 curl 功能清单与适配器。Schannel 的 TLS 和信任根取决于系统补丁，必须以 Win7 的真实 TLS 测试为准。libcurl 版本和构建功能需要可复现地记录，不能只因为头文件存在就宣称 WebSocket 可用。
