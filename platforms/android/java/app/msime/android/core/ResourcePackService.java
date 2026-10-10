package app.msime.android;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.content.pm.ServiceInfo;
import android.graphics.drawable.Icon;
import android.net.ConnectivityManager;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import androidx.annotation.Nullable;
import app.msime.android.home.HostStore;
import app.msime.android.home.MsToast;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;
import org.json.JSONObject;

/**
 * 按需资源包的下载：主进程里的前台服务（dataSync），带进度通知，可以取消。
 *
 * <p>下载只由用户的操作发起：在「添加语言」里加日语、粤语、注音或笔画，打开离线释义，打开离线识别，或在设置页点「下载」「重试」。启动时从不自动下载，服务也不在进程被杀后自己重启（{@link #START_NOT_STICKY}）；没下完的文件留在共享层的续传目录里，下次点下载时用 HTTP Range 接着下。
 *
 * <p>同一个资源包同一时间只有一个下载：这里按资源包记着状态，正在下的再点一次什么也不做（共享层另有 {@code resource_pack_busy} 兜底）。服务不声明 {@code android:process}，所以和设置页一样跑在主进程，:ime 进程从不安装：两个进程同时装同一个包会互相清掉对方的暂存目录。
 *
 * <p>设置页通过 {@link #status} 和 {@link #addListener} 读进度画行状态：下载中 x%、校验中、失败（带原因）。下完的资源包不再记状态，以 {@link ResourcePacks#installed} 为准；键盘在下一次进入输入框时重读，新语言就出现在切换器里。
 */
public final class ResourcePackService extends Service {
    private static final String ACTION_INSTALL = "app.msime.android.action.RESOURCE_PACK_INSTALL";
    private static final String ACTION_CANCEL = "app.msime.android.action.RESOURCE_PACK_CANCEL";
    private static final String EXTRA_PACK = "pack";
    private static final String CHANNEL = "msime_resource_packs";
    private static final int PROGRESS_NOTIFICATION = 0x6d730501;
    /** 每个资源包一条结果通知（下载完成或失败），按 {@link #PACKS} 里的位置错开 id。 */
    private static final int RESULT_NOTIFICATION = 0x6d730510;
    private static final List<String> PACKS = List.of(ResourcePacks.JAPANESE, ResourcePacks.LANGUAGE_DICTIONARIES,
        ResourcePacks.OFFLINE_GLOSSES, ResourcePacks.VOICE_RUNTIME);

    /** 一个资源包正在经历的阶段。下载完成后不再有状态。 */
    public enum Phase { DOWNLOADING, VERIFYING, FAILED }

    /** 一个资源包的下载状态；{@code reason} 只在 {@link Phase#FAILED} 时有，是给用户看的中文原因。 */
    public record Status(Phase phase, long done, long total, @Nullable String reason) {
        public boolean running() {
            return phase != Phase.FAILED;
        }

        public int percent() {
            return total <= 0 ? 0 : (int) Math.min(100, done * 100 / total);
        }
    }

    /** 状态变化的回调，总在主线程上调用；{@code finished} 为真表示这个资源包的下载刚结束（成功、失败或取消），页面应重读安装状态。 */
    public interface Listener {
        void onChanged(String pack, boolean finished);
    }

    private static final Map<String, Status> STATUS = new ConcurrentHashMap<>();
    private static final List<Listener> LISTENERS = new CopyOnWriteArrayList<>();
    /** 用户已经要求取消、但下载还没结束的资源包。共享层的取消标记要等下载线程进入安装后才登记，在那之前的取消只记在这里，由下载线程补上。 */
    private static final Set<String> CANCELLING = ConcurrentHashMap.newKeySet();
    private static final Handler MAIN = MainThreadPolicy.mainHandler();

    // ---- 设置页用的静态入口（主线程） ----

    /** 这个资源包当前的下载状态；没在下、也没失败过时为 null。 */
    @Nullable public static Status status(String pack) {
        return STATUS.get(pack);
    }

    public static void addListener(Listener listener) {
        LISTENERS.add(listener);
    }

    public static void removeListener(Listener listener) {
        LISTENERS.remove(listener);
    }

    /**
     * 用户点了下载：没联网时提示；按流量计费的网络先弹窗说明大小，确认后才开始；其它网络直接开始。正在下的资源包什么也不做。
     *
     * @param context 页面的 Context，用来弹确认框
     * @param size 下载字节数，取自 {@link ResourcePacks.Pack#size()}
     */
    public static void request(Context context, String pack, long size) {
        Status current = STATUS.get(pack);
        if (current != null && current.running()) return;
        ConnectivityManager connectivity = context.getSystemService(ConnectivityManager.class);
        boolean metered = true;
        if (connectivity != null) {
            if (connectivity.getActiveNetwork() == null) {
                MsToast.show(context, "没有网络连接，联网后再下载" + title(pack));
                return;
            }
            metered = connectivity.isActiveNetworkMetered();
        }
        if (!metered) {
            start(context, pack);
            return;
        }
        new MaterialAlertDialogBuilder(context)
            .setTitle("使用移动数据下载？")
            .setMessage("当前网络按流量计费。下载" + title(pack) + "需要 " + size(size)
                + "，下载后只保存在本机。也可以连上 Wi-Fi 后再下载。")
            .setNegativeButton("取消", null)
            .setPositiveButton("下载", (dialog, which) -> start(context, pack))
            .show();
    }

    /** 停下这个资源包的下载；已下载的部分留着，下次接着下。 */
    public static void cancel(String pack) {
        Status current = STATUS.get(pack);
        if (current != null && current.running()) CANCELLING.add(pack);
        ResourcePacks.cancel(pack);
    }

    /** 资源包给用户看的名字。 */
    public static String title(String pack) {
        return switch (pack) {
            case ResourcePacks.JAPANESE -> "日文词典";
            case ResourcePacks.LANGUAGE_DICTIONARIES -> "粤拼、注音和笔画词库";
            case ResourcePacks.OFFLINE_GLOSSES -> "离线释义词典";
            case ResourcePacks.VOICE_RUNTIME -> "本地语音运行库";
            default -> pack;
        };
    }

    /** 下载大小，十进制 MB 保留一位，与发布页上的写法一致。 */
    public static String size(long bytes) {
        return String.format(Locale.ROOT, "%.1f MB", bytes / 1_000_000.0);
    }

    /** 行里显示的状态文字：下载中 x%、校验中、下载失败和原因；没在下也没失败时为 null。 */
    @Nullable public static String describe(@Nullable Status status) {
        if (status == null) return null;
        return switch (status.phase()) {
            case DOWNLOADING -> "下载中 " + status.percent() + "%";
            case VERIFYING -> "校验中…";
            case FAILED -> "下载失败：" + status.reason();
        };
    }

    private static void start(Context context, String pack) {
        Intent intent = new Intent(context, ResourcePackService.class)
            .setAction(ACTION_INSTALL).putExtra(EXTRA_PACK, pack);
        context.startForegroundService(intent);
    }

    // ---- 服务 ----

    @Override public void onCreate() {
        super.onCreate();
        NotificationManager manager = getSystemService(NotificationManager.class);
        if (manager != null) {
            manager.createNotificationChannel(new NotificationChannel(CHANNEL, "资源下载",
                NotificationManager.IMPORTANCE_LOW));
        }
    }

    /** 最近一次 {@link #onStartCommand} 的 startId，只在主线程上读写。 */
    private int lastStartId;

    @Override public int onStartCommand(@Nullable Intent intent, int flags, int startId) {
        lastStartId = startId;
        String action = intent == null ? null : intent.getAction();
        String pack = intent == null ? null : intent.getStringExtra(EXTRA_PACK);
        if (ACTION_CANCEL.equals(action)) {
            List<String> running = runningPacks();
            for (String each : running) cancel(each);
            // 下载已经结束后才点到的取消：这次启动的服务没有事可做，不留在后台。
            if (running.isEmpty()) stopSelf(startId);
            return START_NOT_STICKY;
        }
        // startForegroundService 之后必须很快进入前台，即使这次请求最后什么也不做。
        enterForeground();
        if (ACTION_INSTALL.equals(action) && pack != null && PACKS.contains(pack) && claim(pack)) {
            install(pack);
        }
        if (runningPacks().isEmpty()) stop();
        return START_NOT_STICKY;
    }

    @Nullable @Override public IBinder onBind(Intent intent) {
        return null;
    }

    /** Android 15 起 dataSync 前台服务每天有时长上限，到点时系统要求立即停下：取消全部下载，已下的部分留着续传。 */
    @Override public void onTimeout(int startId, int fgsType) {
        for (String running : runningPacks()) cancel(running);
        stop();
    }

    /** 占住这个资源包；已经在下时返回 false。只在主线程上调用。 */
    private static boolean claim(String pack) {
        Status current = STATUS.get(pack);
        if (current != null && current.running()) return false;
        CANCELLING.remove(pack);
        STATUS.put(pack, new Status(Phase.DOWNLOADING, 0, 0, null));
        notifyListeners(pack, false);
        return true;
    }

    private static List<String> runningPacks() {
        List<String> running = new ArrayList<>(PACKS.size());
        for (String pack : PACKS) {
            Status status = STATUS.get(pack);
            if (status != null && status.running()) running.add(pack);
        }
        return running;
    }

    private void install(String pack) {
        Context context = getApplicationContext();
        ThreadPolicy.startNamedThread("msime-resource-pack-" + pack, () -> {
            String failure = null;
            try {
                if (CANCELLING.contains(pack)) {
                    failure = "";
                } else {
                    ResourcePacks.install(context, pack, mirrors(context),
                        (phase, done, total) -> progress(pack, phase, done, total));
                }
            } catch (ResourcePacks.Failure error) {
                failure = error.cancelled() ? "" : reason(error.code());
            } catch (RuntimeException | LinkageError error) {
                // 共享层不可用（库没加载上等）也是一种失败，交给用户重试，而不是让进程崩掉。
                failure = reason("");
            }
            String result = failure;
            MAIN.post(() -> finished(pack, result));
        });
    }

    /** 用户在共享偏好里填的下载镜像前缀（与本地语音模型共用的 `voice_input.asr_model_mirror`）；没填时为空，按项目镜像、锁文件原地址的顺序下载。 */
    private static List<String> mirrors(Context context) {
        JSONObject snapshot = HostStore.loadPreferences(context);
        JSONObject preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");
        JSONObject voice = preferences == null ? null : preferences.optJSONObject("voice_input");
        String mirror = voice == null ? ""
            : JsonPolicy.strictStringOrEmpty(voice.opt("asr_model_mirror")).trim();
        return mirror.isEmpty() ? List.of() : List.of(mirror);
    }

    /** 共享层的进度回调，在下载线程上：只在阶段或百分比变化时更新状态，免得每个数据块都重画页面和通知。 */
    private void progress(String pack, String phase, long done, long total) {
        // 共享层这时已经登记了这次安装，补上登记之前就到了的取消。
        if (CANCELLING.contains(pack)) ResourcePacks.cancel(pack);
        Phase next = "download".equals(phase) ? Phase.DOWNLOADING : Phase.VERIFYING;
        Status updated = new Status(next, done, total, null);
        Status previous = STATUS.get(pack);
        if (previous != null && previous.phase() == next && previous.percent() == updated.percent()) return;
        STATUS.put(pack, updated);
        MAIN.post(() -> {
            notifyListeners(pack, false);
            updateProgressNotification();
        });
    }

    /** 主线程：一个资源包的下载结束。{@code failure} 为 null 是成功，空串是用户取消，其它是失败原因。 */
    private void finished(String pack, @Nullable String failure) {
        CANCELLING.remove(pack);
        if (failure == null || failure.isEmpty()) {
            STATUS.remove(pack);
        } else {
            STATUS.put(pack, new Status(Phase.FAILED, 0, 0, failure));
        }
        if (failure == null) {
            notifyResult(pack, title(pack) + "已下载", "下次打开键盘时即可使用");
        } else if (!failure.isEmpty()) {
            notifyResult(pack, title(pack) + "下载失败", failure);
        }
        notifyListeners(pack, true);
        if (runningPacks().isEmpty()) stop();
        else updateProgressNotification();
    }

    private static void notifyListeners(String pack, boolean finished) {
        for (Listener listener : LISTENERS) listener.onChanged(pack, finished);
    }

    /** 共享层错误码对应的中文原因。 */
    private static String reason(String code) {
        return switch (code) {
            case "local_model_network" -> "网络连接失败，请检查网络后重试";
            case "local_model_http_status" -> "下载服务器暂时不可用，请稍后重试";
            case "local_model_size_mismatch", "local_model_checksum_mismatch", "local_model_unsafe_archive",
                 "local_model_missing_file" -> "下载的文件校验未通过，请重试";
            case "local_model_io" -> "无法写入文件，请检查手机剩余空间";
            case "local_model_invalid_mirror" -> "下载镜像地址无效，请检查镜像设置";
            case "local_model_invalid_root", "invalid state root" -> "请先完成首次设置";
            case "resource_pack_busy" -> "这个资源包正在下载";
            default -> code.isEmpty() ? "下载组件不可用，请重试" : "下载失败（" + code + "）";
        };
    }

    // ---- 通知 ----

    private void enterForeground() {
        Notification notification = progressNotification();
        if (Build.VERSION.SDK_INT >= 29) {
            startForeground(PROGRESS_NOTIFICATION, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC);
        } else {
            startForeground(PROGRESS_NOTIFICATION, notification);
        }
    }

    /** 没有下载在跑时退出前台并停下服务。按最近的 startId 停：一个下载结束时另一个资源包的启动请求可能已经排在主线程上，那时服务不停，那次请求照常进前台、开始下载。 */
    private void stop() {
        stopForeground(STOP_FOREGROUND_REMOVE);
        stopSelf(lastStartId);
    }

    private void updateProgressNotification() {
        if (runningPacks().isEmpty()) return;
        NotificationManager manager = getSystemService(NotificationManager.class);
        if (manager != null) manager.notify(PROGRESS_NOTIFICATION, progressNotification());
    }

    /** 正在下载的全部资源包合成一条进度：标题列出名字，进度条按已知的总字节数合计；还不知道总大小时是不确定进度。 */
    private Notification progressNotification() {
        List<String> running = runningPacks();
        List<String> titles = new ArrayList<>(running.size());
        long done = 0;
        long total = 0;
        boolean verifying = !running.isEmpty();
        for (String pack : running) {
            titles.add(title(pack));
            Status status = STATUS.get(pack);
            if (status == null) continue;
            done += status.done();
            total += status.total();
            verifying &= status.phase() == Phase.VERIFYING;
        }
        int percent = total <= 0 ? 0 : (int) Math.min(100, done * 100 / total);
        PendingIntent cancel = PendingIntent.getService(this, 0,
            new Intent(this, ResourcePackService.class).setAction(ACTION_CANCEL),
            PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Notification.Builder builder = new Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_ms_download)
            .setContentTitle(titles.isEmpty() ? "正在准备下载" : "正在下载" + String.join("、", titles))
            .setContentText(verifying ? "正在校验文件" : total <= 0 ? "正在连接" : percent + "%")
            .setProgress(100, percent, total <= 0 || verifying)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .addAction(new Notification.Action.Builder(
                Icon.createWithResource(this, R.drawable.ic_ms_delete), "取消", cancel).build());
        PendingIntent open = openApp();
        if (open != null) builder.setContentIntent(open);
        return builder.build();
    }

    private void notifyResult(String pack, String title, String text) {
        NotificationManager manager = getSystemService(NotificationManager.class);
        if (manager == null) return;
        if (Build.VERSION.SDK_INT >= 33
                && checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            return;
        }
        Notification.Builder builder = new Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_ms_download)
            .setContentTitle(title)
            .setContentText(text)
            .setAutoCancel(true);
        PendingIntent open = openApp();
        if (open != null) builder.setContentIntent(open);
        manager.notify(RESULT_NOTIFICATION + PACKS.indexOf(pack), builder.build());
    }

    @Nullable private PendingIntent openApp() {
        Intent launch = getPackageManager().getLaunchIntentForPackage(getPackageName());
        if (launch == null) return null;
        return PendingIntent.getActivity(this, 0, launch,
            PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }
}
