# Agent Note: 在水杉应用里编辑一条剪贴板历史

Status: implemented

## Problem

#5971：网页里选中的网址常常多带几个字，用户想在键盘剪贴板面板里长按那一条、点「编辑」，把多余的字删掉后存回历史，替换原来那一条，以后反复插入。「分词」只能挑词片插入，不改历史本身。整条链路都没有「改文字」：面板操作行没有入口，Android 的 `ClipboardHistoryStore` 和共享存储的 `MobileClipboardAction` 都没有 replace，键盘里也没有可输入的文本框。

## Decision

- **编辑在应用里做，不在键盘里放文本框**，沿用常用语 #5673 的约定。长按操作行在「删除」后面加「编辑」（`ImePanels.renderClipboardItemActions`），点了由 `MSIMEInputService.editClipboardItem` 打开应用里的 `ClipboardEditPage`（`PageId.CLIPBOARD_EDIT`，需要参数才能打开，不进设置搜索）。页面上是预填原文的多行输入框，能用系统的选择手柄精确删字，下面是「取消」「保存」；保存或取消后弹出这一页并 `moveTaskToBack`，回到原来的应用。
- **共享存储新增 `ClipboardHistoryStore::replace`**（`crates/client-core/src/clipboard.rs`），写法同 `remove` / `set_pinned`：拿写锁、重新读最新的历史、按原文找条目，不信任宿主手里的行号。
  - 改完的条目留在原位：时间戳和固定状态都不变，编辑不会让它跳到最前。
  - 新文字和另一条已有的历史相同时两条合并成一条，留在被编辑那条的位置，固定状态取两者之或（`Merged`）。
  - 原条目已经不在（别的宿主删掉或清空）时返回 `NotFound`；新文字和原文相同时 `Unchanged`，不写盘；新文字按移动端规则（`mobile_text_is_valid`）不合规时 `Invalid`，不碰磁盘。落盘格式不变。
- **FFI**：`MobileClipboardAction::Replace { text, replacement }`，原文的长度校验和 `Remove` 相同；响应是 `{replaced, merged, reason: "not_found"|"invalid"|null, entries}`。只新增变体，iOS 和 HarmonyOS 的调用方不受影响，以后可以直接接入。
- **键盘交给编辑页的是键，不是文字**：`ClipboardHistoryPolicy.editKey`（时间戳加文字的散列和长度），放在深链参数 `entry` 里，编辑页在共享存储里按它找到那一条再显示。深链字符串参数最长 256 个字符，剪贴板记录可以长到四万字节；文字也不该进 Intent。宿主入口是 exported 的，参数都带外部标记：这个键只用来选择显示哪一条，别的应用猜不出某一条的键，猜错了页面只说这条已经不在，写回仍要用户自己点「保存」。
- **打开编辑页之前 `forgetCurrentClip()`**：原文多半还在系统剪贴板里。已处理身份存在 `:ime` 进程的 SharedPreferences 里，应用主进程写不了，所以由键盘在打开时记下；不记的话改完回来一打开面板，补读又把原文记回来，编辑等于没生效。
- **改到一半的文字放在 ViewModel 里**（`ClipboardEditPage.Draft`）：旋转、换深浅模式、分屏改尺寸会重建 `HomeActivity`，Fragment 换成新实例，普通字段回到初始值，重新读到原文时就把用户的改动盖掉了（review 发现）。ViewModel 跨这类重建保留，读到原文时不覆盖已有的草稿。不放进 `onSaveInstanceState`：输入框不限长，一大段文字进 Bundle 可能超出 Binder 事务上限；代价是进程被系统杀掉后草稿不保留。
- 提示在 `ClipboardHistoryPolicy.editMessage`：合并和「这条已经不在」都要说出来，不悄悄失败。`check-host.sh` 守住：编辑入口在操作行里、走 `openHostPage(EDIT_PAGE, args)`、先 `forgetCurrentClip`、传键不传字、不弹对话框，页面名和 `PageId` 一致。

## Alternatives considered

- **在键盘里直接编辑**（面板里出现编辑区，按键输入进这块编辑区） — 不离开当前应用，也是 issue 的原话；但输入法服务要自建一个文本缓冲区，把按键提交、组字、光标移动和删除都改写进去，改动面大，和「键盘里不放文本框」的约定冲突，也拿不到系统的选择手柄，裁网址反而更难。产品选了应用内编辑页。
- **编辑后当作最新一条排到最前** — 和「重新复制一次」的效果一样；但产品要求保留原位，用户在原来的位置找它。
- **把原文经 Intent 参数传给编辑页** — 不用再查一次存储；但深链参数会截掉超过 256 字的记录，文字还会经过 Intent，而键只是一个短字符串。
- **由应用在保存时写「已处理」身份** — 只在真正保存后才挡补读；但那份记录在 `:ime` 进程的 SharedPreferences 里，多进程下不可靠，还要把它挪成跨进程的存储，而在键盘打开编辑页时记下就够了。
- **改成已存在的文字时拒绝保存** — 实现更简单；但用户改出来的文字本来就在历史里，留两条一样的不可能（共享存储按文字识别条目），拒绝只会让用户困惑，合并并说明更顺。

## Consequences

- **收益**：多带了字的网址可以改好存回去，位置和固定状态都不变；另一个宿主同时删掉那条时会说出来；共享操作 iOS 和 HarmonyOS 以后可以直接用。
- **代价**：编辑要离开当前应用再回来，回来靠 `moveTaskToBack`，不同 ROM 的任务切换表现可能不同。打开编辑页时就把系统剪贴板当前那一条记为已处理，取消编辑时也一样；如果那一条之前因为隐私模式从没记进历史，之后打开面板也不会补读它。

## Verification

`cargo test -p msime-client-core clipboard` 覆盖原地替换（时间戳和固定状态不变、落盘）、两个方向的合并、旧条目不在、文字没变、新文字不合规、过时副本按最新历史回答和坏文件不被改写、历史文件不存在（从没记过或刚被 `clear` 删掉）时回答 `NotFound` 且不建文件、旧格式的字符串数组被升级成结构化格式；`cargo test -p msime-host-api mobile_clipboard` 覆盖 `replace` 动作的 JSON 往返、合并、两种拒绝原因和不合规请求被拒收；`platforms/android/tests/dictionary/ClipboardHistoryPolicySmoke.java` 覆盖提示文案和编辑键（不含文字、放得进深链参数）；`bash platforms/android/check-host.sh` 编译键盘代码并检查上面的守卫。`ClipboardEditPage` 只在 Gradle 构建里编译。在 API 35 的专用模拟器上走过：长按一条点「编辑」，应用里的编辑页预填原文、没改时「保存」不可点；删掉几个字保存后回到原来的编辑器，共享存储里那一条原地改字、时间戳不变；改的正是系统剪贴板里那一条时，再打开面板没有把原文补读回来；改成另一条已有的文字时两条合并成一条，留在被编辑那条的时间戳上。「这条已经不在」的提示、取消、各家 ROM 上回到原应用的表现和真机没有验证。
