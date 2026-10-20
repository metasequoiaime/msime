package app.msime.android;

import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.Charset;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;

/**
 * 背单词页上要推理的文字：共享层的错误怎样说给用户、词书在选择器里怎样写、导入的文件起什么名字、文件按哪种编码读。
 *
 * <p>和 {@link VocabularyReviewModel} 一样不依赖 Android，`check-host.sh` 的 `VocabularyReviewTextSmoke` 在主机 JVM 上跑到每一条。文字与 iOS 的 `VocabularyReviewStore.Failure` 和设置页一致。
 */
public final class VocabularyReviewText {
    private VocabularyReviewText() {}

    /** 一次导入最多读多少字节，与共享层的 `session::MAX_IMPORT_BYTES`、JNI 的 `kVocabularyReviewRequestLimit` 和 iOS 的 `maximumImportBytes` 相同。请求还要带上目录和日期，所以文本本身比这再小一点也会被入口拒掉，那时报的是同一句「不是可用的词表」。 */
    public static final int MAX_IMPORT_BYTES = 8 * 1024 * 1024;

    /** 导入的词表名最多几个字符，与 iOS 截取文件名的长度相同。 */
    public static final int MAX_IMPORT_NAME_CHARS = 64;

    public static final String UNAVAILABLE = "背单词进度无法读取或保存；已有的进度不会被自动清空。";
    public static final String UNREADABLE = "这个文件不是可用的词表，请检查每行是否为「单词,释义」。";
    public static final String LIBRARY_FULL = "词表数量已达上限，请先删除一个再导入。";
    public static final String MISSING = "这本词表已不在词库中，请重新选择。";
    public static final String BUILTIN = "内置词书不能删除。";

    /**
     * 共享层的错误文本换成用户能照着做的一句话。
     *
     * <p>文件不是词表时单独说：把它报成存储失败，读起来像应用坏了，而实际是文件不对。
     */
    public static String failureMessage(String error) {
        String text = error == null ? "" : error;
        if (text.contains("import")) return UNREADABLE;
        if (text.contains("library is full")) return LIBRARY_FULL;
        if (text.contains("not in the library")) return MISSING;
        if (text.contains("cannot be deleted")) return BUILTIN;
        return UNAVAILABLE;
    }

    /** 选择器和词书行里的写法：「CET-4（3849 词）」。 */
    public static String wordbookLabel(VocabularyReviewModel.Wordbook book) {
        return book.name() + "（" + book.total() + " 词）";
    }

    /** 导入的词表名：文件名去掉扩展名和首尾空白，最多 {@link #MAX_IMPORT_NAME_CHARS} 个字符；什么也不剩时用「导入的词表」。 */
    public static String importName(String displayName) {
        String name = displayName == null ? "" : displayName;
        int slash = name.lastIndexOf('/');
        if (slash >= 0) name = name.substring(slash + 1);
        name = name.strip();
        int dot = name.lastIndexOf('.');
        if (dot > 0) name = name.substring(0, dot).strip();
        if (name.codePointCount(0, name.length()) > MAX_IMPORT_NAME_CHARS)
            name = name.substring(0, name.offsetByCodePoints(0, MAX_IMPORT_NAME_CHARS));
        return name.isEmpty() ? "导入的词表" : name;
    }

    /**
     * 选中的词表文件按哪种编码读成文本。
     *
     * <p>先按 UTF-8 严格解码；不是合法 UTF-8 时，带 UTF-16 字节序标记的按 UTF-16 读，其余按 GB18030 读——Windows 上导出的词表常是这两种。UTF-8 的字节序标记原样留着，共享层的解析会去掉它。
     */
    public static String decodeImport(byte[] bytes) {
        if (bytes == null) return "";
        String utf8 = strict(bytes, StandardCharsets.UTF_8);
        if (utf8 != null) return utf8;
        if (bytes.length >= 2 && ((bytes[0] == (byte) 0xFF && bytes[1] == (byte) 0xFE)
                || (bytes[0] == (byte) 0xFE && bytes[1] == (byte) 0xFF))) {
            // `UTF-16` 按字节序标记选端序，并把标记本身去掉。
            return new String(bytes, StandardCharsets.UTF_16);
        }
        return new String(bytes, Charset.forName("GB18030"));
    }

    private static String strict(byte[] bytes, Charset charset) {
        try {
            return charset.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .decode(ByteBuffer.wrap(bytes))
                .toString();
        } catch (CharacterCodingException error) {
            return null;
        }
    }
}
