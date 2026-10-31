# Agent Note: iOS 部分上屏直接替换标记区

Status: implemented

## Problem

#10284 中输入 `hxgsdzyb` 后点「香港地」，系统编辑器只剩 `yb`。引擎的 `sxcr` 部分选词回归不能覆盖跨进程的 `UITextDocumentProxy` 写入。专用 iOS 26.5 模拟器上的系统扩展与独立原生编辑器已复现：原实现在五秒内始终不保留「香港地」。

## Decision

运行时上屏替换行内预编辑时，用 `insertText` 直接提交并替换旧标记区，随后照常标记剩余组字。原来的 `setMarkedText(提交)`、`unmarkText()`、`setMarkedText(剩余)` 连续代理调用会让剩余组字覆盖选中的汉字；同进程 `UITextView` 探针正常，不能替代跨进程的失败证据。其他直接输入、统计、简繁转换和键位映射不变。

系统回归先按报告原码输入，检查文档完整保留「香港地yb」；再把末键改为 `k`，选「有」，随后输入 `sxcr` 逐字选「上」「傳/传」。`有` 的粤拼是 `jau`，本仓键位表把 `au` 放在 `k`，`yb` 对应 `jou`，当前词库没有它的候选。无候选尾码与宿主丢字分别验证，不为报告样例改映射。

系统扩展的设置共享还依赖模拟器链接权限：构建使用 `CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- CODE_SIGNING_REQUIRED=YES`，确认 App Group 容器存在再验收。操作陷阱同步写入启动 skill。

## 既有笔记审计

- [严格双拼展开](2026-10-29-cantonese-shuangpin-strict-expansion.md)：部分重叠，继续持有引擎映射、边界及排序决定；本篇补宿主验收。
- [iOS 独立粤语双拼入口](../feature/2026-10-27-ios-cantonese-shuangpin-selection.md)：部分重叠，继续持有入口与分号键决定；本篇不改启用列表和默认值。
- [移动端部分选词保留展开面板](2026-10-25-mobile-expanded-partial-selection.md)：无关，面板是否收起不决定宿主文档的标记替换。
- [macOS 上屏重入](2026-10-23-macos-reentrant-text-client-commit.md)：无关，该笔记保护同步重入后的新一代写入；本次临时事件诊断没有发现上屏后的文档重入。

## Alternatives considered

- **保留重标记后 `unmarkText`**：本进程探针中可正确保留汉字，改动最少。但真实系统代理在同一条输入路径失败，不能以进程内实验否定复现。
- **代理调用前提前登记 `noteOwnEdit`**：能保护潜在的同步重入，初步怀疑剩余候选被回调清掉时曾做对照。但提前登记仍没有候选；事件诊断显示选词快照本来就是两键组字、零候选，并没有清空组字的回调，因此不保留无关的时序改动，也不延长回声窗口。

## Verification

- 原实现在系统编辑器复现只剩 `yb`；直接 `insertText` 后实际为「香港地yb」。临时诊断只记录事件及长度，定位结束已移除。
- Xcode 27.1 SDK、iOS 26.5 专用模拟器：`build-for-testing` 通过；`test-without-building` 串行执行粤语逐段、普通拼音及数字原样上屏三项系统扩展专项全部通过，无跳过。粤语项使用独立原生 `MSIMEKeyboardTestHost`，其余两项使用主应用试打编辑框。
- 最终 `bash scripts/verify-local.sh --quick` 通过，复用远端既有缓存；笔记三项门禁和 `git diff --check` 通过。没有建立新的完整构建树。

## Consequences

- 已选汉字由提交操作一次替换旧标记区，剩余字母仍可编辑和选词；普通上屏与原样提交共用此路径，需要一并回归。
- 真实系统扩展回归需要预先启用第三方键盘、完全访问及锁定词库，运行成本高于进程内测试。
- 不覆盖报告中的「开发者」「不能」整词词库与组词需求，不据宿主修复关闭整个 #10284；报告者设备尚待复验。
