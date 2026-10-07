package app.msime.android;

import java.io.ByteArrayInputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;

/** 试用页 AI 对话的流式回复：SSE 按行读、data 字段的取法，以及单行和整个响应的上限。 */
public final class BackendAccountEventStreamSmoke {
    public static void main(String[] args) throws Exception {
        // Multi-byte characters must survive a read that splits them, so feed the stream one byte at a time.
        String body = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"水杉\"},\"finish_reason\":null}]}\r\n\r\n"
            + ": keep-alive\n"
            + "event: message\n"
            + "data:[DONE]\n\n";
        BackendAccount.EventLines lines = new BackendAccount.EventLines(
            new TrickleStream(body.getBytes(StandardCharsets.UTF_8)));
        check(("data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"水杉\"},\"finish_reason\":null}]}")
            .equals(lines.next()), "CRLF lines drop the CR and keep split UTF-8 intact");
        check("".equals(lines.next()), "the blank line that ends an event is returned as an empty line");
        check(": keep-alive".equals(lines.next()), "comment lines are returned as they are");
        check("event: message".equals(lines.next()), "other fields are returned as they are");
        check("data:[DONE]".equals(lines.next()), "the [DONE] line is read");
        check("".equals(lines.next()), "the final blank line is read");
        check(lines.next() == null, "end of stream is reported as null");

        BackendAccount.EventLines unterminated = new BackendAccount.EventLines(
            new ByteArrayInputStream("data: tail".getBytes(StandardCharsets.UTF_8)));
        check("data: tail".equals(unterminated.next()), "a last line without a newline is still returned");
        check(unterminated.next() == null, "end of stream follows the unterminated line");

        check("{\"a\":1}".equals(BackendAccount.eventData("data: {\"a\":1}")), "one space after data: is dropped");
        check("[DONE]".equals(BackendAccount.eventData("data:[DONE]")), "data: without a space is accepted");
        check(" x".equals(BackendAccount.eventData("data:  x")), "only the first space after data: is dropped");
        check(BackendAccount.eventData(": keep-alive") == null, "comment lines carry no data");
        check(BackendAccount.eventData("") == null, "blank lines carry no data");
        check(BackendAccount.eventData("event: message") == null, "other fields carry no data");
        // 空的或不是对象的 data 行不交给 org.json，直接跳过（这里的 android.jar 桩一碰 JSONObject 就会抛 Stub!，所以这几条同时证明了没有去解析）。
        check(BackendAccount.eventObject("") == null, "an empty data line is skipped");
        check(BackendAccount.eventObject("   ") == null, "a blank data line is skipped");
        check(BackendAccount.eventObject("ping") == null, "a non-JSON data line is skipped");
        check(BackendAccount.eventObject("[1,2]") == null, "a non-object data line is skipped");

        byte[] longLine = new byte[BackendAccount.MAX_EVENT_LINE_BYTES + 1];
        java.util.Arrays.fill(longLine, (byte) 'a');
        check(rejects(new ByteArrayInputStream(longLine)), "a line above the per-line bound is rejected");

        check(rejects(new EndlessLines()), "a stream above the whole-response bound is rejected");

        System.out.println("Android account chat event stream passed");
    }

    private static boolean rejects(InputStream input) throws Exception {
        BackendAccount.EventLines lines = new BackendAccount.EventLines(input);
        try {
            while (lines.next() != null) { }
            return false;
        } catch (IllegalStateException expected) {
            return true;
        }
    }

    /** Hands out one byte per read. */
    private static final class TrickleStream extends InputStream {
        private final byte[] bytes;
        private int position;

        TrickleStream(byte[] bytes) {
            this.bytes = bytes;
        }

        @Override public int read() {
            return position < bytes.length ? bytes[position++] & 0xFF : -1;
        }

        @Override public int read(byte[] target, int offset, int length) {
            if (length == 0) return 0;
            if (position >= bytes.length) return -1;
            target[offset] = bytes[position++];
            return 1;
        }
    }

    /** Short lines without end, so only the whole-response bound can stop the reader. */
    private static final class EndlessLines extends InputStream {
        private long served;

        @Override public int read() {
            served++;
            return served % 64 == 0 ? '\n' : 'a';
        }

        @Override public int read(byte[] target, int offset, int length) {
            for (int index = 0; index < length; index++) target[offset + index] = (byte) read();
            return length;
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
