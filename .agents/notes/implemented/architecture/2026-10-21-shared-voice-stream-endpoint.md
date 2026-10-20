# Agent Note: Android 流式语音共用规范化端点描述

Status: implemented

## Problem

共享 Rust 层已经用 `reqwest::Url` 校验移动语音地址，但 Android 的 `DoubaoRecognizer` 又用 `java.net.URI` 解析同一字符串。两套解析器对国际化主机、路径空格、Unicode 路径、IPv6 和端口的结果不同，握手因此可能找不到主机、破坏路径编码或丢失非默认端口。

## Decision

在 `client-providers::voice` 由共享 URL 解析结果生成一个流式 WebSocket 传输描述，包含 socket 主机、端口、Host 头和已序列化的 request target。通过现有 `host-api` JSON 信封和 Android JNI 传给 `DoubaoRecognizer`，Java 只做严格的信封字段检查，不再对 endpoint 做第二次 URI 解析。描述符复用共享 WebSocket 形状校验并拒绝控制字符；端口与百分号文本遵循 URL 库的既有语义，显式端口 0 保留为 0，不回退到 443，也不另写百分号扫描器。request target 保留 URL 库序列化的路径、查询串和空查询。设置页规则的前置决定见[Android 语音服务设置](../feature/2026-10-12-android-voice-provider-settings.md)。

## Alternatives considered

- **继续在 Java 中修补 `URI` 解析**：可以局部接受 IDN 或手动补端口，但仍然维护第二套路径和查询编码规则，不能保证与 Rust 请求构造一致。
- **只把规范化 endpoint 字符串传给 Java**：能解决一部分 IDN 和空格问题，但 Java 仍然会重新解析，且 IPv6、空查询和自定义端口仍由另一套实现解释。
- **在移动语音运行时配置中附带四个字段**：可以让常规配置入口一次得到全部传输参数，无需改偏好格式；但独立的 `Streaming(endpoint, headers)` 入口不经过该配置，仍要重复适配。选择在最终传输入口生成描述符，让两条路径共用同一规则。

## Consequences

- 中文域名、Unicode 或空格路径、IPv6、非默认端口、百分号路径和空查询在共享描述符中拥有确定且可测试的四字段表示。
- Android 流式握手只消费描述符字段；Host 头保留 IPv6 方括号和非默认端口，socket 连接使用无方括号主机。
- JNI 不可用、信封字段类型错误或超限时失败，不回退到 `java.net.URI`。
- 共享 Rust 与 C ABI 测试覆盖中文域名、Unicode 路径、IPv6、非默认端口、百分号路径、空查询和非法端口；Android host 门禁通过 248 个 JVM 冒烟并编译 193 个设备套件源，新增设备用例设计为通过真实 JNI 读取描述符，但尚未在设备执行，编译通过不能视为 JNI 运行验证。
- 本地执行描述符 4 个 Rust 测试与 1 个 C ABI 测试通过；端口 0 与百分号文本的一致性测试先失败，再随删除额外扫描器通过。`client-providers` 与 `host-api` 的全部 target 通过 Clippy，警告按错误处理。
- 非 WebSocket HTTP provider 的配置 schema 不变；Android 流式握手不再依赖 `java.net.URI`。
- API 28/35 设备用例尚未执行；当前不能声称设备验收通过。

## Risks

描述符是跨 Rust、C ABI、JNI 和 Java 的新契约，字段长度和拒绝范围必须在各层一致。路径百分号语义若被手工 decode/re-encode 会改变服务端路由，因此实现必须直接使用 URL 库的序列化路径和查询。
