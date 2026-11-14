# Agent Note: HarmonyOS 删除键四向 flick

Status: implemented

## Problem

HarmonyOS 删除键只有点按退格和长按连删，无法用一次触摸清空光标前文本，也没有对应的恢复动作。#11096 要求上滑清空、下滑恢复、左滑删除一个字符、右滑恢复一个字符，并且不能让普通连删与滑动动作叠加。

## Decision

删除键在 HarmonyOS 宿主复用共享的单指窗口坐标判定：上滑映射为 `CLEAR`，下滑映射为 `RESTORE`，左滑映射为 `DELETE_ONE`，右滑映射为 `RESTORE_ONE`。阈值为 24 vp，超过阈值后锁定首个方向；多指、取消和未超过阈值的触摸不交付 flick。

按下仍沿用现有按键反馈和长按连删。移动首次越过阈值时停止连删，抬手先结束按键，再由 `KeyboardSession` 执行一次 flick，避免重复删除。组字状态下上滑只调用现有 `discardComposition()`，不越过组字边界删除编辑器正文。

正文删除通过 HarmonyOS `InputClient.deleteForwardSync` 完成；这里的 `forward` 是光标前方，方向依据设备实测和现有 `deleteEditorBackwardSync` 契约。清空和单字符删除保存带编辑器代数、光标位置、光标前后文的会话内快照，恢复前重新核对这些条件；外部编辑、光标移动、切换编辑器或恢复成功都会使快照失效。文本上下文和删除长度按用户可见 Unicode 码点处理，以免把 emoji 拆成 UTF-16 半个字符。快照只存在内存中，不记录日志或落盘用户输入。

Android 已有的快速删除与撤回规则是行为参考：[Android 快速删除](2026-10-08-android-backspace-quick-delete.md) 和 [Android 快速删除撤回](2026-10-17-android-quick-delete-undo.md)，但 HarmonyOS 不复制 Android 的弹出提示层，保留当前 ArkUI 删除键外观。

## Alternatives considered

直接把四个方向交给 Android 风格的快速删除浮层可以复用更多代码，但 HarmonyOS 没有同一套键盘覆盖层和编辑器回声接口，会增加触摸目标和跨宿主状态同步；因此只复用判定和撤回边界，把动作接入现有 ArkUI 删除键。

只在 `KeyboardView.ets` 里直接调用编辑器删除可以更快，但会绕过会话的组字边界、编辑器代数和外部编辑失效规则，恢复时也无法可靠判断文本是否已被用户改动；因此把正文操作和快照放在 `KeyboardSession.ets`。

## Consequences

用户可以在 HarmonyOS 删除键上完成四向操作，普通点按、长按连删和取消触摸保持原有路径。快照有界且只保留最近一次清空和最近一次单字符删除，恢复会在上下文变化时安全放弃。当前只完成静态/Node 行为测试和宿主检查，尚未在签名 HarmonyOS 真机或模拟器上验收，PR 必须保持 draft，设备验收完成后再转 ready。
