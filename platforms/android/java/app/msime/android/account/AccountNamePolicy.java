package app.msime.android;

import org.json.JSONException;
import org.json.JSONObject;

/** 昵称编辑与提交只消费共享 Rust 的规范化、合法性和长度上限。 */
public final class AccountNamePolicy {
    private AccountNamePolicy() {}

    public record Presentation(String normalized, boolean valid, int maximumChars, int maximumUtf16Units) {}

    /** 纯计算，不读账号或联网；传输、链接或响应失败时拒绝保存。 */
    public static Presentation present(String name) {
        if (name == null || name.length() > 16 * 1024 || !TextPolicy.validUnicode(name)) return null;
        try {
            String request = new JSONObject().put("name", name).put("id", "").toString();
            return parse(NativeClient.accountName(request));
        } catch (JSONException | IllegalArgumentException | LinkageError error) {
            return null;
        }
    }

    private static Presentation parse(String response) throws JSONException {
        JSONObject value = response == null ? null : JsonPolicy.objectValue(new JSONObject(response));
        if (value == null) return null;
        String normalized = JsonPolicy.strictString(value.opt("normalized"));
        Boolean valid = JsonPolicy.strictBoolean(value.opt("valid"));
        Integer chars = JsonPolicy.strictInteger(value.opt("maximum_chars"));
        Integer units = JsonPolicy.strictInteger(value.opt("maximum_utf16_units"));
        if (normalized == null || !TextPolicy.validUnicode(normalized) || valid == null
                || chars == null || chars <= 0 || chars > 16 * 1024
                || units == null || units < chars || units > 16 * 1024) return null;
        return new Presentation(normalized, valid, chars, units);
    }
}
