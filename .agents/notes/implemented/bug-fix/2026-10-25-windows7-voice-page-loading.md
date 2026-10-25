# Agent Note: Windows 7 语音页异步读取服务目录

Status: implemented

## Problem

Windows 7 原生设置窗口首次打开「语音输入」页时，`VoiceForm::create()` 在 UI 线程同步调用 `msime_client_voice_provider_catalog()`。服务目录读取期间控件尚未完成创建，真实 Win7 验收中页面会短暂空白，用户无法判断窗口是否已经响应。服务目录属于共享宿主接口，读取可能比本地控件创建慢，不能把它放在窗口创建路径上。

## Decision

语音页创建时先绘制三个服务选择器和「正在读取服务列表…」占位项，并在目录返回前禁用页面。进入语音页时发出的后台 `voice.settings / models` 操作同时读取本地模型目录和 `msime_client_voice_provider_catalog()`，完成消息把服务目录交给 `VoiceForm::catalog()`，由 UI 线程填充选择器，再按已保存的偏好重选识别服务、润色服务和豆包流式接口这三个依赖目录的选择器；其他字段不重写。「刷新模型列表」同样发出 `models` 操作并再次带回目录，若在这里重跑整页 `refresh()`，用户尚未保存的编辑会被已保存值覆盖。目录读取失败时仍提供 Windows 系统识别和本地模型，保留页面的可用入口并在结果区域显示失败信息。

本地模型列表与服务目录沿用同一次后台请求，避免新增 UI 线程同步调用或第二套缓存。Windows 7 UI 测试用阻塞的合成 transport 固定创建窗口先返回占位项，再释放后台响应，随后验证服务选择器恢复可用并继续覆盖原有保存、设备和模型操作。

本决定与[Windows 原生语音页](../feature/2026-10-11-windows-settings-voice-page.md)的服务目录来源和页面快照约束相接，也沿用[语音页快捷键目录](../simplification/2026-10-25-voice-hold-key-catalog.md)对跨宿主静态目录的分层；两篇笔记仍分别负责目录来源和快捷键数据，不改变其决定。

最初实现在 `catalog()` 里调用 `refresh(saved_)` 重新应用偏好，但 `saved_` 只是 `voice_input` 子对象，而 `refresh()` 要的是整份偏好文档，会在子对象里找 `voice_input`，找不到就 `enable(false)` 并让 `loaded_` 保持为 false，语音页在目录返回后反而永久禁用。#9722 合入时兼容设置构建正好被 C4456 编译错误挡住，`windows7-voice` 没有运行；#9734 修好编译后它在等待服务选择器可用时超时，`assert` 以 0xc0000409 退出。现在的做法不再借道 `refresh()`，也就没有形状不匹配的入口。

## Alternatives considered

- **继续在 `VoiceForm::create()` 同步读取目录**：实现最小，但会把共享接口延迟直接暴露给 Win7 UI，真实验收已经观察到页面短暂空白，因此不能接受。
- **让 UI 创建后再用定时器轮询目录**：可以避开首次调用阻塞，但会引入额外的生命周期、重复请求和竞态；现有 Worker 已经负责语音模型操作，复用其完成消息更容易保持串行状态。
- **目录返回后用 `refresh(Json{{"voice_input", saved_}})` 重跑整页刷新**：能修好形状错误，但每次「刷新模型列表」或重新进入语音页都会把未保存的编辑改回已保存值，因此只重选依赖目录的选择器。
- **目录失败时整页保持禁用**：能避免不完整的远程选项，但会连带禁用 Windows 系统识别和本地模型；保留这两个宿主本地能力更符合页面的降级行为。

## Consequences

服务目录读取不会再阻塞 Win7 设置窗口创建，页面在等待期间有明确状态，返回后已保存的服务、地址和模型选择仍按原规则恢复。后台 `models` 请求多做一次纯函数目录读取，首次进入语音页的后台任务略有增加；目录失败时润色服务没有远程选项，识别仍可使用系统或本地模式。x86 Win7 没有可用测试机，本次只覆盖真实 Win7 SP1 x64 VM 和可移植 UI 测试，不能据此声称 x86 已验收。

## Verification

- `git diff --check` 通过。
- `platforms/windows/tests/ui/windows7_voice.cpp` 断言「刷新模型列表」再次带回目录后，表单里未保存的语言字段保持不变；整页 `refresh()` 会把它改回已保存值。
- `platforms/windows/tests/ui/windows7_voice.cpp` 增加后台占位和完成后的异步目录断言；该测试需 Windows 原生工具链运行，本机 macOS 未执行 Windows 编译。
- 真实 Win7 SP1 x64 VM100 复现了服务页短暂空白并确认触发路径；本机 macOS 没有 Windows 原生工具链，修复后的二进制尚未重新部署到该 VM。x86 Win7 未覆盖。
