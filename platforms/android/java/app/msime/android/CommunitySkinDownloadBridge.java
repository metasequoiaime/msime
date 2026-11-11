package app.msime.android;

import android.content.Context;
import android.net.Uri;
import android.os.Bundle;
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
            Bundle extras = new Bundle();
            extras.putString(AccountSessionRoutingPolicy.KEY_SKIN_ID, id);
            Bundle reply = application.getContentResolver().call(
                Uri.parse("content://" + AccountSessionRoutingPolicy.authority(application.getPackageName())),
                AccountSessionRoutingPolicy.METHOD_SKIN_DOWNLOAD, null, extras);
            if (reply == null) return unavailable();
            String failure = reply.getString(AccountSessionRoutingPolicy.KEY_ERROR, "");
            if (!failure.isEmpty()) return new CommunityCatalog.Download(null, failure);
            String encoded = reply.getString(AccountSessionRoutingPolicy.KEY_SKIN_DESIGN, "");
            if (encoded.isEmpty()) return unavailable();
            return new CommunityCatalog.Download(new JSONObject(encoded), "");
        } catch (JSONException | RuntimeException | LinkageError error) {
            return unavailable();
        }
    }

    private static CommunityCatalog.Download unavailable() {
        return new CommunityCatalog.Download(null, "皮肤下载失败，请稍后重试。");
    }
}
