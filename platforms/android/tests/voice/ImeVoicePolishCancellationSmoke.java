package app.msime.android;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.lang.reflect.Field;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLStreamHandler;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.TimeUnit;

/** 核对键盘取消会中断润色并释放语音队列；只替换传输层，不访问麦克风或网络。 */
public final class ImeVoicePolishCancellationSmoke {
    public static void main(String[] arguments) throws Exception {
        BlockingConnection connection = new BlockingConnection();
        URL.setURLStreamHandlerFactory(protocol -> "https".equals(protocol) ? new URLStreamHandler() {
            @Override protected java.net.URLConnection openConnection(URL url) { return connection; }
        } : null);
        ImeVoiceEntry entry = new ImeVoiceEntry(null);
        ExecutorService worker = (ExecutorService) field(entry, "worker");
        VoicePolisher previous = null;
        try {
            previous = ownedPolisher(entry);
            VoicePolisher running = previous;
            var request = worker.submit(() -> polish(running));
            check(connection.reading.await(2, TimeUnit.SECONDS), "润色应已开始等待响应");
            var queued = worker.submit(() -> polish(running));

            entry.cancel();
            check(connection.disconnected.getCount() == 0, "取消键盘语音必须中断活动润色连接");
            check(request.get(2, TimeUnit.SECONDS) == null, "取消的润色不能返回文字");
            // 下一次录音也使用这一条队列，旧润色不能继续占住它。
            check(worker.submit(() -> true).get(2, TimeUnit.SECONDS), "取消后语音队列应可继续执行");
            int opened = connection.uploads;
            check(queued.get(2, TimeUnit.SECONDS) == null && opened == 1,
                "取消发生在任务执行前时，旧润色器不能再上传");

            VoicePolisher next = ownedPolisher(entry);
            check(next != previous, "下一次识别必须使用未取消的润色器");
            check(worker.submit(() -> polish(next)).get(2, TimeUnit.SECONDS) == null
                    && connection.uploads == opened + 1,
                "后续识别仍能发起润色；服务失败时保留原转写");
            entry.cancel();
        } finally {
            if (previous != null) previous.cancel();
            worker.shutdownNow();
            check(worker.awaitTermination(2, TimeUnit.SECONDS), "测试结束后工作线程必须退出");
        }
        System.out.println("Android keyboard voice polish cancellation passed");
    }

    private static String polish(VoicePolisher polisher) {
        return polisher.polish("https://synthetic.invalid/v1/chat/completions", "synthetic-model",
            "synthetic-token", "synthetic-prompt", "synthetic-transcript");
    }

    private static VoicePolisher ownedPolisher(ImeVoiceEntry entry) throws Exception {
        for (Field candidate : ImeVoiceEntry.class.getDeclaredFields()) {
            if (candidate.getType() == VoicePolisher.class) {
                candidate.setAccessible(true);
                return (VoicePolisher) candidate.get(entry);
            }
        }
        throw new AssertionError("键盘语音必须持有可取消的润色器");
    }

    private static Object field(Object target, String name) throws Exception {
        Field field = target.getClass().getDeclaredField(name);
        field.setAccessible(true);
        return field.get(target);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static final class BlockingConnection extends HttpURLConnection {
        final CountDownLatch reading = new CountDownLatch(1);
        final CountDownLatch disconnected = new CountDownLatch(1);
        volatile int uploads;

        BlockingConnection() throws IOException { super(new URL("https://synthetic.invalid")); }

        @Override public OutputStream getOutputStream() {
            uploads++;
            return new ByteArrayOutputStream();
        }

        @Override public int getResponseCode() throws IOException {
            reading.countDown();
            try {
                if (!disconnected.await(5, TimeUnit.SECONDS)) throw new IOException("synthetic timeout");
            } catch (InterruptedException error) {
                Thread.currentThread().interrupt();
                throw new IOException("synthetic interruption", error);
            }
            return 503;
        }

        @Override public void disconnect() { disconnected.countDown(); }
        @Override public void connect() { }
        @Override public boolean usingProxy() { return false; }
    }
}
