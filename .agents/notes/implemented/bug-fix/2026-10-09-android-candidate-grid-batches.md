# Agent Note: Android 展开候选网格分批建格子

Status: implemented

## Problem

#6471：只敲一个声母（报告人双拼敲 J，候选条显示 1/606 页）时点展开，面板要卡将近一秒才出来。在 API 35 专用模拟器上用全拼敲 `j` 复现：完整候选有 3632 个，`ImeCandidates.renderExpandedCandidates` 把它们一次全建成按钮放进 `CandidateWrapLayout`，随后整树样式通道给每个格子上一遍色，下一帧再把 3632 个按钮逐个量字宽、排版、录制绘制。打点（临时 `Log`，未提交）六次的结果：打开到第一帧画完 617–1577 ms，中位约 716 ms；其中取完整候选（JNI + 解析 JSON）只占 15–32 ms，建按钮 156–311 ms，测量与布局 360–627 ms，绘制 62–729 ms。瓶颈是按候选总数建和量 View，不是引擎调用。全拼九键的三栏面板中栏（`ImeNineKeyPanel.renderCandidates`）是同一种写法，而且面板开着时每次选拼音、⌫、改筛选都整面重建。

## Decision

- 两个面板（九键三栏面板的来历见 [九宫格展开面板](../feature/2026-10-08-nine-key-expanded-panel.md)）的网格都交给 `ImeCandidateGrid`：先建第一批 48 格，滚动视图的视口底边再往下一屏就超过已建网格底边时追加下一批。规则（批大小、重画保留多少、何时追加）在纯 Java 的 `CandidateGridBatchPolicy`，由 `CandidateGridBatchPolicySmoke` 覆盖。
- 追加由滚动视图的 `OnScrollChangeListener` 触发；网格每次布局后也检查一次（投递到下一轮），第一批填不满一屏多时不等用户滚动就接着追加。
- 打开和重画时，格子仍由随后的整树样式通道 `ImeStyler.applySkin` 上色；滚动中追加的格子没有那一遍，追加时对每格调 `applySkinToView`，传入与整树通道相同的候选上下文（整面网格在 `expandedCandidates` 下是候选上下文，九键中栏不是、靠「候选 」开头的描述被认出），颜色与之前一致。
- 同一代候选的重画（释义晚到、换皮肤）建回上次已建的格子数，否则网格变短、滚动位置被截掉；换了一代从第一批开始并把滚动位置拉回顶部。九键面板收起时 `reset()`，重开从顶部开始，与原来 `renderedGeneration = -1` 的行为一致。
- 取完整候选的接口不变：引擎仍一次返回全部候选，宿主只是不再一次全画出来。

修后同一流程六次：打开到第一帧画完 23–29 ms（中位约 26 ms），其中取完整候选 15–20 ms、建 48 格约 2–3 ms、测量布局约 4 ms；滚动时每批追加 1.2–6.4 ms。从追加出来的格子里选候选能正常上屏。

## Alternatives considered

- **换成 `RecyclerView` + `GridLayoutManager`**：滚到哪建到哪、还能回收滚出视口的格子，是 Android 列表的标准做法，几千格时内存也更省。但格子按字宽排（长词占得宽），`GridLayoutManager` 是等宽列，要么改设计成等宽格，要么自己写一个按宽度换行的 `LayoutManager`；后者比现在这个流式布局复杂得多。分批建在用户实际会滚到的范围内（几百格）已经够快，回收不是瓶颈。
- **引擎分页给完整候选（`allCandidates` 带 offset/limit）**：能把 15–20 ms 的取数也降下来。但要改 host-api 契约和六个宿主的调用方，而这次测到的瓶颈九成以上在 View 上；取数那一段留给以后真成为瓶颈时再做。
- **只在后台线程建 View**：View 只能在主线程创建和挂上，建好之后测量布局仍在主线程，省不掉大头。
- **限制展开面板最多显示前 N 个候选**：最简单，但会让排在后面的生僻字无法从面板里选到，报告人要的是「随翻阅动态生成」，不是截断。

## Consequences

- 收益：打开展开面板的耗时与候选总数无关；九键三栏面板里每次选拼音、⌫、改筛选的重建也只建第一批。
- 代价：网格里的格子不是一次全在 View 树里。依赖整面格子都存在的代码（例如按无障碍描述找第 N 个候选）只对已建出的前几批成立；现有设备冒烟只找第 10 个候选，在第一批里。读屏用户滑到底部时同样会触发追加。
- 已滚出的格子不回收：一直滚到几千格的底部，View 数量与原来全建时相同，只是分摊到了滚动过程里。
- 网格的读屏描述仍报完整候选总数（`完整候选列表；…；N 个候选`），不是已建格数。

## Verification

- `bash platforms/android/check-host.sh`（经 rbuild 在 Mac Studio）：含 `CandidateGridBatchPolicySmoke`。
- `build-apk.sh`（arm64-v8a）出包，本机 API 35 专用模拟器：上面的修前修后数字；连续滑动后截图，追加出的格子样式与第一批一致；从追加的格子选「强」上屏正确；`smoke.sh emulator-5580` 全部设备套件（含 `CandidatePanelDeviceSmoke`、`NineKeyPanelDeviceSmoke`）。
