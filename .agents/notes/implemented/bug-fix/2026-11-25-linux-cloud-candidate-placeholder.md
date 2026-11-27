# Agent Note: Linux 候选窗跳过云候选占位席位

Status: implemented

## Problem

云端回答还没到（或到达但没有新词）时，引擎在候选列表的固定位置放一行 `CandidateSource::CloudPending`（数字 15），文字是「☁」。它是引擎用来[固定云候选位置](../feature/2026-10-17-cloud-candidate-seat-placeholder.md)的空位，选不中：按它的数字、点它，引擎什么都不做但算处理过（`crates/engine/src/session/commit.rs`，#6888）。宿主不该画这一行，也不该给它一个序号——否则它后面每一行的编号都比用户看到的大一位，按第二个选到的是第三个。

Windows（#9273，见 [Windows 候选窗隐藏云占位席位](2026-10-22-windows-hides-cloud-pending-seat.md)）、macOS 和移动三端都这么做了，Linux 的 IBus 与 Fcitx5 到 0.17.0 没有：两个宿主把整页候选原样画出来，用户看到一个 ☁ 格子，按它的数字没有反应（#11850）。同一张页面上真正的第二行标着 3，点或按 2 落到的是占位席位。Linux 的候选操作菜单、数字选词、点选、展开网格和上屏后联想的数字也都按页内下标取行，所以受影响的不止画法：删词、固定、展开里的格子、点选选中的身份都跟着偏一格。

## Decision

Linux 侧的可见性规则集中在 `platforms/linux/src/candidates/CandidateSources.h`，与 Windows 的 `platforms/windows/common/CandidateSources.h` 是同一组规则的两份，注释里互相点名：

- `candidate_source_is_visible` 只跳过 15。旧的 AI 占位行（12）继续画成次要色，与 Windows 一致；共享内核已经不再产出它。
- `visible_page` 返回这一页看得见的行（保留每行自己的 `id`，点选按身份不按数组位置）。
- `visible_page_candidate(page, slot)` 给数字选词、联想数字和以后的按键映射用：第 slot 个看得见的行。
- `visible_page_size(page)` 给行数判断用：候选窗是否为这一页打开、展开是否有「更多」、双拼键位图给候选窗让开多少。

两个宿主的落地点：

- IBus（`platforms/linux/src/core/ClientEngine.cpp`）：`render()` 先取 `visible_page`，候选表、序号、光标位置都按它建；`rendered_candidates` 存这份过滤后的页，所以数字选词、点选、右键动作菜单、删词/固定、上屏后联想这些读 `rendered_candidates` 的入口全部自动跟随，不需要各自再过滤一遍。
- Fcitx5（`platforms/linux/fcitx5/FcitxEngine.cpp`）：`FcitxPage` 用过滤后的页建候选和序号；数字选词、联想空格/数字、日文转换取 `visible_page_candidate`；`render()` 的候选窗条件、`expandAllowed`、`expandedEntries`（网格与卷轴）、键位图让开的行数用 `visible_page_size` 或过滤后的完整候选。

`visible_page`/`visible_page_size` 对非对象行照原样保留或计数，交给下游按原来的方式报错，不在这里吞掉协议错误。

## Alternatives considered

**只在画的时候跳过，选词仍按引擎页内下标。** 这是 Windows 上 #9273 走岔过的路：数字键改了，`;` 和 `'` 仍按原始下标选，隐藏席位在第二格时选到了那个选不中的格子。Linux 受影响的下标入口更多（点选、删词、固定、展开、联想），分开改迟早再漂移，所以统一让「画出来的那一份」同时是选词用的那一份。

**让共享运行时不再产出占位席位。** 席位的作用就是固定位置：云回答到达时在原位替换，本地候选不整体后移，回答没有新词时还留着一个。去掉它要么让云候选到达时整页跳动，要么把「先占位后替换」的逻辑搬到每个宿主；另外 Windows、macOS 和移动端已经按宿主跳过的做法稳定运行，Linux 跟上即可。

**把占位席位做成 `page` 为空时的单独状态（只画辅助行）。** 云端回答可能带着新词而不是只替换席位，那时席位所在的那一页仍要显示；单独状态反而多一条渲染分支。

## Consequences

- 用户不再看到选不中的 ☁ 格子，它后面每行的序号与数字键、点选、删词、固定、展开格子一一对上。
- 整页只剩占位席位时候选窗不打开（之前是一个空面板）；IBus 的辅助行不再为它显示页码。
- 这一份规则在 Linux 上新增一处常量与四个判断函数，以后引擎再加来源时只需要改这里和两个宿主的取页位置，不用再逐入口排查下标。
- 云请求链路本身不改：`#11850` 报告的「Google 云候选一直不出现」另案调查（配置合法、终端里同一个 URL 能返回，剩下服务进程环境差异和失败分类日志两条），这个 PR 只修宿主画出占位席位这个已确认的缺陷。
- 验证边界：`linux-candidate-sources` 是本机可跑的纯函数单测；两个宿主的编译与整套 ctest 由 `platforms/linux/build-container.sh` 覆盖，本次在 Ubuntu 机器上跑过。没有 Fedora 真机桌面，`#11850` 的报告者环境复验留给合并后的 nightly 反馈。
