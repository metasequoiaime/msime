package app.msime.android.nativetest;

import java.io.ByteArrayOutputStream;
import java.lang.ref.WeakReference;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Map;

/** `JniCallbacksTest.cpp` 在桌面 JVM 上驱动的 Java 一侧：一份内存里的会话存储（方法与 NativeClient.SessionStore 相同）和两个 listener，记下每次调用发生在哪条线程上。 */
public final class JniCallbacksFixture {
    private static WeakReference<Object> tracked = new WeakReference<>(null);

    private final Map<Integer, byte[]> items = new HashMap<>();
    /** 为 true 时 load、save、remove 都抛异常。 */
    public volatile boolean failing;
    /** save 写入以后返回的值。 */
    public volatile int saveResult;
    /** 三个存储方法一共被调用了几次。 */
    public volatile int calls;
    /** 最近一次存储方法所在线程的名字。 */
    public volatile String lastThread = "";
    /** 收到的全部增量连起来。 */
    private final ByteArrayOutputStream deltas = new ByteArrayOutputStream();
    public volatile int deltaCalls;
    /** 第几次 onDelta 抛异常（从 1 数起），0 表示不抛。 */
    public volatile int throwOnDelta;
    public volatile int progress;

    /** 为 true 时 load 交出一份新拷贝、save 不留下收到的数组，并用 {@link #track} 记下这两个数组，测试据此看 JNI 一侧有没有留着它们的局部引用。 */
    public volatile boolean trackArrays;

    public synchronized byte[] load(int item) {
        record();
        byte[] value = items.get(item);
        if (!trackArrays || value == null) return value;
        byte[] copy = value.clone();
        track(copy);
        return copy;
    }

    public synchronized int save(int item, byte[] bytes) {
        record();
        items.put(item, bytes.clone());
        if (trackArrays) track(bytes);
        return saveResult;
    }

    public synchronized int remove(int item) {
        record();
        items.remove(item);
        return 0;
    }

    public synchronized void onDelta(byte[] text) {
        deltaCalls++;
        if (deltaCalls == throwOnDelta) throw new IllegalStateException("合成的 listener 失败");
        deltas.write(text, 0, text.length);
    }

    public void onProgress(int completed) {
        progress = completed;
    }

    public synchronized String deltaText() {
        return new String(deltas.toByteArray(), StandardCharsets.UTF_8);
    }

    private void record() {
        calls++;
        lastThread = Thread.currentThread().getName();
        if (failing) throw new IllegalStateException("合成的存储失败");
    }

    /** 记下一个对象，之后用 {@link #collected} 看它是否还被引用着。 */
    public static void track(Object value) {
        tracked = new WeakReference<>(value);
    }

    /** 反复 GC，记下的对象被回收了就返回 true。 */
    public static boolean collected() throws InterruptedException {
        for (int attempt = 0; attempt < 20; attempt++) {
            System.gc();
            if (tracked.get() == null) return true;
            Thread.sleep(10);
        }
        return false;
    }
}
