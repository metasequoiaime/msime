# Agent Note: 共享语音请求统一 curl 头链表所有权

Status: implemented

## Problem

Windows 0.3.0 的 AI 候选请求曾因对同一 `curl_slist` 反复调用 `unique_ptr::reset`，在 `libcurl.dll` 中读取已释放的请求头而触发访问冲突；同一类手写链表代码仍留在共享语音的识别和润色请求中。语音代码当时靠 `release/reset` 恰好避开了释放原表头，但所有权不变量只能靠调用者记住，后续追加头部很容易重新引入同类错误。

## Decision

共享语音新增 `CurlHeaderList` 和 `append_curl_header`。追加空链表时容器接管首次返回的表头；追加非空链表时保留原表头，不把 `curl_slist_append` 返回值再次交给 `reset`。识别和润色请求都通过这个封装构造头部，并用独立 CTest 核对三项头部在追加后仍按顺序完整存在。

## Alternatives considered

- **继续保留每个请求里的 `release/reset` 写法**：改动最小，但所有权规则分散在两个请求路径中，下一次增加请求头仍可能复制候选请求的历史缺陷。
- **直接复用 Windows 候选目录的头部工具**：可以减少一份实现，但会让共享语音依赖 Windows 候选的策略头和 JSON 类型，扩大平台耦合。

## Consequences

语音请求的 `curl_slist` 生命周期由一个跨平台 RAII 封装管理，新增头部不需要手动转移所有权；测试只验证链表构造，不替代真实服务请求和平台崩溃验收。请求头内容、顺序和网络行为保持不变。
