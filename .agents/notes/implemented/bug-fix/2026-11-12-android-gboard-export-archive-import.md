# Agent Note: Android 导入 Gboard 导出的压缩包

Status: implemented

## Problem

用户反馈：Android 最新版导入手机上谷歌输入法（Gboard）的词库，提示「文件里有无法识别的字符，请确认是文本格式」。

这句话对应 client-services 的 `ImportError::ControlCharacters`（`crates/client-services/src/dictionary/import.rs`）：文本里出现 `\n`、`\r`、`\t` 之外的控制字符就整份拒绝。Gboard 的 `# Gboard Dictionary version:1` 文本本身已经能导入（#8822），但 Gboard 在手机上「导出」给的不是文本，而是 `PersonalDictionary.zip`，词条在里面的 `dictionary.txt`。`DictionaryCollectionsStore.importFile` 把选中的文件按 UTF-8 文本原样交给 client-services，压缩包的文件头 `PK\3\4` 里就有两个控制字符，于是得到上面那句提示。

没有拿到报告者的文件，「选的是压缩包」是按代码和 Gboard 的导出格式推出来的原因，不是复现出来的。导入来源对话框的「文本文件」按 `text/plain` 过滤，系统文件选择器里压缩包是灰的；能选到它，是因为有的文件管理器不按类型过滤。按类型过滤的选择器上，用户看到的是导出的文件根本选不了。

## Decision

Android 的文本导入收到 zip 压缩包（以 `PK\3\4` 开头）时，取出里面的词库文本再交给 client-services：

- 有 `dictionary.txt`（不分大小写，不论在哪个目录）就用它。
- 没有时，包里只有一个 `.txt` 才用那一个；有多个说不准该用哪个，不猜。
- 取不到时提示「压缩包里没有找到词库文本，请解压后选择里面的 .txt 文件」，不再说「无法识别的字符」。
- 最多看 64 个条目，解压后超过 `MAX_IMPORT_BYTES`（16 MiB）的条目不读完，按取不到处理。

导入来源的「文本文件」一项改名为「文本文件（.txt，或 Gboard 导出的 .zip）」，文件类型加上 `application/zip`，按类型过滤的选择器里也能选到压缩包。

解压放在 `DictionaryCollectionsStore`（`isZipArchive`、`textFromArchive`），用 `java.util.zip`，不加依赖。搜狗细胞词库走二进制通道，不经过这里。

## Alternatives considered

- 放进共享 Rust（client-services 的导入），所有平台一起支持：桌面用户也可能把手机上导出的压缩包拿到电脑上导入。这次不做，是因为要给 client-services 加一个 zip 解析依赖，而导出这个压缩包的只有手机上的 Gboard，用户在手机上导出、在手机上导入是主要路径。桌面上再有人反馈时把这段规则搬进共享层。
- 只改提示文案，告诉用户先解压：不加解压代码。不选它，是因为 Android 上解压一个文件要另找文件管理器，而包里该用哪个文件是确定的。
- 包里有多个 `.txt` 时取第一个或最大的：能多导入几种包。不选它，是因为取错文件时用户得到的是一份莫名其妙的词库，比一句「请解压后选择」更难处理。

## Consequences

收益：Gboard 导出的压缩包可以直接选、直接导入；别的工具把单个词库文本打成 zip 的也能用。

代价：导入来源对话框第一项的名字变长；「文本文件」一项现在也让选 zip，选了不含词库文本的压缩包会得到一句专门的提示。

没有验证：报告者的文件没有拿到，真实原因如果是文本文件里夹着控制字符（而不是选了压缩包），这次改动帮不上，需要报告者给出文件类型或样例。API 28 专用模拟器已从系统 Downloads 选择合成 `PersonalDictionary.zip`，确认可选择压缩包并解析为 1 条。既有模拟器运行目录提示部分词条未生效，因此不能将解析成功等同于原生词库应用成功；另以独立合成运行目录的设备行为测试核对归档词条确实写入原生词库，不接触用户个人词库。

## Testing

`platforms/android/tests/dictionary/DictionaryCollectionsStoreSmoke.java`：用 `ZipOutputStream` 现做压缩包，覆盖取 `dictionary.txt`（含子目录、大小写、排在后面）、单个 `.txt`、多个 `.txt`、没有 `.txt`、解压后超限、截断的包、空文本，以及「文本文件」一项带 `application/zip` 而纯汉字词表不带。

设备回归 `DictionaryImportDeviceSmoke` 还覆盖已安装产品的 `importFile` → ZIP 解包 → 共享导入 → 原生词库应用，查询合成目录的 `tbl_1_a` 验证 Gboard 词条落盘。原有 74 词、繁忙重试、排序和大集合覆盖均保留；归档场景放在原有排序断言之后、大集合导入之前，使用独立的单音节编码，原有数量断言不变。
