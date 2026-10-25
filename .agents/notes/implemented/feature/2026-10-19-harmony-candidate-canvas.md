# Agent Note: HarmonyOS 候选区绘制画布皮肤

Status: implemented

## Problem

HarmonyOS 的共享扫描器已经接收 Windows 的 canvas 清单，但 ArkUI 只画旧背景与顶部装饰，九宫格和自由挂件没有进入系统键盘。安全回退保证可读，不等于绘制了来源贴图。

## Decision

`KeyboardSession` 从共享扫描器取得 canvas 声明，再用 `skinResource` 取得受校验的资源。每次外观重载生成不可变快照和唯一代次，压缩资源总计最多 16MB，以 Base64 字符串保存（原因与解码边界见[设备验收修正](2026-10-19-harmony-canvas-apng.md)），逐键既不读取文件也不解码。宿主不重新解释 TOML 或自行拼接资源路径。

`CandidateCanvasFrame` 用 Image Kit 异步、逐张解码，普通条和展开网格按快照代次共用引用计数租约；最后一个读者离开时释放位图，已被撤销的异步结果不得重新进入 UI。帧数上限 120、边长上限 2048、累计解码预算 800 万像素。最初选择设备解码器统一提供 GIF、WebP、APNG 帧，是为了少维护一套容器与混合实现，并复用系统格式支持；但 SDK 未保证 APNG 多帧，默认图还可能不属于动画。APNG 现由[设备验收修正](2026-10-19-harmony-canvas-apng.md)中的有界拆帧合成接管，GIF/WebP 保持原生解码，统一时钟播放。GIF 无循环扩展只播一轮，扩展值为额外重复次数，APNG 和 WebP 值是总轮数，0 无限循环，有限循环停在末帧。小于等于 10ms 的帧按 100ms 处理，计时器只在下一帧边界唤醒；隐藏或组件销毁后停止。

九宫格的切片是源像素，边宽是 vp，`stretchDip` 优先，1/2/4 值按 CSS 四边展开。内容前后使用不参与命中测试的实时 Canvas 子节点；原先的 background/overlay 因设备背景不刷新而被[设备验收修正](2026-10-19-harmony-canvas-apng.md)取代，现由 Stack 的 matchParent 图层匹配内容测量尺寸；面板下挂件、面板、候选内容、上层挂件的顺序固定；2in1 画布层消费候选窗透明度，文字不受影响，挂件按九锚点 contain 定位，同 z 值保持清单顺序。手机候选条固定 50vp，皮肤内外留白等比缩放，下限是候选条实际排下的内容：34vp 的候选通道，读音行有东西显示时再加 16vp（有 canvas 时空着的读音行不占位）。原先按一行文字保留 28vp，而候选条竖着排了 50vp，Mate 70 上首选高亮从面板下沿伸出去，所以改为由候选条把自己的内容高度传给 `CandidateCanvasFrame`；读音行占位时 50vp 全给内容，边框缩到零，这是固定高度换来的代价。canvas 下首选高亮用包的 `selected`，不再用按键色配包的文字色。没有候选时候选条是工具栏，canvas 不画，底色用键盘的，不用包的 `surface`：否则皮肤的白卡片配按键前景色的图标，图标几乎看不见。展开网格读取实际内宽，避免九键中栏套用整条宽度。2in1 窗口的尺寸与光标夹紧包含完整留白，预编辑行存在时才增加声明行距。

资源快照存在不代表面板成功。只有 Canvas 真正绘制当前面板后才向共享主题解析器请求 `canvas_panel_drawn=true`，继续由 Rust 叠加取色器；失败保持安全配色，换肤以代次屏蔽迟到回调。旧的非 canvas 背景、顶部装饰和任意 base 的 v2 动画读取不变。

历史检索：[跨平台 schema](2026-10-19-windows-skin-schema-parity.md) 部分重叠，继续拥有扫描与回退契约；[macOS](2026-10-19-macos-candidate-canvas.md)、[Android](2026-10-19-android-candidate-canvas.md) 是其他宿主的实现，分别保留，不吸收或删除。旧笔记没有拥有 ArkUI 九宫格绘制决定。真实 HAP 编译还发现基线重复声明 `DictionaryUpdateOptions`，TypeScript 允许的 interface 合并在 ArkTS 被拒，因此只保留同形的一份定义。

## Alternatives considered

- 使用 ArkUI `borderImage`：代码短且由平台负责切片，但没有可靠的解码成功回调，不能把贴图配色切换与实际绘制绑定，也无法与自由挂件共用可控帧时钟，因此使用 Canvas。
- 在宿主重新解析 TOML 和读取任意路径：可以独立工作，但复制共享扫描器并扩大资源访问边界，因此不采用。
- 将整块 Canvas 放进 Stack 与内容共同测量：容易给前后层安排顺序，但 100% 高的 Canvas 会反过来撑高手机条和桌面卡片，最初因此使用 background/overlay。设备验收发现背景停在首帧，后续以 `LayoutPolicy.matchParent` 配合 `wrapContent` 消除反向撑高问题并恢复实时节点，详见[设备验收修正](2026-10-19-harmony-canvas-apng.md)。

## Consequences

收益是普通候选、展开候选与九键中栏消费同一份资源协议；代价是快照与原生位图占用额外内存，资源预算和引用计数是常驻约束。当前纯逻辑回归通过，新 canvas 用例有 41 条断言，覆盖切片、九锚点、稳定层级、留白、帧预算、三种循环计数与桌面尺寸；`build-native.sh arm64-v8a` 的 Rust/C++/NAPI 构建通过，已执行资源暂存、`ohpm install` 与真实 `hvigorw assembleHap --no-daemon` 编译。验证结果只对应实际执行过的提交版本。

手机模拟器已完成浅/深色动画、损坏面板回退、真实候选点击上屏和隐藏再显示四阶段验收，发现与修复的运行时问题见[设备验收修正](2026-10-19-harmony-canvas-apng.md)。物理设备、2in1 窗口、系统减弱动态效果和固定上游资源截图对比仍未完成。SwiftUI/共享设置预览和社区上传格式不属于此宿主切片，不据此宣称全平台完全兼容。
