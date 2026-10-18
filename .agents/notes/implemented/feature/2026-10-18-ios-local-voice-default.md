# Agent Note: iOS 未配置语音服务时默认使用本地识别

Status: implemented

## Problem

iOS 的语音服务保存在本机 UserDefaults。没有配置时，`CustomServiceConfiguration` 默认选中「自定义」，首次录音因此落入需要接口、模型和密钥的云端服务。用户要求语音默认使用本地识别。

## Decision

新配置与未配置服务的读取结果默认使用 `local`，设置页和录音入口沿用同一份配置。已保存的有效服务选择保持原样；旧配置只有非空接口地址、没有可识别的服务标识时仍按自定义服务读取，避免升级后丢失原来的识别方式。本地模型缺失时沿用现有下载与选用提示，不自动切换云服务。

## Existing notes audit

- [EveryAPI 默认模型修正](../bug-fix/2026-10-12-everyapi-asr-default-model.md) 处理特定服务的模型名，服务选择默认值不归它管理。
- [本地语音准确率](../feature/2026-10-17-local-voice-accuracy.md) 处理模型目录、识别过程和流式模型默认值，与本篇部分重叠；本篇不更换模型目录中的默认模型。
- [语音公共偏好模型](../simplification/2026-10-16-voice-input-preferences.md) 处理共享 React 偏好；iOS 使用独立的本机服务配置，不改其他平台的默认服务。

## Alternatives considered

- 直接把所有读取失败都改为本地：分支最少，但旧版只有接口地址的自定义服务会被覆盖。保留这类旧配置的自定义含义，只让未配置用户使用本地。
- 同时修改共享 Rust 语音默认值：其他宿主也能默认本地，但会超出本次 iOS 调整，而且无法替代 iOS 自己的 UserDefaults 默认值。

## Verification

- 空配置和没有接口的未知服务默认本地，且无需云端接口即可保存选择。
- 已选择的云服务、系统识别和旧自定义接口仍按原配置读取。
- iOS 18.4 模拟器的 `MSIMEServiceTests/CustomServiceTests`（28 项）与 `LocalSpeechModelTests`（20 项）全部通过，覆盖默认选择、旧配置兼容和本地模型契约。
- 产品验证使用 `MSIMEKeyboardExtension` scheme 的原生 App 构建、整包签名校验和 iPhone 17 覆盖安装；仓库门禁使用 `scripts/verify-local.sh --quick` 与 `pnpm run verify-notes`。

## Consequences

本地识别仍需先下载并选用模型；这次只调整默认服务，不把语音模型塞进安装包。
