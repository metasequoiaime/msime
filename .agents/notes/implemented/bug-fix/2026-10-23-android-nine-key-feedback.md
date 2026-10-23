# Agent Note: 安卓九键反馈按已保存设置生效

Status: implemented

## Problem

#9338 报告拼音九键开启按键音、振动和放大预览后没有反应。九键网格没有调用已有的按键预览；反馈文件只在创建输入视图与延迟刷新通过组字保护后读取，复用键盘时第一下仍可能采用旧开关，开始组字又会取消重读。默认声音使用 `AudioManager.playSoundEffect(int)`，用户已打开应用按键音仍受系统触摸音效开关限制。

检索 `KeyboardHaptics`、按键音、按键预览和 `InputViewRefreshPolicy`：既有[振动档位](../../implemented/bug-fix/2026-10-08-keyboard-haptic-levels.md)部分重叠，保留能力降级与跟随系统的决定；[移动反馈契约](../../implemented/simplification/2026-10-15-mobile-feedback-contracts.md)只持有共享展示类型，与原生接入无关；反馈页生命周期处理的是问题提交表单，与本片无关。九键与字母预览没有已有归属笔记。

## Decision

输入视图每次开始显示时立即重读声音、振动与本地触控设置，反馈不依赖会重建引擎的延迟刷新。九键复用字母预览覆盖层和密码保护；按下显示当前键面，松手与取消按所有者关闭，滑动数字更新预览，长按或反向滑动选项出现时关闭预览。默认声音按应用音量调用[双参数系统接口](https://developer.android.com/reference/android/media/AudioManager#playSoundEffect(int,%20float))，应用按键音关闭时不播放；不改变振动档位映射。

## Alternatives considered

- 把设置重读继续放在延迟刷新中：减少弹出时工作，但首次按键仍会使用旧设置，组字还会使本次刷新整个跳过。反馈文件小且已有本地设置读取，显示时读取保证第一下即生效。
- 给九键另建弹窗：键面空间独立，但要重复处理位置、密码保护和多指所有权；复用已有覆盖层可沿用约束。

## Consequences

九键点按预览、数字滑动和选项弹窗共用现有覆盖层，增加的触摸转发保持按钮原本的点按和长按。每次显示键盘多读一次有 4 KiB 上限的反馈文件；反馈不再等待或依赖引擎刷新。默认按键音使用应用声音开关与音量，系统流静音策略仍由 Android 管理。

模拟器验证界面、持久化设置读取和系统播放记录，不能证明报告者 vivo 真机的扬声器与马达体验；振动档位映射保持，真机主观振感仍需要装机确认。

## Verification

- 修复前 APK：API 28 专项确定失败于复用输入视图仍保留旧开关；API 35 专项确定失败于九键按下无预览；API 28 关闭系统触摸音效后，应用开关开启仍无播放记录。
- 修复后 API 28/35：`OptionSheetDeviceSmoke -e key-feedback true` 通过。使用安装产品里的真实服务及九键控件，视图挂在原生宿主 Activity 中，核对即时声音/振动开关和强度、按下/松手、多指归属、数字滑动取消还原、长按选项、禁用和密码框。
- API 28：`OptionSheetDeviceSmoke -e key-feedback sound-only` 通过。关闭系统触摸音效，应用声音开时音频服务新增播放记录，应用声音关时没有新记录，结束恢复系统设置。
- `check-host.sh` 的 256 项 JVM 冒烟、216 个设备源码与 JNI/Java 契约通过；完整原生库与发布宿主 APK 构建通过，开发签名和 16 KiB 对齐通过。
- 最终远端 quick `20261023-050220` 通过；API 28/35 完整 `NineKeyPanelDeviceSmoke` 与 API 35 `LetterHintPunctuationDeviceSmoke -e letter-swipe true` 回归通过。
