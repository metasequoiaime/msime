# Agent Note: iOS 语音标点源文件同步到提交的 Xcode 工程

Status: implemented

## Problem

`platforms/ios/SharedUI/voice/VoicePunctuation.swift` 和对应的 `VoicePunctuationTests.swift` 已由 `platforms/ios/project.yml` 声明，但提交的 `MSIMEClient.xcodeproj/project.pbxproj` 没有同步它们。直接使用提交工程构建 `MSIMEKeyboardExtension` 或测试目标时，引用 `VoicePunctuation` 的 Swift 文件会报 `Cannot find 'VoicePunctuation' in scope`。CI 和部分本地流程先运行 XcodeGen，因此会掩盖这个缺口。

## Decision

每次新增 iOS Swift 源文件后，使用仓库锁定流程在资源占位齐全的环境中运行 `xcodegen generate`，并提交生成工程中对应的文件引用、构建阶段和测试阶段条目。本次将语音标点实现加入 App、键盘扩展、共享测试和键盘测试目标；保留 XcodeGen 生成的资源组稳定排序变化，不手工维护另一套工程清单。

生成工程仍以 `project.yml` 为唯一权威来源；提交的 `project.pbxproj` 只是让不先生成工程的构建入口也能得到与权威来源一致的源文件集合。

## Alternatives considered

- **只在验证命令前重新运行 XcodeGen**：可以绕过当前错误，但其他直接打开提交工程的开发者和工具仍会得到不完整的源文件集合，缺口会反复出现，因此不采用。
- **手工只补一处 `VoicePunctuation.swift`**：能让键盘扩展通过编译，却会漏掉共享测试和键盘测试目标，且容易与下一次生成结果分叉，因此不采用。

## Consequences

提交工程包含语音标点实现与测试的全部目标引用，直接构建不会再因缺少该类型而失败；后续新增源文件仍必须重新生成并检查工程差异。工程文件会带有 XcodeGen 对资源组顺序的稳定调整，但不改变资源内容或产品行为。

验证：在临时补齐 `target/ios/EngineResources` 的占位资源后，本地 `xcodegen generate` 成功；生成差异仅包含语音标点源文件、测试文件及生成器稳定的许可证资源组排序，随后移除占位资源。

关联实现笔记：[iOS 语音标点方式](../feature/2026-11-02-voice-punctuation-mode.md)。
