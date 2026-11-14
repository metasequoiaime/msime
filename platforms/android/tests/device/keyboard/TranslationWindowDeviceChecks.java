package app.msime.android.test;

import android.content.Intent;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.ResultReceiver;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.net.ServerSocket;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.concurrent.atomic.AtomicReference;
import org.json.JSONObject;

/** 回环合成服务验证真正的窗口编辑、共享请求、预览、撤销旧译文与手动交付，不访问外部服务。 */
final class TranslationWindowDeviceChecks {
    private TranslationWindowDeviceChecks() {}

    static void run(DeviceSmoke smoke) throws Exception {
        File options = new File(smoke.getTargetContext().getFilesDir(), "runtime-options.json");
        String directory = new JSONObject(new String(Files.readAllBytes(options.toPath()), StandardCharsets.UTF_8))
            .getString("preferences_directory");
        File preferences = new File(directory, "preferences.json");
        byte[] original = Files.readAllBytes(preferences.toPath());
        AtomicReference<String> delivered = new AtomicReference<>();
        AtomicReference<Throwable> failure = new AtomicReference<>();
        AtomicReference<JSONObject> requested = new AtomicReference<>();
        android.app.Activity activity = null;
        try (ServerSocket server = new ServerSocket(0, 1, java.net.InetAddress.getByName("127.0.0.1"))) {
            server.setSoTimeout(60000);
            Thread response = new Thread(() -> {
                try {
                  int requests = android.os.Build.VERSION.SDK_INT >= 35 ? 2 : 1;
                  for (int index = 0; index < requests; index++) try (var socket = server.accept()) {
                    socket.setSoTimeout(10000);
                    var reader = new java.io.BufferedReader(new java.io.InputStreamReader(socket.getInputStream(), StandardCharsets.UTF_8));
                    int length = 0;
                    String line;
                    while ((line = reader.readLine()) != null && !line.isEmpty()) {
                        if (line.toLowerCase(java.util.Locale.ROOT).startsWith("content-length:"))
                            length = Integer.parseInt(line.substring(15).trim());
                    }
                    char[] body = new char[length];
                    int read = 0;
                    while (read < length) {
                        int count = reader.read(body, read, length - read);
                        if (count < 0) throw new java.io.IOException("合成请求正文缺失");
                        read += count;
                    }
                    requested.set(new JSONObject(new String(body)));
                    byte[] answer = "{\"data\":\"synthetic translation\"}".getBytes(StandardCharsets.UTF_8);
                    var output = socket.getOutputStream();
                    output.write(("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: "
                        + answer.length + "\r\nConnection: close\r\n\r\n").getBytes(StandardCharsets.US_ASCII));
                    output.write(answer);
                    output.flush();
                  }
                } catch (Throwable error) { failure.set(error); }
            }, "translation-synthetic-server");
            response.start();
            JSONObject snapshot = new JSONObject(new String(original, StandardCharsets.UTF_8));
            JSONObject config = snapshot.getJSONObject("preferences");
            config.put("candidate_translations", false).put("learning", false).put("cloud_candidates", false)
                .put("translation_account", false).put("translation_ai", false)
                .put("niutrans", new JSONObject().put("enabled", false))
                .put("aliyun_translation", new JSONObject().put("enabled", false))
                .put("tencent_tmt", new JSONObject().put("enabled", false))
                .put("custom_translation", new JSONObject().put("enabled", true)
                    .put("endpoint", "http://127.0.0.1:" + server.getLocalPort()).put("api_key", ""));
            if (config.optJSONObject("ai_assistant") != null) config.getJSONObject("ai_assistant").put("enabled", false);
            Files.write(preferences.toPath(), snapshot.toString().getBytes(StandardCharsets.UTF_8));
            ResultReceiver receiver = new ResultReceiver(new Handler(Looper.getMainLooper())) {
                @Override protected void onReceiveResult(int code, Bundle data) {
                    if (code == android.app.Activity.RESULT_OK && data != null)
                        delivered.set(data.getString("translation_text"));
                }
            };
            smoke.stage = "translation window opens";
            Intent intent = new Intent().setClassName(smoke.getTargetContext().getPackageName(), "app.msime.android.TranslationActivity")
                .putExtra("translation_receiver", receiver).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            activity = smoke.startActivitySync(intent);
            smoke.stage = "typed source remains in translation window";
            AccessibilityNodeInfo source = smoke.awaitAny(smoke.described("翻译原文"));
            Bundle text = new Bundle();
            text.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, "synthetic source");
            if (!source.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, text)) throw new AssertionError("原文不可编辑");
            smoke.stage = "translation actions remain above visible keyboard";
            smoke.tap(smoke.described("翻译原文"));
            android.graphics.Rect keyboard = new android.graphics.Rect();
            smoke.awaitAny(smoke.key("更多")).getBoundsInScreen(keyboard);
            for (String title : new String[] {"复制译文", "插入"}) {
                android.graphics.Rect bounds = new android.graphics.Rect();
                smoke.awaitAny(node -> DeviceSmoke.equalsText(title, node.getText())).getBoundsInScreen(bounds);
                if (bounds.isEmpty() || bounds.bottom > keyboard.top)
                    throw new AssertionError("翻译操作被输入法遮住");
            }
            smoke.tap(node -> DeviceSmoke.equalsText("翻译", node.getText()) && node.isClickable());
            smoke.stage = "translation preview uses shared request";
            smoke.awaitAny(node -> DeviceSmoke.equalsText("synthetic translation", node.getText()));
            if (failure.get() != null || requested.get() == null)
                throw new AssertionError("合成服务未完成");
            JSONObject body = requested.get();
            if (!body.getString("source_lang").equals("ZH") || !body.getString("target_lang").equals("EN")
                    || !body.getString("text").equals("synthetic source") || delivered.get() != null)
                throw new AssertionError("请求源语言错误或预览时提前上屏");
            smoke.stage = "manual translation insertion";
            smoke.tap(node -> DeviceSmoke.equalsText("插入", node.getText()) && node.isClickable());
            long deadline = android.os.SystemClock.uptimeMillis() + 5000;
            while (delivered.get() == null && android.os.SystemClock.uptimeMillis() < deadline) android.os.SystemClock.sleep(50);
            if (!"synthetic translation".equals(delivered.get())) throw new AssertionError("手动插入未交付译文");
            smoke.stage = "reverse translation descriptor";
            ClassLoader loader = smoke.getTargetContext().getClassLoader();
            Class<?> backend = loader.loadClass("app.msime.android.BackendTranslationClient");
            var rust = backend.getDeclaredMethod("rust", String.class);
            rust.setAccessible(true);
            Object gloss = rust.invoke(null, "en");
            var request = gloss.getClass().getDeclaredMethod("request", java.util.List.class, String.class);
            request.setAccessible(true);
            Object plan = request.invoke(gloss, java.util.List.of("synthetic source"), "zh");
            var bytes = plan.getClass().getDeclaredMethod("body"); bytes.setAccessible(true);
            JSONObject reverse = new JSONObject(new String((byte[]) bytes.invoke(plan), StandardCharsets.UTF_8));
            if (!reverse.getString("source_lang").equals("EN") || !reverse.getString("target_lang").equals("ZH"))
                throw new AssertionError("账号反向翻译源语言仍固定为中文");
            // API 28 的功能面板不提供完整无障碍树；API 35 实走原编辑器与输入法之间的返回路径。
            if (android.os.Build.VERSION.SDK_INT >= 35) {
                smoke.stage = "translation entry from real IME menu";
                smoke.restartIme();
                smoke.shellDiscard("am start -W -n app.msime.android.test/.EditorActivity");
                smoke.tap(smoke.field("msime-test-plain"));
                AccessibilityNodeInfo editor = smoke.await(smoke.field("msime-test-plain"));
                text.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, "synthetic chat");
                if (!editor.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, text))
                    throw new AssertionError("合成聊天框不可编辑");
                smoke.tap(smoke.key("更多"));
                smoke.showTool("翻译");
                smoke.tap(smoke.tool("翻译"));
                AccessibilityNodeInfo input = smoke.awaitAny(smoke.described("翻译原文"));
                text.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, "synthetic source");
                if (!input.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, text))
                    throw new AssertionError("工具入口的原文不可编辑");
                smoke.tap(node -> DeviceSmoke.equalsText("翻译", node.getText()) && node.isClickable());
                smoke.awaitAny(node -> DeviceSmoke.equalsText("synthetic translation", node.getText()));
                smoke.stage = "manual translation returns to original editor";
                smoke.tap(node -> DeviceSmoke.equalsText("插入", node.getText()) && node.isClickable());
                smoke.await(smoke.field("msime-test-plain").and(node ->
                    DeviceSmoke.equalsText("synthetic chatsynthetic translation", node.getText())));
            }
            response.join(10000);
            if (response.isAlive() || failure.get() != null) throw new AssertionError("合成服务未完成");
        } finally {
            if (activity != null) {
                android.app.Activity launched = activity;
                smoke.runOnMainSync(launched::finish);
            }
            Files.write(preferences.toPath(), original);
        }
    }
}
