# Agent Note: 日语模型留在进程级缓存里，切到日文时后台预热

Status: implemented

## Problem

#6716 顺带报了「第一个字慢半拍」。日文这一条从代码上能确认：`JapaneseProvider` 在第一次查询时才经 `JapaneseDictionary::shared` 取模型，而 `shared` 只留 `Weak`。Android 每进一个输入框都 `stop(false)` → `NativeClient.destroy` 重建会话，最后一个强引用随旧会话消失，于是每个新输入框的第一个假名都要在主线程上把 66 MB 的 `msime-japanese.dat` 重新映射、逐条校验并重建前缀索引。别的大表（`NgramTable::shared`、`shared_sentence_model`）早就是进程级强缓存，日文是例外。

`Weak` 来自最初的移植（e2945caa9c），没有笔记说明理由；从 db018dc31c 的说明看，在意的是 iOS 键盘扩展的内存上限——那次把模型从堆改成只读映射，让 66 MB 成为系统可回收的干净页。要保留的就是这个意图：长期持有的部分要能在内存紧张时交出去。

## Decision

- `JapaneseDictionary::shared` 把读好的模型强引用留在进程级缓存里（最多 2 个路径，满了挤掉最早的），会话都结束了也不放手。`PRELOADED`（网页引擎交来的字节）仍然优先，语义不变。
- 每次取用都对一遍文件身份：长度、修改时间，unix 加设备号和 inode，Windows 加创建时间；身份取自打开文件的句柄，就是被映射的那个 inode。Android 的引导资源（`files/bootstrap/resources/msime-japanese.dat`）和资源包（`<state_root>/resource-packs/japanese/msime-japanese.dat`）都是固定路径、更新时 rename 替换，所以同一路径换了文件就读新的；文件不在或换了的条目在下一次取用（`shared`、预热）时丢掉，不再钉住已删文件的映射和磁盘空间。没有取用就不检查：Android 的资源包由主进程的 `ResourcePackService` 安装，模型却映射在 :ime 进程里，在安装路径上调 `release_shared` 碰不到那份缓存，所以不这样做。
- 读失败不缓存，文件下次出现就能读到。读文件时不持缓存锁，另有一把 `LOADING` 锁让后台预热和第一个查询撞上时只读一遍；读到一半被 `release_shared` 清过的结果按代次判断，不写回。
- `Session::reset_cache`（宿主的清缓存动作：各宿主的维护快捷键，以及 iOS 键盘扩展的 `didReceiveMemoryWarning`）调用 `JapaneseDictionary::release_shared` 放掉缓存的全部强引用；正在用它的会话照常用到结束。放掉和因满了被挤出去的模型降为 `Weak` 留在缓存里，还有会话在用时，再要同一路径拿回的就是这一份，并重新被强引用着；没人用了才在下一次取用时丢掉。否则清缓存之后、旧会话还拿着模型时来的新会话会另读一份，内存告警时反而多出一份。读到一半被清过的结果也按这个办法只留 `Weak`。引擎内部因为学习、删词而调的 `InputSession::reset_cache` 不碰它，否则每次提交都会把缓存清掉。
- 预热只在 `ProviderRegistry::activate(SchemeType::JapaneseRomaji)` 时做（建会话的初始方案、切方案、临时日文都经过这里），在名为 `msime-japanese-model` 的后台线程读；已在缓存、已预载或正在预热时什么也不做。起不了线程（网页引擎）时第一个查询照旧自己读。`JapaneseProvider::new` 仍然不读模型。另一个入口是日文混输（实验选项，默认关）：开着时建会话就经 `ProviderRegistry::warm_up_japanese` 预热同一份模型，见 [全拼、双拼里混进一行日文整段转换](../feature/2026-10-12-mixed-japanese-candidates.md)。

## Alternatives considered

- **照搬 `NgramTable::shared` 的按路径 LRU（`Option<Arc<_>>`）** — 现成模式，代码最少；但它按路径缓存且缓存 `None`：Android 的固定路径被 rename 换成新模型后，进程会一直用旧的、钉住已删文件，读失败一次就永远失败到进程结束。
- **在 `JapaneseProvider::new` 里起预热** — 最早、最简单；但完整版的每个会话都按 `SchemeSet::ALL` 构造这个 provider，不管用户加没加日文键盘，`backend/text_tools.rs` 还每次请求构造一个。这会让只打中文的用户和内存吃紧的 iOS 键盘扩展每次启动都读 66 MB 并建索引，违背 provider 文档里「不打日文的会话不读这个文件」。
- **在 `activate` 里同步读** — 不用线程；但 `activate` 就在 Android 主线程的 `NativeClient.create` 和切方案里，等于把卡顿从第一个按键挪到键盘弹出的那一刻。
- **`release_shared` 直接清空、不留 `Weak`** — 最简单；但清缓存时持有模型的会话照常活着，下一个会话找不到就另读一份，同一路径在进程里有两份模型和两份前缀索引，iOS 内存告警时正好走这条路。
- **在资源包安装、删除路径上调 `release_shared`** — 能让已删文件的磁盘块尽早还回去；但 Android 的安装在主进程，缓存在 :ime 进程，调不到；其他宿主上也只是省下「下一次取用」之前那段时间。
- **只在缓存里留 `Weak`、让宿主保持会话不重建** — 不改引擎；但 Android 按输入框重建会话是有意的（每个输入框各自的选项和隔离），为一个缓存去改会话生命周期代价远大于收益，其他宿主也会遇到同样的问题。

## Consequences

- **收益**：Android 上切到日文后模型在后台读好，之后每个新输入框的第一个假名不再重新校验 66 MB；所有宿主的 `text_tools` 日文请求也复用同一份模型。Rust 测试覆盖：会话放手后仍是同一份、rename 换文件后读新的、删除后放手、读失败不缓存、清缓存放掉、清缓存或挤出去时仍被持有的交回同一份、满了挤掉最早的、切到日文才预热、预热和第一个查询撞上只读一遍、读到一半被清的结果不写回强引用（`decoder.rs`、`registry.rs`、`session/tests.rs`；测试用 `#[cfg(test)]` 的读文件计数和读前暂停点）。
- **代价**：进程不再在最后一个日文会话结束时自动放掉模型（前缀索引的堆内存和映射）；要靠宿主的清缓存动作。iOS 在内存告警时调它，Android 在 `onTrimMemory` 达到运行中低内存或进入后台 LRU、以及旧版 `onLowMemory` 时调无 handle 的进程缓存释放入口。资源包被删、又再也没有会话取日文模型时，旧文件的磁盘块要到下一次 `shared` 或预热、清缓存且没有会话持有、或进程退出才还回去。每次 `shared` 多几次 `stat`（最多 2 个路径）。
- **Windows 风险**：Windows 上被映射的文件不能删除或被 rename 覆盖。Server 进程现在会在所有会话结束后继续映射模型，资源包更新或卸载如果在另一个进程里替换这个文件，可能失败；原先只有持有模型的会话活着时才会这样。没有在 Windows 上验证。
- **未验证**：没有在设备上测量第一个假名的耗时；issue 里「所有语言第一个字都慢」的说法在代码里找不到对应原因，这次不处理。
