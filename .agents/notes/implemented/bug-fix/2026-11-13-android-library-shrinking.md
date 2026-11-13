# Agent Note: Android 保留宿主契约后裁剪未用依赖代码

Status: implemented

## Problem

#11115 的 full/pinyin 瘦包再次越过 132,000,000 字节预警线。实际 nightly APK 中 DEX 约 18.9 MB，发布构建关闭 R8，未使用的依赖代码一起进入包。原生库与词库已经做过瘦身，单独缩短资源路径仍无法把下载到的拼音包压回预警线。

## Decision

发布构建只启用 R8 的未引用代码裁剪，不改类名、不内联或改写方法、不裁剪资源。第一步完整保留 `app.msime.android` 的类、接口及全部成员，覆盖 BuildConfig 反射、JNI 回调、Fragment 和按名字创建的宿主实现；反射调用的 AndroidX FileProvider 同样显式保留。第三方依赖沿用随 AAR/JAR 发布的 consumer keep 规则。

这承接[RELR 与无损图标瘦身](2026-11-05-android-slim-apk-relr-webp.md) 当时推迟的依赖裁剪，不改变其原生重定位格式、资源内容和体积阈值。保留整个宿主包比精细 keep 牺牲一部分节省量，但动态入口集中在宿主，先保证既有运行时契约，避免把首次启用 R8 同全面混淆绑在一起。

## Alternatives considered

- 只启用 AAPT2 资源稀疏编码和路径缩短：不改运行时功能，验证面较小；实际旧拼音包从 132,187,757 降到 132,049,355 字节，签名前仍超过预警线，不能独立解决且余量不足。
- 完整 R8 优化与混淆：省得更多，但首次接入同时改变成员名、代码布局和动态创建路径，增加诊断与回归范围；这次只裁剪未用代码。
- 压缩 DEX 或原生库：能明显减小下载，却增加安装解压占用，已有笔记明确避免；继续保持可直接映射。

## 已取得证据

同一源码、锁定资源、ABI 和开发签名的 full/pinyin 瘦包均为 `132200045 → 122316002` 字节，减少 `9884043` 字节，继续低于原来的 `132000000` 预警线。DEX 未压缩大小分别为 `18918304 → 9036092` 和 `18918240 → 9036028` 字节。

比较两份 APK 的 DEX access flags：排除 `ACC_SYNTHETIC` 编译器生成物后，宿主及 FileProvider 的 `1408` 个源码声明类、`20379` 个声明成员完整保留。D8 与 R8 会生成不同的 lambda 和桥接类，这些编译器实现不作为按名字反射的稳定入口。`1360` 个资源与原生库条目逐字节比较，只有基于新 DEX 重建的 `assets/dexopt/baseline.prof` 和 `baseline.profm` 允许变化，其余无差异。

API 35 使用最终 full 瘦包执行崩溃、系统退出与 ANR 专项，整套通过。输入、设置和手写套件使用同代码的 R8 full 构建补齐锁定语言词库与词书，正在 API 28、35 专用 AVD 验收，不把缺资源的瘦包回退当裁剪结果。API 28 另用 `msime-client-test-api28-11115@emulator-5582`，避免占用其他会话的 `5566`。

## Consequences

同源、同资源、同签名的 full/pinyin 包都低于预警线；数据资源和原生库逐字节不变，宿主类与成员完整保留。设备运行仍需覆盖 API 28 与 API 35 的原生入口、输入、设置、反射创建、剪贴板及手写；体积和静态契约不能替代这些行为证据。

## Verification

依赖内的反射不能只靠静态调用图证明，必须核对第三方 consumer keep 及设备行为。包体积下降不代表功能验证通过。本轮 quick（`20261113-171138`）通过。API 35 完整设备套件已出现组字重启候选为空、剪贴板分词启动与提示位置失败，API 28 相应三项通过；正在用同源未裁剪构建对照。完整设备验收与对照未完成前保持 draft，暂不发布。

## Sources

Android 官方[逐步启用优化](https://developer.android.com/topic/performance/app-optimization/adopt-optimizations-incrementally) 提供只裁剪代码的开关；[keep 规则说明](https://developer.android.com/topic/performance/app-optimization/keep-rules-overview) 解释反射与 JNI 的动态边界。
