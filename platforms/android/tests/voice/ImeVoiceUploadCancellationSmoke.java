package app.msime.android;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.atomic.AtomicBoolean;

/** 核对整句上传识别与键盘的松手结束、取消共用生命周期，不打开麦克风或网络。 */
public final class ImeVoiceUploadCancellationSmoke {
    public static void main(String[] arguments) throws Exception {
        ImeVoiceEntry entry = new ImeVoiceEntry(null);
        ExecutorService worker = (ExecutorService) get(entry, "worker");
        try {
            HttpAsrRecognizer upload = new HttpAsrRecognizer();
            set(entry, "upload", upload);
            set(entry, "recognizing", true);
            Method finish = ImeVoiceEntry.class.getDeclaredMethod("finishListening");
            finish.setAccessible(true);
            finish.invoke(entry);
            check(((AtomicBoolean) get(upload, "stopped")).get(), "结束聆听必须停止上传识别的音频采集");
            check(!((AtomicBoolean) get(upload, "cancelled")).get(), "正常结束必须保留转写请求");
            entry.cancel();
            check(((AtomicBoolean) get(upload, "cancelled")).get(), "取消键盘语音必须取消上传");
            check(get(entry, "upload") == null, "取消后必须释放旧上传识别器");
        } finally {
            worker.shutdownNow();
        }
        System.out.println("Android keyboard upload voice lifecycle passed");
    }

    private static Object get(Object target, String name) throws Exception {
        Field field = target.getClass().getDeclaredField(name);
        field.setAccessible(true);
        return field.get(target);
    }

    private static void set(Object target, String name, Object value) throws Exception {
        Field field = target.getClass().getDeclaredField(name);
        field.setAccessible(true);
        field.set(target, value);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
