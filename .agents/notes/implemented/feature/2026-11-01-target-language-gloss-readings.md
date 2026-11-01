# Agent Note: 法日西俄德韩释义行的读音

Status: implemented

## Problem

「显示读音」打开后，六个平台都能给英文释义行标音标（`pronunciations/en-phonetic.db`，ECDICT），macOS 和 Windows 还能给日文行读罗马字（系统分词器、微软日语输入法）。可翻译目标语言有七种，离线释义（`resources/offline-glosses.lock.json`，维基词典）已经给法、日、西、俄、德、韩六种出词，这六种的释义行却没有读音：Linux、Android、iOS、鸿蒙连日文也没有；共享层的 `msime_client_pronunciation_request` 遇到 `language` 不是 `en` 的项整批拒绝。Android 和 iOS 还有一处潜伏的错：它们只看「目标语言里有没有英语」，就把释义的第一行当英文去查，法语目标的 `chat` 会被读成英语的 /tʃæt/。

## Decision

**数据来源与离线释义相同**：英文维基词典，经 Wiktextract 抽取、由 kaikki.org 发布，CC BY-SA 4.0，`docs/third-party.md` 已登记。离线释义取的是英语词条里的译文表，译文行不带读音；读音取的是同一版维基词典里目标语言词条自己的 `sounds`，kaikki.org 按语言单独发布（`kaikki.org-dictionary-French.jsonl.gz` 等六个，格式与离线释义读的英语 dump 相同）。离线释义锁住的 2026-09-02 那一周的 dump 已被 kaikki.org 覆盖、取不回来，过滤后的输入也只留了英语词条的译文行，所以读音用 2026-10-25 那一周的六个 dump（Wiktextract `f9fe161`，维基词典 2026-10-01 的转储），在 Mac Studio 上下载和过滤，过滤后的 13 MB 输入随产物发布在 `metasequoiaime/chinese-ime-lm` 的 `target-pronunciations-2026.10.25`，由新的 `resources/target-pronunciations.lock.json` 固定（六个原始 dump 的地址、ETag、长度和 SHA-256，过滤后的输入，产物）。读音按目标语言的词查，与离线释义从哪一周来无关；下次刷新离线释义时两者从同一周的 dump 一起重建。

**每种语言读什么**（`scripts/build_target_pronunciations.py`）：

- 法、西、俄、德、韩用 `sounds[].ipa`，原样保留维基词典写的斜线或方括号。斜线的音位标音优先于方括号的语音标音（西语两种都有，俄语和韩语只有方括号），不带地区标签的优先，其次是各语言的标准音（法国、西班牙、德国、首尔），古旧、方言、非标准的丢掉；一格里写了两种读法或带注释的整格不用。同一个词有几个词条时取最常见词性（名词、动词、形容词……）的第一个。屈折形式（俄语 `кошки`、法语 `chattes`）有自己的 IPA，但释义写的是原形，丢掉。
- 日语用赫本式罗马字（`forms[]` 里标 `romanization` 的那一个），不用 `sounds` 里的 IPA。`sounds` 有一个很窄的 IPA（`[ne̞ko̞]`），日语学习者从来不学它；还有假名读法（`sounds[].other`）。用罗马字是因为 macOS 和 Windows 早就在日文释义行下面画系统生成的罗马字，其他平台给同一种读音，用户在哪台设备上看到的都是一类东西；而且日文释义大多本来就是假名（`リンゴ`、`じゃがいも`），假名读音对它们什么也没加，罗马字照样有用。纯假名的词条多半只是指向汉字词条的异体，没有自己的读音，所以每个汉字词条的假名读法也作为一个键（`林檎` 是 `りんご`），片假名的释义折成平假名去对。
- 只收离线释义用到的词：六份 `zh-<lang>.db` 里每个释义的每个词条，连同它的小写。宿主读的是释义行的第一个词条，绝大多数行就出自这六份文件；收全部原形词要约 13 MB，收用到的不到 4 MB（六个文件合计 3,848,081 字节，每种 1.4 万到 2.1 万词）。在线翻译出来的行，第一个词条在这些词里时照样有读音。
- 覆盖（按离线释义每个中文词的第一个词条算，整条精确匹配）：法 76%、西 74%、俄 68%、韩 68%、日 70%、德 59%。查不到的大多是多词短语（`parterre de fleurs`、`in Stein gemeißelt`）和专名。六种都够用，没有删掉哪一种；德语低一些是因为德语释义里复合词和短语多，抽查查得到的读音本身没有问题。

**格式与 `en-phonetic.db` 相同**：每种语言一个 `<lang>-phonetic.db`，表 `<lang>_phonetics(word, phonetic)`，键是维基词典的写法（`Katze`、`кот`、`猫`），`meta.kind = <lang>_phonetic`，`user_version = 1`，外加必须随它们分发的 `target-pronunciations-NOTICE.txt`。目录叫 `target-pronunciations/`，随包时在资源目录旁，下载时是资源包 `target-pronunciations` 的安装目录，与 `pronunciations/` 的做法一样。

**共享层**：`msime_engine::host::phonetics(database, language, words)` 按语言拼表名和 `kind`，语言只接受 `PHONETIC_LANGUAGES` 里的七个；`english_phonetics` 是它的 `en`。`msime_client_pronunciation_request` 的 `language` 现在可以是七种之一，每种语言各查各的表，回答按请求顺序；非英文行只读第一个分隔符前的词条，去掉括号注释，整条查（先原样、再小写），查不到就不标。不像英文那样逐词拼：法语的联诵、俄语随词形移动的重音，拼出来是错的读音。某种语言没装表时它的项没有读音，不是错误。

**宿主**：每行释义按它的目标语言读，读音按「语言 + 文字」存：同样的字母在别的语言里是另一个词。

- macOS 和 Windows：法、西、俄、德、韩行交给共享层，日文行照旧读系统罗马字。表随安装包带着（`stage-resources.sh` 第九个参数、`package-release.sh`、`Prepare-PackageFiles.ps1`），与英文表一样不分版本。
- Linux：日文行也交给共享层，因为没有系统分词器。表不随安装包，设置页在「显示读音」下面多一行「离线外语读音词典」，只在目标语言有这六种之一时出现，点了才下载。
- Android、iOS：只有一行读音，读主目标语言那一行（iOS 有两种目标语言时英语优先，与原来相同）；修掉了上面那个把法语当英语读的错。设置页同样多一行「离线外语读音词典」，主目标语言不是英语时出现。
- 鸿蒙：表随 HAP 带着（`stage-resources.sh` 第六个参数，`MSIME_REQUIRE_PRONUNCIATIONS=1` 时也必须在），键盘扩展暂存到引擎资源旁；按主目标语言读，日文行也交给共享层。
- 设置页的开关在所有平台都叫「显示读音」（Linux、鸿蒙、Android、iOS 原来叫「显示英文音标」），说明写明英文给音标、日文给罗马音、其他给国际音标。出厂仍是关闭。读屏对 IPA 念「音标」，对罗马字念「读音」。

## Alternatives considered

**用离线释义锁住的同一份 dump。** 这样读音和释义出自同一周，复现时少一份输入，也是计划里写的要求。做不到：kaikki.org 每周覆盖同一个地址，2026-09-02 的 dump 已经取不回来；离线释义保存的过滤后输入只有英语词条的译文行，没有任何目标语言词条，也就没有 `sounds`。从译文行里能拿到的只有俄、日、韩译文附带的罗马化（`roman`），不是 IPA，法、西、德一个字也没有。

**用 kaikki 的 raw dump（`raw-wiktextract-data.jsonl.gz`）**，离线释义的生成器也接受它，上游还把按语言的 postprocessed dump 标成了 deprecated。它约 3.1 GB，从 Studio 下载只有每秒一百多 KB，要七个小时；六个按语言的 dump 合计 420 MB，并行下载八分钟，内容是同一次抽取的子集，格式和离线释义读的英语 dump 一样。将来按语言的 dump 停发时，生成器的 `filter` 读 raw dump 也能工作（它只看 `lang_code`）。

**日语用 JMdict**：读音更全，有标准的假名和频度标注。JMdict 是新的第三方来源（EDRDG 许可，要求随附说明并定期更新），这次只允许用已登记的来源，而且固定版本的锁和「定期更新」的要求有冲突。

**日语给假名而不是罗马字**：中文用户学日语，汉字词标假名（ふりがな）是最常见的读法。但大多数日文释义本身就是假名，标了等于重复；macOS 和 Windows 已经给罗马字，换成假名会让同一个开关在不同平台上读出两种东西，要么连带改掉那两端已经发布的行为。

**把六张表并进现有的 `pronunciations` 资源包**，设置页不用多一行。资源包是否已装按锁里的文件清单严格比对，清单一变，所有已经下载过英文音标表的 Android、Linux 用户都会变成「需更新」，在他们回设置页重新下载之前连英文音标也没了；只用英文的用户也平白多下 3.8 MB。

**收全部原形词（约 13 MB）**，在线翻译出来的冷僻词也有读音。离线释义用到的词已经覆盖了绝大多数会画出来的行；鸿蒙、macOS、Windows 要随包带，三倍多的体积换来的只是在线翻译里少数冷僻词。

**多词短语逐词拼读**，像英文那样，覆盖还能再高十几个点。法语词与词之间的联诵和省音、俄语的重音位置都随上下文变，逐词拼出来的读音会教错；查不到就不标，比标一个错的好。

## Consequences

- **收益**：六种翻译目标语言的释义行都有读音，六个平台一致；Linux、Android、iOS、鸿蒙的日文行也有了罗马字。Android 和 iOS 不再把非英语释义按英语读。
- **代价**：macOS、Windows 安装包和鸿蒙 HAP 各多约 3.8 MB；Linux、Android、iOS 想看外语读音要再下载一次约 3.8 MB。
- 读音和离线释义来自不同的两周 dump（2026-10-25 与 2026-09-02）。读音按目标语言的词查，不受影响；离线释义刷新时一起重建，`--glosses` 指向新的 `zh-<lang>.db`。
- 新的网络行为只有资源包下载，与其他资源包同一条路：用户镜像、`https://dl.msime.app/gh/`、GitHub 原地址，PRIVACY.md 已写明；查表不联网。
- 产物在 `metasequoiaime/chinese-ime-lm` 的 `target-pronunciations-2026.10.25`；构建可复现：用锁里的过滤后输入和离线释义的六个数据库运行 `build_target_pronunciations.py build`（`--dump-date`、`--source-revision` 取锁里的值），同一个 SQLite 版本下逐字节相同。
- 测试：`scripts/test-target-pronunciations.py`（取读音的规则、过滤、确定性），`scripts/test-language-data-notices.py`（NOTICE 在锁里、在每个随包渠道里），共享层 `phonetics` 与 `msime_client_pronunciation_request` 的单测，各宿主的读音规则单测。
- 需要真机核对：六个平台在法语、日语、俄语目标下的读音位置和读屏；Linux、Android、iOS 的下载行。
