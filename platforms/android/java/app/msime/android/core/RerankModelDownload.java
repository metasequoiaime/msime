package app.msime.android;

import android.app.job.JobInfo;
import android.app.job.JobScheduler;
import android.content.ComponentName;
import android.content.Context;
import java.util.Set;
import java.util.concurrent.atomic.AtomicBoolean;
import org.json.JSONObject;

/**
 * 整句候选「增强」的逐键重排模型（资源包 {@link ResourcePacks#RERANK_MODEL}）什么时候要下载，以及排后台下载任务。
 *
 * <p>「增强」可能从来不是用户自己选的（缺省值、别的设备同步过来），只靠设置页里选「增强」时下载，多数人永远拿不到模型。所以输入法进程和设置应用看到「增强」开着、模型没装时排一个 JobScheduler 任务（{@link #scheduleIfWanted}）：只在不按流量计费的网络上运行，电量和存储不低时才跑，重启后仍在。任务本身是主进程里的 {@code home.RerankModelJobService}：输入法进程只排任务、从不联网。按流量计费的网络上不在后台下，由设置应用回到前台时问一次（HomeActivity）。
 */
public final class RerankModelDownload {
    static final int JOB_ID = 0x6d730403;
    /** 跑下载的任务；按名字引用，这个类不依赖设置应用那一侧的代码。 */
    private static final String JOB_SERVICE = "app.msime.android.home.RerankModelJobService";
    /** 本进程已经排过（或确认已经排着）任务；每个进程只问一次 JobScheduler，输入法每次读偏好都会走到 {@link #scheduleIfWanted}。 */
    private static final AtomicBoolean SCHEDULED = new AtomicBoolean();

    private RerankModelDownload() {}

    /**
     * 偏好里整句候选是不是「增强」：`sentence_association` 的 `word_lattice` 与 `neural_keyboard` 都开。缺键时按共享偏好的缺省读，与设置页的三档（ExpressionPage 的 sentenceLevel）同一个口径，改缺省时两处一起改。
     */
    public static boolean enhanced(JSONObject preferences) {
        JSONObject association = preferences == null ? null : preferences.optJSONObject("sentence_association");
        boolean lattice = association == null || association.optBoolean("word_lattice", true);
        boolean neural = association != null && association.optBoolean("neural_keyboard", false);
        return lattice && neural;
    }

    /** 这个版本、这份偏好下要不要这个模型：只有带拼音的版本（full、拼音版，`neural_keyboard` 为真的那两个）挂逐键重排，五笔、日文、越南文、藏文版选了「增强」也用不上。 */
    public static boolean wanted(JSONObject preferences) {
        return AppEdition.current().offers("quanpin") && enhanced(preferences);
    }

    /**
     * 「增强」开着、模型没装时排后台下载任务；每个进程只排一次，已经排着的不重排。任何线程、任何进程都可以调，最多一次 JobScheduler 调用。
     *
     * @param installed 本机已完整安装的资源包 id（{@link ResourcePacks#installedIds}）
     */
    public static void scheduleIfWanted(Context context, JSONObject preferences, Set<String> installed) {
        if (installed.contains(ResourcePacks.RERANK_MODEL) || !wanted(preferences)) return;
        schedule(context);
    }

    /** 排后台下载任务，调用方已经确认「增强」开着、模型没装（设置页刚选了增强）。每个进程只排一次，已经排着的不重排。 */
    public static void schedule(Context context) {
        if (!SCHEDULED.compareAndSet(false, true)) return;
        JobScheduler scheduler = context.getSystemService(JobScheduler.class);
        if (scheduler == null || scheduler.getPendingJob(JOB_ID) != null) return;
        JobInfo job = new JobInfo.Builder(JOB_ID, new ComponentName(context.getPackageName(), JOB_SERVICE))
            .setRequiredNetworkType(JobInfo.NETWORK_TYPE_UNMETERED)
            .setRequiresBatteryNotLow(true)
            .setRequiresStorageNotLow(true)
            .setPersisted(true)
            .build();
        scheduler.schedule(job);
    }

    /** 用户切回「标准」或「关闭」：撤掉还没跑的任务；之后再开「增强」时重新排。 */
    public static void cancelScheduled(Context context) {
        SCHEDULED.set(false);
        JobScheduler scheduler = context.getSystemService(JobScheduler.class);
        if (scheduler != null) scheduler.cancel(JOB_ID);
    }
}
