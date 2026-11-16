package app.msime.android;

import android.content.Context;
import android.net.Uri;
import java.io.IOException;
import org.json.JSONException;
import org.json.JSONObject;

/** 输入法进程请求主进程下载社区皮肤；输入法本身不联网。 */
public final class CommunitySkinDownloadBridge {
    private CommunitySkinDownloadBridge() {}

    /** 通过同 UID 的非导出 provider 获取完整设计，失败时返回可直接展示的文案。 */
    public static CommunityCatalog.Download download(Context context, String id) {
        if (context == null || !CommunityCatalog.validDownloadId(id))
            return new CommunityCatalog.Download(null, "这类作品还不能从这里保存。");
        try {
            Context application = context.getApplicationContext();
            Uri uri = new Uri.Builder().scheme("content")
                .authority(AccountSessionRoutingPolicy.authority(application.getPackageName()))
                .appendPath(CommunitySkinDownloadStream.PATH).appendPath(id).build();
            JSONObject reply;
            try (var input = application.getContentResolver().openInputStream(uri)) {
                reply = CommunitySkinDownloadStream.read(input);
            }
            String failure = reply.optString("error", "");
            if (!failure.isEmpty()) return new CommunityCatalog.Download(null, failure);
            JSONObject design = reply.optJSONObject("design");
            return design == null ? unavailable() : new CommunityCatalog.Download(design, "");
        } catch (IOException | JSONException | RuntimeException | LinkageError error) {
            return unavailable();
        }
    }

    private static CommunityCatalog.Download unavailable() {
        return new CommunityCatalog.Download(null, "皮肤下载失败，请稍后重试。");
    }
}
