# Agent Note: Android 输入失败保留可脱敏的类别

Status: implemented

## Problem

#7837 的「输入连接失败」同时来自共享响应拒绝、响应格式异常、编辑器拒绝写入和 JNI 链接错误。十七处调用只执行 `fail()`，异常类别被丢弃；`ImeDebugOverlay` 的 P19 输入事件又没有失败类型。已提供的两份 Android 日志不能区分这些故障，不能凭提示把问题定为 InputConnection。

## Decision

共享层明确回 `ok: false` 与编辑器拒绝更新使用带枚举原因的 `JSONException` 子类；格式损坏保留普通解析异常。所有现有失败调用传入捕获的异常，由固定四种类别构造一条 logcat 错误记录，再执行原来的停止与直接输入回退。不记录异常原文、堆栈、输入、应用包名或路径，不改变 P19 白名单和上传格式，不重放按键。

设备回归从安装产品的类加载器驱动实际 `value` 与 `apply`，用合成响应和拒绝组字的 InputConnection 验证不同类别，防止只测测试 APK 的副本。隔离服务对象固定 26 键布局提示，不读取用户 SharedPreferences，并关闭自己的工作线程。普通及 `--core` 设备冒烟都运行该回归，未知原设备故障仍保持未确认。

## Historical audit

[反馈页连续输入](../../implemented/testing/2026-10-14-android-feedback-input-stability.md)部分重叠，保留真实反馈页验证及禁止盲目重放的决定。[原生诊断包上传](../../implemented/bug-fix/2026-10-14-android-diagnostics-native-bundle-upload.md)处理日志运输，本篇处理宿主失败原因被丢弃，双方互链。[恢复已取消的启动](../../implemented/bug-fix/2026-10-14-android-resume-cancelled-engine-startup.md)不变，分类不会改变恢复条件。

后续实现见[本地诊断包失败类别](2026-10-14-android-input-failure-local-export.md)，将固定类别保留在本地 ZIP，同时按服务端白名单过滤上传。

## Alternatives considered

- 直接把 Throwable 与堆栈交给现有 `ImeLog.e`：不用定义类别，定位最详细；但 JSON 异常可能包含组字原文，不能作为输入路径默认日志。
- 在 P19 的输入事件里加错误码或新 kind：上传包能携带原因；现有共享过滤与上传读取只接受十种事件且禁止额外字段，必须单独调整跨端及服务契约，不能静默借用已有字段。
- 失败后重建并重放按键：能缓解瞬时问题；但编辑器可能已成功上屏而仅后续组字失败，重放会重复文字，本次不采用。

## Verification

API 28 的旧安装产品在 `runtime rejection category` 断言失败，缺少失败分类。修复后，实际 `value` 的 `ok: false` 分类为 `runtime_rejected`；损坏 JSON、缺少标志、字符串布尔和无对象值分类为 `invalid_response`。实际 `apply` 使用合成拒写连接，分类为 `editor_rejected`，开始、组字写入和结束各一次。JNI 类别使用合成 `UnsatisfiedLinkError`，没有卸载或破坏原生库。带合成私人标记且禁止读取 `getMessage`/`getCause` 的异常仍得到固定文本，证明分类不依赖异常原文。

原生完整宿主重新构建，签名和 16 KB 对齐通过。API 28 的 `smoke.sh --core --feedback --diagnostics` 通过中文上屏退格、分类专项、真实反馈页 50 次上屏和五轮键盘重开、原生诊断包到上传编码、剪贴板视图/Activity 重建与失败重试。本地 quick、notes 和差异检查通过。此轮验证 API 28，不占用其他会话正在使用的 API 35 设备；原真实反馈页已有的 API 28/35 证据保留。本篇不把本地分类回归当成原设备崩溃复现。

## Consequences

所有十七处失败调用保留来源类别供 logcat 定位，普通 JSON 捕获范围和原停止/直接输入回退保持，按键与文字不重放。P19 白名单和上传格式保持，原诊断 ZIP 不增加这些错误记录，因此已有附件仍不能区分失败原因。分类只能缩小后续排查范围，不能解释或关闭 #7837；PR 保持 `Refs #7837`。
