# Agent Note: 皮肤预览的解码失败归属图片读取结果

Status: implemented

## Problem

外部皮肤的装饰图与背景图用独立布尔值记录解码失败，并在图片读取结果变化后的 passive effect 中清空。首帧提交时先收到解码错误，随后执行的清空 effect 会把失败覆盖，错误图片继续显示，CI 的图片失败回退测试因此偶发失败。

## Decision

`useSkinPreviewAssets` 与 `usePreviewBackground` 的错误状态记录发生解码失败的图片读取结果对象。只有当前图片结果与该对象相同时才显示失败；刷新后即使 URL 和字节完全相同，新的读取结果也可重试。装饰图与背景图都遵守这一归属，不用加载后的清空 effect。

## Alternatives considered

- **在测试中等待更久再触发错误。** 可以避开 CI 上的窗口，但不能修复首帧收到错误后又被 effect 覆盖的产品竞态；布局 effect 中触发错误能稳定复现。
- **按 URL 记录失败。** 字符串易比较，但刷新同一损坏文件后即使内容和 URL 没变，也必须允许重新解码；以读取结果对象区分每次加载。

## Verification

布局 effect 中触发首帧错误的装饰图回归用例在修复前稳定失败，修复后通过；背景图同样保持回退，刷新同一图片后可以重试。外部皮肤的 66 项测试通过，包含图片读取、解码失败、旧响应与刷新用例。

## Consequences

状态最多保留上一份失败结果及其 URL，直到下一次失败或组件卸载；不额外保留图片字节。读取、CSP、清单和公开 hook 返回形状不变。

## 当场审计

检索 proposed、implemented、rejected 中的图片预览、解码失败和 useSkinImage：[外部皮肤预览几何](../../implemented/simplification/2026-10-15-skin-preview-geometry.md)部分重叠，只负责几何及透明度；[外部扫描代次](../../implemented/simplification/2026-10-12-external-scan-generation.md)部分重叠，继续负责读取响应归属。其余移动设置预览和 Windows 原生预览笔记与此状态竞态无关，没有被本决定取代的笔记。
