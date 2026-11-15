import app.msime.android.DictionaryCollectionsStore;
import app.msime.android.JsonPolicy;
import app.msime.android.NumberPolicy;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.List;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

public final class DictionaryCollectionsStoreSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) throws Exception {
        check(DictionaryCollectionsStore.validName("工作"));
        check(DictionaryCollectionsStore.validName("字".repeat(32)));
        check(!DictionaryCollectionsStore.validName("字".repeat(33)));
        check(!DictionaryCollectionsStore.validName(""));
        check(!DictionaryCollectionsStore.validName(" 工作"));
        check(!DictionaryCollectionsStore.validName("工\n作"));
        check(DictionaryCollectionsStore.validName("😀".repeat(32)));

        check(DictionaryCollectionsStore.validPinyin("shui'shan"));
        check(DictionaryCollectionsStore.validPinyin("pinyin"));
        check(!DictionaryCollectionsStore.validPinyin("'pin"));
        check(!DictionaryCollectionsStore.validPinyin("pin'"));
        check(!DictionaryCollectionsStore.validPinyin("pin''yin"));
        check(!DictionaryCollectionsStore.validPinyin("Pin"));
        check(!DictionaryCollectionsStore.validPinyin(""));
        check(DictionaryCollectionsStore.normalizePinyin(" Shui Shan ").equals("shui'shan"));
        check(DictionaryCollectionsStore.normalizePinyin("hou’xuan").equals("hou'xuan"));
        check(DictionaryCollectionsStore.displayCode("hou'xuan'xiang").equals("hou’xuan’xiang"));
        check(DictionaryCollectionsStore.normalizeCantonese(" Nei5  Hou2 ").equals("nei5 hou2"));
        check(DictionaryCollectionsStore.normalizeCantonese("gwong’dung'waa").equals("gwong dung waa"));
        check(DictionaryCollectionsStore.validCantonese("nei hou"));
        check(DictionaryCollectionsStore.validCantonese("nei5 hou2"));
        check(!DictionaryCollectionsStore.validCantonese("nei7 hou"));
        check(!DictionaryCollectionsStore.validCantonese("nei  hou"));
        check(!DictionaryCollectionsStore.validCantonese("nei-hou"));
        check(!DictionaryCollectionsStore.validCantonese(""));
        check(DictionaryCollectionsStore.pendingDescription(0).isEmpty());
        check(DictionaryCollectionsStore.pendingDescription(1).equals("，还有 1 条等键盘同步"));
        check(DictionaryCollectionsStore.pendingDescription(128406).equals("，还有 128,406 条等键盘同步"));

        check(NumberPolicy.groupedCount(128406).equals("128,406 条"));
        check(NumberPolicy.groupedCount(-3).equals("0 条"));

        check(DictionaryCollectionsStore.nameFromFile("网络流行语.txt").equals("网络流行语"));
        check(DictionaryCollectionsStore.nameFromFile("luna_pinyin.dict.yaml").equals("luna_pinyin"));
        check(DictionaryCollectionsStore.nameFromFile(".txt").equals("导入的词库"));
        check(DictionaryCollectionsStore.nameFromFile("字".repeat(40) + ".txt").equals("字".repeat(32)));
        check(DictionaryCollectionsStore.formatForFile("a.scel", "txt").equals("scel"));
        check(DictionaryCollectionsStore.formatForFile("a.dict.yaml", "txt").equals("rime"));
        check(DictionaryCollectionsStore.formatForFile("a.txt", "hans").equals("hans"));
        check(DictionaryCollectionsStore.binaryFormat("scel") && !DictionaryCollectionsStore.binaryFormat("txt"));
        check(Boolean.TRUE.equals(JsonPolicy.strictBoolean(Boolean.TRUE)));
        check(JsonPolicy.strictBoolean("true") == null);
        check(JsonPolicy.strictString("synthetic") != null);
        check(JsonPolicy.strictString(7) == null);
        check("synthetic export".equals(JsonPolicy.strictString("synthetic export")));
        check(JsonPolicy.strictString(7) == null);
        check(JsonPolicy.strictInteger(Integer.valueOf(7)) == 7);
        check(JsonPolicy.strictInteger("7") == null);
        check(JsonPolicy.strictLong(Long.valueOf(7)) == 7L);
        check(JsonPolicy.strictLong(7.0) == null);
        check(DictionaryCollectionsStore.nonNegativeInteger(Integer.valueOf(7)) == 7);
        check(DictionaryCollectionsStore.nonNegativeInteger(Integer.valueOf(-1)) == null);
        check(DictionaryCollectionsStore.nonNegativeInteger(Double.valueOf(7.5)) == null);

        List<DictionaryCollectionsStore.ImportSource> sources = DictionaryCollectionsStore.importSources(
            List.of("txt", "standard", "windows", "hans", "rime"));
        check(sources.size() == 3);
        check(sources.get(0).format().equals("txt") && sources.get(1).format().equals("rime")
            && sources.get(2).format().equals("hans"));
        check(DictionaryCollectionsStore.importSources(List.of("txt", "scel")).get(1).label().contains(".scel"));
        check(DictionaryCollectionsStore.importSources(List.of()).isEmpty());

        DictionaryCollectionsStore.View view = new DictionaryCollectionsStore.View(List.of(
            new DictionaryCollectionsStore.Collection("c1", "网络流行语", "pinyin", "community", "r1", true, 10, 0),
            new DictionaryCollectionsStore.Collection("c2", "工作", "pinyin", "user", "", false, 0, 0)),
            List.of("txt"), 0, null);
        check(view.installed("r1") && !view.installed("r2"));
        check(view.find("c2").name().equals("工作") && view.find("c3") == null);

        check(DictionaryCollectionsStore.failureMessage("builtin_locked").contains("内置"));
        check(DictionaryCollectionsStore.failureMessage("import_malformed").equals("文件不完整或已损坏，请重新下载。"));
        // 细胞词库按 base64 传，上限比文本小，超过时在读进请求之前就拒绝，不碰 Context。
        check(DictionaryCollectionsStore.MAX_SCEL_IMPORT_BYTES == 8 * 1024 * 1024);
        check(DictionaryCollectionsStore.importFile(null, "scel", "搜狗",
            new byte[DictionaryCollectionsStore.MAX_SCEL_IMPORT_BYTES + 1]).failure()
            .equals(DictionaryCollectionsStore.failureMessage("collections_too_large")));
        check(DictionaryCollectionsStore.failureMessage(null).equals(DictionaryCollectionsStore.failureMessage("x")));
        check(DictionaryCollectionsStore.BUILTIN_PINYIN.equals("builtin:pinyin"));
        check(DictionaryCollectionsStore.exportBytesAfterPage(
            DictionaryCollectionsStore.MAX_EXPORT_BYTES - 1, "a")
            == DictionaryCollectionsStore.MAX_EXPORT_BYTES);
        check(DictionaryCollectionsStore.exportBytesAfterPage(
            DictionaryCollectionsStore.MAX_EXPORT_BYTES - 1, "ab") < 0);
        check(DictionaryCollectionsStore.exportBytesAfterPage(
            DictionaryCollectionsStore.MAX_EXPORT_BYTES - 3, "中")
            == DictionaryCollectionsStore.MAX_EXPORT_BYTES);

        // Gboard 导出的是 PersonalDictionary.zip，词条在里面的 dictionary.txt：压缩包要解出文本再导入，不能当文本读成一串控制字符。
        byte[] gboard = "# Gboard Dictionary version:1\n\nshuishan\t水杉\tzh-CN\n".getBytes(StandardCharsets.UTF_8);
        check(!DictionaryCollectionsStore.isZipArchive(gboard));
        check(!DictionaryCollectionsStore.isZipArchive(new byte[] {'P', 'K'}));
        check(!DictionaryCollectionsStore.isZipArchive(null));
        byte[] archive = zip(new String[] {"dictionary.txt"}, new byte[][] {gboard});
        check(DictionaryCollectionsStore.isZipArchive(archive));
        check(Arrays.equals(DictionaryCollectionsStore.textFromArchive(archive), gboard));
        // 没有条目的合法 ZIP 只有中央目录结束签名；仍应走压缩包错误提示，而不是按文本解析控制字符。
        byte[] emptyArchive = zip(new String[0], new byte[0][]);
        check(DictionaryCollectionsStore.isZipArchive(emptyArchive));
        check(DictionaryCollectionsStore.textFromArchive(emptyArchive) == null);
        // 有 dictionary.txt 就用它，不论排在第几个、在不在子目录里；别的条目不看。
        check(Arrays.equals(DictionaryCollectionsStore.textFromArchive(zip(
            new String[] {"readme.txt", "Export/Dictionary.TXT", "icon.png"},
            new byte[][] {"x".getBytes(StandardCharsets.UTF_8), gboard, new byte[] {0, 1}})), gboard));
        // 没有 dictionary.txt 时，只有一个 .txt 才用它；多个说不准，没有也取不到。
        check(Arrays.equals(DictionaryCollectionsStore.textFromArchive(zip(
            new String[] {"words.txt", "icon.png"}, new byte[][] {gboard, new byte[] {0}})), gboard));
        check(DictionaryCollectionsStore.textFromArchive(zip(
            new String[] {"a.txt", "b.txt"}, new byte[][] {gboard, gboard})) == null);
        check(DictionaryCollectionsStore.textFromArchive(zip(
            new String[] {"icon.png"}, new byte[][] {new byte[] {0}})) == null);
        // 解压后超过导入上限的条目不读完；截断的包按取不到处理。
        check(DictionaryCollectionsStore.textFromArchive(zip(new String[] {"dictionary.txt"},
            new byte[][] {new byte[DictionaryCollectionsStore.MAX_IMPORT_BYTES + 1]})) == null);
        check(DictionaryCollectionsStore.textFromArchive(Arrays.copyOf(archive, archive.length / 2)) == null);
        // 空的 dictionary.txt 解得出来，由导入报「文件是空的」。
        check(DictionaryCollectionsStore.textFromArchive(zip(new String[] {"dictionary.txt"}, new byte[][] {new byte[0]})).length == 0);
        check(DictionaryCollectionsStore.failureMessage("import_archive_without_text").contains(".txt"));
        boolean offersZip = false;
        for (DictionaryCollectionsStore.ImportSource source : DictionaryCollectionsStore.importSources(List.of("txt", "hans"))) {
            if (source.format().equals("txt")) offersZip = Arrays.asList(source.mimeTypes()).contains("application/zip");
            else check(!Arrays.asList(source.mimeTypes()).contains("application/zip"));
        }
        check(offersZip);
    }

    static byte[] zip(String[] names, byte[][] contents) throws IOException {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try (ZipOutputStream output = new ZipOutputStream(bytes)) {
            for (int index = 0; index < names.length; index++) {
                output.putNextEntry(new ZipEntry(names[index]));
                output.write(contents[index]);
                output.closeEntry();
            }
        }
        return bytes.toByteArray();
    }
}
