package app.msime.android.home;

import android.app.job.JobParameters;
import android.app.job.JobService;
import android.content.Context;
import app.msime.android.MainThreadPolicy;
import app.msime.android.RerankModelDownload;
import app.msime.android.ResourcePackService;
import app.msime.android.ResourcePacks;
import app.msime.android.ThreadPolicy;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import org.json.JSONObject;

/**
 * 整句候选「增强」的逐键重排模型的后台下载任务，由 {@link RerankModelDownload} 排在不按流量计费的网络上，跑在主进程。
 *
 * <p>任务开始时重读实时偏好再判断一次：排任务之后用户切回了「标准」、模型已经装好，或者设置页正在下它，都什么也不做。下载与设置页共用 {@link ResourcePackService} 记的状态，页面上看得到进度，也能取消；不进前台、不发通知。连不上和服务器出错交给 JobScheduler 按退避重试，没下完的部分下次用 HTTP Range 接着下；校验不过等其他失败不重试，设置页那一行显示原因和「重试」。
 */
public final class RerankModelJobService extends JobService {
    private volatile Thread worker;

    @Override public boolean onStartJob(JobParameters params) {
        Context context = getApplicationContext();
        Thread thread = ThreadPolicy.namedThread("msime-rerank-model-job", () -> jobFinished(params, run(context)));
        worker = thread;
        thread.start();
        return true;
    }

    /** 系统停下任务（网络换成了按流量计费、电量低等）：取消下载，留着已下的部分，条件满足时再跑。 */
    @Override public boolean onStopJob(JobParameters params) {
        ResourcePacks.cancel(ResourcePacks.RERANK_MODEL);
        Thread running = worker;
        if (running != null) running.interrupt();
        return true;
    }

    /** 在任务的工作线程上下载；返回是否要 JobScheduler 稍后重试。 */
    private static boolean run(Context context) {
        String pack = ResourcePacks.RERANK_MODEL;
        JSONObject snapshot = HostStore.prepared(context) ? HostStore.loadPreferences(context) : null;
        if (!RerankModelDownload.wanted(snapshot == null ? null : snapshot.optJSONObject("preferences"))) return false;
        ResourcePacks.Pack listed = ResourcePacks.pack(context.getFilesDir(), pack);
        if (listed == null || listed.installed()) return false;
        // 设置页那边的状态只在主线程上改：先在主线程上占住，正在下（设置页刚点了下载）或正在卸载时不下。
        AtomicBoolean claimed = new AtomicBoolean();
        CountDownLatch latch = new CountDownLatch(1);
        MainThreadPolicy.mainHandler().post(() -> {
            claimed.set(ResourcePackService.claimInBackground(pack));
            latch.countDown();
        });
        try {
            if (!latch.await(10, TimeUnit.SECONDS) || !claimed.get()) return false;
        } catch (InterruptedException interrupted) {
            return true;
        }
        try {
            ResourcePacks.install(context, pack, ResourcePackService.mirrors(context),
                (phase, done, total) -> ResourcePackService.backgroundProgress(pack, phase, done, total));
            ResourcePackService.backgroundFinished(pack, null, null);
            return false;
        } catch (ResourcePacks.Failure failure) {
            ResourcePackService.backgroundFinished(pack, failure.code(), failure.getMessage());
            // 系统停下任务时 onStopJob 已经要求重排，这里不再要求；连不上和服务器出错按退避重试，其余（校验不过、空间不够等）等用户在设置页点重试。
            return "local_model_network".equals(failure.code()) || "local_model_http_status".equals(failure.code());
        } catch (RuntimeException | LinkageError error) {
            ResourcePackService.backgroundFinished(pack, "", null);
            return false;
        }
    }
}
