# Agent Note: Android 语音润色状态规则下沉到共享层

Status: implemented

## Problem

Android 的 `VoicePolishSettingsPolicy.status` 在 Java 中单独判断润色开关、共享配置和凭据是否存在，并生成「未填写 API Key」或「配置不完整」文案。共享 Rust 已经负责语音识别状态，润色状态仍可能与共享设置页和其他宿主分叉。

## Decision

在 `client-providers::voice::provider` 增加纯函数 `mobile_voice_polish_status`，经 Host API 与 Android JNI 暴露。Java 只传入三个事实并严格解析返回的状态信封，继续负责控件布局和测试对话框标题。保留现有中文文案、空值语义和 JNI 失败时的安全回退。

## Alternatives considered

- **继续在 Java 中保留判断**：改动最小，但 Android 会继续拥有一份与共享设置规则平行的业务逻辑。
- **把整页状态和控件迁到 Rust**：能减少更多 Java 代码，但会越过宿主界面边界，改变本片无关的布局与生命周期。

## 影响与验证

Rust、Host API、JNI 和 Android Java 适配已接通；`VoicePolishSettingsPolicy` 只保留 Android 的方案、自定义槽位和测试标题等界面适配。共享规则覆盖开关关闭、配置完整、缺凭据和配置不完整四种情况，错误响应不会产生误导性文案。

## Verification

- `cargo test --locked -p msime-client-providers voice::provider::tests::mobile_voice_polish_status_explains_missing_configuration`
- `cargo test --locked -p msime-host-api mobile_voice_polish_status_export_uses_shared_status_rules`
- `cargo fmt --all -- --check`
- `git diff --check`
- Android JNI C++ 交叉语法检查
- `platforms/android/check-host.sh`：266 项 JVM 冒烟、240 个设备套件源文件编译、JSON null 读取检查、偏好键检查及服务 Java/API、清单和资源检查均通过

## Risks

新增一个稳定的 C/JNI 入口，需要同步头文件、JNI 桥和 Java 声明；没有偏好、文件、网络或 Android 资源变化。
