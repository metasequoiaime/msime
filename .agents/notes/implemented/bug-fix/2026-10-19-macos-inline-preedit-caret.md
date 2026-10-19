# Agent Note: macOS 行内组字显式声明预编辑样式

Status: implemented

## Problem

#8842 报告 macOS 输入框的组字显示为整段高亮，左右移动后无法辨认插入点。共享 Apple 文本桥已经传入零长度选区，但普通组字仍传裸字符串；只有分段选词和整句改字携带下划线与子句属性，普通组字依赖 IMK 与客户端的默认标记样式。替身测试只核对选区，无法约束标记样式。默认样式是否是报告客户端隐藏插入点的直接原因，尚未在该客户端中复现确认。

## Decision

macOS 的非空普通组字也使用属性字符串，显式提供单下划线和单一标记子句。选区仍来自既有光标映射，长度为零；不插入可提交的光标字符，不固定用户字体和颜色。UIKit 继续使用普通字符串；已选词段与整句改字的子句样式保持。

## Alternatives considered

- **在组字内容中插入竖线。** 不依赖客户端是否绘制插入点，位置直观；但会改变应用的标记文字、辅助功能读音与文本长度，需要额外映射，因此使用原生标记属性和选区。

## Verification

新增回归覆盖原始按键与拼音分词的六个插入位置，核对文字、零长度选区、单下划线、单一子句以及不覆盖字体与前景/背景颜色。真实 `NSTextView` 消费桥接结果，确认文字与标记范围、选区及 `shouldDrawInsertionPoint`。回归在旧实现下因普通组字没有属性字符串而失败，修复后通过。既有文本桥测试继续覆盖已选词段、整句改字、隐藏、标点与提交；控制器路由替身按文字内容接收属性字符串，样式验证留在文本桥测试。

在远端 macOS 27.0.1 arm64 上运行 `bash scripts/verify-local.sh --quick` 通过，包含原生宿主与共享 Apple 桥接层构建；同轮 Linux 容器原生测试 92/92 通过。最终源码重新构建后，`ctest --test-dir target/macos-isolated --output-on-failure` 153/153 通过，`ctest --test-dir target/apple-bridge --output-on-failure` 1/1 通过。本机运行 `pnpm run verify-notes` 与 `git diff --check`。未在报告的 macOS 26.7.1 环境或截图应用中完成真实输入验收。

## Consequences

显式样式让普通组字与已有分段组字使用同一类标记契约，每次非空组字更新会多创建一个属性字符串。客户端可能有自己的插入点绘制策略；裸字符串在原生 `NSTextView` 中同样可以返回 `shouldDrawInsertionPoint = YES`，因此该断言仅验证选区有效，不能证明报告中的可见性问题已解决。报告没有注明具体客户端，本次未完成截图应用的实际输入验收。

## Existing notes audit

检索活跃与被否笔记中的 `TextClient`、`setMarkedText`、`selectionRange`、行内预编辑和组字光标。[预编辑偏好叶子类型](../simplification/2026-10-15-preedit-preferences.md) 仅持有偏好类型，不持有宿主标记样式，与本次没有共享决策；整句改字与词段选择仍各自持有分段行为，本次不修改；释义换行的 `TextClient` 命中无关。未找到本次普通组字标记样式的既有归属。
