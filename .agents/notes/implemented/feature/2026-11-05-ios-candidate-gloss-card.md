# Agent Note: iOS 长按候选打开释义卡片

Status: implemented

## Problem

#10915 要求手机键盘长按候选时打开一张「释义」卡片，看完整译文并能上屏或复制译文，三端界面和规则一致，设计由维护者在 2026-11-05 定下。iOS 原来长按候选条或展开网格的候选弹出 `UIMenu`，里面是以词定字、可上屏释义、优先显示、固定排位、删除词条、朗读和加入生词本；译文只在候选下面画一行，长了被截断，没有来源说明，也没有复制。

## Decision

- 候选条词块和展开网格格子都不再挂 `menu`，改挂 0.5 秒的 `UILongPressGestureRecognizer`，同时提供读屏操作「查看释义」，两者走同一个入口。条件与原来的长按菜单相同：写中文的方案、不在本地模式、候选条上不是邮箱后缀；英文建议、韩文汉字、日文长按照旧没有动作。
- 卡片 `CandidateGlossCardView`（`KeyboardExtension/Sources/candidate/CandidateGlossCard.swift`）从候选条打开时经 `installPanel` 装进按键区，就是 `keyAreaPanel`；从展开网格打开时叠在网格上，关掉卡片回到网格。顶栏上盖一层透明按钮 `glossCardDismissArea`：点卡片外的候选栏只关卡片、不选词，组字留着。卡片自己的「收起」、品牌键和收起键（经 `closeKeyboardPicker`）、键盘收起（`viewWillDisappear`）、换输入框（`textDidChange` 里文档标识变了）都关卡片；那个位置上的候选换了或组字结束时 `refreshGlossCard` 关卡片。关闭的时机与 HarmonyOS 的 #10924 相同。
- 内容 `CandidateGlossCardContent`：标题「释义」、候选原文、每个目标语言一段，来源行写「语言 · 来源名」，来源名是离线释义、水杉账号、腾讯翻译、小牛翻译、自定义服务、AI 辅助之一。每段用的是候选条下面画的同一条译文（`glossEntry`，离线和在线谁优先与 `preferredGloss` 一致），只是不受「仅当前高亮」限制，缓存里有就用。离线释义按半角或全角分号拆成义项逐条列出，与 Android 拆可上屏释义的分法相同；在线译文是一整条。没有译文时显示「暂无译文」；只在没有可用的在线翻译服务（候选翻译关着或没选服务）时再补一句「可以在设置里开启翻译服务」，开着服务却还没答上来时不补，与 HarmonyOS 的 `CandidateGlossCardPolicy.showsHint` 同一条规则。
- 「上屏译文」上屏第一目标语言的第一条译文并结束组字，做法与原来菜单里上屏释义相同（先确认候选还是眼前这一个，再 `insertOwnText` 和 `session.cancel()`）。「复制译文」写同一条文字进 `UIPasteboard.general`；没有完全访问时卡片上写明「开启『允许完全访问』后才能复制译文」并读屏播报，不写剪贴板。第一目标语言没有译文时两个按钮都不可用，即使第二目标语言有。
- 原来长按菜单的全部项原样进卡片左下角「更多操作」（`glossCardMenu`，仍由 `candidateMenuElements` 生成），选任一项先关卡片再做原来的事。没有可做的项时不出这个按钮。
- 在线翻译：卡片打开时，这个候选还没有译文、用户开着候选翻译并选了服务、有完全访问、且候选在会话的当前页上，就经 `CandidateTranslationStore.requestSingle` 为这一个词请求一次。它不等打字停顿，单独记一个任务，候选条刷新时的 `refresh`/`cancel` 不碰它；关卡片、换服务和内存告急时取消；失败不重试。发送的仍是当前页的中文候选词，没有超出 PRIVACY.md「候选翻译」一节写的范围，所以 PRIVACY.md 不改。「仅当前高亮」时长按非高亮候选也会为它请求这一次：长按就是用户在指这个词，PRIVACY.md 没有按高亮限定发送范围。
- 第二目标语言：iOS 触屏键盘的 `CandidateTranslationPreference.secondary` 恒为 nil（候选栏只画一种语言），卡片照这个读，所以 iOS 上只有一段。内容模型和请求都按「有第二目标语言就两段」写，`secondary` 将来放开时卡片自动列出两段。

## Alternatives considered

- **保留系统菜单，把卡片放进菜单的第一项**：改动最小，原有操作的位置完全不变。但 `UIMenu` 只能列文字项，放不下大字原文、多条义项和来源行，也盖不住按键区，与另外两端的卡片对不上；issue 写明原菜单项并进卡片的次要操作。
- **把原菜单各项摊成卡片上的一排按钮**：少点一次。但以词定字、固定排位（五个位置）和确认删除本身是分组和子菜单，摊开后一屏放不下，又要在三处重复菜单的可用条件；收进「更多操作」能直接复用 `candidateMenuElements`，条件只有一处。
- **卡片打开时没有离线释义就现查一次离线词典**：关着「离线英文释义」时也能看到释义。但 issue 写的是已经取到的释义直接用，没有译文时提示去设置里打开；现查还要在卡片里另走一套离线查询和代次校验。
- **展开网格里任何候选都请求在线翻译**：网格第一页之后的候选也能有在线译文。但 PRIVACY.md 写的是「当前页的中文候选词」，超出就要先改隐私说明交维护者审阅；当前页之外的候选只显示已有的离线释义。

## Consequences

- 长按候选不再弹系统菜单，原来的七类操作多点一次「更多操作」。读屏用户经「查看释义」打开卡片，候选的读屏提示改成「轻点输入，长按查看释义」。
- 卡片开着时顶栏被透明按钮盖住，候选栏、读音行和展开箭头都只用来关卡片。
- `KeyboardCandidatePanelView` 的 `menuElements` 参数换成 `onLongPress`；格子随单元格复用，按 `candidateIndex` 报眼下显示的候选。
- 卡片只在打开期间存在，内容是几段短文字，不缓存任何东西，不增加键盘扩展的常驻内存；单个请求的回答进同一个有上限的 `CandidateTranslationStore` 缓存。
- 测试：`CandidateGlossCardTests` 覆盖内容拆分和来源名、空状态与提示、按钮可用性、从候选条和展开网格打开、收起 / 点候选栏 / 换输入框 / 键盘收起四种关法、上屏译文、没有和有完全访问时的复制、「更多操作」里原菜单项齐全且以词定字照旧上屏首字。`KeyboardExtensionEditorUITests.testCandidateLongPressOpensTheGlossCardInTheSystemEditor` 在模拟器的真实系统键盘里长按、打开「更多操作」、收起再上屏译文。未在实体 iPhone 上验收；在线翻译服务的单个请求只有单元层面的条件检查，没有接真实服务验证。
- 展开网格的格子复用与长按的关系见 [iOS 大候选面板](../bug-fix/2026-10-19-ios-large-candidate-panel.md)。
