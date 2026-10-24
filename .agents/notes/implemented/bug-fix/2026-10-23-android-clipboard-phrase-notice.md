# Agent Note: 剪贴板添加常用语的独立提示

Status: implemented

## Problem

#9351：剪贴板中「添加到常用语」的结果复用候选区诊断行，提示出现和消失都会触发整页重画。用户把这种变化感受为等待和卡顿，希望把提示放到手机屏幕顶部，原位置保持正常列表。

目前保存已经在 `preferencesWorker` 执行，先验证提示带来的布局变化，不把后台文件保存误判为主线程阻塞。

## Decision

仅为剪贴板添加常用语的校验、成功和失败结果提供独立的临时提示；不改设备信息、引擎错误等其他诊断反馈。保存继续在 `preferencesWorker` 执行，反馈不调用 `render()`，不占诊断行，不改变剪贴板卡片的位置。

Android 11 之前使用带自定义视图的系统 `Toast`，在状态栏和挖孔下方居中定位；这段系统版本仍支持顶部定位，系统管理独立提示窗口，避开 Android 9 的 IME 子窗口裁剪。Android 11 及以后使用输入法窗口令牌下的 `TYPE_APPLICATION_ATTACHED_DIALOG`，配合 `FLAG_LAYOUT_IN_SCREEN`、`FLAG_NOT_FOCUSABLE` 和 `FLAG_NOT_TOUCH_MODAL`，可点按关闭且不取编辑器焦点。不申请悬浮窗权限。

重复提示替换前一条，约三秒后消失；输入结束、收起键盘、重建输入视图、配置变化和服务销毁时撤销窗口、Toast 与定时回调。异步保存结果绑定提示代次，收起键盘后的迟到结果不能再显示；保存本身照常完成。

## Existing notes

检索 `diagnosticView`、诊断行、输入提示和 `PopupWindow`：与[设备信息复制反馈](../../implemented/feature/2026-10-17-device-info-copy.md)部分重叠；其诊断行保底用途不变，本片只处理添加常用语。候选借用、插件失败提示和候选删除菜单笔记属于其他消息来源，不改变它们的行为。现有 `ImeLayoutRows` 注释指出 API 28 的选项子窗口受 IME 父窗口裁剪，这是需要设备验证的边界。本片没有完全吸收或作废的旧笔记。

## 原型证据

`PopupWindow` 关闭裁剪后，API 28 的窗口报告坐标虽然在顶部，实际截图却出现从提示下沿延伸到 IME 顶沿的黑色长条，不能只以 `dumpsys` 的坐标认定成功。普通 `TYPE_APPLICATION_PANEL` 也存在同样的像素问题；`TYPE_APPLICATION_ATTACHED_DIALOG` 虽在 API 35 正常，API 28 只报告位置，实际没有提示像素。API 28 的 `AccessibilityController` 过滤 `FLAG_NOT_TOUCHABLE` 窗口；旧版 Toast 因此结合固定无障碍通知文字与屏幕像素验收，不能靠交互窗口树证明可见。现代子窗口允许点按关闭，以保留无障碍可见性。

## 截图与提示寿命

API 28 的短暂 Toast 使用同一张截图完成顶部定位、PNG 留证、深色背景断言与提示下方画面对照，调用方在 `finally` 释放位图。原实现定位后等待 250 毫秒再截图，两次截图之间提示可能已消失；设备诊断中第一帧确实有顶部提示，约 0.9 秒后的下一帧已经没有提示，后续深色采样为 0。截图和扫描自身也耗时，等待固定时长不能保证仍在提示寿命内。现代子窗口仍按无障碍节点定位，随后直接截图验证。

保留固定通知文字、顶部四分之一区域、深色阈值、提示下方画面一致性、焦点、列表位置与消失时限的全部断言。

## Alternatives considered

- 延长 Toast 寿命或放宽深色像素阈值：可以让旧的两次截图更容易通过；但会改变产品时序或允许提示实际不可见，无法修复坐标与像素来自不同帧的问题。
- 全版本统一使用系统文字 Toast：系统管理窗口与生命周期，实现最少；但 Android R 起面向 R 及以上的文字 Toast 忽略 `setGravity`，不能满足屏幕顶部位置，见[官方 API 说明](https://developer.android.com/reference/android/widget/Toast#setGravity(int,%20int,%20int))。
- `PopupWindow` 应用子窗口：复用现有键盘选项菜单方案，生命周期和父窗口关联最直接；但 API 28 的实际像素与窗口坐标不一致，不能作为跨版本的屏幕顶部提示。
- `TYPE_INPUT_METHOD_DIALOG` 独立窗口：定位不受 IME 父窗口的内容边界约束；但两套设备均不显示，AOSP API 28 `PhoneWindowManager.checkAddPermission` 要求 `INTERNAL_SYSTEM_WINDOW`，普通输入法不能依赖。
- 全版本统一使用 `TYPE_APPLICATION_ATTACHED_DIALOG`：现代系统有可交互的无障碍窗口，单一路径便于维护；API 28 的实际截图没有提示，不能以窗口报告的位置替代像素验收。
- 申请全局悬浮窗权限：可以独立控制跨应用提示的位置；为一句常用语操作结果增加独立授权和跨应用窗口的管理成本，收益不足。
- 继续使用诊断行：能复用当前已有的提示、超时与无障碍机制；但会占用诊断行，并把局部操作结果交给全局键盘重画，保留了本 issue 报告的干扰。

## Testing

- 同帧修正后的正式测试 APK 在 API 28 上通过原完整 `ClipboardSegmentActionsDeviceSmoke -e phrase-notice true`，187.54 秒；验证包含全部后续保存结果、列表、焦点和迟到结果场景。API 35 同一完整专项也通过（421.04 秒，含首次启动、安装与原初始化流程）；两 API 均使用固定 `bb417ff80b` 基线的原生开发 APK，其 `ImeOperationNotice.java` 与本次测试分支基线的源文件摘要一致。这些结果不冒充新默认分支的全部设备套件通过。

- API 28 安装原生 APK 后通过完整 `ClipboardSegmentActionsDeviceSmoke -e phrase-notice true`，覆盖成功、重复、无效正文、损坏文件、列表不动、焦点状态保持、超时、收起键盘撤销迟到结果与重新输入。
- API 35 安装同一原生 APK 后通过完整专项，包含点按关闭、编辑器窗口焦点基线与系统无障碍 10 秒提示时长；设置在结束后恢复。
- API 28/35 的默认 `ClipboardSegmentActionsDeviceSmoke` 均通过，覆盖分词分享、常用语保存、复制、最近记录与插入。
- 像素检查要求提示位置实际有深色背景，提示下方到键盘之间保持原始编辑器画面，不能只认窗口坐标。API 28 的旧版 Toast 结合固定无障碍通知文字与像素定位；现代子窗口查可交互的无障碍节点。
- Android 9 的长按预览流程可能已改变输入框节点焦点，专项比较添加前后的真实状态，并要求编辑器窗口继续取焦点、末尾无需再次聚焦即可输入。
- 最终夹具的远端 `scripts/verify-local.sh --quick`（`20261023-074701`）、本地 `pnpm run verify-notes` 与 `git diff --check` 均通过。

## Consequences

添加常用语仍在后台保存，独立反馈不调用全局键盘重画，也不再借用诊断行。消息代次把异步保存和显示生命周期分开，收起键盘不取消保存，但会丢弃迟到的提示。

两条窗口路径增加维护成本。旧版 Toast 不能点按关闭，依靠超时或收起键盘撤销；现代子窗口可点按关闭，并尊重系统无障碍建议时长。顶部位置避开状态栏和挖孔。实际设备覆盖是 API 28/35 模拟器；其他 Android 版本和厂商窗口策略仍需后续真机反馈。若后续最低版本提高到 Android 11，可移除旧版 Toast 路径。
