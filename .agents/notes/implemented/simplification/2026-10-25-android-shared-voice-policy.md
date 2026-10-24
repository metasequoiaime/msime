# Agent Note: Android 语音服务策略下沉到共享层

Status: implemented

## Problem

`VoiceProviderPolicy` 仍在 Android Java 中判断哪些服务需要联网凭据，以及豆包鉴权模式是否为旧版 `legacy`。这些判断与共享 Rust 的服务解析、凭据和请求规则属于同一份业务策略；继续留在 Java 会让设置页保留一份容易漂移的服务分类和鉴权分支。

## Decision

在 `client-providers::voice::provider` 中统一计算服务是否为联网服务和鉴权模式是否为旧版，并通过 Host API 与 JNI 返回严格的布尔 JSON 信封。`VoiceProviderPolicy` 只负责传入 Android 字段、解析共享成功信封并在桥接或字段类型异常时安全回退为 `false`；`VoicePage` 的网络字段、App ID 显示和凭据保存流程保持不变。

共享函数把非空且不是 `system` 或 `local` 的 provider 归为联网服务，把去除首尾空白后等于 `legacy` 的 auth mode 归为旧版鉴权。C ABI 和 JNI 对两个输入分别限制为 256 字节 UTF-8，Java 入口同时检查 Unicode 和 UTF-8 长度；响应只接受严格的 JSON 布尔值。

## Alternatives considered

- **继续保留 Java 判断**：改动最小，但 Android 仍会复制共享服务规则，后续新增离线服务或鉴权模式时容易只改一边。
- **把整页凭据编辑器迁到 Rust**：能删除更多 Java 逻辑，但会越过原生设置界面边界，改变本片不相关的表单状态和生命周期。

## Consequences

共享 Rust、Host API、JNI 和 Java 之间多了一个小型纯函数契约，需要同步头文件、导出清单和三层测试；服务分类与鉴权模式的唯一业务判断归入共享层，Android 只保留字段适配和严格解析，且不读取偏好、文件或网络。

验证结果：`cargo fmt --all -- --check`、`pnpm run verify-notes`、`cargo clippy --locked -p msime-client-providers --all-targets -- -D warnings`、`cargo clippy --locked -p msime-host-api --all-targets -- -D warnings` 和 `bash platforms/android/check-host.sh` 均通过；其中 Android 主机检查报告 161 个 JNI 方法匹配、266 个 JVM 冒烟和 240 个设备套件源文件编译通过。共享 Rust 与 Host API 新增单测、Android 设备设置冒烟覆盖真实 JNI、策略解析和服务分类；`python3 scripts/test-android-jni-names.py` 也通过。
