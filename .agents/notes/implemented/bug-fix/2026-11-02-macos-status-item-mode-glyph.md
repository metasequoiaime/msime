# Agent Note: macOS 中/英状态项画带框的模式字形

Status: implemented

## Problem

#10630：macOS 27.0.1、水杉 0.56.1 上打开「菜单栏显示中/英状态」后，切换中英文时菜单栏上的状态项一动不动。

状态项本身的接线是对的：`syncSystemInputModeForClient:` 每次切换都把控制器算出的模式交给 `MSIMEInputModeStatusItem`，`ShortcutTest.mm` 的 `TestInputModeStatusItem` 用替身断言了中、英、双之间的切换。问题在图像：#8548 嫌状态项的「中」和旁边系统输入源的「中」长得一样，#8556 的处理是让状态项固定画产品 logo，模式只留在悬停提示和读屏标签里。于是状态项不论什么模式都是同一张 logo，这个开关存在的全部意义（#7066：不加「英」输入源也能在菜单栏看到中英文状态）就没了。设置页对这个开关的说明仍写着「中文时是所用方案（中、双、五等），英文时是「英」」，与实际行为对不上。

这与 macOS 27 的 API 变化无关。macOS 27 上进程启用不了键盘输入模式（`TISEnableInputSource` 返回 noErr 而状态不变，见 `docs/macos-parity.md`），所以只加了「中」的用户系统输入源图标永远停在「中」，这正是状态项要补的缺口；`NSStatusItem` 的行为没有变，状态项一直在跟着模式更新，只是每次换上的都是同一张图。

## Decision

**状态项画当前模式的字形，外面套一圈圆角框。** `MSIMEInputModeStatusItemImage`（`platforms/macos/src/input/InputModeStatusItem.mm`）按 `MSIMEInputModeMenuIconFileIn` 取这个模式在系统输入法菜单里的那张 TIFF（本版本没声明的模式取主模式 `.Hans` 的，与系统菜单一致），把它画进一张 18×18 点的模板图，外沿描一圈圆角矩形。字形随模式变（中、英、双、五、日、한……），解决 #10630；圆角框让它与旁边系统输入源那个四周留白的光秃字形一眼可分，保住 #8548 的目标。图像用 `drawingHandler` 按需绘制并标成模板图，随菜单栏的浅色、深色着色。

bundle 里找不到模式图标时退回产品 logo（`MSIMEClientInputMethodMenuIcon.tiff`），不加框；连它也没有时返回 nil，状态项照常建出来，只是没有图像。#8556 提到的「旧版模式资源缺失时没有可显示的图像」由这条回退覆盖。

**图像单独成函数。** `MSIMEInputModeStatusItemImage(info, modeID, bundle)` 不碰菜单栏，新测试 `input-mode-status-item`（`platforms/macos/tests/input/InputModeStatusItemTest.mm`）用仓库的 `resources` 目录当 bundle、用与 `Info.plist.in` 同一取值路径的合成字典调用它。

## Alternatives considered

- **状态项固定画产品 logo（#8556 的做法）**：最强的理由是它与系统输入源图标绝不会混淆，且只要 bundle 有 logo 就总有图可画，不随模式产生多套视觉规则。不选它：状态项的唯一用途是显示中英文状态，固定图像让它失去用途（#10630）；悬停提示要把鼠标移上去才看得见，起不到「常驻显示」的作用。
- **把系统输入源的模式图标改成 logo，状态项保留光秃的模式字形**（#8548 提出的第一种）：最强的理由是状态项与系统菜单里的字形完全一致。不选它：要改 Info.plist 里每个模式的 `tsInputModeMenuIconFileKey`，影响所有不开状态项的用户；系统输入法菜单和切换条里所有模式会变成同一张 logo，用户就分不清中、双、五、日了。
- **恢复成与系统输入源一模一样的字形**：最强的理由是零新绘制代码，直接回退 #8556。不选它：#8548 指出的重复问题会原样回来，只加了「中」的用户菜单栏上会长期并排两个「中」。
- **用颜色区分**：最强的理由是不改字形大小。不选它：模板图只取 alpha，菜单栏会统一着色；不用模板图又要自己处理浅色、深色和菜单栏的高亮态。

## Consequences

- **收益**：切换中英文、换方案时状态项的字形跟着变，与设置页说明一致；带框的字形与旁边系统输入源的字形可以区分。
- **代价**：状态项比系统输入源字形略大（18 点对 16 点），字形本身大小不变，框占外圈。框的线宽（1.25 点）和圆角（4 点）只在离屏渲染里看过，真机在浅色、深色菜单栏和刘海屏下的观感还没确认。
- 本篇取代 2026-10-17 的「状态项使用产品标志」笔记：其理由（与系统输入源图标区分、资源缺失时有图可画）都已写进上面的备选和回退里，原篇已删除。状态项的整体设计、生命周期和测试替身见 [macOS 菜单栏输入模式状态项](../feature/2026-10-12-macos-input-mode-status-item.md)。

## Verification

- `input-mode-status-item` 测试：中、英、双三个模式的图像（按 alpha 逐像素比较）两两不同；图像是模板图；外沿一圈有着色而光秃的模式字形没有；未声明的五笔画主模式的字形；模式图标缺失时退回 logo。在改动前（固定 logo）运行时以「Chinese and English modes draw the same status item image」失败，改动后通过。
- 真机（已安装构建、图形会话）没有做：菜单栏上的实际观感和切换时的刷新。
