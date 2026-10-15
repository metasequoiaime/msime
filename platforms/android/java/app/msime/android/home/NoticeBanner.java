package app.msime.android.home;

import app.msime.android.TextPolicy;

import android.content.Context;
import android.view.LayoutInflater;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.fragment.app.Fragment;
import app.msime.android.NativeClient;
import app.msime.android.JsonPolicy;
import app.msime.android.R;
import app.msime.android.TextPolicy;
import app.msime.android.ViewPolicy;
import io.noties.markwon.AbstractMarkwonPlugin;
import io.noties.markwon.Markwon;
import io.noties.markwon.MarkwonConfiguration;
import java.io.File;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 设置页顶上的公告：服务端发给 Android 应用的通知，一次一张，点「知道了」后这台设备不再显示。
 *
 * <p>公告源、它的一分钟缓存和已关闭的 id 都在共享的 client-services 存储里（`msime_client_notices`）；本类在应用首页打开时向它询问，并绘制剩下的最新一条公告。键盘进程从不调用它：公告属于应用内容，输入法不能轮询。
 *
 * <p>正文是简单的 Markdown，由 Markwon 的核心渲染，它不支持 HTML：公告里的原始 HTML 从不被解释，图片也不加载。链接在浏览器里打开，而且只限 http、https 和 mailto 链接。
 */
final class NoticeBanner {
    private static final String DIRECTORY = "notices";

    record Notice(String id, String title, String body) {}

    private NoticeBanner() {}

    /** Fetch (or reuse the cached feed) off the main thread and show the newest notice in `container`. */
    static void load(Fragment fragment, LinearLayout container) {
        HostTask.run(fragment, NoticeBanner::fetch, notices -> show(fragment, container, notices));
    }

    private static List<Notice> fetch(Context context) {
        try {
            JSONObject request = new JSONObject()
                .put("directory", directory(context))
                .put("platform", "android")
                .put("channel", "app");
            JSONObject root = new JSONObject(NativeClient.notices(request.toString()));
            JSONObject value = JsonPolicy.objectValue(root);
            JSONArray items = value == null ? null : value.optJSONArray("items");
            List<Notice> notices = new ArrayList<>(items == null ? 0 : items.length());
            for (int index = 0; items != null && index < items.length(); index++) {
                JSONObject item = items.optJSONObject(index);
                if (item == null) continue;
                String id = JsonPolicy.strictString(item.opt("id"));
                String title = JsonPolicy.strictString(item.opt("title"));
                String body = JsonPolicy.strictString(item.opt("body"));
                if (id == null || title == null || body == null) continue;
                title = TextPolicy.trimmed(title);
                if (id.isEmpty() || title.isEmpty()) continue;
                notices.add(new Notice(id, title, body));
            }
            return notices;
        } catch (Exception | LinkageError error) {
            // No notices is the right thing to show when the feed or the host cannot answer.
            android.util.Log.i("MSIMENotices", "Notices unavailable", error);
            return List.of();
        }
    }

    private static void show(Fragment fragment, LinearLayout container,
            @Nullable List<Notice> notices) {
        container.removeAllViews();
        if (notices == null || notices.isEmpty()) return;
        Context context = container.getContext();
        Notice notice = notices.get(0);
        View card = LayoutInflater.from(context).inflate(R.layout.item_notice, container, false);
        ((TextView) card.findViewById(R.id.notice_title)).setText(notice.title());
        TextView body = card.findViewById(R.id.notice_body);
        if (notice.body().isBlank()) {
            ViewPolicy.hide(body);
        } else {
            markwon(context).setMarkdown(body, notice.body());
        }
        ViewPolicy.bindClick(card.findViewById(R.id.notice_dismiss), () -> {
            List<Notice> rest = new ArrayList<>(notices.subList(1, notices.size()));
            show(fragment, container, rest);
            HostTask.run(fragment, worker -> dismiss(worker, notice.id()), ignoredResult -> {});
        });
        container.addView(card);
    }

    private static Boolean dismiss(Context context, String id) {
        try {
            JSONObject request = new JSONObject().put("directory", directory(context)).put("id", id);
            JSONObject response = new JSONObject(NativeClient.noticeDismiss(request.toString()));
            return JsonPolicy.strictTrue(response.opt("ok"));
        } catch (Exception | LinkageError error) {
            android.util.Log.i("MSIMENotices", "Dismissal not saved", error);
            return false;
        }
    }

    private static String directory(Context context) {
        File directory = new File(context.getFilesDir(), DIRECTORY);
        return directory.getAbsolutePath();
    }

    private static Markwon markwon(Context context) {
        return Markwon.builder(context)
            .usePlugin(new AbstractMarkwonPlugin() {
                @Override public void configureConfiguration(@NonNull MarkwonConfiguration.Builder builder) {
                    builder.linkResolver((view, link) -> open(view.getContext(), link));
                }
            })
            .build();
    }

    private static void open(Context context, String link) {
        LinkActions.openMarkdown(context, link);
    }
}
