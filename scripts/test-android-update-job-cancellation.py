#!/usr/bin/env python3
"""执行自动更新任务的真实启动/停止方法，检查下载取消不会逃出工作线程。"""

from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT / "platforms/android/java/app/msime/android"


def main():
    if not shutil.which("javac") or not shutil.which("java"):
        print("skipped: Android 更新任务取消回归需要 JDK")
        return
    source = (JAVA / "home/UpdateJobService.java").read_text(encoding="utf-8")
    start = source.index("    @Override public boolean onStartJob(")
    end = source.index("    static SharedPreferences store(", start)
    methods = source[start:end].replace("@Override ", "")
    # 仅替换平台和网络边界；启动、停止、下载回调及异常处理均取自产品源码。
    harness = r'''
import app.msime.android.ThreadPolicy;
import java.io.File;
import java.util.concurrent.CancellationException;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

public final class UpdateJobCancellationSmoke {
    private volatile Thread worker;
    private int notifications;
    private int completions;
    private boolean retry;
    private static final CountDownLatch downloading = new CountDownLatch(1);
    private static final CountDownLatch stopped = new CountDownLatch(1);
    private static String mode;

    static final class JobParameters {}
    static final class Context { File getCacheDir() { return null; } }
    static final class AppEdition {
        static AppEdition current() { return new AppEdition(); }
        String id() { return "full"; }
    }
    static final class FilePolicy { static void deleteQuietly(File file) {} }
    static final class UpdateApi {
        static final class Failure extends Exception {
            Failure(Throwable cause) { super(cause); }
        }
        record Release(String version) {}
        record Update(Release release) {}
        interface Progress { void onProgress(long done, long total); }
        Update check(Object channel, String edition, String version) throws Failure {
            if (mode.equals("network")) throw new Failure(new java.io.IOException());
            return new Update(new Release("1.2.3"));
        }
        File download(Update update, File cache, Progress progress) {
            if (mode.equals("cancel")) {
                downloading.countDown();
                boolean interrupted = false;
                while (stopped.getCount() != 0) {
                    try { stopped.await(); }
                    catch (InterruptedException expected) { interrupted = true; }
                }
                if (interrupted) Thread.currentThread().interrupt();
                progress.onProgress(1, 2);
            }
            return new File("synthetic-update.apk");
        }
        static void verifyArchive(Context context, File apk) throws Failure {}
    }
    Context getApplicationContext() { return new Context(); }
    static boolean autoUpdate(Context context) { return true; }
    static boolean updatedByPlay(Context context) { return false; }
    static Object channel(Context context) { return null; }
    static String currentVersion(Context context) { return "1.0.0"; }
    void notifyReady(Context context, String version, File apk) { notifications++; }
    void jobFinished(JobParameters params, boolean needsRetry) { completions++; retry = needsRetry; }

__METHODS__

    public static void main(String[] args) throws Exception {
        AtomicReference<Throwable> escaped = new AtomicReference<>();
        Thread.setDefaultUncaughtExceptionHandler((thread, failure) -> {
            if (thread.getName().equals("msime-update-job")) escaped.set(failure);
            else failure.printStackTrace();
        });
        for (String scenario : new String[] {"success", "network", "cancel"}) {
            mode = scenario;
            UpdateJobCancellationSmoke service = new UpdateJobCancellationSmoke();
            JobParameters params = new JobParameters();
            if (!service.onStartJob(params)) throw new AssertionError("worker did not start");
            if (scenario.equals("cancel")) {
                if (!downloading.await(5, TimeUnit.SECONDS)) throw new AssertionError("download did not start");
                if (!service.onStopJob(params)) throw new AssertionError("stopped job should reschedule");
                stopped.countDown();
            }
            service.worker.join(5000);
            if (service.worker.isAlive()) throw new AssertionError("worker did not finish");
            if (escaped.get() != null) throw new AssertionError("exception escaped update worker", escaped.get());
            if (service.completions != 1) throw new AssertionError("job completion missing");
            if (service.notifications != (scenario.equals("success") ? 1 : 0))
                throw new AssertionError("cancelled/failed update must not notify");
            if (service.retry != scenario.equals("network")) throw new AssertionError("retry policy changed");
        }
        System.out.println("Android 更新任务：成功、网络失败和下载取消均通过");
    }
}
'''.replace("__METHODS__", methods)
    with tempfile.TemporaryDirectory(prefix="msime-update-job-") as temporary:
        target = Path(temporary)
        program = target / "UpdateJobCancellationSmoke.java"
        program.write_text(harness, encoding="utf-8")
        subprocess.run(["javac", "-d", str(target), str(JAVA / "ThreadPolicy.java"), str(program)], check=True)
        subprocess.run(["java", "-cp", str(target), "UpdateJobCancellationSmoke"], check=True, timeout=20)


if __name__ == "__main__":
    main()
