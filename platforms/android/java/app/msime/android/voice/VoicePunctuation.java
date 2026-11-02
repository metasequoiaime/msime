package app.msime.android;

import org.json.JSONException;
import org.json.JSONObject;

/**
 * 语音标点（共享偏好 `voice_input.punctuation_mode`，#10558）：识别结果上屏前去掉或替换识别器加的标点。
 *
 * <p>规则只在共享层（`msime_client_voice_punctuation`）一处，这里只认取值、调用它。取值由共享解析 `msime_client_mobile_voice_configuration` 的 `punctuationMode` 给出，键盘里的聆听、识别窗口和离线识别窗口都从 {@link VoiceConfiguration} 拿到同一个值。只处理最终文本：所有识别服务（本地、系统、豆包、整句上传）和润色之后、上屏或存进语音结果之前，中间结果照原样显示。
 */
public final class VoicePunctuation {
    /** 识别结果原样上屏，出厂默认，也是加这个选项之前的行为。 */
    public static final String AUTO = "auto";
    /** 设置页的四个选项，与 {@link #LABELS} 一一对应。 */
    public static final String[] MODES = {AUTO, "omit_trailing", "omit_all", "space"};
    public static final String[] LABELS = {"智能添加", "省略句末标点", "省略所有标点", "空格代替标点"};

    private VoicePunctuation() {}

    /** 偏好或共享解析里的值；不是字符串或不认识时按 {@link #AUTO}，与共享层一致。 */
    public static String mode(Object value) {
        String text = JsonPolicy.strictString(value);
        if (text == null) return AUTO;
        for (String mode : MODES) {
            if (mode.equals(text)) return mode;
        }
        return AUTO;
    }

    /** 设置页显示的名字。 */
    public static String label(String mode) {
        String normalized = mode(mode);
        for (int index = 0; index < MODES.length; index++) {
            if (MODES[index].equals(normalized)) return LABELS[index];
        }
        return LABELS[0];
    }

    /**
     * 按 `mode` 处理一段最终识别结果。{@link #AUTO}、空文本不经过共享层。整段只有标点时返回空串，调用方按没有识别到文字处理。共享层调用失败时返回原文：标点没去掉不值得让这次识别的内容丢掉。
     */
    public static String apply(String text, String mode) {
        if (text == null || text.isEmpty() || AUTO.equals(mode(mode))) return text;
        try {
            JSONObject request = new JSONObject().put("text", text).put("mode", mode(mode));
            JSONObject value = JsonPolicy.objectValue(new JSONObject(NativeClient.voicePunctuation(request.toString())));
            if (value == null) return text;
            String processed = JsonPolicy.strictString(value.opt("text"));
            return processed == null ? text : processed;
        } catch (JSONException | RuntimeException | LinkageError error) {
            return text;
        }
    }
}
