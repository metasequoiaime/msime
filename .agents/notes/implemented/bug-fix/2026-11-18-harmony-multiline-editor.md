# Agent Note: 鸿蒙多行文本框允许中文组字

Status: implemented

## Problem

鸿蒙输入法接到多行文本框类型时，原来的编辑器特征转换只把单行文字及部分专用文字类型视为可组字文本；多行类型被当作未知类型，中文模式下实体字母键交回应用。报告者反馈微信及可能其他应用有中文模式直出英文的现象；带多行修复的源码版安装后，报告者确认微信可以使用。没有取到微信实际返回的类型，不能断定多行类型是微信问题的根因。

## Decision

`KeyboardSession` 把运行时的 `inputMethodEngine` 类型常量和 `inputMethod.TextInputType.MULTILINE` 一并交给 `EditorPatternPolicy` 转成编辑器特征。多行普通文本可以进入组字；密码、验证码、数字和未知类型沿用原有处理。SDK 的 `inputMethodEngine.PATTERN_*` 没有多行常量，`inputMethod.TextInputType` 包含 `MULTILINE`。OpenHarmony 在[固定提交的输入法控制器](https://github.com/openharmony/miscservices_inputmethod/blob/60200686c36b770d59ff7527e350545f723bc831/frameworks/inputmethod_controller/src/input_method_controller.cpp)保存输入框上报的原始类型，并在输入法扩展接口传递该值。

## Alternatives considered

把所有未知类型按文字处理可以覆盖更多未来编辑器类型，但可能把敏感或非文本类型交给组字引擎；只接受 SDK 已声明的多行类型。

## Consequences

`editor-pattern-policy.test.ts` 使用合成系统常量调用生产转换和 `EditorPolicy`，覆盖多行、普通文本、密码、验证码、数字、未知类型和专用文字模式。严格 TypeScript 编译和 Node 测试通过；在测试输出的独立副本中去掉多行分支时，同一测试按预期失败。尚未在官方签名构建上进行设备验收，未获取微信返回的编辑器类型，也未证明其他应用的问题全部由此引起。
