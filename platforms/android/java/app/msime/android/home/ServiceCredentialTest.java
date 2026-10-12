package app.msime.android.home;

import androidx.annotation.Nullable;
import app.msime.android.JsonPolicy;
import app.msime.android.NativeClient;
import org.json.JSONException;
import org.json.JSONObject;

/** Shared response parsing and failure handling for credential test requests. */
final class ServiceCredentialTest {
    private static final String FAILURE = "测试没能完成，请稍后重试。";

    record Result(boolean ok, String message) {}

    private ServiceCredentialTest() {}

    static Result voice(String preferencesDirectory, String service) {
        try {
            JSONObject request = new JSONObject()
                .put("preferences_directory", preferencesDirectory)
                .put("service", service);
            JSONObject value = new JSONObject(NativeClient.testVoiceCredential(request.toString()))
                .optJSONObject("value");
            if (value == null) return failed();
            return new Result(value.optBoolean("ok", false), value.optString("message", ""));
        } catch (JSONException | RuntimeException | LinkageError error) {
            return failed();
        }
    }

    static Result api(String service, JSONObject config) {
        try {
            JSONObject request = new JSONObject().put("service", service).put("config", config);
            JSONObject root = new JSONObject(NativeClient.testApiCredential(request.toString()));
            JSONObject value = JsonPolicy.objectValue(root);
            if (value == null) return failed();
            return new Result(JsonPolicy.strictBoolean(value.opt("ok"), false),
                JsonPolicy.strictStringOrEmpty(value.opt("message")));
        } catch (JSONException | RuntimeException | LinkageError error) {
            return failed();
        }
    }

    private static Result failed() {
        return new Result(false, FAILURE);
    }
}
