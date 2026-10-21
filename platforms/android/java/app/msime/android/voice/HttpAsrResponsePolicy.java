package app.msime.android;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 读取整句语音服务回复的状态，不依赖录音或网络。 */
public final class HttpAsrResponsePolicy {
    private HttpAsrResponsePolicy() {}

    /** 明确失败或未完成的 JSON 回复不能作为转写正文。 */
    public static boolean responseAllowed(String response) {
        try {
            JSONObject document = new JSONObject(response);
            Object error = document.opt("error");
            boolean hasError = error != null && error != JSONObject.NULL;
            JSONArray choices = document.optJSONArray("choices");
            JSONObject choice = choices == null ? null : choices.optJSONObject(0);
            String finishReason = choice == null || !choice.has("finish_reason")
                || choice.isNull("finish_reason") ? null : choice.optString("finish_reason");
            return !hasError && !"length".equals(finishReason)
                && !"content_filter".equals(finishReason);
        } catch (JSONException error) {
            return false;
        }
    }
}
