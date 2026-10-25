import app.msime.android.OfflineVoiceSession;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.atomic.AtomicInteger;

/** 覆盖录音开始、完成解码、取消及迟到回调共用的一次性请求状态。 */
public final class OfflineVoiceSessionSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) throws InterruptedException {
        OfflineVoiceSession completed = new OfflineVoiceSession();
        check(completed.start() && completed.listening(), "请求只开始一次采集");
        check(!completed.start(), "重复恢复窗口不重复打开麦克风");
        check(completed.stop() && !completed.listening(), "完成先停止采集，等待解码");
        check(!completed.stop(), "解码期间再次点完成无效");
        check(completed.finish() && completed.finished(), "成功结束一次请求");
        check(!completed.finish() && !completed.start(), "结束后不再次返回或开始");

        OfflineVoiceSession denied = new OfflineVoiceSession();
        check(denied.finish() && !denied.start(), "授权前取消也阻止后续启动");

        OfflineVoiceSession racing = new OfflineVoiceSession();
        racing.start();
        CountDownLatch ready = new CountDownLatch(2);
        CountDownLatch release = new CountDownLatch(1);
        AtomicInteger deliveries = new AtomicInteger();
        Runnable callback = () -> {
            ready.countDown();
            try {
                release.await();
                if (racing.finish()) deliveries.incrementAndGet();
            } catch (InterruptedException error) {
                Thread.currentThread().interrupt();
                throw new AssertionError(error);
            }
        };
        Thread success = new Thread(callback);
        Thread cancel = new Thread(callback);
        success.start();
        cancel.start();
        ready.await();
        release.countDown();
        success.join();
        cancel.join();
        check(deliveries.get() == 1, "完成与取消竞争只能返回一次");
        System.out.println("OfflineVoiceSessionSmoke passed");
    }
}
