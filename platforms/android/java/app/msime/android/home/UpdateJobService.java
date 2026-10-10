package app.msime.android.home;

import android.Manifest;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.job.JobInfo;
import android.app.job.JobParameters;
import android.app.job.JobScheduler;
import android.app.job.JobService;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.InstallSourceInfo;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Build;
import androidx.core.content.FileProvider;
import app.msime.android.AppEdition;
import app.msime.android.AppVersionPolicy;
import app.msime.android.FilePolicy;
import app.msime.android.R;
import app.msime.android.ThreadPolicy;
import app.msime.android.UpdateApi;
import java.io.File;
import java.util.concurrent.TimeUnit;

/**
 * 自动更新：开关打开后每天一次（需要联网）在后台检查更新，有新版本就下载、核对 SHA-256 与签名，再发一条通知，点了交给系统安装器。系统安装器必须由用户确认，所以这里从不静默安装。
 *
 * <p>开关默认关闭，和更新通道一起存在宿主自己的 `msime_home_v1` 里，不进共享偏好、不同步：这是这台设备上这个安装包的事。从 Google Play 安装的包（安装来源是 `com.android.vending`）不做自更新，Play 的政策不允许，关于页也不显示这几项。
 *
 * <p>这里同时放着关于页和后台任务共用的几个静态方法：读写这两项设置、排程、判断安装来源、当前版本、把核对过的安装包交给系统安装器。
 */
public final class UpdateJobService extends JobService {
    static final String STORE = "msime_home_v1";
    static final String KEY_AUTO = "update_auto";
    static final String KEY_CHANNEL = "update_channel";
    static final int JOB_ID = 0x6d730401;
    static final String NOTIFICATION_CHANNEL = "msime_updates";
    static final int NOTIFICATION_ID = 0x6d730402;
    private static final String PLAY_STORE = "com.android.vending";
    private static final String APK_TYPE = "application/vnd.android.package-archive";

    private volatile Thread worker;

    @Override public boolean onStartJob(JobParameters params) {
        Context context = getApplicationContext();
        if (!autoUpdate(context) || installedFromPlay(context)) return false;
        worker = ThreadPolicy.namedThread("msime-update-job", () -> {
            boolean retry = false;
            try {
                UpdateApi api = new UpdateApi();
                UpdateApi.Update update = api.check(channel(context), AppEdition.current().id(), currentVersion(context));
                if (update != null) {
                    File apk = api.download(update, context.getCacheDir(), (done, total) -> {
                        if (Thread.currentThread().isInterrupted()) {
                            throw new java.util.concurrent.CancellationException("update download cancelled");
                        }
                    });
                    try {
                        UpdateApi.verifyArchive(context, apk);
                    } catch (UpdateApi.Failure rejected) {
                        FilePolicy.deleteQuietly(apk);
                        throw rejected;
                    }
                    notifyReady(context, update.release().version(), apk);
                }
            } catch (UpdateApi.Failure failure) {
                // 连不上或服务器出错时交给 JobScheduler 按退避重试；校验不通过的包已经删掉，明天再查。
                retry = failure.getCause() instanceof java.io.IOException;
            } finally {
                jobFinished(params, retry);
            }
        });
        worker.start();
        return true;
    }

    @Override public boolean onStopJob(JobParameters params) {
        Thread running = worker;
        if (running != null) running.interrupt();
        return true;
    }

    static SharedPreferences store(Context context) {
        return context.getSharedPreferences(STORE, Context.MODE_PRIVATE);
    }

    static boolean autoUpdate(Context context) {
        return store(context).getBoolean(KEY_AUTO, false);
    }

    static UpdateApi.Channel channel(Context context) {
        return UpdateApi.Channel.fromId(store(context).getString(KEY_CHANNEL, UpdateApi.Channel.STABLE.id()));
    }

    static void setChannel(Context context, UpdateApi.Channel channel) {
        store(context).edit().putString(KEY_CHANNEL, channel.id()).apply();
    }

    /** 保存开关并相应地排上或取消每天一次的任务。 */
    static void setAutoUpdate(Context context, boolean enabled) {
        store(context).edit().putBoolean(KEY_AUTO, enabled).apply();
        JobScheduler scheduler = context.getSystemService(JobScheduler.class);
        if (scheduler == null) return;
        if (!enabled) {
            scheduler.cancel(JOB_ID);
            return;
        }
        JobInfo job = new JobInfo.Builder(JOB_ID, new ComponentName(context, UpdateJobService.class))
            .setPeriodic(TimeUnit.DAYS.toMillis(1))
            .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
            .setPersisted(true)
            .build();
        scheduler.schedule(job);
    }

    /** 安装来源是 Google Play 时为真；读不出来源时按不是 Play 处理。 */
    @SuppressWarnings("deprecation")
    static boolean installedFromPlay(Context context) {
        return PLAY_STORE.equals(installer(context));
    }

    @SuppressWarnings("deprecation")
    static String installer(Context context) {
        PackageManager manager = context.getPackageManager();
        try {
            if (Build.VERSION.SDK_INT >= 30) {
                InstallSourceInfo source = manager.getInstallSourceInfo(context.getPackageName());
                return source.getInstallingPackageName();
            }
            return manager.getInstallerPackageName(context.getPackageName());
        } catch (PackageManager.NameNotFoundException | IllegalArgumentException unknown) {
            return null;
        }
    }

    /** 当前安装的版本名；读不到时是 `0`，任何发布都比它新。 */
    static String currentVersion(Context context) {
        return AppVersionPolicy.versionName(context, "0");
    }

    /** 把核对过的安装包交给系统安装器的 Intent。 */
    static Intent installIntent(Context context, File apk) {
        Uri uri = FileProvider.getUriForFile(context, context.getPackageName() + ".files", apk);
        return new Intent(Intent.ACTION_VIEW)
            .setDataAndType(uri, APK_TYPE)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_ACTIVITY_NEW_TASK);
    }

    /** 本应用还没有被允许安装未知应用时，打开系统里对应的授权页。 */
    static Intent unknownSourcesIntent(Context context) {
        return new Intent(android.provider.Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
            Uri.parse("package:" + context.getPackageName())).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
    }

    private static void notifyReady(Context context, String version, File apk) {
        NotificationManager manager = context.getSystemService(NotificationManager.class);
        if (manager == null) return;
        if (Build.VERSION.SDK_INT >= 33
                && context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            return;
        }
        manager.createNotificationChannel(new NotificationChannel(NOTIFICATION_CHANNEL, "应用更新",
            NotificationManager.IMPORTANCE_DEFAULT));
        Intent target = context.getPackageManager().canRequestPackageInstalls()
            ? installIntent(context, apk) : unknownSourcesIntent(context);
        PendingIntent open = PendingIntent.getActivity(context, 0, target,
            PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Notification notification = new Notification.Builder(context, NOTIFICATION_CHANNEL)
            .setSmallIcon(R.drawable.ic_ms_download)
            .setContentTitle(context.getString(R.string.app_name) + " " + version + " 已下载")
            .setContentText("点按安装新版本")
            .setContentIntent(open)
            .setAutoCancel(true)
            .build();
        manager.notify(NOTIFICATION_ID, notification);
    }
}
