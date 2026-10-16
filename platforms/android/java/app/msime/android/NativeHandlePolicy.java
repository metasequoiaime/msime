package app.msime.android;

/** Java 只传递 JNI 登记的正整数身份；原生指针不跨过这层边界。 */
public final class NativeHandlePolicy {
    private NativeHandlePolicy() {}

    public static long requirePositive(long handle) {
        if (handle <= 0) throw new IllegalArgumentException("Invalid native handle");
        return handle;
    }

    public static boolean isOptional(long handle) {
        return handle >= 0;
    }
}
