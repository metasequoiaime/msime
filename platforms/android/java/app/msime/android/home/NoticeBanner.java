package app.msime.android.home;

import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.view.LayoutInflater;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.fragment.app.Fragment;
import app.msime.android.NativeClient;
import app.msime.android.R;
import io.noties.markwon.AbstractMarkwonPlugin;
import io.noties.markwon.Markwon;
import io.noties.markwon.MarkwonConfiguration;
import java.io.File;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 设置页顶上的公告：服务端发给 Android 应用的通知，一次一张，点「知道了」后这台设备不再显示。
 *
 * <p>The feed, its one-minute cache and the dismissed ids live in the shared client-core store (msime_client_notices); this class asks it when the app home opens and draws the newest notice that is left. Never called from the keyboard process: notices are app content, and the input method must not poll.
 *
 * <p>The body is simple Markdown rendered by Markwon's core, which has no HTML support: raw HTML in a notice is never interpreted, and images are not loaded. Links open in the browser, and only http, https and mailto ones.
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
        List<Notice> notices = new ArrayList<>();
        try {
            JSONObject request = new JSONObject()
                .put("directory", directory(context))
                .put("platform", "android")
                .put("channel", "app");
            JSONObject root = new JSONObject(NativeClient.notices(request.toString()));
            JSONObject value = root.optBoolean("ok", false) ? root.optJSONObject("value") : null;
            JSONArray items = value == null ? null : value.optJSONArray("items");
            notices = new ArrayList<>(items == null ? 0 : items.length());
            for (int index = 0; items != null && index < items.length(); index++) {
                JSONObject item = items.optJSONObject(index);
                if (item == null) continue;
                String id = item.optString("id", "");
                String title = item.optString("title", "").trim();
                if (id.isEmpty() || title.isEmpty()) continue;
                notices.add(new Notice(id, title, item.optString("body", "")));
            }
        } catch (Exception | LinkageError error) {
            // No notices is the right thing to show when the feed or the host cannot answer.
            android.util.Log.i("MSIMENotices", "Notices unavailable", error);
        }
        return notices;
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
            body.setVisibility(View.GONE);
        } else {
            markwon(context).setMarkdown(body, notice.body());
        }
        card.findViewById(R.id.notice_dismiss).setOnClickListener(ignored -> {
            List<Notice> rest = new ArrayList<>(notices.subList(1, notices.size()));
            show(fragment, container, rest);
            HostTask.run(fragment, worker -> dismiss(worker, notice.id()), ignoredResult -> {});
        });
        container.addView(card);
    }

    private static Boolean dismiss(Context context, String id) {
        try {
            JSONObject request = new JSONObject().put("directory", directory(context)).put("id", id);
            return new JSONObject(NativeClient.noticeDismiss(request.toString())).optBoolean("ok", false);
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
        Uri uri = Uri.parse(link);
        String scheme = uri.getScheme() == null ? "" : uri.getScheme().toLowerCase(Locale.ROOT);
        if (!scheme.equals("https") && !scheme.equals("http") && !scheme.equals("mailto")) return;
        try {
            context.startActivity(new Intent(Intent.ACTION_VIEW, uri)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        } catch (RuntimeException error) {
            // No browser: the link stays text, which is all a notice link is.
        }
    }
}
