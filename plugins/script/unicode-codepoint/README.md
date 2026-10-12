# unicode-codepoint：Unicode 码位注释

在候选旁边注上 Unicode 码位：单字「中」注「U+4E2D 基本区」，「𠀀」注「U+20000 扩展B」，「😀」注「U+1F600」，「❤️」注「U+2764 U+FE0F」。适合查生僻字在哪个扩展区、分辨长得一样的字（兼容汉字、私用区字符）、看 emoji 里藏着哪些不可见字符。

| 项 | 值 |
| --- | --- |
| 钩子 | `candidates` |
| 能力 | `read_candidates`（读当前候选窗口的文本）、`annotate_candidates`（给候选加注释） |
| 许可证 | MIT，包里不含第三方数据 |
| 模块 | 约 3.1 KiB，内存 2 页 |

插件只读候选的文字，不读编码、引擎注释和上屏内容；不改顺序，也不隐藏候选。注释显示在候选旁边单独的一格里，不替换引擎自己的注释（拼音、释义等）。

## 规则

- **单个字符**：注 `U+` 加大写十六进制，至少 4 位（`U+0061`、`U+4E2D`、`U+20000`、`U+10FFFF`）。落在下表的区段里时，后面再加一个空格和区段名；其他字符（字母、标点、假名、谚文、emoji 等）只注码位。
- **emoji 序列和异体字序列**：2 到 4 个码位、并且除第一个以外每个码位都是附着成分（变体选择符 U+FE00..U+FE0F、异体字选择符 U+E0100..U+E01EF、零宽连接符 U+200D、键帽符号 U+20E3、肤色修饰符 U+1F3FB..U+1F3FF、标签字符 U+E0020..U+E007F）或紧跟在零宽连接符之后，或者是两个地区指示符组成的旗帜。逐个注出码位，用空格隔开，不加区段名：「1️⃣」注「U+0031 U+FE0F U+20E3」，「🇨🇳」注「U+1F1E8 U+1F1F3」，「👩🏻‍💻」注「U+1F469 U+1F3FB U+200D U+1F4BB」，「葛󠄀」注「U+845B U+E0100」。
- **其他多字候选不注**：「中国」「ab」「a中」这类普通词组和中英混排、超过 4 个码位的序列（例如五个码位的「👨‍👩‍👧」、两面旗帜「🇨🇳🇯🇵」）、带组合附加符号的「é」（e 加 U+0301）都不注。普通词组逐字注码位没有用处，还会盖住其他插件对同一行的注释。
- **AI 占位行不注**：等待 AI 回答时的「…」和没有回答时的「—」（来源 12）跳过；其他来源的「…」「—」照常注码位。云候选、AI 候选、用户词、快捷短语都照常注释（注释不受宿主锁定座位的限制）。
- 窗口里只看前 20 行，这是宿主给插件的窗口大小；第 21 行以后的候选翻页看到时没有注释。
- 注释最长 32 个 UTF-16 码元。单个字符最长是「U+10FFFF 补充私用区B」（15 个）；按上面的序列规则，4 个码位的注释最长正好 32 个，所以不会因为太长被拒。

### 区段名

范围取自 Unicode 17.0 的 Blocks.txt，名字指整个区段，包括其中还没有分配的码位：

| 区段 | 范围 | 注释里的名字 |
| --- | --- | --- |
| CJK Radicals Supplement | U+2E80..U+2EFF | 部首补充 |
| Kangxi Radicals | U+2F00..U+2FDF | 康熙部首 |
| Ideographic Description Characters | U+2FF0..U+2FFF | 结构描述符 |
| CJK Strokes | U+31C0..U+31EF | 笔画 |
| CJK Unified Ideographs Extension A | U+3400..U+4DBF | 扩展A |
| CJK Unified Ideographs | U+4E00..U+9FFF | 基本区 |
| Private Use Area | U+E000..U+F8FF | 私用区 |
| CJK Compatibility Ideographs | U+F900..U+FAFF | 兼容汉字 |
| CJK Unified Ideographs Extension B | U+20000..U+2A6DF | 扩展B |
| CJK Unified Ideographs Extension C | U+2A700..U+2B73F | 扩展C |
| CJK Unified Ideographs Extension D | U+2B740..U+2B81F | 扩展D |
| CJK Unified Ideographs Extension E | U+2B820..U+2CEAF | 扩展E |
| CJK Unified Ideographs Extension F | U+2CEB0..U+2EBEF | 扩展F |
| CJK Unified Ideographs Extension I | U+2EBF0..U+2EE5F | 扩展I |
| CJK Compatibility Ideographs Supplement | U+2F800..U+2FA1F | 兼容汉字补充 |
| CJK Unified Ideographs Extension G | U+30000..U+3134F | 扩展G |
| CJK Unified Ideographs Extension H | U+31350..U+323AF | 扩展H |
| CJK Unified Ideographs Extension J | U+323B0..U+3347F | 扩展J |
| Supplementary Private Use Area-A | U+F0000..U+FFFFF | 补充私用区A |
| Supplementary Private Use Area-B | U+100000..U+10FFFF | 补充私用区B |

- 扩展区在表里按码位排序，不按字母：扩展 I 排在 F 和兼容汉字补充之间，G、H、J 在第三平面。
- 兼容汉字（例如 U+F900「豈」）和对应的统一汉字（U+8C48「豈」）长得一样，但经过 Unicode 规范化（NFC）会被换成统一汉字；看到「兼容汉字」说明这个候选在某些软件里可能变成另一个码位。
- 私用区的字符没有标准含义，显示成什么取决于字体。
- 「〇」（U+3007）、全角标点、假名不在表里，只注码位。

## 什么时候不注释

- 没有授权（`candidate_count` 返回 `DENIED`）：日志 `skip: denied`。
- 本地模式不允许读文本（@ 提及、网址、命令、表达式、Unicode 输入、日期时间、快捷短语这些模式）：宿主不给插件候选窗口，日志 `skip: absent`。临时英文、临时日文、超级简拼、emoji、颜文字模式照常注释。
- 窗口为空，或者没有一行符合上面的规则：日志 `none`。

## 和其他插件一起用

几个插件给同一行加注释时，按「脚本插件（实验）」里的执行顺序，后面的覆盖前面的。这个插件只注单个字符和 emoji 序列，所以和 traditional-glyph（繁体字形注释）只在单字候选上争同一格：两个都开时，排在后面的那个生效。

## fuel

每行最多读 16 字节（4 个码位最多 16 字节，更长的文本宿主直接返回 `LIMIT`，不复制），按字节解码不分支，注释直接写进日志缓冲再交给宿主。只有文本是「…」或「—」的行才多查一次来源。实测（rustc 1.97.1、wasmi 2.0，20 行的窗口）：

| 窗口 | fuel |
| --- | --- |
| 1 行「中」 | 约 1 100 |
| 20 行单字「中」 | 约 16 800 |
| 20 行单字 U+10FFFF（查区段表最远的情况） | 约 18 800 |
| 20 行都不注释的 4 码位文本（「aaaa」「中国人」「中a中a」，解码到底才放弃） | 约 12 500 |
| 20 行「👩🏻‍💻」（4 个码位的 emoji 序列） | 约 33 700 |
| 20 行 U+100000 U+200D U+100000 U+E01EF（最长的 32 码元注释，最坏情况） | 约 34 600 |

`candidates` 单次上限 125 000，最坏情况不到三分之一；同一个按键里刷新、展开等多次调用共用每轮 125 000，最坏的窗口连调三次也在一轮之内。

## 夹具

`fixture.jsonl` 覆盖：各汉字区段的首尾码位、私用区、兼容汉字；标点、字母、假名、谚文、单码位 emoji；各种 emoji 序列和异体字序列；普通词组、中英混排、超长候选、五个码位的序列、两面旗帜、组合附加符号、空文本；云、AI、AI 占位、用户词、快捷短语；正好 32 个码元的注释；空窗口；网址模式；临时日文模式；同一轮里的第二次调用；超过 20 行的窗口；以及上表的最坏情况。插件每个事件写一行日志：注释过的行写成 `ann 0=U+4E2D 基本区; 3=U+1F600`，否则是 `none` 或 `skip: <原因>`。`expected.jsonl` 是去掉 fuel 之后的期望输出，由 `scripts/test-script-plugins.sh unicode-codepoint` 核对。

## 数据和许可证

插件不带数据文件。区段范围是 Unicode 标准的事实，按 Unicode 17.0 的 Blocks.txt 写在源码里（`src/lib.rs` 的 `BLOCK_TABLE`，编译时检查顺序和分平面的切分点），中文名是插件自己的译名，没有复制任何 Unicode 数据文件。代码以 MIT 许可证发布，见 `pack/LICENSE.txt`。
