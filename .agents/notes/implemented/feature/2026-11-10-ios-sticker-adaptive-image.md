# Agent Note: iOS 表情包优先提交自适应图片

Status: implemented

## Problem

用户希望手机表情包点按后直接进入聊天，而不是所有平台都先复制到剪贴板。iOS 键盘扩展的 `UITextDocumentProxy` 仍以文本操作为主，但 iOS 18 为支持自适应图片字形的编辑器增加了可选的图片插入能力；旧的三端贴纸决定没有覆盖这条新能力。

## Decision

iOS 点按水杉娘贴纸后仍在后台把 WebP 转成透明 PNG。回到主线程时，若当前代理对象实际实现 `UITextInput`、编辑器声明支持自适应图片字形且有选区，就用 `NSAdaptiveImageGlyph` 直接插入；任一条件不满足，沿用完全访问下的 PNG 剪贴板回退和长按粘贴提示。iOS 17 以及不支持该富文本能力的聊天应用不伪装成直发成功。

Android 另有独立改动：有输入连接就先尝试 `commitContent`，不按微信包名限制；本篇只记录 iOS 与 HarmonyOS 的边界。

HarmonyOS 当前 ArkTS `@ohos.inputMethodEngine.InputClient` 公开接口只有文字和编辑操作；旧 Java `TextInputClient.insertRichContent` 文档不能直接用于现有 ArkTS 宿主。因此 HarmonyOS 暂不伪造一个不存在的通用直发接口，继续写入图片剪贴板并提示粘贴，待 ArkTS/原生宿主提供可编译、可验证的富内容接口后再接入。

## Alternatives considered

- **继续所有 iOS 应用只走剪贴板**：兼容面最大，但在支持自适应图片的编辑器中白白多一步，不能满足新系统提供的能力，因此补上条件直发。
- **通过私有 UIKit 方法或按应用包名发送**：可能覆盖更多聊天应用，但违反公开 API 边界或把行为写死在不稳定的应用实现上，不采用。
- **在 HarmonyOS ArkTS 中按 Java 文档拼一个 `RichContent` 对象**：当前接口声明和宿主类型都没有这个构造方式，无法证明能跨进程编解码，编译和设备验收也没有依据，不采用。

## Consequences

支持 iOS 18 自适应图片字形的编辑器可以点按后直接收到贴纸；普通文本框、未实现该能力的聊天应用和旧系统仍会得到原来的剪贴板回退。回退继续要求 iOS 键盘完全访问。HarmonyOS 的用户体验暂时不变，但边界保持真实，后续接入富内容 API 时可从同一 `MascotStickerStore` 的 PNG/剪贴板路径扩展并补设备验收。
