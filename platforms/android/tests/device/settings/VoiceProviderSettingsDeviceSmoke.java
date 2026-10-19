package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import org.json.JSONException;
import org.json.JSONObject;

/** 只用合成内存文档验证产品的语音服务切换与真实 JNI，不保存或发送凭据。 */
public final class VoiceProviderSettingsDeviceSmoke extends Instrumentation {
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            runCases();
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED\n");
            finish(Activity.RESULT_OK, result);
        } catch (Throwable error) {
            Throwable cause = error.getCause() == null ? error : error.getCause();
            result.putString("stream", "MSIME_VOICE_SETTINGS_FAILED: " + cause.getClass().getSimpleName() + "\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }

    private void runCases() throws Exception {
        ClassLoader loader = getTargetContext().getClassLoader();
        Class<?> type = loader.loadClass("app.msime.android.VoiceProviderSettings");
        Method change = type.getMethod("switchProvider", JSONObject.class, String.class, String.class);
        JSONObject voice = new JSONObject().put("asr_provider", "groq")
            .put("asr_token", "合成当前🍃").put("asr_endpoint", "https://example.invalid/synthetic-proxy")
            .put("asr_model", "synthetic-custom").put("asr_model_path", "synthetic-local-model")
            .put("asr_tokens", new JSONObject().put("openai", "合成目标🍃").put("invalid", false))
            .put("doubao_auth_mode", "legacy").put("polish_endpoint", "https://example.invalid/polish");
        change.invoke(null, voice, "asr", "openai");
        check("合成目标🍃".equals(voice.getString("asr_token")), "中文与补充平面字符经 JNI 恢复");
        check("合成当前🍃".equals(voice.getJSONObject("asr_tokens").getString("groq")), "当前凭据收进旧槽");
        check(!voice.getJSONObject("asr_tokens").has("invalid"), "沿用旧页面只保留字符串槽位的语义");
        check(voice.getString("asr_endpoint").isEmpty() && voice.getString("asr_model").isEmpty(), "清空自定义地址和模型");
        check("synthetic-local-model".equals(voice.getString("asr_model_path")), "已选本地模型不被清除");
        check("legacy".equals(voice.getString("doubao_auth_mode")), "豆包鉴权方式不被改写");
        check("https://example.invalid/polish".equals(voice.getString("polish_endpoint")), "另一类服务的字段保留");
        String unchanged = voice.toString();
        change.invoke(null, voice, "asr", "openai");
        check(unchanged.equals(voice.toString()), "同一服务不写补丁");
        voice.put("asr_token", "synthetic-openai-new");
        change.invoke(null, voice, "asr", "groq");
        check("合成当前🍃".equals(voice.getString("asr_token")), "切回恢复旧服务凭据");
        check("synthetic-openai-new".equals(voice.getJSONObject("asr_tokens").getString("openai")), "离开时以输入框为准");
        voice.put("asr_token", "");
        change.invoke(null, voice, "asr", "synthetic-future-provider");
        check(voice.getString("asr_token").isEmpty() && !voice.getJSONObject("asr_tokens").has("groq"), "空凭据删除旧槽，新服务不继承");
        for (String offline : new String[] {"system", "local"}) {
            JSONObject current = new JSONObject().put("asr_provider", "openai").put("asr_token", "synthetic-online")
                .put("asr_tokens", new JSONObject().put(offline, "synthetic-stale"));
            change.invoke(null, current, "asr", offline);
            check(current.getString("asr_token").isEmpty() && !current.getJSONObject("asr_tokens").has(offline), "离线服务不留凭据");
            check("synthetic-online".equals(current.getJSONObject("asr_tokens").getString("openai")), "离线切换保留在线服务槽");
        }
        JSONObject polish = new JSONObject().put("polish_provider", "openai").put("polish_token", "synthetic-polish")
            .put("polish_endpoint", "https://example.invalid/custom").put("polish_model", "synthetic-custom")
            .put("polish_tokens", new JSONObject().put("deepseek", "synthetic-deepseek"));
        change.invoke(null, polish, "polish", "deepseek");
        check("synthetic-deepseek".equals(polish.getString("polish_token")), "润色走同一共享换槽");
        check(polish.getString("polish_endpoint").isEmpty() && polish.getString("polish_model").isEmpty(), "润色清空策略相同");

        Method apply = type.getDeclaredMethod("applyPatch", JSONObject.class, String.class, String.class, String.class);
        apply.setAccessible(true);
        JSONObject patch = new JSONObject().put("provider", "openai").put("token", "synthetic-target")
            .put("tokens", new JSONObject()).put("endpoint", "").put("model", "");
        for (String field : new String[] {"provider", "token", "tokens", "endpoint", "model"}) {
            JSONObject invalid = new JSONObject(patch.toString());
            invalid.put(field, field.equals("provider") ? "wrong-provider" : true);
            rejectUnchanged(apply, voice, new JSONObject().put("ok", true).put("value", invalid).toString());
        }
        rejectUnchanged(apply, voice, new JSONObject().put("ok", "true").put("value", patch).toString());
        rejectUnchanged(apply, voice, new JSONObject().put("ok", true).put("value",
            new JSONObject(patch.toString()).put("tokens", new JSONObject().put("openai", 17))).toString());
        rejectUnchanged(apply, voice, new JSONObject().put("ok", true).put("value",
            new JSONObject(patch.toString()).put("endpoint", "https://example.invalid/not-cleared")).toString());
        Method bridge = loader.loadClass("app.msime.android.NativeClient").getMethod("voiceProviderUpdate", String.class);
        for (String oversized : new String[] {"x".repeat(1_048_577), "合".repeat(400_000)}) {
            try {
                bridge.invoke(null, oversized);
                throw new AssertionError("超限请求必须拒绝");
            } catch (InvocationTargetException error) {
                check(error.getCause() instanceof IllegalArgumentException, "请求在 JNI 前按 UTF-8 字节限制大小");
            }
        }
    }

    private static void rejectUnchanged(Method apply, JSONObject voice, String response) throws Exception {
        String before = voice.toString();
        try {
            apply.invoke(null, voice, "asr", "openai", response);
            throw new AssertionError("损坏补丁不能写入文档");
        } catch (InvocationTargetException error) {
            check(error.getCause() instanceof JSONException, "损坏补丁返回固定解析失败");
            check(before.equals(voice.toString()), "失败不部分改写当前文档");
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
