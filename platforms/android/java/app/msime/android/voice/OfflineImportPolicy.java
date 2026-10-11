package app.msime.android;

import java.util.List;
import java.util.Locale;

/**
 * 从本地文件安装本地语音模型或语音运行库时的规则：用户选的文件哪些要复制进缓存、复制最多读多少、对话框里怎么列出要下载的文件。
 *
 * <p>系统文件选择器给的是 content URI，共享层的安装器只认绝对路径，所以先把文件复制进 {@code cacheDir}，安装完删掉。共享层按长度和 SHA-256 认文件、不看名字，长度对不上的文件不可能被用上，复制它只是白占空间（HarmonyOS 的导入同样只复制长度对得上的）。文件提供方报不出长度时照样复制，但读到比最大的那个文件还多就停下：那不会是要的文件。
 */
public final class OfflineImportPolicy {
    /** 一次交给共享层的文件数上限，与 C ABI 的 {@code files} 上限相同。 */
    public static final int MAX_FILES = 16;

    private OfflineImportPolicy() {}

    /**
     * 这个文件要不要复制。
     *
     * @param size 文件提供方报的长度，报不出来时为负数
     * @param expected 要导入的文件的长度
     */
    public static boolean shouldCopy(long size, long[] expected) {
        if (size < 0) return true;
        for (long value : expected) if (value == size) return true;
        return false;
    }

    /** 复制一个文件最多读的字节数：要导入的文件里最大的那个。超过它的不会是要的文件。 */
    public static long copyLimit(long[] expected) {
        long limit = 0;
        for (long value : expected) limit = Math.max(limit, value);
        return limit;
    }

    /** 对话框里的一行：上游文件名和大小。 */
    public static String fileLine(String name, long size) {
        return "· " + name + "（" + String.format(Locale.ROOT, "%.1f MB", size / 1_000_000.0) + "）";
    }

    /** 对话框正文：先说怎么做，再逐行列出要下载的文件。 */
    public static String instructions(List<String> lines) {
        return "先用浏览器或下载工具下载下面的文件，再点「选择文件」把它们一起选上。按内容认文件，改了文件名也没关系。\n\n"
            + String.join("\n", lines);
    }
}
