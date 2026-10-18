package app.msime.android.home;

import android.content.Context;
import app.msime.android.NativeClient;
import app.msime.android.policy.HostOptionsPolicy;
import org.json.JSONObject;

/** 原生设置只传递动作；版本、节流、下载与校验由共享 Rust 实现。 */
final class DictionaryUpdates {
    private DictionaryUpdates() {}

    static JSONObject request(Context context, String command, Boolean enabled) {
        try {
            String options = HostOptionsPolicy.readRuntimeOptions(context.getFilesDir());
            if (options.isEmpty()) return null;
            JSONObject action = new JSONObject().put("operation", "updates").put("command", command);
            if (enabled != null) action.put("enabled", enabled);
            JSONObject reply = new JSONObject(NativeClient.personalDictionaryRequest(new JSONObject()
                .put("options", new JSONObject(options)).put("action", action).toString()));
            return reply.optBoolean("ok") ? reply.optJSONObject("value") : null;
        } catch (Exception | LinkageError error) {
            return null;
        }
    }

    static String message(JSONObject status) {
        if (status == null) return "词库更新暂不可用，请稍后重试";
        if (!status.isNull("error")) return "更新失败，继续使用原词库";
        String phase = status.optString("phase");
        if ("checking".equals(phase)) return "正在检查词库更新…";
        if ("downloading".equals(phase)) return "正在下载词库…";
        if (status.optBoolean("pending_restart")) return "词库已准备好，下次启动输入法或重新打开键盘时生效";
        return "词库版本：" + status.optString("version") + "；个人词条和学习记录保留";
    }
}
