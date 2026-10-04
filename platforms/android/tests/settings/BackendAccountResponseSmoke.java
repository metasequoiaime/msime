package app.msime.android;

import java.io.ByteArrayInputStream;
import java.io.InputStream;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;

public final class BackendAccountResponseSmoke {
    public static void main(String[] args) throws Exception {
        Method readBounded = BackendAccount.class.getDeclaredMethod("readBounded", InputStream.class);
        readBounded.setAccessible(true);

        // A valid cloud-clipboard page can contain fifty four-thousand-unit entries. Keep this
        // fixture synthetic and below the shared one-megabyte JSON response bound.
        StringBuilder document = new StringBuilder("{\"enabled\":true,\"items\":[");
        for (int index = 0; index < 50; index++) {
            if (index > 0) document.append(',');
            document.append("{\"id\":\"")
                .append(String.format("%064x", index + 1L))
                .append("\",\"text\":\"")
                .append("字".repeat(4_000))
                .append("\",\"updated_at\":\"2026-10-04T00:00:00Z\"}");
        }
        document.append("]}");
        byte[] page = document.toString().getBytes(StandardCharsets.UTF_8);
        try {
            byte[] response = (byte[]) readBounded.invoke(null, new ByteArrayInputStream(page));
            check(response.length == page.length,
                "a valid account response below the shared one-megabyte bound is preserved");
        } catch (InvocationTargetException error) {
            throw new AssertionError("a valid account response below the shared one-megabyte bound is rejected", error.getCause());
        }

        byte[] oversized = new byte[1024 * 1024 + 1];
        boolean rejected = false;
        try {
            readBounded.invoke(null, new ByteArrayInputStream(oversized));
        } catch (InvocationTargetException error) {
            rejected = error.getCause() instanceof IllegalStateException;
        }
        check(rejected, "responses above the shared one-megabyte bound are rejected");

        rejected = false;
        try {
            BackendAccount.requiredBooleanField(null);
        } catch (IllegalStateException error) {
            rejected = true;
        }
        check(rejected, "clipboard responses without a boolean enabled field are rejected");

        rejected = false;
        try {
            BackendAccount.requiredBooleanField("false");
        } catch (IllegalStateException error) {
            rejected = true;
        }
        check(rejected, "clipboard responses with a non-boolean enabled field are rejected");
        check(BackendAccount.requiredBooleanField(Boolean.TRUE), "enabled=true is accepted");
        check(!BackendAccount.requiredBooleanField(Boolean.FALSE), "enabled=false is accepted");

        check(!BackendAccount.validClipboardItem(new BackendAccount.ClipboardItem(
            "a".repeat(64), "safe\u0000hidden", "2026-10-04T00:00:00Z")),
            "add clipboard rejects control characters in the returned text");

        System.out.println("Android account response bounds and fields passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
