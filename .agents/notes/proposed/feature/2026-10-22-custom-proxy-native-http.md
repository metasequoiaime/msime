# Agent Note: 自定义代理覆盖 macOS 原生 HTTP

Status: proposed

## Problem

`network.proxy_mode`（system/none/custom）加 `network.proxy_server` 只作用于共享 Rust 栈的网络客户端（reqwest），macOS 原生层的 HTTP 不读它：翻译缓存 `platforms/macos/src/translation/TranslationCache.mm` 走 NSURLSession，C++ 侧还有各自的 native transport。用户填了自定义代理后，共享栈流量走代理、原生 HTTP 仍直连，设置项名不副实。

## Proposal

把自定义代理逐条接进各原生网络栈：macOS 的 NSURLSession 走 `connectionProxyDictionary` 按 `proxy_server` 配 HTTP/HTTPS 代理，其余原生栈同理盘点后逐个接。接之前先在原生层读一次共享偏好（host-api 已暴露网络偏好给宿主），改动范围是每个平台原生网络代码 + 逐平台验证。

## Alternatives considered

- **只记差异不接**：自定义代理对共享栈有效、对原生栈失效的现状不解释给用户的说法没有变化——这不是正确性问题，是覆盖面不全；但保持现状会让设置项的承诺继续落空，所以立项而不是关闭。
- **统一把原生 HTTP 迁到共享 Rust 栈**：覆盖面一次解决，但翻译缓存等栈是原生实现，迁移范围比接代理大得多。

## Acceptance criteria

- `proxy_mode = "custom"` + `proxy_server` 后，macOS 上翻译请求走用户代理（可用本地代理计数验证）。
- `proxy_mode = "none"` 时原生栈不继承系统代理（确认 `connectionProxyDictionary` 显式置空的行为）。

## Risks

- NSURLSession 的代理字典语义与 reqwest 的 `Proxy::http(s)` 不完全同构，`server` 串里的 scheme/端口/认证字段要在两边解析一致。
- 其他桌面/移动宿主若有同类原生栈，需要同样的逐平台盘点；立项时先做 macOS，其他平台另开。
