# Agent Note: macOS 隐藏行内预编辑用全角空格占位

Status: implemented

## Problem

用户反馈 0.60.0 上在 Electron 聊天软件里用双拼时，组字期间按退格，候选窗里的编码少一个键，输入框里已经上屏的字也被删掉一个；连按两次，「做的」两个字都没了。用户的预编辑显示选的是「不显示」（`tsf_preedit_style=empty`）。

[macOS 隐藏行内预编辑仍保持输入法接管](2026-11-09-macos-hidden-preedit-ime-ownership.md)（#11172，0.59.2 起）让共享文本桥在这种样式下组字期间发送 `setMarkedText:@""`。IMK 收到了调用，但 Chromium 与 Electron 判断这枚键是否被输入法接管时，看的是按键前后客户端有没有非空 marked text、这次有没有插入文字。空串在它们那里就是没有组字，于是原始 keydown 照样交给网页。字母键单独一个 keydown 在网页里不产生文字，所以看不出问题；退格的删除发生在 keydown 上，就把已上屏的字删掉了。引擎没有问题：小鹤双拼下组字期间每次退格都回 `handled=true`。

## Decision

`shared/apple/TextClient.mm` 在 macOS 上、行内预编辑隐藏且仍有组字（编码、暂存词段或整句改字）时，向客户端写一个全角空格 U+3000 作为 marked text，选区在 0；组字结束时照常写空串清掉它。`clientHasMarkedText` 按实际写入的内容记为真，所以组字清空的那一次清理一定会发出。iOS 不走这条分支，UIKit 的文档代理不受影响。

## Alternatives considered

- **保留空 marked text，在控制器里对 Chromium 系客户端单独吞键。** 宿主返回 `YES` 本来就表示吞键，问题出在 Chromium 不看这个返回值，只看 marked text，所以宿主这边吞不住；按 bundle id 识别 Chromium 系应用还得维护一张永远不全的名单。
- **用零宽空格 U+200B 占位。** 它不会让后面的文字右移一格，看起来更「隐形」，但没有成熟输入法用过，部分终端和编辑器对零宽字符的宽度与光标处理不一致。鼠须管关闭行内预编辑时用的就是全角空格，它的注释写明半角字符会让基线跳动，这里沿用它验证过的做法。
- **改回完全不发 marked text。** 那是 #11172 修掉的状态，部分编辑器会把原始双拼编码交给应用。

## Consequences

隐藏样式下组字期间，插入点处会出现一个全角空格宽的空白，组字结束后消失；候选窗仍按 marked text 起点定位，与之前位置一致。由于客户端确实持有 marked text，组字结束和焦点切换时的清理不再被跳过。`TextClientTest` 中断言隐藏样式写空串的三处改成断言写全角空格，并新增一条「组字被删空时清掉占位」的断言。
