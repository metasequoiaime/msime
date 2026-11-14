# Agent Note: Android 阿里云机器翻译

Status: implemented

## Problem

#11401 要求 Android 接入阿里云翻译。现有服务不包含阿里云机器翻译；百炼属于 AI 路径，不能代替机器翻译的协议。

## Decision

维护者在本任务中确认标准版与专业版都支持。接入 `alimt/2018-10-12` 的通用版 `TranslateGeneral` 与专业版 `Translate`，使用 ACS3-HMAC-SHA256、固定官方 HTTPS 接入点及 AccessKey。签名包含实际发送的 `content-type`、`host` 和全部 `x-acs-*` 头，表单 UTF-8 字节原样传输。共享 Rust 持有默认值、校验、目录、签名和响应解析，Android 原生页只填表和搬运签名后的原始 UTF-8 字节。新服务默认关闭，已有服务默认值保持。

配置只属于本机，加入设置导出排除项、凭据槽位、恢复默认保留和代理失效关闭逻辑。选择服务互斥，配置不全不回落，不把密钥写日志；缓存随服务、地域、版本、场景和密钥变化失效。每个候选单独请求，保留过期取消、离线优先与退避。

## 历史笔记审计

- [Android 自有翻译服务](../../implemented/feature/2026-10-12-android-user-translation-services.md)：部分重叠，保留其缓存、传输及离线优先约束；该篇只接已有三种服务的范围属于 #7078，本次独立扩展。
- [翻译凭据偏好](../../implemented/simplification/2026-10-16-translation-credential-preferences.md)：部分重叠，沿用凭据槽位与仅本机边界。
- [移动端显式 AI 翻译](../../implemented/bug-fix/2026-10-29-mobile-ai-translation-explicit.md)：部分重叠，保持 AI 显式选择，不把机器翻译归为 AI。
- [翻译回复结构](../../implemented/bug-fix/2026-10-15-custom-translation-reply-shape.md)：部分重叠，沿用失败与合法空译文分开处理；阿里云另按 `Code` 与 `Data.Translated` 校验。

## Alternatives considered

- 旧 REST 接口最直接的优点是已有接入示例多；不用，因为官方推荐 OpenAPI 2018-10-12，不能混用旧鉴权与新响应。
- 百炼可复用现有 AI 配置，开发量较少；不用，因为 issue 指定机器翻译，产品、计费和调用契约不同。
- 引入阿里云完整 SDK 能减少手写签名；不用，因为共享层已经具备 HMAC/SHA256，签名纯描述器可让 Android 保留既有传输与取消边界，避免新增网络依赖进入 client-core。

## Verification

共享签名使用独立 Python HMAC/SHA256 固定向量；5 项阿里云核心测试、1 项连接探测测试、15 项凭据回归、16 项设置文件回归通过；共享库 Clippy 以 `-D warnings` 通过。Android JNI 头文件交叉编译、350 项 JVM 冒烟、设备套件编译与 `compileFullReleaseJavaWithJavac` 通过。真实 APK 与设备 JNI 回放、最终 quick、截图上传尚在执行，不把仅编译计为设备验收。

设备回归复用现有 `HardwareTranslationDeviceSmoke`，可用 `-e aliyun-only true` 单独执行合成描述器与原生选路；不增加已达 100 项上限的 instrumentation。生产方法经产品类加载器调用，不用测试替身代替 Rust。

## Consequences

无真实 AccessKey 时只能证明协议构造和合成响应，不能声称实际付费服务调用成功。专业版支持的语言对由服务判断，错误进入退避。仅 Android 接入界面，其他平台不自动启用该服务。

## Sources

- https://help.aliyun.com/zh/sdk/product-overview/v3-request-structure-and-signature
- https://help.aliyun.com/zh/machine-translation/developer-reference/api-alimt-2018-10-12-translategeneral
- https://help.aliyun.com/zh/machine-translation/developer-reference/api-alimt-2018-10-12-translate
- https://help.aliyun.com/zh/machine-translation/developer-reference/api-alimt-2018-10-12-endpoint
