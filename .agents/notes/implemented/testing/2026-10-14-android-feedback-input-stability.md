# Agent Note: Android 反馈页连续输入的设备回归边界

Status: implemented

## Problem

#7837 报告填写反馈时至少五次出现「输入连接失败」，但没有提供平台、设备、系统版本、输入法版本、稳定按键序列或故障堆栈。Android 的 `fail` 同时接收运行时响应解析失败、共享层拒绝操作和 JNI 链接错误；编辑器拒绝写入也转成 `JSONException`，因此提示不能证明根因是 InputConnection。

## Decision

先在真实原生反馈页通过键盘连续组字、确认、退格及收起重开复现，使用合成文字，检查完整文字和计数更新。`FeedbackInputStabilityDeviceSmoke` 运行 50 次 `nihao` 加空格上屏，分五组退格一次并重新显示键盘；测试不提交反馈，结束后恢复偏好、输入模式和默认输入法。测试失败只输出固定阶段，不记录编辑器原文、原生响应或异常原文。

API 28 与 API 35 专用 arm64 AVD 均通过该回归；本地通过只能说明当前构建在这两种环境的反馈页输入路径未复现，不能替代未知原设备的证据。因此反馈输入路径只增加设备回归和运行入口，不增加自动重试，也不把 #7837 标为已由生产代码修复。

## Related notes audit

[恢复已取消的引擎启动](../../implemented/bug-fix/2026-10-14-android-resume-cancelled-engine-startup.md)部分重叠，保留失败启动不在每次显示时盲目重试的决定。输入连接被拒和原生协议失败不能复用同一自动重试条件。[丢弃过期启动任务](../../implemented/bug-fix/2026-10-14-android-stale-engine-startup.md)继续保护不同输入框的代次。

[诊断上传读取共享层包格式](../bug-fix/2026-10-14-android-diagnostics-native-bundle-upload.md)补充排查时发现的日志传递缺陷，不把上传漏读与原输入失败归为同一根因。

## Alternatives considered

- 在看到失败提示后无条件重建并重放最后一键：可以很快掩盖瞬时失败；但响应可能已提交文字，重放会重复上屏，JNI 或协议错误也不能靠重复操作修好，不能作为修复。
- 只在合成编辑器验证组字：存储和字段易控制；但原报告来自反馈页，必须先覆盖真实页面的 TextWatcher 与计数更新。

## Consequences

反馈页的连续输入、删除、计数和收起重开现在有 API 28/35 设备回归入口：`smoke.sh emulator-5566 --core --feedback` 与 `smoke.sh emulator-5580 --feedback`。测试会修改并恢复测试偏好，因此失败时仍需检查设备是否回到原输入法。原报告环境未知，后续拿到平台、版本和脱敏堆栈后仍需按失败类别补充验证；在此之前不得宣称原报告全部解决。

[输入失败类别](../bug-fix/2026-10-14-android-input-failure-categories.md)补上宿主丢弃失败来源的诊断缺口，只输出固定 logcat 类别，不改变原停止回退或 P19 上传格式；原报告根因仍未确认。
