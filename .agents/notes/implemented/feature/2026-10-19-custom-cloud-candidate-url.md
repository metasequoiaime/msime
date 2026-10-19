# Agent Note: 可配置云候选 HTTP 来源

Status: implemented

## Problem

#8764 的云候选固定发送到 Google，用户无法选择自己的服务。macOS 传输层还限制 Google 主机，单改设置或 URL 构造不能完成切换。

## Decision

六个平台共用 `cloud_candidate_provider` 本机偏好：来源为 Google 或自定义接口，支持 GET/POST、`headers_json`、JSON 请求体模板、标准 candidates / Google / 点分 JSON 路径、100–10000 毫秒超时和缓存开关。旧文档缺省为 Google，新装仍默认关闭联网。只把当前未上屏的拼写代入 `{query}`，不代入前文、用户词库或历史。JSON 模板先解析、只替换字符串值，再序列化，避免把拼写拼成 JSON 结构。

请求构造和响应解析都在 `client-core`，`host-api` 把 `cloud_request` 描述符放入已有的在线查询，不新增 C ABI 符号。各宿主只执行描述符；Linux socket 服务只回传原始 JSON，Rust 负责解析，避免脚本成为另一份协议实现。公网只用 HTTPS，HTTP 只允许规范的 localhost、127.0.0.1 与 ::1；禁重定向、cookie、系统 HTTP 缓存，单次回包最多 256 KiB。

缓存只驻留本进程内存，以整份配置摘要、语言与当前拼写分隔；成功结果保留十分钟，空或无效结果保留六十秒。描述符带规范化缓存响应，宿主命中时不建立连接。禁用缓存后不读写这份 HTTP 结果缓存。配置变化立即清除已显示的云行和旧查询代次；移动端去重签名只加来源摘要，不包含请求头令牌。自定义配置不参与设置导出、账号同步或诊断快照。

macOS/Linux/HarmonyOS 共享设置页提供完整字段，Windows/iOS/Android 使用原生设置表单。首次联网说明与设置开关说明显示服务主机，不显示可能带令牌的查询串。官方检索仅找到搜狗企业 SDK / 百度开放平台合作入口，未确认可公共使用的云候选 HTTP 契约，不内置这两家未承诺稳定的私有接口；用户可以配置自己的合规服务，包括通过 JSON 模板与路径接入 OpenAI 兼容接口。

历史检索：云候选固定席位决定与本改动部分重叠，沿用单云候选席位；[关闭时撤销云行](../../implemented/bug-fix/2026-10-13-host-api-cloud-disable-invalidates-candidates.md)覆盖关闭，本改动补来源切换，互链并保留。其余云候选性能、移动端资格和 AI 席位笔记不改对应决定。

## Verification

共享用例覆盖 GET 编码、POST JSON 注入隔离、URL/请求头/模板边界、点分路径、缓存来源与凭据隔离、关闭缓存、诊断与导出脱敏。Host API 回归覆盖 Google → 自定义 GET → POST 的组字中切换、撤销旧行与拒绝迟到回包，并覆盖云描述符与较长 AI 配置共存。React 回归从空地址开始完成自定义配置再一次保存；macOS 调度测试走 POST 描述符。

各接收 `OnlineQuery` 的 ABI、JNI、Apple 桥接、Windows worker / ServerSession 和 Linux socket 使用 128 KiB 上限，避免完整 HTTP 配置与 AI 提示词共同超过原先 16 KiB 后静默丢回包。候选列表保留原上限。Apple 保存快照、Windows 的 HostOptions 与延迟偏好队列、Linux 的会话配置也对齐 Rust ABI 已有的 1 MiB 上限，使较长 HTTP 模板能保存并在下次会话重建时载入；prepareHost 的路径请求仍限制 16 KiB。

Android 设备回归发现新配置误用了 `deny_unknown_fields`，违反已有的未来字段兼容契约，现改为与其它偏好组一致，保留原始文档里的未来字段。HarmonyOS 真打包还暴露同文件已有的 `DictionaryUpdateOptions` 重复声明，移除完全相同的一份。

Host API 将展开后超过 URL 或请求体上限的查询标为本次不具备云候选资格；旧宿主的无描述符兼容路径因此不能把自定义请求改发给 Google。回归使用合法的 2048 字节模板与较长合成拼写覆盖这一边界。macOS 的 POST 调度同样检查云开关和资格，关闭时撤销进行中的请求。iOS 传输替身测试关闭进程缓存，避免前一测试的缓存命中吞掉预期的网络调用。

本机运行 client-core 426、Host API 883、Runtime 324 项测试及 Clippy，均通过；共享设置页四项回归、类型检查与生产构建通过。macOS 原生全套 153 项中 152 项一次通过，`settings-glass` 外观同步断言独立重跑通过，云请求、桥接和调度用例均通过。iOS 重建共享库后在模拟器运行在线候选 23 项，通过。Linux 容器原生 90 项通过；HarmonyOS 完成真实 HAP 编译与逻辑测试。Windows 完成 x86 语法、x64 交叉链接及本机 worker 执行测试，WinUI 设置编译交由 Windows CI 覆盖。

Android API 28/35 都运行四个共享 crate 的设备测试，并在最终安装 APK 上实际验证 GET/POST、请求头、缓存不联网、禁重定向与 200 毫秒总超时；API 28 核心装包冒烟完整通过。API 35 通用冒烟首次在剪贴板导出断言失败，该项独立重跑通过，整套未重跑。基础输入冒烟临时使用合成 26 键偏好，结束后恢复原偏好与文件所有者，不清产品数据。

## Alternatives considered

- 仅新增自定义 GET URL 与响应路径：issue 允许此最小版本，范围小且可快速交付；用户明确要求本次完成完整 HTTP 配置并接通各平台，因此采用共享描述符并同时接入全部宿主。
- 在 macOS 单独构造自定义地址和解析 JSON：改动局部；会复制协议规则，因此构造与解析仍归共享层。

## Consequences

GET 地址可能含服务访问令牌，配置不进入导出和同步。不同来源仍只取一条候选，多个回答不增加云候选席位；OpenAI 接口的模型和提示词由用户的 JSON 模板给出，不复用 AI 候选的前文或 token。共享设置页与 iOS、Windows 编辑草稿后整组保存；Android 逐项保存，先配置方法和地址后再选自定义来源。新增字段缺省兼容旧配置，默认 Google 行为不变。

验证分级只按实际运行记录报告；ArkTS 打包不代表 HarmonyOS 设备入口已验收，MinGW 编译不覆盖 WinUI 设置窗口。
