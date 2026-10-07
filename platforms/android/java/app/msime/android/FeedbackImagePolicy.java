package app.msime.android;

import java.io.IOException;
import java.io.InputStream;

/** 反馈截图的源文件边界；解码前先把选择器流收进有上限的缓冲区。 */
public final class FeedbackImagePolicy {
    /** 允许常见手机照片重新编码，同时拒绝异常大的压缩输入。 */
    public static final int MAX_SOURCE_BYTES = 16 * 1024 * 1024;

    private FeedbackImagePolicy() {}

    /** 读入一份可重复解码的源数据；超过上限或输入为空时返回 null。 */
    public static byte[] readSource(InputStream input) throws IOException {
        return HttpBodyPolicy.readBounded(input, MAX_SOURCE_BYTES);
    }
}
