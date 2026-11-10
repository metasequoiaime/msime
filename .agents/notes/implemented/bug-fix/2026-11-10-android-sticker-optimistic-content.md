# Agent Note: Android 表情包兼容未声明图片类型的输入框

Status: implemented

## Problem

Android 水杉娘表情包原先只有在输入框通过 `EditorInfo.contentMimeTypes` 声明收 `image/png` 时才调用 `commitContent`。部分聊天输入框没有声明这个字段，但实际仍实现了 `InputConnection.commitContent`，导致点击表情后输入法直接退回剪贴板，用户需要离开键盘再长按粘贴，和同类表情包的点击发送行为不一致。

## Decision

`ImeStickers` 在有当前输入连接时一律先尝试 `commitContent`，不再把 `contentMimeTypes` 当作是否尝试的前置条件。输入框返回 `false` 或抛出边界异常后，保留现有顺序：带有 `SUPPORT_SOGOU_EXPRESSION = 1` 的输入框走微信私有表情命令，其余输入框把同一份 FileProvider URI 放进剪贴板并提示长按粘贴。已声明图片类型的输入框路径不变，输入框没有输入连接时仍不发送。

设备夹具增加一个不声明图片 MIME 类型但接受 `commitContent` 的合成编辑器，表情面板测试点击第一张水杉娘贴纸后核对它收到的 `image/png` 内容，覆盖这条兼容路径。

## Alternatives considered

- **继续严格依赖 `contentMimeTypes`**：符合 Android 编辑器声明的保守解释，也不增加对不规范输入框的尝试，但无法覆盖反馈中的聊天应用；现有回退只能复制，不能满足点击直接发送，因此不采用。
- **只按目标应用包名白名单绕过声明**：可以把行为限定在已确认的应用，但用户没有提供稳定的包名，其他同类聊天输入框仍会失败，也会把应用兼容性写死在输入法里，因此不采用。

## Consequences

遵守 `commitContent` 契约的编辑器仍按返回值决定是否接收；未声明但实现了该接口的编辑器现在可以直接收到贴纸。真正不支持图片的编辑器仍会回退到原来的剪贴板提示。极少数错误地返回 `true` 却不处理内容的编辑器可能不会触发剪贴板回退，这属于编辑器违反 `commitContent` 返回值契约的情况，设备验证需要覆盖报告者应用以确认其实际处理结果。

`StickerPackModel.commitsContent` 继续保留给最近复制图片等严格按声明判断的路径，避免把未声明的普通图片发送策略一并放宽；本次只改变内置表情包的发送顺序。
