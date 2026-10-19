# Agent Note: Windows 候选皮肤播放 APNG 与 GIF 帧

Status: implemented

## Problem

[Windows canvas 绘制](2026-10-19-windows-candidate-canvas.md)只解码和显示首帧。固定上游 msime-windows `dd018f79858314ed542a47c528608752b13bd261` 已按时钟播放 APNG 与 GIF，自由挂件及传统装饰在这里仍静止；GIF 的子矩形首帧还可能被错误当作整张逻辑画布。

## Decision

从上游固定提交的 `ui/AnimatedImage` 移植有界 APNG 拆帧、WIC 静态 PNG 解压、GIF 处置合成及有限循环时钟。代码与测试的来源、许可证和本地调整一起登记，不搬运相邻工作区的未提交文件。

候选窗拥有独立的图片播放缓存，不把时钟塞进材质合成器。外观更换清空缓存，逐键与逐帧复用已解码像素；每张动画只保留当前 GPU 位图，帧边界更新像素。加载与共享扫描器一致限制为每资源 8MiB、边长 2048、120 帧、累计八百万像素，所有缓存共享解码预算。静态、损坏与超预算动画沿用可读静态回退。WIC WebP 能力与上游相同，依赖系统编解码器，不能把缺少它的机器计为已验证。

Canvas 面板、两层挂件、旧背景和传统装饰使用同一图片时钟。只有绘制过且后续会变化的动画才安排下一帧定时器；隐藏、失焦立即隐藏、销毁、系统关闭动画时撤销定时器。隐藏再显示重置时钟，有限循环完成保留末帧；减少动态效果沿用静态解码的默认图。图片变化不修改候选身份、焦点或点击区域。

历史检索：[Windows canvas](2026-10-19-windows-candidate-canvas.md)部分重叠，本篇取代只画首帧的妥协，几何与配色仍归原篇；[原生宿主生命周期](../architecture/2026-10-13-candidate-skin-v2-native-host-lifecycle.md)保留隐私隐藏路径和候选身份围栏；[装饰图片动画](2026-10-16-skin-decoration-idle-motion-and-apng.md)保留清单语义；Linux、其他宿主、社区与材质的决定分别保留，不在本篇改写。

## Alternatives considered

- 把上游整个 DeviceResources 的动画缓存和时钟一起覆盖：与来源 API 最接近，但本地材质合成、设备恢复与缓存已经独立演进，覆盖整份实现会扩大回归范围；只复用解码器，候选窗持有播放生命周期。
- 只播放 APNG：覆盖两款官方皮肤的大部分资源，代码较少；但同一上游皮肤还有 GIF，自由挂件必须按逻辑画布和 disposal 合成，不能继续静止。
- 每拍重新从磁盘读取并解码当前帧：不需要保留动画像素，但会把文件访问和 WIC 解码放进帧循环，增加输入延迟，因此只在资源更换后准备一次。

## Testing

MinGW x64 与 x86 均以 `-Wall -Wextra -Werror` 独立编译并链接解码器与 `windows-candidate-image-animation.exe`，在 Wine 10 与 Xvfb 中分别执行，均为 15 项全部通过。两种架构的候选窗与截图工具通过同等警告级别的语法检查；本机没有完整 vcpkg 树，Windows 产品全量链接仍由 PR CI 执行。覆盖 APNG 混合与处置、排除默认图、损坏与预算、GIF 透明度与逻辑画布、有限循环末帧、真正的 Direct2D 红蓝像素变化、当前 GPU 位图复用、删除源文件后的缓存播放、设备目标更换、仅绘制资源安排唤醒、损坏资源负缓存，以及四张合法动画耗尽共享 32,000,000 字节后第五张回退和清空后的预算回收。

32 位源码的流长度检查用转换回原类型比较来保留截断防护，避免 `size_t` 只有 32 位时出现恒假比较警告。Wine 镜像显式安装 i386 的 OpenGL 与 Mesa：只装 `wine32` 在关闭推荐依赖时缺 `libGL.so.1`，解码测试虽能过，Direct2D 的 WIC 像素目标却建不起来。补齐后，两种架构都实际验证像素换帧。

本机原有 Windows 无系统 API 套件 159 项通过；新测试因依赖 WIC 与 Direct2D，由 Windows 构建和 Wine 单独执行，不把 macOS 跳过计为通过。原生 Windows CI 先运行这 15 项断言，再以 `--verify-image-animation` 在专用桌面的真实 `CandidateWindow` 上检查 APNG、GIF、传统 APNG 装饰无输入时的定时换帧、休眠／唤醒、减少动态效果和隐藏／重开，输出变化前后截图。该选项临时切换非持久化系统偏好并恢复原值；Wine 不支持实际 DirectComposition 合成，不能代替该验收。[Windows GNU CI](https://github.com/metasequoiaime/msime/actions/runs/39275207672/job/118137747854) 全量链接宿主、TSF DLL、Server 与测试通过。[Windows 2025 原生图库](https://github.com/metasequoiaime/msime/actions/runs/39275207672/job/118140505874) 的 15 项断言通过，三个动画场景的换帧、休眠／唤醒、减少动态效果和隐藏／重开均通过；下载产物逐对核对后，APNG 挂件、GIF 挂件、传统 APNG 装饰分别有 221、96、237 个 RGB 像素变化。该证据覆盖真实 CandidateWindow 绘制组件，未覆盖已安装 TSF 在真实编辑器中的输入。

## Consequences

Windows 候选窗的 canvas 面板、自由挂件、传统背景和装饰共用上游动画语义，并在静止或隐藏时停止唤醒。皮肤几何、配色和候选身份仍遵循原有边界。旧篇只画首帧的并行修改妥协由本篇取代，未覆盖的原生设置预览和 Linux 绘制仍独立存在。

CPU 帧快照与 GPU 当前帧占用内存，WIC WebP 支持因系统而异。首次解码仍发生于 UI 线程，但只在外观资源加载时执行，有严格尺寸与帧预算；不增加逐键文件访问。不宣称已完成 Linux 原生面板或其余全部平台验收。
