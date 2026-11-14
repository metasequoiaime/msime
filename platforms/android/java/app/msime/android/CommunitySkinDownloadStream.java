package app.msime.android;

import android.content.ContentProvider;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import java.io.FileNotFoundException;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import org.json.JSONException;
import org.json.JSONObject;

/** 完整皮肤设计走管道，照片的 Base64 字符串不进入 Binder 事务。 */
public final class CommunitySkinDownloadStream {
    public static final String PATH = "skin-download";
    // 共享设计最多含 512000 字节照片；为 Base64 和其余有界字段留出空间。
    private static final int MAX_REPLY_BYTES = 1024 * 1024;

    private CommunitySkinDownloadStream() {}

    public static ParcelFileDescriptor open(ContentProvider provider, Uri uri,
            JSONObject design, String failure) throws FileNotFoundException {
        byte[] bytes;
        try {
            bytes = new JSONObject().put("design", design).put("error", failure)
                .toString().getBytes(StandardCharsets.UTF_8);
            if (bytes.length > MAX_REPLY_BYTES) throw new FileNotFoundException("skin reply too large");
        } catch (JSONException error) {
            throw new FileNotFoundException("skin reply unavailable");
        }
        return provider.openPipeHelper(uri, "application/json", null, bytes,
            (output, request, mime, options, body) -> {
                try (var stream = new ParcelFileDescriptor.AutoCloseOutputStream(output)) {
                    stream.write(body);
                } catch (IOException ignored) {
                    // 读端取消或退出时关闭写端，不把设计或账号信息写进日志。
                }
            });
    }

    public static JSONObject read(InputStream input) throws IOException, JSONException {
        if (input == null) throw new IOException("skin reply unavailable");
        return new JSONObject(new String(HttpBodyPolicy.readRequired(input, MAX_REPLY_BYTES),
            StandardCharsets.UTF_8));
    }
}
