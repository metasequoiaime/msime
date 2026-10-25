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
        Class<?> policy = loader.loadClass("app.msime.android.VoiceProviderPolicy");
        Method credential = policy.getMethod("validCredential", String.class, String.class);
        check((Boolean) credential.invoke(null, "openai", "") && (Boolean) credential.invoke(null, "openai", "密钥"),
            "共享 JNI 接受空值和 Unicode 凭据");
        check(!(Boolean) credential.invoke(null, "openai", "key\nvalue")
                && !(Boolean) credential.invoke(null, "doubao", "密钥")
                && !(Boolean) credential.invoke(null, "openai", "x".repeat(16 * 1024 + 1)),
            "共享 JNI 拒绝控制字符、豆包非 ASCII 和超长凭据");
        Method network = policy.getMethod("network", String.class);
        check((Boolean) network.invoke(null, "openai") && (Boolean) network.invoke(null, "doubao")
                && !(Boolean) network.invoke(null, "system") && !(Boolean) network.invoke(null, "local")
                && !(Boolean) network.invoke(null, "") && !(Boolean) network.invoke(null, (Object) null),
            "共享 JNI 统一服务是否联网的判断");
        Method legacyAuth = policy.getMethod("legacyAuth", String.class);
        check((Boolean) legacyAuth.invoke(null, "legacy") && (Boolean) legacyAuth.invoke(null, " legacy ")
                && !(Boolean) legacyAuth.invoke(null, "") && !(Boolean) legacyAuth.invoke(null, "api_key")
                && !(Boolean) legacyAuth.invoke(null, (Object) null),
            "共享 JNI 统一豆包旧版鉴权模式的判断");
        Method trim = loader.loadClass("app.msime.android.TextPolicy")
            .getMethod("trimUnicodeWhitespace", String.class);
        Method hasToken = loader.loadClass("app.msime.android.home.VoicePage")
            .getDeclaredMethod("hasToken", String.class, JSONObject.class, String.class);
        hasToken.setAccessible(true);
        for (String whitespace : new String[] {"\u0085", "\u00a0", "\u202f", "\u3000"}) {
            check((Boolean) credential.invoke(null, "doubao", whitespace), "共享预检允许空白删除");
            String normalized = (String) trim.invoke(null, whitespace);
            check(normalized.isEmpty(), "保存时使用与 Rust 相同的空白裁剪");
            check(!(Boolean) hasToken.invoke(null, normalized, null, "openai"), "空白当前凭据显示为未设置");
            check(!(Boolean) hasToken.invoke(null, "", new JSONObject().put("openai", whitespace), "openai"),
                "旧空白槽位显示为未设置");
            String wrapped = whitespace + "synthetic-key" + whitespace;
            check((Boolean) credential.invoke(null, "doubao", wrapped)
                    && "synthetic-key".equals(trim.invoke(null, wrapped)), "豆包预检与保存裁剪一致");
        }
        Method status = policy.getMethod("status", String.class, String.class, boolean.class);
        check("密钥未填写完整，暂用系统识别".equals(status.invoke(null, "doubao", null, true)),
            "共享 JNI 返回云端服务的回退状态");
        check("没有可用的系统识别服务，可改用本地模型或豆包".equals(
                status.invoke(null, "system", null, false)),
            "共享 JNI 返回系统识别不可用状态");
        check("使用设备自带的语音识别服务".equals(status.invoke(null, null, null, true))
                && "使用设备自带的语音识别服务".equals(status.invoke(null, "", null, true)),
            "未选择服务时空 UTF-8 请求仍按系统识别解释");
        Method localStatus = policy.getMethod("localStatus", boolean.class, boolean.class);
        check("还需准备本地语音运行库，请在下方「本地识别准备」下载或导入".equals(
                localStatus.invoke(null, true, false)),
            "共享 JNI 返回本地运行库状态");
        check(localStatus.invoke(null, true, true) == null
                && status.invoke(null, "openai", "openai", false) == null
                && status.invoke(null, "local", "local", true) == null,
            "已就绪的配置无需状态提示");
        check("还没有选用可用模型，请在下方「本地模型」选用、下载或导入".equals(
                localStatus.invoke(null, false, false)),
            "缺少模型时优先提示模型");
        Class<?> polishPolicy = loader.loadClass("app.msime.android.VoicePolishSettingsPolicy");
        Method polishStatus = polishPolicy.getMethod("status", boolean.class, boolean.class, boolean.class);
        check(polishStatus.invoke(null, false, false, false) == null,
            "关闭润色时没有状态提示");
        check(polishStatus.invoke(null, true, true, true) == null,
            "已配置的润色没有状态提示");
        check("未填写 API Key，识别结果不会润色".equals(
                polishStatus.invoke(null, true, false, false)),
            "共享 JNI 返回缺少润色凭据状态");
        check("配置不完整，识别结果不会润色".equals(
                polishStatus.invoke(null, true, false, true)),
            "共享 JNI 返回润色配置不完整状态");
        Method statusParse = policy.getDeclaredMethod("parseStatus", String.class);
        statusParse.setAccessible(true);
        check("合成状态".equals(statusParse.invoke(null,
                "{\"ok\":true,\"value\":{\"status\":\"合成状态\"}}")),
            "状态内容来自共享信封");
        for (String invalid : new String[] {"", "{}",
                "{\"ok\":\"true\",\"value\":{\"status\":\"合成状态\"}}",
                "{\"ok\":false,\"value\":{\"status\":\"合成状态\"}}",
                "{\"ok\":true,\"value\":{}}",
                "{\"ok\":true,\"value\":{\"status\":17}}",
                "{\"ok\":true,\"value\":{\"status\":\"\"}}"}) {
            check(statusParse.invoke(null, invalid) == null, "损坏状态信封不展示猜测文案");
        }
        Method policyParse = policy.getDeclaredMethod("parsePolicyField", String.class, String.class);
        policyParse.setAccessible(true);
        String policyResponse = "{\"ok\":true,\"value\":{\"network\":true,\"legacy_auth\":true}}";
        check((Boolean) policyParse.invoke(null, policyResponse, "network")
                && (Boolean) policyParse.invoke(null, policyResponse, "legacy_auth"),
            "服务策略严格解析共享成功信封");
        check(!(Boolean) policyParse.invoke(null,
                "{\"ok\":true,\"value\":{\"network\":\"true\"}}", "network")
                && !(Boolean) policyParse.invoke(null, "{\"ok\":false}", "network"),
            "损坏服务策略不产生真值");
        Method credentialParse = policy.getDeclaredMethod("credentialAccepted", String.class);
        credentialParse.setAccessible(true);
        check((Boolean) credentialParse.invoke(null, "{\"ok\":true,\"value\":{\"accepted\":true}}"),
            "严格成功信封允许保存");
        check(!(Boolean) credentialParse.invoke(null, "{\"ok\":true,\"value\":{\"accepted\":\"true\"}}"),
            "字符串布尔值不允许保存");
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
