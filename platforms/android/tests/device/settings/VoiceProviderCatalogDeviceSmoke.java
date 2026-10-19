package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 经安装产品的 JNI 读取真实共享目录，并用合成响应核对 Android 的严格解析与名称更新。 */
public final class VoiceProviderCatalogDeviceSmoke extends Instrumentation {
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            runCases();
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED\n");
            finish(Activity.RESULT_OK, result);
        } catch (Throwable error) {
            Throwable cause = error.getCause() == null ? error : error.getCause();
            result.putString("stream", "MSIME_VOICE_CATALOG_FAILED: " + cause.getClass().getSimpleName() + "\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }

    private void runCases() throws Exception {
        ClassLoader loader = getTargetContext().getClassLoader();
        Class<?> client = loader.loadClass("app.msime.android.NativeClient");
        Class<?> type = loader.loadClass("app.msime.android.VoiceProviderCatalog");
        String raw = (String) client.getMethod("voiceProviderCatalog").invoke(null);
        JSONObject envelope = new JSONObject(raw);
        check(Boolean.TRUE.equals(envelope.opt("ok")), "共享目录成功信封");
        JSONObject value = envelope.getJSONObject("value");
        Method parse = type.getMethod("parse", String.class);
        Object catalog = parse.invoke(null, raw);
        check(Boolean.TRUE.equals(type.getMethod("available").invoke(catalog)), "真实目录可用");
        compareProviders(type, catalog, "asr", value.getJSONArray("asr"), 2);
        compareProviders(type, catalog, "polish", value.getJSONArray("polish"), 0);
        check("豆包".equals(type.getMethod("asrLabel", String.class).invoke(catalog, "doubao")),
            "JNI 的中文名称保持 UTF-8");
        check("双向流式".equals(type.getMethod("doubaoEndpointLabel", String.class).invoke(catalog, "")),
            "留空取共享默认接口");
        List<?> endpoints = (List<?>) type.getMethod("doubaoEndpoints").invoke(catalog);
        JSONArray expected = value.getJSONArray("doubao_stream_endpoints");
        check(endpoints.size() == expected.length(), "流式接口数量来自共享目录");
        for (int index = 0; index < endpoints.size(); index++) {
            Object entry = endpoints.get(index);
            for (String field : new String[] {"id", "label", "title", "endpoint"})
                check(expected.getJSONObject(index).getString(field).equals(entry.getClass().getMethod(field).invoke(entry)),
                    "流式接口字段保持共享值");
        }

        JSONObject renamed = new JSONObject(raw);
        renamed.getJSONObject("value").getJSONArray("asr").getJSONObject(0).put("title", "合成新名称🍃");
        Object updated = parse.invoke(null, renamed.toString());
        check("合成新名称🍃".equals(type.getMethod("asrLabel", String.class).invoke(updated, "doubao")),
            "目录名称变化不要求修改 Java 名称表");

        reject(parse, new JSONObject(raw).put("ok", "true"));
        JSONObject wrongId = new JSONObject(raw);
        wrongId.getJSONObject("value").getJSONArray("asr").getJSONObject(0).put("id", 17);
        reject(parse, wrongId);
        JSONObject duplicate = new JSONObject(raw);
        JSONArray asr = duplicate.getJSONObject("value").getJSONArray("asr");
        asr.put(asr.getJSONObject(0));
        reject(parse, duplicate);
        JSONObject empty = new JSONObject(raw);
        empty.getJSONObject("value").put("polish", new JSONArray());
        reject(parse, empty);
        JSONObject missing = new JSONObject(raw);
        missing.getJSONObject("value").remove("doubao_stream_endpoints");
        reject(parse, missing);
        JSONObject wrongDefault = new JSONObject(raw);
        wrongDefault.getJSONObject("value").getJSONArray("asr").getJSONObject(0)
            .put("endpoint", "wss://example.invalid/not-a-preset");
        reject(parse, wrongDefault);
        JSONObject wrongLabel = new JSONObject(raw);
        wrongLabel.getJSONObject("value").getJSONArray("doubao_stream_endpoints").getJSONObject(0).put("label", true);
        reject(parse, wrongLabel);
    }

    private static void compareProviders(Class<?> type, Object catalog, String service, JSONArray expected,
            int platformCount) throws Exception {
        List<?> entries = (List<?>) type.getMethod(service).invoke(catalog);
        check(entries.size() == expected.length() + platformCount, "平台只追加自己的服务");
        for (int index = 0; index < expected.length(); index++) {
            Object entry = entries.get(index + platformCount);
            for (String field : new String[] {"id", "title"})
                check(expected.getJSONObject(index).getString(field).equals(entry.getClass().getMethod(field).invoke(entry)),
                    "名称与顺序保持共享值");
        }
    }

    private static void reject(Method parse, JSONObject value) throws Exception {
        try {
            parse.invoke(null, value.toString());
            throw new AssertionError("损坏目录不能变成可选列表");
        } catch (InvocationTargetException error) {
            check(error.getCause() instanceof JSONException, "损坏目录返回固定解析失败");
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
