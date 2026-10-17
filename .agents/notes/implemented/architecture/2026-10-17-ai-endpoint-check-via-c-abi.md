# Agent Note: 各宿主的 AI 接口地址策略改问 Rust

Status: implemented

## Problem

AI 辅助接口地址的规则（https 不限主机，http 只能指向本机或局域网）原来在 Android、HarmonyOS、iOS SharedUI、macOS 和 Windows 各有一份手写实现（见 [AI 辅助接口允许本机和局域网的 http 地址](../feature/2026-10-08-ai-endpoint-local-http.md)），靠 `shared/contracts/ai-endpoint/cases.json` 保持一致。用例只能收各平台 URL 解析器结论一致的写法，其余写法各平台各说各的：带前导零或十六进制的 IPv4、`[::ffff:…]` 这类地址在 Android、iOS、macOS 上按「公网明文」拒绝，在 Rust 是「不合规」；Windows 只在主机部分查反斜杠；Android 的 `java.net.URI` 认不出国际化域名，同一个 https 地址在设置页能存 Token、在 Android 上却不能用。#8201 要把各平台的账号和云端逻辑收进 C 接口，`msime_client_ai_endpoint_check`（`{origin, problem, hint}`，不做 I/O）早已给 Windows 原生设置窗口导出，五份副本可以直接换掉。

## Decision

- 五个宿主都改问 `msime_client_ai_endpoint_check`，原来的类名、函数名保留，只剩读回答、按回答放行的那一层；规则只在 `crates/client-providers/src/ai/endpoint.rs`。C 接口拒绝请求或回答读不懂时一律按不合规处理，不发请求。
  - Windows：`src/candidate/AiEndpointPolicy.h` 的 `check_ai_endpoint` 问 Rust；`ai_endpoint_protocol` 另要求原文以 `http://` 或 `https://` 开头，因为 Rust 解析前去掉首尾空格，curl 拿到的却是原文。`msime-windows-pump` 不再链接 `ws2_32`（只有旧副本的 `inet_pton` 用它）。
  - macOS：`src/core/AIEndpointPolicy.h` 的 `MSIMEAIEndpointCheck(endpoint)`、`MSIMEAIEndpointOrigin(endpoint)`，经 `apple-client` 链接 host 库；去掉了从没用过的 `components` 出参。
  - iOS：`SharedUI/core/AIEndpointPolicy.swift` 经 `@_silgen_name` 调 C 接口，`validatedURL` 在 Rust 放行之外还要求系统能解析出带主机的 `URL`。`crates/tauri-mobile-platform` 的那份不动（Tauri 外壳不是平台产品）。
  - Android：新增 JNI `NativeClient.aiEndpointCheck`（`aiEndpointCheckRaw`，上限 64 KiB），`AiEndpointPolicy` 的结论和来源键都取自它；`uri()` 在 Rust 放行而 `java.net.URI` 解析不了时为 null，`AiPolishConfiguration` 据此按无效处理。原文交给 Rust，首尾空格由 Rust 去掉、制表符和换行仍是控制字符，与原来先查原文再去空白的顺序相同。
  - HarmonyOS：新增 NAPI `aiEndpointCheck`；`keyboard/settings/AiEndpoint.ets` 调它，`AiEndpointPolicy.ts` 只剩读回答的 `parse`（node 测试能跑），`AiModelCatalogPolicy.modelsUrl` 改为接收检查函数。
- 共享设置页的 `packages/ui/src/settings/ai-endpoint-policy.ts` 和 Linux provider 的 `ai_endpoint_check` 不在这一步，继续跑 `cases.json`。
- 公网 http 被拒时各宿主给用户看的那句话（`CLEARTEXT_REASON`、`cleartextMessage` 等）不变，Rust 的 `hint` 没有换进来。

## Alternatives considered

- **保留各平台副本，只补齐用例** — 不跨 FFI，每次判断少一次 JSON 往返；但用例只能放各解析器结论一致的写法，正是用例之外的写法在各平台分叉，补用例补不到它们，而每改一次规则要改六种语言。
- **JVM 冒烟和 node 测试也经真实的原生库跑** — 冒烟能直接核对 Rust 的结论；但 JVM 冒烟只有 android.jar 的桩、加载不了 `libmsime_android.so`，node 加载不了鸿蒙的 `.so`。所以 Android 的 JVM 冒烟按 `cases.json` 的表作答（`tests/voice/AiEndpointContract.java`，表里没有的地址直接失败），真实 JNI 由设备冒烟 `AiEndpointCheckDeviceSmoke` 核对；鸿蒙的 node 测试核对读回答的那一步，NAPI 本身要在设备上看。

## Consequences

- **收益**：设置页存 Token 用的来源键和键盘、联想请求放不放行，在五个宿主上都是 Rust 的同一个结论；规则只改一处。
- **代价与已知上限**：每次判断多一次 FFI 和 JSON 往返（纯函数，微秒级，不联网）。用例之外的写法有几处结论变了，记在 #8201 的迁移差异里：前导零或十六进制 IPv4、`[::ffff:…]` 的 http 地址从「公网明文」变成「不合规」，说明换成「请填写完整的接口地址」那一句；Windows 拒收查询串里的反斜杠；Android 放行国际化域名的 https 地址（来源键是 punycode；`java.net.URI` 认不出这种主机，`AiEndpointPolicy.uri()` 把主机换成 Rust 来源键里的 punycode 再构造，润色、AI 候选和模型目录都按它发出），路径里有空格这类 `java.net.URI` 不收的地址在 `check()` 里就按不合规处理，设置页也不收，与原来相同。Android 的 JVM 冒烟和鸿蒙的 node 测试不再碰真实规则，两端的接线只在设备上验得到。

## Verification

- Rust 的用例：`cargo test -p msime-client-providers --lib ai::endpoint`。
- Windows：ctest `windows-ai-endpoint-policy`（链接 host 库，含查询串反斜杠和首空格两条）。macOS：ctest `ai-endpoint-policy`、`ai-settings-window`、`cloud-candidate-request`。iOS：`KeyboardTests/settings/AIEndpointPolicyTests.swift`。
- Android：`bash platforms/android/check-host.sh`（JVM 冒烟），设备上 `smoke.sh` 的 `AiEndpointCheckDeviceSmoke`。HarmonyOS：`bash platforms/harmony/tests/run.sh`。
