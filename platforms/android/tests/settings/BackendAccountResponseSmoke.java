package app.msime.android;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;

public final class BackendAccountResponseSmoke {
    public static void main(String[] args) throws Exception {
        // Rust 的 providers 交回的开关经 JsonPolicy.strictTrue 读（BackendAccount#providers），字符串 "true" 不算开。
        check(JsonPolicy.strictTrue(Boolean.TRUE), "boolean provider flag is enabled");
        check(!JsonPolicy.strictTrue("true"),
            "string provider flag is not enabled");
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
            byte[] response = HttpBodyPolicy.readRequired(new ByteArrayInputStream(page), 1024 * 1024);
            check(response.length == page.length,
                "a valid account response below the shared one-megabyte bound is preserved");
        } catch (IOException error) {
            throw new AssertionError("a valid account response below the shared one-megabyte bound is rejected", error);
        }

        byte[] oversized = new byte[1024 * 1024 + 1];
        boolean rejected = false;
        try {
            HttpBodyPolicy.readRequired(new ByteArrayInputStream(oversized), 1024 * 1024);
        } catch (IOException error) {
            rejected = true;
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

        rejected = false;
        try {
            BackendAccount.requiredStringField(null);
        } catch (IllegalStateException error) {
            rejected = true;
        }
        check(rejected, "clipboard responses without a string field are rejected");
        rejected = false;
        try {
            BackendAccount.requiredStringField(42);
        } catch (IllegalStateException error) {
            rejected = true;
        }
        check(rejected, "clipboard responses with a numeric string field are rejected");
        check("synthetic reply".equals(BackendAccount.requiredStringField("synthetic reply")),
            "clipboard responses with a string field are accepted");

        rejected = false;
        try {
            BackendAccount.optionalStringField(42, "fallback");
        } catch (IllegalStateException expected) {
            rejected = true;
        }
        check(rejected, "account responses with numeric string fields are rejected");
        check("fallback".equals(BackendAccount.optionalStringField(null, "fallback")),
            "missing optional account string uses its fallback");
        check("synthetic".equals(BackendAccount.optionalStringField("synthetic", "fallback")),
            "string account response is accepted");

        check(!BackendAccount.validClipboardItem(new BackendAccount.ClipboardItem(
            "a".repeat(64), "safe\u0000hidden", "2026-10-04T00:00:00Z")),
            "add clipboard rejects control characters in the returned text");
        check(!BackendAccount.validClipboardItem(new BackendAccount.ClipboardItem(
            "a".repeat(64), "safe", "2026-10-04\uD800")),
            "clipboard metadata rejects malformed Unicode");

        check(BackendAccount.validClipboardSearch("a".repeat(1024)),
            "clipboard search accepts the shared byte limit");
        check(BackendAccount.validClipboardSearch("你".repeat(341)),
            "clipboard search counts UTF-8 bytes at the boundary");
        check(!BackendAccount.validClipboardSearch("你".repeat(342)),
            "clipboard search rejects text above the shared UTF-8 byte limit");

        System.out.println("Android account response bounds and fields passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
