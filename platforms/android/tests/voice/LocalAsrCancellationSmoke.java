import app.msime.android.LocalAsrRecognizer;

/** 尚未准备模型时取消不应触及原生句柄，也不能再校验或打开录音资源。 */
public final class LocalAsrCancellationSmoke {
    public static void main(String[] args) throws Exception {
        LocalAsrRecognizer recognizer = new LocalAsrRecognizer();
        try {
            recognizer.cancel();
        } catch (LinkageError error) {
            throw new AssertionError("创建原生会话之前取消不应调用 JNI", error);
        }
        recognizer.diagnostics(kind -> {
            throw new AssertionError("取消后的请求仍进入资源准备：" + kind);
        });
        try {
            recognizer.recognize(null, null, null, "", null, null, null);
            throw new AssertionError("取消后不应开始识别");
        } catch (LocalAsrRecognizer.Refused refused) {
            if (refused.failure() != LocalAsrRecognizer.Failure.CANCELLED) {
                throw new AssertionError("取消必须优先于模型或运行库检查", refused);
            }
        }
        LocalAsrRecognizer preparing = new LocalAsrRecognizer();
        java.util.concurrent.CountDownLatch entered = new java.util.concurrent.CountDownLatch(1);
        java.util.concurrent.CountDownLatch resume = new java.util.concurrent.CountDownLatch(1);
        java.util.concurrent.atomic.AtomicReference<Throwable> failure = new java.util.concurrent.atomic.AtomicReference<>();
        preparing.diagnostics(kind -> {
            entered.countDown();
            try {
                if (!resume.await(5, java.util.concurrent.TimeUnit.SECONDS)) throw new AssertionError("准备屏障超时");
            } catch (InterruptedException error) {
                Thread.currentThread().interrupt();
                throw new AssertionError(error);
            }
        });
        Thread worker = new Thread(() -> {
            try {
                preparing.recognize(null, null, null, "", null, null, null);
                failure.set(new AssertionError("准备期间取消后仍开始识别"));
            } catch (LocalAsrRecognizer.Refused refused) {
                if (refused.failure() != LocalAsrRecognizer.Failure.CANCELLED) failure.set(refused);
            } catch (Throwable error) {
                failure.set(error);
            }
        });
        try {
            worker.start();
            if (!entered.await(5, java.util.concurrent.TimeUnit.SECONDS)) throw new AssertionError("未进入资源准备");
            preparing.cancel();
        } finally {
            resume.countDown();
            worker.join(5000);
        }
        if (worker.isAlive()) throw new AssertionError("取消后准备线程没有退出");
        if (failure.get() != null) throw new AssertionError("准备阶段没有优先处理取消", failure.get());
        System.out.println("LocalAsrCancellationSmoke passed");
    }
}
