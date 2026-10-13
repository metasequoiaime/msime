package app.msime.android.home;

import android.content.Context;
import androidx.fragment.app.Fragment;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.CommunitySkinCache;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.Consumer;

/**
 * 刷新 {@link CommunitySkinCache}：拉全社区皮肤目录写进本机缓存，键盘的皮肤面板据此列出全部社区皮肤（键盘进程不为浏览目录联网），设置里的社区 Tab 通过完整列表按搜索和分类分页加载。
 *
 * <p>以前只有底部的「社区」tab 在打开时刷新它；那个 tab 撤掉以后，皮肤页在缓存过期时刷新（{@link CommunitySkinCache#stale(Path, long)}），完整的社区皮肤列表每次打开都刷新。最多翻十页，任何一页失败都保留旧缓存。同一时刻只跑一次，两个页面同时触发时后来的那次直接跳过。
 */
final class CommunitySkinCatalogue {
    private static final int MAX_PAGES = 10;
    private static final AtomicBoolean RUNNING = new AtomicBoolean();

    private CommunitySkinCatalogue() {}

    /**
     * 在网络线程上刷新缓存，结果交回主线程（页面已经不在时不交）。
     *
     * @param force 为 false 时缓存还新鲜就不联网
     * @param done 收到 true 表示写了新缓存，false 表示没有必要或另一次刷新正在进行，null 表示联网失败、缓存保持原样
     */
    static void refresh(Fragment fragment, boolean force, Consumer<Boolean> done) {
        HostTask.runNetwork(fragment, context -> fetch(context, force), done);
    }

    private static Boolean fetch(Context context, boolean force) {
        String directory = HostStore.directory(context);
        if (directory.isEmpty()) return Boolean.FALSE;
        Path root = Paths.get(directory);
        if (!force && !CommunitySkinCache.stale(root, System.currentTimeMillis())) return Boolean.FALSE;
        if (!RUNNING.compareAndSet(false, true)) return Boolean.FALSE;
        try {
            CommunityCatalog catalog = new CommunityCatalog(context);
            List<CommunitySkinCache.Entry> entries = new ArrayList<>(MAX_PAGES * CommunityRequest.PAGE_SIZE);
            for (int page = 0; page < MAX_PAGES; page++) {
                CommunityCatalog.Page result = catalog.list(CommunityRequest.Kind.SKIN, "", entries.size(), null);
                if (result == null || result.failed()) return null;
                for (CommunityCatalog.Item item : result.items()) {
                    if (item.payload() != null) {
                        entries.add(new CommunitySkinCache.Entry(item.id(), item.name(), item.author(), item.payload()));
                    }
                }
                if (!result.hasMore() || result.items().isEmpty()) break;
            }
            try {
                CommunitySkinCache.write(root, entries);
            } catch (java.io.IOException error) {
                android.util.Log.w("MSIMECommunity", "Skin catalogue cache was not written", error);
                return null;
            }
            return Boolean.TRUE;
        } finally {
            RUNNING.set(false);
        }
    }
}
