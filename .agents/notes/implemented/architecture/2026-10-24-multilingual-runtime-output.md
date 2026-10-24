# Agent Note: 连续多语言候选在共享运行时逐段输出

Status: implemented

## Problem

#8921 的[会话候选](2026-10-24-multilingual-session-candidates.md)已保留语言分段，但宿主只知道整行字符串。传统的整行 OpenCC 会把日文汉字也当中文转换；候选重排后按旧下标取分段还会错配词面。完整产品仍归[统一输入提案](../../proposed/feature/2026-10-24-unified-language-composition.md)。

## Decision

`Preferences::multilingual_input` 使用独立的 `MultilingualInputPreferences`，语言枚举归 client-core，默认关闭、预选中英日。整项为默认值时不写入文档，旧配置缺项仍读为关闭；关闭时保留所选语言，开启后按配置顺序去重，不足两种时返回空集合。未知语言和错误字段类型由反序列化拒绝。现有 `MixedInputPreferences` 继续保持 `Copy`，不引入对 Engine 的依赖。

`EngineOptions` 显式传递语言集合，Host API 按共享偏好、版本提供的全拼方案、日文资源可用性筛选；有效语言不足两种时关闭。五笔混拼不视为全拼方案。初建和资源变化后的空闲重建沿用同一映射。`EngineSnapshot::multilingual_conversion` 转述实际来源 18 行的原始编码及 `Conversion`；不扩展随座位排列的平行数组。

Runtime 只在来源、编码、当前输入、完整词面和连续范围全部匹配时消费分段。输入须为非空小写 ASCII、在既有输入预算内，范围完整覆盖且至少涉及两种语言。候选展示只转换 `Chinese` 段，保留引擎缓存和选择身份；简繁设置变更、有效分段变更均使候选视图失效。

选择在刷新前保留转换、暂存中文前缀及明确的边缘方向。最终上屏必须以已保存的前缀和原始选中文字开头，才逐段转换：前缀按中文处理，混合行仅转中文，标点或完成操作的尾部保留。转换在 AI 上文和宿主字符宽度处理之前完成。来源 18 的成功提交禁止宿主整行简繁，即使元数据失效也保留原文，避免误改日文。下一键、失焦和每次新选择都清理本次状态。

以词定字复用 `CandidateEdge::character`，由引擎自身的首末汉字规则判定。运行时按明确方向找到所属语言段，同一个汉字分别出现在中文和日文段时也不能只凭字符猜语言。来源 18 不参与整行中文释义。

`Command::CommitCandidate` 保留引擎选择原始首行的语义，运行时经引擎顺序映射逆查其呈现座位，并在提交词面确实匹配后记录选择。`SelectionHow::FirstCandidateCommand = 12` 只追加枚举值，同步脚本 ABI 的 `first_candidate_command` 表和契约穷举，以及作者 C SDK 的 `MSIME_HOW_FIRST_CANDIDATE_COMMAND` 和 Rust SDK 的 `how::FIRST_CANDIDATE_COMMAND`，不复用高亮或点选的含义。两份可复制绑定由 `pack-tool` 的既有 SDK 完整性测试逐项核对；只更新运行时会让此测试失败，原有 ABI 编号和版本保持不变。

## Host output

六宿主展示来源 18 时保留运行时词面，提交按 `commit_context.script_conversion` 决定是否继续整行转换；来源 18 不请求整行中文释义。Windows 展开条目保留来源，展示文字的身份比较也使用相同的来源规则。iOS 快照及标点替换保留提交转换标记，候选重用缓存包含来源，以词定字菜单沿用对应来源。

语言集合变更等待空闲重建，繁简投影即时更新 Runtime，不结束组字。Windows 快捷键和 macOS 外观通知通过附加 C ABI `msime_client_set_multilingual_output` 同步投影，后续明确偏好更新替换这个临时值；该接口不写文档，变体取最新共享偏好。iOS 与 Harmony 切换后读取新视图，避免重画旧的来源 18 字符串。

## Windows 7 设置

Win7 原生设置沿用同一总开关、语言集合及普通全拼版本门槛；候选混输或连续多语言关闭时，语言控件禁用但选择保留。日文入口按产品版本隐藏时仍保留已有选择。只补缺省字段，显式空列表保持为空。`PreferencesStore` 精准允许缺省的 `multilingual_input` 根对象首次创建，整对象原子保存仍交由共享 Rust 校验与持久化，不开放任意缺失路径。

大型窗口翻译单元在 MSVC Debug 编译中实际触发 `C1128`；仅窗口静态库增加 `/bigobj`，避免为本次入口拆分窗口所有权。原生 HWND 和真实 Rust 存储集成验证在 Windows 11 上运行，不代替 Win7 SP1 设备验收。

## Existing note audit

- [统一输入提案](../../proposed/feature/2026-10-24-unified-language-composition.md)：部分重叠，产品偏好、资源门控和六宿主输出接线落在本篇；四套设置与产品验收仍归提案。
- [会话候选](2026-10-24-multilingual-session-candidates.md)：部分重叠，沿用默认关闭、唯一最佳路径和来源 18，本篇补充 Host 元数据与共享展示输出。
- [日文整段混输](../feature/2026-10-12-mixed-japanese-candidates.md)：部分重叠，旧来源 13 的整段语义不变，新路径不复用它。

## Alternatives considered

- 把语言列表并入 `MixedInputPreferences` 能让所有混输配置集中，但它是多个宿主直接复制的 `Copy` 值；增加 `Vec` 会迫使无关消费点改契约，因此新增独立的共享偏好项。
- 各宿主各自调用 OpenCC 并拼片段，能贴近现有候选展示代码，但六套实现会重复身份校验、边缘语言判定和尾部保护，因此集中在已有依赖 client-core 的 Runtime，不给纯引擎引入 client-base。
- 全句禁用简繁最容易保护日文，但中文片段和暂存中文前缀会漏转，因此只对可证明的中文片段转换，元数据失效才保留原样。
- 按字符定位边缘语言可以少传方向，但中文和日文可包含相同汉字，因此保留明确的首末方向，复用引擎的汉字规则。

## Verification

共享偏好先以缺字段和缺类型的编译错误确认测试覆盖新契约，补齐类型后默认关闭与语言去重专项通过。随后真实 `PreferencesStore` 保存、重载、关闭时保留子集、非法 JSON 字段和未知语言回归通过；独立审查补充的非默认 section 未知字段保留回归也通过，client-core 全部 538 项单测通过。损坏语言列表仍按既有恢复规则处理：正常读取和自动恢复不改写未知语言，用户显式恢复时逐字段救援可保留 `enabled`，并将损坏的列表恢复为默认三语。

Host 语言映射先得到 30 通过、2 失败，补映射后元数据仍缺失，得到 31 通过、1 失败。Runtime 六项测试先得到 1 通过、5 失败；逐段修复后全通过。新增边缘选字和释义回归得到 6 通过、3 失败，修复后全通过。独立复审再发现命令提交和候选 epoch 两处遗漏，真实回归得到 9 通过、2 失败；修复后引擎及 Host 32 项、Runtime 11 项专项全部通过，37 份源文件与笔记散列一致。

真实合成词库的 Host Session 与 Runtime 覆盖点选、结束组字、首末汉字、原始首行提交命令及重排后的命令；重排路径明确让展示首行不等于引擎首行，断言实际选择事实的座位、来源及路径。元数据回归只改变语言分段、不改变词面及编码，验证候选 epoch 递增。独立复审关闭两项阻断。

同源 Windows Runtime 全套 353 项通过。正规 `target/android-cargo` 的两个测试程序在指定 API35 `emulator-5590` 执行，引擎与 Host 专项 32 项、Runtime 专项 11 项全部通过，部署前后程序 SHA-256 及源码散列一致。这是 `adb shell` UID 测试，未安装 APK，也不代表应用沙箱、设置或系统 IME 验收。

Windows 的引擎、Runtime、client-core 和 script-runtime 全部 target 严格 clippy、工作区格式、Host API ABI 枚举契约、Host API 构建及正规 CMake 配置通过；既有链接器 stdout 诊断保留。首轮完整 quick 唯一失败是早期 proposed 笔记缺少必需章节，本篇转为 implemented 并通过 1819 篇笔记及 4 份归档封印检查后，使用原门禁重跑，不改测试基线或脚本。笔记转为 implemented 后，同一 `scripts/verify-local.sh --quick` 在 Windows runner 重跑退出 0；既有 131 项通过、2 项按宿主差异排除、116 项交 Windows 原生构建，Linux/Docker、Apple、Harmony 等缺失环境按脚本跳过。

本机 Host API 的普通全拼版本过滤、少于两种可用语言、下载日文包和焦点资源刷新专项已通过。真实 Host Session 覆盖组字期间语言变更延后、中文简繁即时投影、来源 18 上屏禁止二次转换；附加接口保持原候选身份。共享设置三项交互覆盖默认关、显式空列表、关闭保留选择以及不支持日文时保留存储列表，全部通过。

后续宿主接入验证：本机引擎及 Host API 严格 clippy 通过；iOS 专用模拟器上的 `InputBridgeResponseTests` 八项全部通过，包含来源 18 的提交转换边界。Windows 同源引擎多语言 34 项、Host API 两项专项通过，原生宿主及原生设置均编译完成。设置页新增版本条件使用 `if constexpr`，并将既有资源撤回回调的分支改为编译期 `else`，消除 MSVC 的常量条件与不可达代码警告，不改变撤回行为。

Windows 原生套件初次加载了测试目录中的旧 DLL；同步本次 Host API DLL 并校验 SHA-256 后，261 项中 251 项通过，另十项失败仍待主线集成后核对，不能记为全套通过。Windows 严格 clippy 仍被既有剪贴板 `Result<_, ()>` 签名拦截。Harmony 文本契约同时检查三处候选展示传来源、来源 18 跳过整行转换和提交上下文边界，不能只把原签名检查删除；共享偏好字段检查要求类型别名，因此设置类型沿用 `export type`。

最新 Harmony 原生库、共享设置 rawfile 和 `assembleHap` 均构建通过。Android 专用 API35 `emulator-5590` 安装原生发布形态 APK 后，应用 UID 的 `MultilingualInputDeviceSmoke` 通过：共享偏好开启三语、真实 IME 输入 `hellowangluogakkou` 得到 `hello网络学校`，组字期间用既有 `Ctrl+Shift+F` 切换后显示并上屏 `hello網絡学校`，日文「学校」未被整行转换。首次设备用例试图在组字时寻找隐藏的工具栏，改用可在组字中工作的快捷键；不为测试改变产品工具栏策略。测试结束恢复偏好文档和原输入法。与固定 develop 整合后的 APK SHA-256 为 `494ce1fb2f32e88c06687745d6afc98d447695d031051964fd8f3801dbb2b26c`；此结论只覆盖 API35 专用模拟器，未扩写为 API28 或真机验收。

### 固定 develop 整合验证

功能整合固定 develop `54d8a185c55323984f0a3fe1397c435b9382e2d0`，保留双方偏好回归。完整 `scripts/verify-local.sh --quick` 退出 0：portable Windows 179 项通过；Linux 原生注册 111 项，其中 110 项通过、`linux-ibus-demand-activation` 按环境条件跳过，无失败；Rust 工作区、Android Rust/Java、WASM、Linux 桌面外壳、macOS 原生及 Apple bridge 编译通过。此前 Linux 聚焦提示断言在本轮通过，尚未查明此前失败原因，未修改测试或宣称修复时序问题。本机 Windows 原生阶段因无 MinGW 配置跳过，Harmony 资源当时未 staging，分别用以下原生构建补充。

整合后的引擎多语言 34 项、Host API 两项、偏好四项和共享设置交互三项通过，引擎及 Host API 严格 clippy 通过。iOS 重新构建原生工程并在专用模拟器运行 `InputBridgeResponseTests` 八项，通过后关闭专用模拟器。Android 重编发布形态 APK，并在指定 API35 `emulator-5590` 再次通过应用 UID 的真实 IME 多语言专项，沿用上述候选及繁简切换断言，结束后恢复偏好及原 IME。

Harmony 原生库、共享设置、依赖安装和核心资源 staging 完成后，`hvigorw assembleHap --no-daemon` 成功；核对最终 HAP 包含两个原生库、拼音数据库、设置 HTML、许可证及音效。该产物未签名、未做设备验收；此前未 staging 的构建不作为有效打包证据。 本地门禁需显式设置 `DEVECO_SDK_HOME` 与 DevEco 的 `JAVA_HOME`；保留仓库 Node 24 在 `PATH` 前部，用 `NODE_HOME=/Applications/DevEco-Studio.app/Contents/tools/node` 单独给 Hvigor 指定配套 Node 18。把 DevEco Node 放到全局 `PATH` 前面会让 pnpm 因最低 Node 版本失败。设置页与 HAP 在分离 Node 的环境中均已重新构建通过。

Windows 同源 Host DLL、原生宿主、WinUI 设置及 Win7 设置编译通过。Win7 存储先重现缺省对象保存被拒绝（`invalid_change`），精确开放路径后保存成功并保留空列表；真实 HWND 与 Rust 存储的 `windows7-settings` 集成通过。韩文候选夹具原先把恰好 4096 字节误作溢出，现同时断言 4096 保留读音、4097 才舍弃。脚本注释夹具在 MSVC 下实际于首个 `is_array()` 断言失败，明确使用 `json::array` 后定向测试通过；没有更改产品候选策略来迎合夹具。

SSH Session 0 中的 Windows 原生全套 267 项中 261 项通过，初步失败：`windows-fullscreen-foreground`、`windows-gloss-column-keys`、`windows-clipboard-history`、`windows-server`、`windows-reply-composer` 和 `msimeui-tests`。严格 clippy 的既有剪贴板签名失败继续保留。该轮快照在执行前后核对 514 个源码路径，Win7 与两处夹具修补另有定向原生验证。独立宿主复审确认 Win7 漏接已补齐，未知字段经 Win7 UI 保存的直接回归仍为覆盖限制。

### Windows 原生失败定位

同一批 `windows-fullscreen-foreground`、`windows-server` 和 `msimeui-tests` 可执行程序，从 SSH 的 Session 0 改为临时任务调度器的交互令牌，在已登录的 Session 3 运行后全部通过；任务结束即移除。前者的诊断矩形确实铺满显示器，但 Session 0 的 `IsWindowVisible` 为假。此证据只说明这些用例依赖可用桌面，不能代替 TIP 在编辑器中的多语言验收，也不通过改产品可见性判定来掩盖失败。

其余三项由实际失败分支定位到旧夹具假设：释义列高亮的是无释义云占位，夹具显式关闭云候选并断言首项合成词及释义；Home/End 旧用例把 Unicode `U` 当作无候选，但它实际生成原文回退行，直接改成回退断言仍在后续 no-op 断言失败，因此改为无词库、关闭云占位的普通全拼 `zz`，保留真正空候选分支及原有逐键视图不变断言；剪贴板最大容量用例显式选择 1000 条，仍检查默认 50 条，并按已有安全契约拒绝清除不可信叶重解析点，保留链接与外部目标内容。后两例补异常行号输出，免得原生异常退出丢失定位信息。三个完整用例在 Windows 定向复跑全部通过，不修改产品路由、容量默认值或文件访问边界。随后校验 517 个源码路径，恢复正式无诊断的全屏测试并重编，在交互桌面 Session 3 运行全部原生套件，267/267 项通过（40.05 秒）；执行后源码散列仍一致，临时计划任务已移除。

### PR CI 的 SDK 完整性补齐

首轮 PR CI 的 `Script plugins` 因作者 SDK 漏同步新枚举值失败，本地 `pack-tool --test sdk` 重现一项通过、一项失败。补齐 C 与 Rust 的 `FirstCandidateCommand = 12` 常量后两项通过；`msime-pack-tool` 与 `msime-script-runtime` 全套随后通过，原有四项性能测量按测试定义忽略。此遗漏属于本次新增 ABI 的配套缺口，不归为 develop 基线问题。

### Apple CI 的测试契约补齐

首轮 iOS CI 在 Xcode 26.3 对旧 `MorePanelFoldTests.expectedTitles` 的长数组拼接表达式报类型检查超时。拆为顺序追加，保留每一标题、条件与短路读取次序。本机 Xcode 27.0 的专用 iOS 26.5 模拟器随后通过该类 12 项及 `InputBridgeResponseTests` 8 项，合计 20 项；模拟器已关闭并删除。此结果不代替 CI 的 Xcode 26.3 验证。

macOS 首轮四项 CI 失败在本地原样重现：三个快捷键测试的会话替身未实现新增输出 setter，覆盖检查还保留了 `traditional_chinese_variant` 的旧豁免。只补 setter 后仍有两个布局断言失败，独立复审定位到返回基类空视图覆盖组字状态。普通替身现在显式保存夹具当前视图，布局与繁简两处注入时同步；展开网格从自己的 `gridView` 动态生成，保留代次、分页及派生释义。未删产品通知、未将 setter 总返回 nil，也未放松原布局或滚动断言。真正读取该偏好的共享 `ffi/input.rs` 已由 macOS 调用，因此删除过时豁免。

重编 `shortcut-test` 后，原四项 `shortcut`、`shortcut-expanded-scroll`、`shortcut-translations`、`preference-coverage` 全部通过；派生网格的 `shortcut-translation-font-size` 也通过。首次补 setter 的两项失败日志保留，不因日志文件名带 green 而记为通过。本轮提交前完整 quick 通过，Windows 本机交叉编译与 pipe-only 仍按缺少工具链跳过。

### 再同步 develop 的合并边界

接上远端已同步固定 develop `a33e619f80b83d01b98d4ce4820910bd119dbe06` 的合并提交 `c12da1c7dc9af33dbe72ef33763072596f304ac9`，不重建或覆盖远端历史。唯一冲突是上游也修补了 iOS 长数组类型检查表达式；保留该文件的上游版本，显式 `[String]` 与展开的条件均不改变标题、顺序或短路读取。两份偏好 Rust 文件的自动合并结果按仓库 edition 2021 格式化。独立三方合并复核确认其他共有文件没有语义丢失，中文优先、混合备选、来源 18 与 SDK 相对本分支前一提交保持不变。

合并后首次 macOS 链接因本地旧 Host API 库缺少上游新增 `cross_commit_spacing` 接口失败，重编同源库后成功。五项专项中四项 CTest 通过，`shortcut-expanded-scroll` 两次超过原有 60 秒上限；同环境直接运行原程序和原断言退出 0，用时 72.31 秒，采样在 AppKit 约束布局中。保留超时证据，不修改时限、不把该次 CTest 记为全绿。固定合并提交的 macOS CI 原生 184 项随后全部通过，展开滚动专项耗时 12.51 秒；iOS CI 的两个修补目标类也通过。

该合并树的完整 quick 退出 0。本机再次运行 iOS 专项时编译完成，但安装测试宿主在 CoreSimulator 桥接中持续等待超过 20 分钟；中断本任务的测试后退出 75，专用模拟器已关闭并删除，不记录为测试通过。CI 结果与本机旧轮的 20 项通过分别保留，不能把编译完成当成本轮设备运行通过。

同一合并提交的 Windows CI 在 `windows7-settings` 登录后的 `preferences_load` 断言失败，其余 13 项通过。上游将登录方式名称集中到共享资料响应后，`AccountForm` 要求 `providerNames`，而合成传输仍只返回 `user` 和 `providers`。补齐资料、改名、上传及移除头像四类响应的名称表，不修改产品校验或原断言。在本任务既有 Windows 11 检出中核对旧文件散列，只同步当前两个账号头文件后重编原测试，重现同一断言失败（28.15 秒）；仅更新夹具重编后该完整集成用例通过（34.61 秒），执行后再次核对三文件散列。该对照定位夹具契约，未声称刷新了整棵 Windows 源码、重跑原生全套或完成 Win7 真机验收。独立增量复审无阻断。

## Consequences

Host API 已注入所选语言并按普通全拼能力、至少两种可用语言和日文模型路径门控；六宿主沿相同来源及上屏标记消费输出。共享 React 和 Windows（WinUI 与 Win7）、Android、iOS 原生设置提供默认关闭的总开关及语言集合，关闭保留选择，显式空列表不恢复为默认三语。iOS 活着的键盘会话重载列表包含 `multilingual_input`。后台日文准备不阻塞连续输入的契约见[词库适配笔记](2026-10-24-multilingual-dictionary-adapter.md)。不能把内部逐段回归或交叉编译通过写成 #8921 全平台真实系统入口已验收。
