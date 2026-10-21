# Agent Note: Fcitx5 在 X11 上按输入上下文绘制原生画布

Status: implemented

## Problem

`classicui` 的主题只能表示静态九宫格，不能表示独立行距和任意层级、锚点的挂件。逐帧改写主题也不能补出这样的场景。Fcitx5 5.0.21 是 Debian 12 验收镜像的真实版本，早于 5.0.24 的 `setCustomInputPanelCallback`，不能把后者当作所有发行版都有的入口。

## Decision

只在有真实 X11 输入上下文、选中有效 canvas 包且当前仍使用水杉接管的 `classicui` 主题时，按上下文创建无焦点的原生候选窗口。窗口使用 Cairo/Pango，在内存里绘制九宫格、自由挂件、预编辑与候选，并由原有 `CandidateWord` 和 `PageableCandidateList` 回调处理选择和翻页。候选身份及围栏仍归原有运行时。

原生窗口成功显示后，保留 `clientPreedit`，清空交给桌面 UI 的候选数据；失败则保留完整的原 `InputPanel`。不改客户端能力位，不替换全局 UI 插件，不读取其私有对象布局。失焦、清空、切换输入法和销毁均隐藏窗口。Xwayland 只有上下文明确属于 X11 显示时才进入；Wayland 上不创建普通顶层窗口冒充 input-popup。

### 旧笔记审计

- [渲染边界](../../implemented/architecture/2026-10-13-cross-platform-candidate-rendering-boundaries.md)：部分重叠，本篇只翻转已具备原生窗口协议的 X11 canvas 所有权；Wayland、IBus、其他 UI 和非 canvas 不变。
- [静态 canvas 主题](../../implemented/feature/2026-10-19-linux-fcitx-canvas-panel.md)：部分重叠，保留为未接管场景的静态路径。
- [图片格式](../../implemented/feature/2026-10-20-linux-canvas-image-formats.md)：部分重叠，复用安全读取与像素转换，不把静态解码当动画完成。
- [真实 GTK 验收](../../implemented/bug-fix/2026-10-19-linux-canvas-gtk-acceptance.md)：部分重叠，沿用隔离 daemon 与真实编辑器，并增加原生窗口的指针与焦点验收。
- [宿主生命周期](../../implemented/architecture/2026-10-13-candidate-skin-v2-native-host-lifecycle.md)：部分重叠，保持隐藏停止、显示重新计时的要求。其他检索命中是其他平台或输入语义，与本次窗口所有权无关。

## Alternatives considered

继续用 `classicui` 的主题最能保留桌面行为和发行版兼容性，因此仍作为回退；它没有任意绘制入口，不能承担完整画布。

只用新版按上下文回调可以复用框架的批处理更新，但最低支持版本会把 Debian 12 排除，而且 Wayland 的 popup 包装类不是已安装的稳定开发接口。直接复制内部类布局会引入无法验证的跨版本 ABI，不能采用。

另开 Wayland 普通窗口实现最少，但合成器没有输入法 popup 的定位与权限保证，不能作为输入法面板。

## Consequences

字体、序号、候选尺寸与间距由[原生画布布局](../feature/2026-10-20-linux-canvas-layout.md)接入；测量、绘制与点击共用解析结果。

X11 canvas 可以绘制超出九宫格主题表达范围的自由挂件和独立行距，继续兼容 Debian 12 的 Fcitx5 5.0.21，不提升现有最低框架版本。代价是插件开始拥有一条原生候选窗口通路，需要维护指针配对、右键菜单、缩放和焦点生命周期。资源只在换肤或文件戳变化时加载，不新增空闲帧循环。没有合成器时用 ShapeBounding 提供二值透明，将半透明像素合在主题底色上；有合成器时用 ARGB。ShapeInput 只包含候选和翻页按钮，挂件与留白不吞鼠标。

GTK 3 移动整个窗口后，在下一次光标更新时上报新的根坐标；原生窗口按框架上报的位置移动，不承诺在客户端尚未上报时追踪窗口。跨屏编辑器选择光标所在屏幕的工作区，无法确认时才回退到既有显示器选择逻辑。超过可用工作区的画布保留桌面面板，不截掉候选使之无法选择。

图片动画由[有界动画解码与播放](../feature/2026-10-20-linux-canvas-animation.md)接入本篇的 X11 窗口：面板和挂件缓存完整画布帧，以单调时钟按最近帧边界唤醒，候选刷新、菜单和定位不重启；隐藏取消计时，重新显示及资源变化从首帧开始。静态桌面回退仍使用单帧 API。完整皮肤几何及材质尚有缺口；Wayland 需要输入法 popup 角色及框架稳定入口，IBus、GNOME Shell、kimpanel 仍需各自实现和验收，不代表 Linux 或全平台兼容目标完成。

## Testing

`linux-candidate-native-canvas` 检查 Cairo 几何、独立行距、九种锚点、前后层、透明中心与命中边界。六项 `fcitx5-native-canvas-{horizontal,vertical}`（普通、`-scale2` 与 `-composited`） 在独立 Xvfb、D-Bus 和真实 GTK 编辑器里检查挂件、行距热更新、框架坐标跟随、明暗、坏图回退与修复、旧鼠标释放、滚轮翻页、左键上屏、右键置顶写入合成词库、失焦及切换输入法隐藏。图片、输入、配置和词库均为专用合成夹具，不连接用户桌面。`-composited` 启动独立 `xcompmgr`，先等待其取得合成器 selection，再检查候选窗口确实使用 32 位 visual；普通和 2 倍用例覆盖没有合成器时的 Shape 回退。

返回第一页时只比较选中候选行的像素：引擎按需扩充缓存会把总页数从 30 改成 31，不能将这类页数变化误判成候选错误。焦点切换先等待 GTK 映射窗口及 Fcitx 上下文焦点，再激活和输入；焦点恢复已经清空组合，不另加异步 Escape 重置干扰后续按键。

`linux-fcitx5-contract` 跟随共用菜单函数检查词库来源限制和取消固定条件，并确认框架菜单与原生菜单都走这一函数；不能继续要求抽取前的内联代码拼写。运行时的动作身份与第三方主题所有权仍由真实 Fcitx 测试覆盖。

完整 quick 门禁通过（构建机 run `20261020-040313`）：Linux 原生 93/93、full/wubi 构建及安装隔离、Linux 设置应用、Rust 工作区、Android target 和 Java 宿主、WebAssembly、macOS 与共享 Apple 桥接均通过。该构建副本缺少 HarmonyOS 打包前置及设置依赖，Windows 原生／交叉构建与 pipe 编译也因工具链缺失跳过；Windows 可在当前宿主执行的策略测试为 171 项通过、2 项宿主差异排除、62 项需要 Windows 构建。参考仓库对比、无缓存的词库再生成和部分可选工具检查仍按原脚本跳过，不将其报告成完整跨平台设备验收。

选中条、逐角高亮、悬停与预编辑配色由[原生画布外观](../../implemented/feature/2026-10-20-linux-canvas-appearance.md)补充，保留本篇其余范围与生命周期规则。

「只在选中 canvas 包时进入」已被 [X11 原生画布默认](2026-10-21-linux-x11-native-candidate-default.md) 取代：X11 上水杉接管的 classicui 主题一律由原生窗口画，非 canvas 主题按 macOS 结构；本篇的窗口生命周期、回退与 Wayland 边界仍然成立。
