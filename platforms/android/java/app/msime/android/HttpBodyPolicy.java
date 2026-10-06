package app.msime.android;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.function.BooleanSupplier;

/** Shared bounded reader for HTTP response bodies. */
public final class HttpBodyPolicy {
    private static final int COPY_BUFFER_BYTES = 16 * 1024;

    private HttpBodyPolicy() {}

    /** Copy a stream without allocating a buffer for every response. */
    public static void copy(InputStream input, OutputStream output) throws IOException {
        if (input == null || output == null) throw new NullPointerException();
        byte[] buffer = new byte[COPY_BUFFER_BYTES];
        int count;
        while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
    }

    /** Reads at most {@code limit} bytes, returning {@code null} when the body is larger. */
    public static byte[] readBounded(InputStream input, int limit) throws IOException {
        return readBounded(input, limit, () -> false);
    }

    /** Reads a bounded body and turns an over-limit response into an I/O failure. */
    public static byte[] readRequired(InputStream input, int limit) throws IOException {
        byte[] body = readBounded(input, limit);
        if (body == null) throw new IOException("response too large");
        return body;
    }

    /** Reads a bounded body while allowing a caller to stop between input chunks. */
    public static byte[] readBounded(InputStream input, int limit,
            BooleanSupplier cancelled) throws IOException {
        if (input == null || limit < 0) return null;
        if (cancelled == null || cancelled.getAsBoolean()) return null;
        ByteArrayOutputStream output = new ByteArrayOutputStream(BoundsPolicy.atMost(limit, 8192));
        byte[] buffer = new byte[8192];
        int count;
        while ((count = input.read(buffer)) != -1) {
            if (cancelled.getAsBoolean()) return null;
            if (output.size() + count > limit) return null;
            output.write(buffer, 0, count);
        }
        return output.toByteArray();
    }

    /** Reads a bounded body until the monotonic deadline, returning null after it expires. */
    public static byte[] readWithin(InputStream input, int limit, long deadlineNanos)
            throws IOException {
        if (input == null || limit < 0) return null;
        ByteArrayOutputStream output = new ByteArrayOutputStream(BoundsPolicy.atMost(limit, 8192));
        byte[] buffer = new byte[8192];
        int count;
        while (System.nanoTime() < deadlineNanos && (count = input.read(buffer)) != -1) {
            if (System.nanoTime() >= deadlineNanos || output.size() + count > limit) return null;
            output.write(buffer, 0, count);
        }
        return System.nanoTime() < deadlineNanos ? output.toByteArray() : null;
    }
}
