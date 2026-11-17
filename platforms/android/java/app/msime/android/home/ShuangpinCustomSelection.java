package app.msime.android.home;

import app.msime.android.AppEdition;
import app.msime.android.KeyboardScheme;
import app.msime.android.SchemePreferences;
import app.msime.android.ShuangpinCustomKeypad;
import org.json.JSONException;
import org.json.JSONObject;

/** 同一次保存选中双拼入口和码表，避免旧的触屏选择把刚保存的方案覆盖掉。 */
public final class ShuangpinCustomSelection {
    private ShuangpinCustomSelection() { }

    /** 删除已保存的表；仅使用自定义表时回退小鹤，不改变其他键盘或内置方案。 */
    public static JSONObject remove(JSONObject snapshot) {
        try {
            JSONObject pending = new JSONObject(snapshot.toString());
            JSONObject values = pending.getJSONObject("preferences");
            values.remove("shuangpin_custom_profile");
            if ("custom".equals(values.optString("shuangpin_profile"))) {
                values.put("shuangpin_profile", "xiaohe");
            }
            return pending;
        } catch (JSONException | RuntimeException error) {
            return null;
        }
    }

    public static JSONObject select(JSONObject snapshot, String profile, JSONObject table) {
        try {
            AppEdition edition = AppEdition.current();
            if (!KeyboardScheme.XIAOHE.offeredBy(edition)) return null;
            String selectedProfile = ShuangpinCustomKeypad.normalizedProfile(profile);
            KeyboardScheme scheme = KeyboardScheme.fromPreferences(
                "shuangpin", selectedProfile, "twenty_six_key", edition);
            JSONObject pending = SchemePreferences.withScheme(snapshot, scheme, null);
            if (pending == null) return null;
            JSONObject values = pending.getJSONObject("preferences");
            // 自定义和智能 ABC 共用普通双拼入口，必须在入口映射之后恢复实际码表名。
            values.put("shuangpin_profile", selectedProfile);
            if (table != null) values.put("shuangpin_custom_profile", table);
            return pending;
        } catch (JSONException | RuntimeException error) {
            return null;
        }
    }
}
