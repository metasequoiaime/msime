# Agent Note: 候选窗的像素尺寸向上取整，量尺寸和绘制才会得出同一个折行结论

Status: implemented

## Problem

打半个拼音（`ni`）时，第 6 个候选「犀牛(rh)」被画在候选窗**外面**：卡片只有一行高，那一项孤零零落在卡片下方的桌面上，颜色也不随卡片背景。打完整拼音（`nihao`）则正常。

候选窗排两次版：`card_bounds` 用 `candidate_card_size` 量出卡片尺寸、据此设定窗口像素尺寸；绘制时再拿**客户区像素除回 DIP**（`frame.card_width`）调用同一个 `candidate_page_layout` 排一次。两次之间隔着一次取整：

```cpp
static_cast<int64_t>(panel_width * scale + 0.5)   // 四舍五入到整像素
```

四舍五入向下时，绘制拿到的可用宽度比量出来的**窄半个像素**。而「这一页能不能排成一行」是个硬比较——`candidate_single_line_columns` 判断 `natural_total <= line_width`，成立就返回按比例压缩的列宽、**强制不折行**，不成立就返回 `nullopt` 让 `candidate_page_layout` 折行。半个像素足以让它翻面：量尺寸那次判定「放得下」，于是高度只给了一行；绘制那次判定「放不下」而折行，第二行的候选就落在卡片外。

半个拼音的候选最宽（六项都带编码提示），总宽正好顶在这个边界上。这与应用无关，只与候选总宽卡在哪个像素上有关；[隐藏云占位席位](2026-10-22-windows-hides-cloud-pending-seat.md)之后更容易撞上——原本第 6 格是文字极短的「☁」，总宽够不着边界，换成真候选就顶上了。

## Decision

卡片的像素尺寸**向上取整**，宽高都是：

```cpp
static_cast<int64_t>(std::ceil(panel_width * scale))
```

绘制拿到的宽度因此永远不小于量出来的，两次排版的折行结论必然一致。代价是窗口可能比需要的大一个像素，看不出来。

**never** 改回四舍五入：这里差半个像素不是「差半个像素」的问题，而是会翻转一个布尔判断，后果是整行候选画到窗口外面。

## 阴影边距也按整像素扣（#10856）

光把卡片向上取整还不够。窗口四周的透明阴影边距（缺省左右各 32 DIP）在 `card_bounds` 里是四舍五入成整像素留的，候选缩放不是 100% 时 32 DIP 不是整像素（110% 是 35.2，留 35）。绘制原来拿 float 的客户区 DIP（`GetClientSize`）减去**没取整的** DIP 边距，被舍掉的零点几像素就从卡片上扣了回去，卡片比量出来的窄，同样的翻面又出现了：#10856 里窗口按一行定的高度，第 6 个候选折到第二行、画到卡片外。按 6 个两字候选、96 到 192 DPI、50% 到 200% 缩放、12 到 32 号字扫一遍，修之前约 18% 的组合会翻面，100% 缩放且没有 canvas 时一组也不会，所以只在调过候选缩放或用了 canvas 皮肤（外留白、挂件伸出去的距离可以是任意小数）时出现。

现在的规则，三处一起成立：

- `card_bounds` 用 `candidate_shadow_pixels` 把边距取整成像素给窗口留边，`shadow_insets_` 记的是这几个整像素除回 DIP（再加 canvas 内边距），`shadow_pixels_` 记下像素本身。卡片在窗口里的位置因此正好落在像素边界上。
- 绘制用 `candidate_card_extent`：客户区像素减去 `shadow_pixels_`，用和 `card_bounds` 同一个 `layout_scale` 换回 DIP，再扣 canvas 内边距，全程 double，不走 float 的 `GetClientSize`。
- 两次换算之间仍可能差几个 ulp（例如 192 DPI、110% 时绘制拿到 484.99999999999994，量出来是 485）。`candidate_single_line_columns` 的两处比较和 `candidate_page_layout` 的折行判断都加上 `candidate_layout_fits`（1e-6 DIP），和 `candidate_item_layout` 本来就用的那个容差是同一个常量。它只吸收浮点误差，不是下面否决的那种半像素容差。

## Alternatives considered

- **把量尺寸那次的布局结果带到绘制**，不重算 — 根治，两次排版合成一次就不可能分叉。否决：`card_bounds` 与绘制之间隔着窗口创建、DPI 变化、皮肤重载和合成交换链的分档，布局结果要跨这些事件缓存并保证失效正确，比取整的改动大得多，风险也高。向上取整让两次输入一致，等价于让它们得出同一结论。
- **在 `candidate_single_line_columns` 的比较里留容差**（`natural_total <= line_width + 0.5`，或 #10856 分诊时提的约 1 DIP） — 一行改动。否决：容差要选得比任何一次取整误差都大、又不能大到让真正放不下的页强行挤成一行；而误差来自 DPI 缩放，不是固定的半像素。治的是症状不是因。现在的 `candidate_layout_fits` 是 1e-6，只管浮点运算顺序不同带来的 ulp 级差别，取整误差由上面的整像素换算消掉。
- **绘制时也用 `candidate_card_size` 重新量一遍**，不读客户区 — 两边输入一致。否决：客户区才是窗口真实能画的范围，按量出来的宽度画会在窗口被工作区夹断时画到看不见的地方。

## Consequences

- **收益**：折行结论在量尺寸和绘制之间一致，候选不再画到卡片外。
- **代价**：卡片宽高各可能多一个像素。
- **重访信号**：如果以后把两次排版合并成一次（上面否决的第一条），这个向上取整就不再是必需的，但留着也无害。

## Verification

Windows 11，装机后在 Windows Terminal 和记事本里打 `ni`：六个候选都在卡片内，卡片高度随折行增长。修复前 Terminal 里第 6 项画在卡片外的桌面上。

阴影边距那一半（#10856）：`platforms/windows/tests/ui/candidate_card_size.cpp` 末尾按上面的范围扫一遍，用 `candidate_shadow_pixels`、`candidate_card_pixels`、`candidate_card_extent` 复现 `card_bounds` 定窗口和绘制换回卡片宽度的过程，要求绘制拿到的宽度不比量出来的窄、排出来仍是一行。按修之前的算式（float 客户区减没取整的边距）这条用例失败；只做整像素换算、不加 `candidate_layout_fits` 时剩 7 个 ulp 级的组合失败。没有在 Windows 真机上按调过的候选缩放复验。
