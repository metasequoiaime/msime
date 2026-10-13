# Agent Note: macOS 账号释义缓存绑定登录代次

Status: implemented

## Problem

macOS 输入法进程会跨文本输入客户端复用进程级账号释义缓存，但缓存键只有固定的 `account` scope。设置应用退出或切换账号时，输入法进程仍可能存活；旧账号已经缓存的释义会被新账号显示，在途的旧账号回复也可能写入新账号的缓存。

## Decision

账号释义缓存键加入当前保存登录的匿名标记。Swift 后端在选定账号会话时捕获由用户 ID 和登录 session ID 派生的 opaque marker，并把它放进每个通知 payload；匿名账号使用独立的零标记。原生输入法每次读取当前标记构造缓存键，登录标记变化时使控制器的旧请求失效。回复到达时必须携带与当前标记相同的数值，否则在持久化和缓存前丢弃。

## Alternatives considered

- **继续使用固定 `account` scope**：账号切换后无法区分旧缓存，保留跨账号显示和持久化风险。
- **每次账号切换清空整个翻译缓存**：可以清除旧值，但会影响其他来源的释义，并且无法阻止已经在途的旧回复重新写回。
- **把账号 ID 或 session UUID 交给 Objective-C++**：能组成缓存键，但扩大了跨语言边界上的身份暴露；opaque marker 已足够表达登录代次。

## Consequences

同一登录代次内仍可跨控制器复用释义；退出、重新登录、切换账号或切换到匿名会话后旧账号释义不会被读取。切换发生在请求进行期间时，旧回复不会写入新账号的内存缓存或学习词库。

## Verification

`BackendAccountTests` 覆盖通知 payload 的 marker；macOS `ShortcutTest` 覆盖不同 marker 的迟到回复被丢弃、零 marker 的匿名缓存键和现有账号释义回归。`test_backend_account_swift.sh` 构建并运行通过；本地完整 CMake 构建还需要先生成 `target/debug/libmsime_host_api.a`。
