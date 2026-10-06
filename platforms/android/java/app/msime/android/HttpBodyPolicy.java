package app.msime.android;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;

/** Shared bounded reader for HTTP response bodies. */
public final class HttpBodyPolicy {
    private HttpBodyPolicy() {}

    /** Reads at most {@code limit} bytes, returning {@code null} when the body is larger. */
    public static byte[] readBounded(InputStream input, int limit) throws IOException {
        return read(input, limit, false, 0);
    }

    /**
     * 与 {@link #readBounded} 相同，但读完之前已经过了 {@code deadlineNanos}（{@link System#nanoTime()} 的时刻）也返回 null。
     *
     * <p>HttpURLConnection 的 readTimeout 只限制两次读之间的空闲时间，服务端每隔不到时限发一小段就永远不会超时，所以整体时限要在这里另算。
     */
    public static byte[] readWithin(InputStream input, int limit, long deadlineNanos) throws IOException {
        return read(input, limit, true, deadlineNanos);
    }

    private static byte[] read(InputStream input, int limit, boolean timed, long deadlineNanos)
            throws IOException {
        if (input == null || limit < 0) return null;
        ByteArrayOutputStream output = new ByteArrayOutputStream(Math.min(limit, 8192));
        byte[] buffer = new byte[8192];
        int count;
        while ((count = input.read(buffer)) != -1) {
            if (timed && System.nanoTime() - deadlineNanos > 0) return null;
            if (output.size() + count > limit) return null;
            output.write(buffer, 0, count);
        }
        return output.toByteArray();
    }
}
