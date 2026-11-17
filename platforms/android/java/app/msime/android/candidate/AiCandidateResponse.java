package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 候选回包在保留切分元信息时交给 Rust，只有验证后的文字能进入会话。 */
final class AiCandidateResponse {
    private AiCandidateResponse() {}

    static List<String> parse(String body, String query, int limit) {
        if (!OnlineCandidatePolicy.acceptsAiBody(body)) return List.of();
        try {
            JSONObject request = new JSONObject().put("op", "parse")
                .put("query", new JSONObject(query)).put("body", body).put("limit", limit);
            JSONObject reply = new JSONObject(NativeClient.aiCandidateProtocol(request.toString()));
            if (!JsonPolicy.strictBoolean(reply.opt("ok"), false)) return List.of();
            JSONArray values = reply.optJSONArray("value");
            if (values == null) return List.of();
            List<String> texts = new ArrayList<>(values.length());
            for (int index = 0; index < values.length(); index++) {
                String text = JsonPolicy.strictString(values.opt(index));
                if (text != null) texts.add(text);
            }
            return OnlineCandidatePolicy.aiCandidates(texts, limit);
        } catch (JSONException | RuntimeException | LinkageError error) {
            return List.of();
        }
    }
}
