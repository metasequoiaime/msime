package app.msime.android;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.List;

public final class BackendAccountResponseSmoke {
    public static void main(String[] args) throws Exception {
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
        check(rejected, "chat responses without string content are rejected");
        rejected = false;
        try {
            BackendAccount.requiredStringField(42);
        } catch (IllegalStateException error) {
            rejected = true;
        }
        check(rejected, "chat responses with numeric content are rejected");
        check("synthetic reply".equals(BackendAccount.requiredStringField("synthetic reply")),
            "chat responses with string content are accepted");

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

        check(BackendAccount.validChatModels(List.of(
                new BackendAccount.ChatModel("synthetic-model")), "synthetic-model"),
            "model catalog accepts a valid default");
        check(!BackendAccount.validChatModels(List.of(
                new BackendAccount.ChatModel("synthetic-model"),
                new BackendAccount.ChatModel("synthetic-model")), "synthetic-model"),
            "model catalog rejects duplicate ids");
        check(!BackendAccount.validChatModels(List.of(
                new BackendAccount.ChatModel("你".repeat(100))), "你".repeat(100)),
            "model catalog applies UTF-8 byte bounds");
        check(!BackendAccount.validChatModels(List.of(
                new BackendAccount.ChatModel("bad\u0000model")), "bad\u0000model"),
            "model catalog rejects control characters");

        List<BackendAccount.ChatMessage> messages = new java.util.ArrayList<>();
        messages.add(new BackendAccount.ChatMessage("system", "system prompt"));
        for (int index = 1; index < 16; index++) {
            messages.add(new BackendAccount.ChatMessage(index % 2 == 0 ? "assistant" : "user",
                "你".repeat(1_000)));
        }
        check(BackendAccount.validChatRequest(messages, "synthetic-model"),
            "chat accepts the shared sixteen-message and UTF-8 bounds");
        check(!BackendAccount.validChatRequest(List.of(
                new BackendAccount.ChatMessage("user", "bad\u0000text")), "synthetic-model"),
            "chat request rejects disallowed controls");
        check(!BackendAccount.validChatRequest(List.of(
                new BackendAccount.ChatMessage("user", "valid")), "bad\u0000model"),
            "chat request rejects control characters in the model");
        check(!BackendAccount.validChatResponse("user", "synthetic reply"),
            "chat response requires an assistant role");
        check(!BackendAccount.validChatResponse("assistant", "bad\u0000reply"),
            "chat response rejects disallowed controls");

        check(!BackendAccount.validClipboardItem(new BackendAccount.ClipboardItem(
            "a".repeat(64), "safe\u0000hidden", "2026-10-04T00:00:00Z")),
            "add clipboard rejects control characters in the returned text");

        System.out.println("Android account response bounds and fields passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
