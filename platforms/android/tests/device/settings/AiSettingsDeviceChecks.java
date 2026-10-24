package app.msime.android.test;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.graphics.Rect;
import android.os.SystemClock;
import android.view.View;
import android.view.ViewGroup;
import android.view.accessibility.AccessibilityNodeInfo;
import android.widget.TextView;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 原生 AI 页与真实 JNI 的设备回归；只用合成凭据和回环服务，最后恢复原 AI 设置。 */
public abstract class AiSettingsDeviceChecks extends DeviceSmoke {
    private Activity activity;
    private Object page;
    private Class<?> pageType;
    private Class<?> storeType;
    private Class<?> settingsType;

    @Override protected String successDescription() { return "AI presets, native model selection, origin credentials and stale model replies"; }

    @Override protected void runChecks() throws Exception {
        ClassLoader loader = getTargetContext().getClassLoader();
        settingsType = loader.loadClass("app.msime.android.AiProviderSettings");
        storeType = loader.loadClass("app.msime.android.home.HostStore");
        pageType = loader.loadClass("app.msime.android.home.AiSettingsPage");
        stage = "AI preset JNI and strict catalog parsing";
        contracts(loader);
        JSONObject original = new JSONObject(ai().toString());
        try {
            JSONObject seed = new JSONObject(original.toString()).put("enabled", false).put("provider", "deepseek")
                .put("endpoint", "").put("model", "")
                .put("endpoints", new JSONObject()).put("models", new JSONObject()).put("tokens", new JSONObject()
                    .put("https://api.openai.com:443", "synthetic-openai")
                    .put("https://fixture.invalid:443", "synthetic-other"));
            writeAi(seed);
            activity = startActivitySync(new Intent().setClassName(getTargetContext(), "app.msime.android.home.HomeActivity")
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_MULTIPLE_TASK)
                .putExtra("msime_open_page", "AI_SETTINGS"));
            awaitText("服务商");
            long deadline = SystemClock.uptimeMillis() + 10000;
            while (page == null && SystemClock.uptimeMillis() < deadline) {
                onMain(() -> {
                    Object manager = activity.getClass().getMethod("getSupportFragmentManager").invoke(activity);
                    for (Object fragment : (List<?>) manager.getClass().getMethod("getFragments").invoke(manager))
                        if (pageType.isInstance(fragment)) page = fragment;
                });
                if (page == null) SystemClock.sleep(100);
            }
            check(page != null, "真实导航创建 AI 页");

            stage = "native provider selection fills empty defaults";
            clickPage("服务商");
            tap(option("DeepSeek"));
            awaitAi("endpoint", "https://api.deepseek.com/chat/completions");
            check(!ai().getString("model").isEmpty(), "再次选择默认服务补齐模型");
            String deepseekModel = ai().getString("model");
            clickPage("服务商");
            tap(option("OpenAI"));
            awaitAi("endpoint", "https://api.openai.com/v1/chat/completions");
            awaitAi("model", "gpt-4.1-mini");
            check("synthetic-openai".equals(ai().getJSONObject("tokens").getString("https://api.openai.com:443")),
                "切换没有搬走或删除 origin 凭据");
            clickPage("模型");
            tap(option("自定义…"));
            setInput("synthetic-custom-model");
            tap(option("保存"));
            awaitAi("model", "synthetic-custom-model");
            stage = "native provider selection restores each provider model";
            clickPage("服务商");
            tap(option("DeepSeek"));
            awaitAi("provider", "deepseek");
            awaitAi("endpoint", "https://api.deepseek.com/chat/completions");
            awaitAi("model", deepseekModel);
            clickPage("服务商");
            tap(option("OpenAI"));
            awaitAi("provider", "openai");
            awaitAi("endpoint", "https://api.openai.com/v1/chat/completions");
            awaitAi("model", "synthetic-custom-model");

            stage = "fetch and select models through the native product page";
            try (ModelsServer server = new ModelsServer(false, false)) {
                configureLoopback(server);
                clickPage("获取模型列表");
                server.awaitRequest();
                awaitText("已获取 2 个模型");
                clickPage("模型");
                tap(option("synthetic-catalog-b"));
                awaitAi("model", "synthetic-catalog-b");
                check("synthetic-other".equals(ai().getJSONObject("tokens").getString("https://fixture.invalid:443")),
                    "选择目录模型保留其他 origin 的凭据");
            }

            stage = "provider changes discard a delayed model catalog";
            try (ModelsServer server = new ModelsServer(true, false)) {
                configureLoopback(server);
                clickPage("获取模型列表");
                server.awaitRequest();
                clickPage("服务商");
                tap(option("OpenAI"));
                awaitAi("provider", "openai");
                server.release();
                awaitNotBusy();
                checkNoFetchedModels();
                awaitAi("model", "synthetic-custom-model");
                check("synthetic-current".equals(ai().getJSONObject("models").getString("custom")),
                    "切换后原服务的模型留在自己的槽位");
            }

            stage = "credential edits discard a delayed model catalog";
            try (ModelsServer server = new ModelsServer(true, false)) {
                configureLoopback(server);
                clickPage("获取模型列表");
                server.awaitRequest();
                clickPage("凭据");
                setInput("synthetic-updated-model-key");
                tap(option("保存"));
                long credentialDeadline = SystemClock.uptimeMillis() + 10000;
                while (!"synthetic-updated-model-key".equals(ai().getJSONObject("tokens").optString(server.origin()))
                        && SystemClock.uptimeMillis() < credentialDeadline) SystemClock.sleep(100);
                check("synthetic-updated-model-key".equals(ai().getJSONObject("tokens").optString(server.origin())),
                    "原生凭据控件只写当前 origin");
                server.release();
                awaitNotBusy();
                checkNoFetchedModels();
            }

            stage = "external endpoint edits discard a delayed model catalog";
            try (ModelsServer server = new ModelsServer(true, false)) {
                configureLoopback(server);
                clickPage("获取模型列表");
                server.awaitRequest();
                String endpoint = "http://127.0.0.1:1/v1/chat/completions";
                writeAi(ai().put("endpoint", endpoint));
                server.release();
                awaitNotBusy();
                checkNoFetchedModels();
                awaitAi("endpoint", endpoint);
            }

            stage = "unavailable catalog preserves custom model and manual choice";
            try (ModelsServer server = new ModelsServer(false, true)) {
                configureLoopback(server);
                clickPage("获取模型列表");
                server.awaitRequest();
                awaitText("获取失败，可使用预设或手动填写模型名称");
                awaitAi("model", "synthetic-current");
                clickPage("模型");
                tap(option("synthetic-current"));
                awaitAi("model", "synthetic-current");
            }
        } finally {
            if (activity != null) runOnMainSync(activity::finish);
            writeAi(original);
        }
    }

    private void contracts(ClassLoader loader) throws Exception {
        Class<?> nativeType = loader.loadClass("app.msime.android.NativeClient");
        String raw = (String) nativeType.getMethod("aiProviderCatalog").invoke(null);
        Method parse = settingsType.getMethod("parseCatalog", String.class);
        List<?> catalog = (List<?>) parse.invoke(null, raw);
        JSONArray catalogEntries = new JSONObject(raw).getJSONArray("value");
        Set<String> required = Set.of("everyapi", "openai", "anthropic", "gemini", "deepseek", "qwen",
            "kimi", "zhipu", "minimax", "volcengine", "siliconflow", "groq", "openrouter", "ollama", "custom");
        Set<String> actual = new HashSet<>();
        for (int index = 0; index < catalogEntries.length(); index++) actual.add(catalogEntries.getJSONObject(index).getString("id"));
        check(actual.containsAll(required), "完整共享目录包含既有全部服务");
        JSONObject wrong = new JSONObject(raw).put("ok", "true");
        reject(parse, wrong);
        JSONObject duplicate = new JSONObject(raw);
        JSONArray entries = duplicate.getJSONArray("value");
        entries.put(entries.getJSONObject(0));
        reject(parse, duplicate);
        JSONObject typed = new JSONObject(raw);
        typed.getJSONArray("value").getJSONObject(0).put("desktop_only", "false");
        reject(parse, typed);
        JSONObject ai = new JSONObject().put("enabled", true).put("provider", "deepseek")
            .put("endpoint", "https://fixture.invalid/custom").put("model", "synthetic-custom")
            .put("tokens", new JSONObject().put("https://fixture.invalid:443", "合成凭据🍃"))
            .put("prompt_custom_2", "合成提示词");
        // 每个服务商各记一份接口地址和模型（#9250）：切走时现值写进原服务商的槽位，目标服务商没有槽位就用它的预设，切回来按槽位原样恢复。
        JSONObject openai = null;
        for (int index = 0; index < catalogEntries.length(); index++)
            if ("openai".equals(catalogEntries.getJSONObject(index).getString("id"))) openai = catalogEntries.getJSONObject(index);
        check(openai != null, "共享目录包含 openai");
        Method switchProvider = settingsType.getMethod("switchProvider", JSONObject.class, String.class);
        switchProvider.invoke(null, ai, "openai");
        check(openai.getString("endpoint").equals(ai.getString("endpoint")), "切到没有槽位的服务商时用它的预设地址");
        check(openai.getString("model").equals(ai.getString("model")), "切到没有槽位的服务商时用它的预设模型");
        check("https://fixture.invalid/custom".equals(ai.getJSONObject("endpoints").getString("deepseek")), "自定义地址存进原服务商的槽位");
        check("synthetic-custom".equals(ai.getJSONObject("models").getString("deepseek")), "自定义模型存进原服务商的槽位");
        switchProvider.invoke(null, ai, "deepseek");
        check("https://fixture.invalid/custom".equals(ai.getString("endpoint")), "切回时恢复自定义地址");
        check("synthetic-custom".equals(ai.getString("model")), "切回时恢复自定义模型");
        check("合成凭据🍃".equals(ai.getJSONObject("tokens").getString("https://fixture.invalid:443")), "合成 origin 凭据不变");
        check("合成提示词".equals(ai.getString("prompt_custom_2")), "切换保留提示词槽");
    }

    private static void reject(Method parse, JSONObject response) throws Exception {
        try {
            parse.invoke(null, response.toString());
            throw new AssertionError("损坏目录必须拒绝");
        } catch (InvocationTargetException error) {
            check(error.getCause() instanceof JSONException, "目录失败按固定解析错误返回");
        }
    }

    private void configureLoopback(ModelsServer server) throws Exception {
        JSONObject next = ai().put("enabled", false).put("provider", "custom")
            .put("endpoint", server.endpoint()).put("model", "synthetic-current");
        next.getJSONObject("tokens").put(server.origin(), "synthetic-model-key");
        writeAi(next);
        onMain(() -> {
            Method reload = pageType.getDeclaredMethod("reload");
            reload.setAccessible(true);
            reload.invoke(page);
        });
        awaitText("synthetic-current");
        long deadline = SystemClock.uptimeMillis() + 10000;
        boolean[] matched = {false};
        do {
            onMain(() -> {
                var field = pageType.getDeclaredField("fetchedFor");
                field.setAccessible(true);
                Object request = field.get(page);
                if (request != null) {
                    Method endpoint = request.getClass().getDeclaredMethod("endpoint");
                    endpoint.setAccessible(true);
                    matched[0] = server.endpoint().equals(endpoint.invoke(request));
                }
            });
            if (!matched[0]) SystemClock.sleep(100);
        } while (!matched[0] && SystemClock.uptimeMillis() < deadline);
        check(matched[0], "页面已读取本次回环服务地址");
        waitForIdleSync();
    }

    private JSONObject ai() throws Exception {
        JSONObject snapshot = (JSONObject) storeType.getMethod("loadPreferences", Context.class).invoke(null, getTargetContext());
        check(snapshot != null, "产品共享偏好可读");
        return snapshot.getJSONObject("preferences").getJSONObject("ai_assistant");
    }

    private void writeAi(JSONObject ai) throws Exception {
        JSONObject snapshot = (JSONObject) storeType.getMethod("loadPreferences", Context.class).invoke(null, getTargetContext());
        snapshot.getJSONObject("preferences").put("ai_assistant", ai);
        check(storeType.getMethod("savePreferences", Context.class, JSONObject.class).invoke(null, getTargetContext(), snapshot) != null,
            "合成 AI 设置经产品共享存储保存");
    }

    private void awaitAi(String key, String expected) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 10000;
        while (!expected.equals(ai().optString(key)) && SystemClock.uptimeMillis() < deadline) SystemClock.sleep(100);
        check(expected.equals(ai().optString(key)), "原生控件已保存预期字段：" + key);
        waitForIdleSync();
    }

    private void checkNoFetchedModels() throws Exception {
        onMain(() -> {
            var field = pageType.getDeclaredField("fetchedModels");
            field.setAccessible(true);
            check(((List<?>) field.get(page)).isEmpty(), "过期目录没有进入当前服务的模型列表");
        });
    }

    private void awaitNotBusy() throws Exception {
        long deadline = SystemClock.uptimeMillis() + 12000;
        boolean[] busy = {true};
        do {
            onMain(() -> {
                var field = pageType.getDeclaredField("modelsBusy");
                field.setAccessible(true);
                busy[0] = field.getBoolean(page);
            });
            if (busy[0]) SystemClock.sleep(100);
        } while (busy[0] && SystemClock.uptimeMillis() < deadline);
        check(!busy[0], "模型请求正常收尾");
    }

    /** 将原生设置行滚入视口，再通过真实无障碍点击路径操作。 */
    private void clickPage(String label) throws Exception {
        awaitText(label);
        onMain(() -> {
            TextView view = find(activity.getWindow().getDecorView(), label);
            check(view != null, "原生设置行存在：" + label);
            view.requestRectangleOnScreen(new Rect(0, 0, view.getWidth(), view.getHeight()), true);
        });
        tap(option(label));
    }

    private void setInput(String value) {
        AccessibilityNodeInfo field = awaitAny(node -> node.isVisibleToUser() && node.isEditable());
        android.os.Bundle args = new android.os.Bundle();
        args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, value);
        check(field.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args), "合成模型输入可编辑");
    }

    private void awaitText(String label) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 12000;
        boolean[] found = {false};
        do {
            onMain(() -> found[0] = activity != null && find(activity.getWindow().getDecorView(), label) != null);
            if (!found[0]) SystemClock.sleep(100);
        } while (!found[0] && SystemClock.uptimeMillis() < deadline);
        check(found[0], "原生页出现预期文案：" + label);
    }

    private static Predicate<AccessibilityNodeInfo> option(String label) {
        return node -> node.isVisibleToUser() && (label.contentEquals(node.getText() == null ? "" : node.getText())
            || label.contentEquals(node.getContentDescription() == null ? "" : node.getContentDescription()));
    }

    private static TextView find(View root, String label) {
        if (!root.isShown()) return null;
        if (root instanceof TextView view && label.contentEquals(view.getText())) return view;
        if (root instanceof ViewGroup group) for (int index = 0; index < group.getChildCount(); index++) {
            TextView value = find(group.getChildAt(index), label);
            if (value != null) return value;
        }
        return null;
    }

    @FunctionalInterface private interface Checked { void run() throws Exception; }
    private void onMain(Checked action) throws Exception {
        AtomicReference<Throwable> failure = new AtomicReference<>();
        runOnMainSync(() -> { try { action.run(); } catch (Throwable error) { failure.set(error); } });
        if (failure.get() instanceof Exception error) throw error;
        if (failure.get() instanceof Error error) throw error;
    }

    private static void check(boolean condition, String message) { if (!condition) throw new AssertionError(message); }

    /** 标准模型目录的回环夹具；不输出请求或凭据，关闭时取消等待并检查工作线程。 */
    private static final class ModelsServer implements java.io.Closeable {
        private final ServerSocket listener;
        private final CountDownLatch received = new CountDownLatch(1);
        private final CountDownLatch allowed = new CountDownLatch(1);
        private final AtomicReference<Throwable> failure = new AtomicReference<>();
        private final Thread worker;

        ModelsServer(boolean delayed, boolean unavailable) throws Exception {
            listener = new ServerSocket(0, 1, InetAddress.getByName("127.0.0.1"));
            listener.setSoTimeout(12000);
            if (!delayed) allowed.countDown();
            worker = new Thread(() -> {
                try (Socket socket = listener.accept()) {
                    socket.setSoTimeout(12000);
                    BufferedReader reader = new BufferedReader(new InputStreamReader(socket.getInputStream(), StandardCharsets.UTF_8));
                    check("GET /v1/models HTTP/1.1".equals(reader.readLine()), "共享模型目录请求路径");
                    boolean authorized = false;
                    for (String line; (line = reader.readLine()) != null && !line.isEmpty();) {
                        if ("authorization: Bearer synthetic-model-key".equalsIgnoreCase(line)) authorized = true;
                    }
                    check(authorized, "模型请求只使用当前回环 origin 的合成凭据");
                    received.countDown();
                    check(allowed.await(10, TimeUnit.SECONDS), "延迟模型响应按时释放");
                    String body = unavailable ? "{}" : "{\"data\":[{\"id\":\"synthetic-catalog-a\"},{\"id\":\"synthetic-catalog-b\"}]}";
                    String response = "HTTP/1.1 " + (unavailable ? "503 Unavailable" : "200 OK")
                        + "\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: "
                        + body.getBytes(StandardCharsets.UTF_8).length + "\r\n\r\n" + body;
                    socket.getOutputStream().write(response.getBytes(StandardCharsets.UTF_8));
                    socket.getOutputStream().flush();
                } catch (Throwable error) { failure.set(error); received.countDown(); }
            }, "synthetic-ai-models");
            worker.start();
        }
        String origin() { return "http://127.0.0.1:" + listener.getLocalPort(); }
        String endpoint() { return origin() + "/v1/chat/completions"; }
        void awaitRequest() throws Exception {
            check(received.await(12, TimeUnit.SECONDS) && failure.get() == null, "回环服务收到合法模型请求");
        }
        void release() { allowed.countDown(); }
        @Override public void close() throws java.io.IOException {
            release();
            listener.close();
            try {
                worker.join(2000);
            } catch (InterruptedException error) {
                Thread.currentThread().interrupt();
                throw new java.io.IOException("synthetic_server_interrupted", error);
            }
            check(!worker.isAlive(), "回环服务线程退出");
            if (failure.get() != null) throw new AssertionError("回环服务夹具失败", failure.get());
        }
    }
}
