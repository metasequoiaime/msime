# Agent Note: 翻页展开长出新生页时高亮必须前进

Status: implemented

## Problem

Windows 桌面端翻到后面几页之后，按数字键 1 上屏的是翻页之前那一页的第一个候选，屏幕上明明显示的是别的词（#9517）。

复现：输入 `szyd`，第一页第一个候选是「是这样的」；连按几次 `+` 往后翻；在靠后的某一页按 `1`，上屏的仍是「是这样的」。前几页一切正常，只有翻过若干页之后开始出错，出错时的候选恰好是前几页的内容。

根因在 `crates/input-runtime/src/runtime.rs` 的 `expand_for_next_page`。引擎对单字母查询只发二十四个候选，其余的在被要求时才给。打开最后一页时会先展开，让那一页一次就显示满。展开之后 `len` 变大、`(len - 1) / page_size` 也跟着变大，用户原来停的地方可能已经不是末页了。但那个函数只按展开前的 `len` 判断「这一页原本不满」，于是返回 `true`，`dispatch` 里 `Action::NextPage if filled_current_page` 分支让高亮原地不动。

候选集换了、页码没动，两个东西就此脱节。候选窗照常按新的候选集重画，数字键选词读的却是 `highlighted` 推出的页码：`ServerSession.cpp` 的数字分支按槽位在当前页里取行，页由 `highlighted / page_size` 算出来。于是按 1 拿到的是旧页第一行。

这条路径本身不校验帧，鼠标点选会经过 `render_serial` 核对（`SessionController.cpp` 的 `request_selection` / `request_page`），键盘数字键不经过，所以脱节时不报错，静默选错。

## Decision

`expand_for_next_page` 在展开之后重新取一次 `visible_len`。只有当展开确实没有长出新页时，才允许沿用「留在原地」的结论；长出新页就返回 `false`，让 `dispatch` 走正常前进。

```rust
let grown = self.visible_len();
let stayed = page == last_page && !page_was_full;
let grown_last_page = grown.saturating_sub(1) / self.page_size;
if stayed && grown_last_page > last_page {
    return Ok(false);
}
Ok(stayed)
```

`saturating_sub` 是防御：`expand_cached_candidates` 返回 `true` 意味着引擎给了候选，但这里不该依赖那个前提去做可能下溢的减法。

留在原地的原意（注释里写着「advancing would step over the candidates that just showed up」）没有变，只是收窄到真正适用的情形：当前页从短变满、页数不变。

新增用例 `paging_past_the_short_last_page_moves_the_highlight`。用三个候选、每页五个、藏七个构造：初始只有一页且不满，按 `+` 展开后候选数变成十个，末页从第 0 页后移到第 1 页，高亮必须落到第 1 页。用例同时断言候选窗看到的第一行与选中结果一致，把「看到的」和「上屏的」钉在一起。

原有的 `paging_reaches_candidates_the_engine_withheld` 用的是十二个候选、每页五个、藏八个，那条路径上 `page_was_full` 恒为真，`stayed` 从一开始就是假，所以从未踩到这个缺陷，也解释了这个缺陷为什么能留到现在。

## Alternatives considered

- **保留高亮不动，改为在展开时把候选集还原。** 表面上能维持「看到的和选中的一致」，代价是展开本身失去意义：引擎被要求给出更多候选之后又要退回去，下一次翻页还得再来一遍，而且 `bump_candidates_epoch` 已经让候选窗换了帧。
- **数字键选词时校验帧并拒绝过期请求。** 更彻底，能覆盖所有「画面和内部状态不一致」的情形。没有放进这次：`ServerSession` 的数字分支拿不到候选窗那一帧的 `render_serial`，要校验就得把帧标识顺着 TSF 的按键包传到 Server，动的是跨进程契约，和这个确定性逻辑错误不是一回事。
- **让引擎一次把候选全给出来。** 省掉展开这一步，问题自然消失。没有采用：二十四个的截断本来就是为单字母查询的首屏开销设的，见 `expand_for_next_page` 上方注释。

## Consequences

- 停在不满的末页按 `+`，现在会真的翻到下一页。行为上和「这一页很短，翻过去看看后面还有什么」的直觉一致。
- 展开之后确实没有新页的情形（候选刚好填满当前页）仍然原地不动，原注释描述的行为保留。
- 已知的遗留：脱节仍可能在别的路径出现，只要候选集被异步改写而页码不动，比如云联想的迟到回包。`a_late_cloud_candidate_keeps_the_page_the_user_opened` 覆盖的那条路径已经保证页码不动、候选窗也不换帧，是另一套处理。

## Verification

- 本机按维护者习惯只做静态检查，Rust 编译与单测交给 CI（`ci-platforms.yml` 覆盖 `input-runtime`）。
- 待 CI 上 `cargo test -p msime-input-runtime` 通过，重点看新增的 `paging_past_the_short_last_page_moves_the_highlight`。
- 真机复验：输入 `szyd`，连按 `+` 翻过若干页，每次按 `1`，上屏的应当始终是当前屏幕第一行。
